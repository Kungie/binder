//! The Agents screen's terminals: one pseudo-terminal per agent running the Mac's own `ssh -tt` into a tmux
//! session on the server. Bytes pass through untouched; the window's xterm.js decodes them.

use binder_core::agents::{Agent, exit_message};
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

type Sink = Arc<Mutex<Box<dyn Fn(Vec<u8>) + Send>>>;
type OnExit = Arc<Mutex<Option<Box<dyn FnOnce(String) + Send>>>>;

/// How much of the last output is kept, for the line shown when `ssh` fails.
const TAIL: usize = 512;

struct Running {
    generation: u64,
    master: Box<dyn MasterPty + Send>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    sink: Sink,
    on_exit: OnExit,
    #[cfg_attr(not(test), allow(dead_code))]
    pid: Option<u32>,
}

#[derive(Default)]
pub struct Terminals {
    running: Arc<Mutex<HashMap<Agent, Running>>>,
    generation: AtomicU64,
}

fn size(cols: u16, rows: u16) -> PtySize {
    PtySize { rows: rows.max(1), cols: cols.max(1), pixel_width: 0, pixel_height: 0 }
}

impl Terminals {
    /// Starts `program` in a pseudo-terminal for `agent`, unless one is running: then the new sink and exit
    /// callback replace the old ones (a reloaded window takes over the same child). True when a child started.
    pub fn open(
        &self,
        agent: Agent,
        program: &str,
        args: &[String],
        cols: u16,
        rows: u16,
        on_bytes: impl Fn(Vec<u8>) + Send + 'static,
        on_exit: impl FnOnce(String) + Send + 'static,
    ) -> Result<bool, String> {
        let mut running = self.running.lock().unwrap();
        if let Some(r) = running.get(&agent) {
            *r.sink.lock().unwrap() = Box::new(on_bytes);
            *r.on_exit.lock().unwrap() = Some(Box::new(on_exit));
            let _ = r.master.resize(size(cols, rows));
            return Ok(false);
        }

        let pair = native_pty_system().openpty(size(cols, rows)).map_err(|e| e.to_string())?;
        let mut cmd = CommandBuilder::new(program);
        cmd.args(args);
        cmd.env("TERM", "xterm-256color");
        let mut child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
        // Only the child keeps the slave open, so the reader sees the end when the child goes.
        drop(pair.slave);
        let reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
        let writer = pair.master.take_writer().map_err(|e| e.to_string())?;

        let generation = self.generation.fetch_add(1, Ordering::Relaxed);
        let sink: Sink = Arc::new(Mutex::new(Box::new(on_bytes)));
        let exit: OnExit = Arc::new(Mutex::new(Some(Box::new(on_exit))));
        let tail = Arc::new(Mutex::new(Vec::<u8>::new()));
        let (done_tx, done_rx) = mpsc::channel::<()>();
        let killer = child.clone_killer();
        let pid = child.process_id();

        {
            let (sink, tail) = (sink.clone(), tail.clone());
            std::thread::spawn(move || read_loop(reader, sink, tail, done_tx));
        }
        {
            let (running, exit, tail) = (self.running.clone(), exit.clone(), tail.clone());
            std::thread::spawn(move || {
                let code = match child.wait() {
                    Ok(s) if s.signal().is_none() => Some(s.exit_code() as i32),
                    _ => None,
                };
                // The last output can still be in flight; wait for the reader briefly, not forever
                // (a process the child left behind may hold the terminal open).
                let _ = done_rx.recv_timeout(Duration::from_millis(500));
                let last = String::from_utf8_lossy(&tail.lock().unwrap()).into_owned();
                {
                    let mut running = running.lock().unwrap();
                    if running.get(&agent).is_some_and(|r| r.generation == generation) {
                        running.remove(&agent);
                    }
                }
                if let Some(f) = exit.lock().unwrap().take() {
                    f(exit_message(code, &last));
                }
            });
        }

        running.insert(
            agent,
            Running { generation, master: pair.master, writer: Arc::new(Mutex::new(writer)), killer, sink, on_exit: exit, pid },
        );
        Ok(true)
    }

    /// Typed keys and pasted text, as bytes. The map is unlocked before writing, so a full terminal
    /// can't stall the other agent.
    pub fn write(&self, agent: Agent, data: &[u8]) -> Result<(), String> {
        let writer = self.running.lock().unwrap().get(&agent).map(|r| r.writer.clone()).ok_or("Not connected.")?;
        let mut w = writer.lock().unwrap();
        w.write_all(data).and_then(|_| w.flush()).map_err(|e| e.to_string())
    }

    pub fn resize(&self, agent: Agent, cols: u16, rows: u16) -> Result<(), String> {
        match self.running.lock().unwrap().get(&agent) {
            Some(r) => r.master.resize(size(cols, rows)).map_err(|e| e.to_string()),
            None => Ok(()),
        }
    }

    /// Ends the local `ssh`; the tmux session on the server stays. No exit message: the caller asked for it.
    pub fn close(&self, agent: Agent) {
        let r = self.running.lock().unwrap().remove(&agent);
        if let Some(mut r) = r {
            r.on_exit.lock().unwrap().take();
            let _ = r.killer.kill();
        }
    }

    pub fn close_all(&self) {
        for agent in [Agent::Claude, Agent::Qwen] {
            self.close(agent);
        }
    }

    #[cfg(test)]
    fn pid(&self, agent: Agent) -> Option<u32> {
        self.running.lock().unwrap().get(&agent).and_then(|r| r.pid)
    }
}

impl Drop for Terminals {
    fn drop(&mut self) {
        self.close_all();
    }
}

/// Runs `program` with `input` on its stdin and waits for it. The terminals copy through `pbcopy` this way, since
/// the window can't write the clipboard without a click.
pub fn pipe_to(program: &str, args: &[&str], input: &str) -> Result<(), String> {
    let mut child = std::process::Command::new(program)
        .args(args)
        // An app opened from Finder has no LANG, and pbcopy would then read the bytes as Mac Roman.
        .env("LANG", "en_US.UTF-8")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    child.stdin.take().ok_or("No stdin.")?.write_all(input.as_bytes()).map_err(|e| e.to_string())?;
    let status = child.wait().map_err(|e| e.to_string())?;
    if status.success() { Ok(()) } else { Err(format!("{program} failed.")) }
}

fn read_loop(mut reader: Box<dyn Read + Send>, sink: Sink, tail: Arc<Mutex<Vec<u8>>>, done: mpsc::Sender<()>) {
    let mut buf = [0u8; 8192];
    loop {
        match reader.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let bytes = buf[..n].to_vec();
                {
                    let mut t = tail.lock().unwrap();
                    t.extend_from_slice(&bytes);
                    let over = t.len().saturating_sub(TAIL);
                    t.drain(..over);
                }
                (sink.lock().unwrap())(bytes);
            }
        }
    }
    let _ = done.send(());
}

#[cfg(test)]
mod tests {
    use super::*;
    use binder_core::agents::{Agent, exit_message};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    fn sh(script: &str) -> Vec<String> {
        vec!["-c".into(), script.into()]
    }

    /// Collects output until `done` says it has enough, or 5 s pass.
    fn collect(rx: &mpsc::Receiver<Vec<u8>>, done: impl Fn(&[u8]) -> bool) -> Vec<u8> {
        let mut out = Vec::new();
        let end = Instant::now() + Duration::from_secs(5);
        while Instant::now() < end && !done(&out) {
            if let Ok(b) = rx.recv_timeout(Duration::from_millis(100)) {
                out.extend(b);
            }
        }
        out
    }

    #[test]
    fn bytes_pass_through_unchanged() {
        let t = Terminals::default();
        let (tx, rx) = mpsc::channel();
        t.open(Agent::Claude, "/bin/sh", &sh("printf 'g\\303'; sleep 0.2; printf '\\274l'"), 80, 24, move |b| tx.send(b).unwrap(), |_| {}).unwrap();
        let out = collect(&rx, |o| o.ends_with(b"l"));
        assert_eq!(out, "gül".as_bytes());
    }

    #[test]
    fn write_reaches_the_child() {
        let t = Terminals::default();
        let (tx, rx) = mpsc::channel();
        t.open(Agent::Qwen, "/bin/sh", &sh("stty -echo; read x; printf 'got:%s' \"$x\""), 80, 24, move |b| tx.send(b).unwrap(), |_| {}).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        t.write(Agent::Qwen, b"hi\r").unwrap();
        let out = collect(&rx, |o| o.ends_with(b"got:hi"));
        assert!(out.ends_with(b"got:hi"), "{}", String::from_utf8_lossy(&out));
    }

    #[test]
    fn open_twice_reuses() {
        let t = Terminals::default();
        assert_eq!(t.open(Agent::Claude, "/bin/sh", &sh("sleep 30"), 80, 24, |_| {}, |_| {}), Ok(true));
        let pid = t.pid(Agent::Claude).unwrap();
        assert_eq!(t.open(Agent::Claude, "/bin/sh", &sh("sleep 30"), 80, 24, |_| {}, |_| {}), Ok(false));
        assert_eq!(t.pid(Agent::Claude), Some(pid));
        assert_eq!(t.resize(Agent::Qwen, 100, 30), Ok(()));
    }

    #[test]
    fn exit_reports_message() {
        let t = Terminals::default();
        let (tx, rx) = mpsc::channel();
        t.open(Agent::Claude, "/bin/sh", &sh("exit 127"), 80, 24, |_| {}, move |m| tx.send(m).unwrap()).unwrap();
        let m = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(m, exit_message(Some(127), ""));
        let end = Instant::now() + Duration::from_secs(2);
        while t.pid(Agent::Claude).is_some() && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(t.pid(Agent::Claude), None);
    }

    #[test]
    fn piped_text_reaches_the_program() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("clip");
        let script = format!("cat > '{}'", out.display());
        pipe_to("/bin/sh", &["-c", &script], "gül ✻").unwrap();
        assert_eq!(std::fs::read_to_string(out).unwrap(), "gül ✻");
    }

    #[test]
    fn piped_programs_read_utf8_without_a_lang() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("lang");
        let script = format!("printf %s \"$LANG\" > '{}'", out.display());
        pipe_to("/bin/sh", &["-c", &script], "").unwrap();
        assert_eq!(std::fs::read_to_string(out).unwrap(), "en_US.UTF-8");
    }

    #[test]
    fn drop_kills_children() {
        let t = Terminals::default();
        t.open(Agent::Claude, "/bin/sh", &sh("sleep 30"), 80, 24, |_| {}, |_| {}).unwrap();
        let pid = t.pid(Agent::Claude).unwrap() as i32;
        drop(t);
        let end = Instant::now() + Duration::from_secs(2);
        while unsafe { libc::kill(pid, 0) } == 0 && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_ne!(unsafe { libc::kill(pid, 0) }, 0, "child {pid} still running");
    }
}

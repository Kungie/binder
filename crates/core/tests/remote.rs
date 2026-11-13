use binder_core::remote::quote;
use std::process::Command;

#[test]
fn quote_survives_hostile_text() {
    for text in ["a b", "it's", "$(rm -rf ~)", "`id`", "two\nlines", "", "\"double\" and 'single'"] {
        let out = Command::new("sh").arg("-c").arg(format!("printf %s {}", quote(text))).output().unwrap();
        assert_eq!(String::from_utf8(out.stdout).unwrap(), text, "{text:?}");
    }
}

use binder_core::Error;
use binder_core::remote::{Remote, SshRemote};
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

/// A stand-in for ssh: a shell script that ignores ssh's arguments.
fn fake_ssh(dir: &std::path::Path, body: &str) -> SshRemote {
    let path = dir.join("ssh");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    SshRemote { program: path.to_string_lossy().into_owned(), ..SshRemote::new("pi") }
}

#[test]
fn stdin_reaches_the_script_as_data() {
    let d = tempfile::tempdir().unwrap();
    let r = fake_ssh(d.path(), "exec sh -s");
    let data = "first line\nBINDER_STDIN_9f2c\necho INJECTED > pwned.txt\n'quotes' $(id) `id` \\ end";
    let out = r.run(&format!("cd {} && cat", quote(&d.path().to_string_lossy())), Some(data)).unwrap();
    assert_eq!(out, data);
    assert!(!d.path().join("pwned.txt").exists());
}

#[test]
fn stalled_ssh_hits_the_deadline() {
    let d = tempfile::tempdir().unwrap();
    let r = SshRemote { deadline: Duration::from_millis(300), ..fake_ssh(d.path(), "sleep 10") };
    let started = Instant::now();
    assert!(matches!(r.run("true", None), Err(Error::Remote(_))));
    assert!(started.elapsed() < Duration::from_secs(3), "{:?}", started.elapsed());
}

#[test]
fn ssh_is_batch_with_keepalive_and_target_after_dashes() {
    let d = tempfile::tempdir().unwrap();
    let r = fake_ssh(d.path(), "echo \"$@\"");
    let out = r.run("true", None).unwrap();
    for arg in ["BatchMode=yes", "ConnectTimeout=5", "ServerAliveInterval=5", "ServerAliveCountMax=2", "-- pi sh -l -s"] {
        assert!(out.contains(arg), "{arg} missing from {out}");
    }
}

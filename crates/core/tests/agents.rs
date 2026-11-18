use binder_core::agents::*;
use std::collections::HashMap;

fn home() -> Pane {
    Pane::Claude { project: None, dir: None }
}

fn project() -> Pane {
    Pane::Claude { project: Some("binder".into()), dir: Some("~/code/binder".into()) }
}

fn ollama(model: &str) -> Pane {
    Pane::Ollama { model: model.into() }
}

#[test]
fn sessions_are_named_by_pane() {
    assert_eq!(home().session(), "binder-claude");
    assert_eq!(project().session(), "binder-claude-binder");
    assert_eq!(ollama("qwen3:4b").session(), "binder-ollama");
}

#[test]
fn slugs_fit_tmux() {
    assert_eq!(slug("Better Termin"), "better-termin");
    assert_eq!(slug("erenkahraman.dev"), "erenkahraman-dev");
    assert_eq!(slug("  Ünlü__Proje!! "), "nl-proje");
    assert_eq!(slug("..."), "project");
}

#[test]
fn remote_command_creates_or_attaches() {
    let c = remote_command(&home()).unwrap();
    assert!(c.starts_with("tmux -u start-server \\; set -g history-limit 10000 \\; new-session -A -s binder-claude "), "{c}");
    assert!(c.contains("bash -lc"));
    assert!(c.contains("claude; exec bash -l"));
    assert!(c.contains("set-option -t binder-claude status off"));
    assert!(c.contains("set-option -t binder-claude mouse on"));
    assert!(c.contains("set-option -t binder-claude set-clipboard on"));
    let q = remote_command(&ollama("qwen3:4b")).unwrap();
    assert!(q.contains("new-session -A -s binder-ollama "));
    assert!(q.contains("docker exec -it ollama ollama run qwen3:4b; exec bash -l"));
}

#[test]
fn a_project_session_starts_in_its_folder() {
    let c = remote_command(&project()).unwrap();
    assert!(c.contains("new-session -A -s binder-claude-binder "), "{c}");
    assert!(c.contains("cd \"$HOME\"/"), "{c}");
    assert!(c.contains("code/binder"), "{c}");
    assert!(c.contains("&& claude; exec bash -l"), "{c}");
    assert!(c.contains("mkdir -p \"$HOME\"/"), "{c}");
}

#[test]
fn any_binder_session_can_be_attached() {
    let p = Pane::Attach { session: "binder-qwen".into() };
    assert_eq!(p.session(), "binder-qwen");
    let c = remote_command(&p).unwrap();
    assert!(c.contains("new-session -A -s binder-qwen "), "{c}");
    assert!(remote_command(&Pane::Attach { session: "main".into() }).is_err());
    assert!(remote_command(&Pane::Attach { session: "binder-x;reboot".into() }).is_err());
}

#[test]
fn bad_model_names_are_refused() {
    assert!(valid_model("qwen3:4b"));
    assert!(valid_model("hf.co/org/model-GGUF:Q4_K_M"));
    assert!(!valid_model(""));
    assert!(!valid_model("x; rm -rf ~"));
    assert!(!valid_model("a b"));
    assert!(remote_command(&ollama("x;y")).is_err());
}

#[test]
fn ssh_args_are_interactive_and_key_only() {
    let a = ssh_args("demo-dev@10.0.7.2", &ollama("qwen3:4b")).unwrap();
    assert_eq!(
        &a[..10],
        ["-tt", "-o", "BatchMode=yes", "-o", "ConnectTimeout=5", "-o", "ServerAliveInterval=15", "-o", "ServerAliveCountMax=2", "--"]
    );
    assert_eq!(a[10], "demo-dev@10.0.7.2");
    assert_eq!(a[11], remote_command(&ollama("qwen3:4b")).unwrap());
    assert_eq!(a.len(), 12);
}

#[test]
fn target_is_one_argument() {
    let a = ssh_args("it's a host", &home()).unwrap();
    assert_eq!(a[10], "it's a host");
    assert_eq!(a.len(), 12);
}

#[test]
fn panes_serialize_with_a_kind() {
    assert_eq!(serde_json::to_string(&ollama("m")).unwrap(), r#"{"kind":"ollama","model":"m"}"#);
    let p: Pane = serde_json::from_str(r#"{"kind":"claude","project":null,"dir":null}"#).unwrap();
    assert_eq!(p, home());
}

#[test]
fn exit_messages() {
    assert_eq!(exit_message(Some(127), "bash: tmux: command not found"), "tmux isn't installed on the server.");
    assert_eq!(
        exit_message(Some(255), "x\nssh: connect to host 10.0.7.2 port 22: Connection refused\r\n"),
        "ssh: connect to host 10.0.7.2 port 22: Connection refused"
    );
    assert_eq!(exit_message(Some(255), ""), "Couldn't reach the server.");
    assert_eq!(exit_message(Some(0), ""), "Detached.");
    assert_eq!(exit_message(None, ""), "Disconnected.");
    assert_eq!(exit_message(Some(1), ""), "Session ended.");
}

#[test]
fn a_dropped_session_shows_ssh_line_without_terminal_codes() {
    let tail = "\x1b[?25l\x1b[3;5H\u{273b}\x1b[?25hTimeout, server lab not responding.\r\n";
    assert_eq!(exit_message(Some(255), tail), "Timeout, server lab not responding.");
    let osc = "\x1b]52;c;aGk=\x07\x1b[0mConnection to lab closed by remote host.\r\n";
    assert_eq!(exit_message(Some(255), osc), "Connection to lab closed by remote host.");
    assert_eq!(exit_message(Some(255), "\x1b[2J\x1b[H"), "Couldn't reach the server.");
}

#[test]
fn claude_states_come_from_the_screen() {
    let working = "> fix the tests\n\n✻ Thinking… (12s · ↑ 1.2k tokens · esc to interrupt)\n\n\n";
    assert_eq!(classify("claude", working).0, State::Working);
    let waiting = "Bash command\n  cargo test\nDo you want to proceed?\n❯ 1. Yes\n  2. No\n";
    assert_eq!(classify("node", waiting).0, State::Waiting);
    let idle = "● Done. All 42 suites pass.\n\n> \n  ? for shortcuts\n";
    let (state, line) = classify("claude", idle);
    assert_eq!(state, State::Idle);
    assert_eq!(line, "? for shortcuts");
    assert_eq!(classify("bash", "demo-dev@lab:~$ \n").0, State::Exited);
    assert_eq!(classify("docker", ">>> Send a message\n").0, State::Idle);
}

#[test]
fn sessions_parse_from_one_read() {
    let out = "\n--binder:pane-- binder-claude claude\n✻ Working (3s · esc to interrupt)\n\n\n--binder:pane-- binder-ollama docker\n>>> hi\n\n--binder:pane-- binder-claude-site bash\n$ \n";
    let s = parse_sessions(out);
    assert_eq!(s.len(), 3);
    assert_eq!(s[0].session, "binder-claude");
    assert_eq!(s[0].state, State::Working);
    assert_eq!(s[1].session, "binder-ollama");
    assert_eq!(s[1].line, ">>> hi");
    assert_eq!(s[2].state, State::Exited);
    assert!(sessions_script().contains("grep '^binder-'"));
    // tmux names the pane's shell; the program it runs is the shell's child.
    assert!(sessions_script().contains("--ppid"));
}

#[test]
fn alerts_fire_once_when_work_stops() {
    let mut seen: HashMap<String, State> = HashMap::new();
    let s = |session: &str, state| Session { session: session.into(), state, line: String::new() };
    assert!(alerts(&mut seen, &[s("binder-claude", State::Working)]).is_empty());
    assert_eq!(alerts(&mut seen, &[s("binder-claude", State::Waiting)]), vec![("binder-claude".to_string(), State::Waiting)]);
    assert!(alerts(&mut seen, &[s("binder-claude", State::Waiting)]).is_empty());
    assert!(alerts(&mut seen, &[s("binder-claude", State::Working)]).is_empty());
    assert_eq!(alerts(&mut seen, &[s("binder-claude", State::Idle)]), vec![("binder-claude".to_string(), State::Idle)]);
    // A session seen for the first time already idle says nothing.
    assert!(alerts(&mut seen, &[s("binder-claude-x", State::Idle)]).is_empty());
    // A session that went away and came back starts fresh.
    assert!(alerts(&mut seen, &[]).is_empty());
    assert!(seen.is_empty());
}

#[test]
fn close_only_takes_binder_sessions() {
    assert_eq!(close_script("binder-claude-site").unwrap(), "tmux kill-session -t '=binder-claude-site'");
    assert!(close_script("main").is_err());
    assert!(close_script("binder-x; reboot").is_err());
}

#[test]
fn models_parse_from_ollama_list() {
    let out = "NAME              ID              SIZE      MODIFIED\nqwen3:4b          e55aed6fe643    2.5 GB    3 weeks ago\nllama3.2:latest   a80c4f17acd5    2.0 GB    2 days ago\n";
    assert_eq!(parse_models(out), vec!["qwen3:4b", "llama3.2:latest"]);
    assert!(parse_models("").is_empty());
}

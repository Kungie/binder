use binder_core::agents::*;

#[test]
fn remote_command_creates_or_attaches() {
    let c = remote_command(Agent::Claude);
    assert!(c.starts_with("tmux start-server \\; set -g history-limit 10000 \\; new-session -A -s binder-claude "), "{c}");
    assert!(c.contains("bash -lc"));
    assert!(c.contains("claude; exec bash -l"));
    assert!(c.contains("set-option -t binder-claude status off"));
    assert!(c.contains("set-option -t binder-claude mouse on"));
    assert!(c.contains("set-option -t binder-claude set-clipboard on"));
    let q = remote_command(Agent::Qwen);
    assert!(q.contains("new-session -A -s binder-qwen "));
    assert!(q.contains("docker exec -it ollama ollama run qwen3:4b; exec bash -l"));
}

#[test]
fn ssh_args_are_interactive_and_key_only() {
    let a = ssh_args("demo-dev@10.0.7.2", Agent::Qwen);
    assert_eq!(
        &a[..10],
        ["-tt", "-o", "BatchMode=yes", "-o", "ConnectTimeout=5", "-o", "ServerAliveInterval=15", "-o", "ServerAliveCountMax=2", "--"]
    );
    assert_eq!(a[10], "demo-dev@10.0.7.2");
    assert_eq!(a[11], remote_command(Agent::Qwen));
    assert_eq!(a.len(), 12);
}

#[test]
fn target_is_one_argument() {
    let a = ssh_args("it's a host", Agent::Claude);
    assert_eq!(a[10], "it's a host");
    assert_eq!(a.len(), 12);
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
fn agent_names_serialize_lowercase() {
    assert_eq!(serde_json::to_string(&Agent::Claude).unwrap(), "\"claude\"");
    assert_eq!(Agent::Qwen.session(), "binder-qwen");
}

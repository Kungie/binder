import Darwin
import Foundation

/// Claude Code hook plumbing: reading the event from stdin, writing the SessionStart reply.
public enum HookIO {
    public static func readInput(_ data: Data) -> HookInput? {
        try? JSONDecoder().decode(HookInput.self, from: data)
    }

    public static func sessionStartOutput(_ card: String) -> Data {
        let reply: [String: Any] = ["hookSpecificOutput": ["hookEventName": "SessionStart", "additionalContext": card]]
        return (try? JSONSerialization.data(withJSONObject: reply, options: [.withoutEscapingSlashes])) ?? Data()
    }

    /// Claude Code keeps a project's transcripts in `~/.claude/projects/<path with every non-alphanumeric as ->`.
    public static func claudeProjectFolder(for path: String) -> String {
        String(path.map { $0.isLetter || $0.isNumber ? $0 : "-" })
    }

    /// Starts `path args…` in its own session with no terminal and no output, and doesn't wait for it.
    /// Used for slow follow-up work (the summarizer) so a hook can return at once.
    @discardableResult
    public static func spawnDetached(_ path: String, _ args: [String]) -> Bool {
        var attr: posix_spawnattr_t?
        posix_spawnattr_init(&attr)
        defer { posix_spawnattr_destroy(&attr) }
        posix_spawnattr_setflags(&attr, Int16(POSIX_SPAWN_SETSID))
        var actions: posix_spawn_file_actions_t?
        posix_spawn_file_actions_init(&actions)
        defer { posix_spawn_file_actions_destroy(&actions) }
        for fd: Int32 in [0, 1, 2] {
            posix_spawn_file_actions_addopen(&actions, fd, "/dev/null", fd == 0 ? O_RDONLY : O_WRONLY, 0)
        }
        let argv = ([path] + args).map { strdup($0) } + [nil]
        defer { argv.forEach { free($0) } }
        var pid: pid_t = 0
        return posix_spawn(&pid, path, &actions, &attr, argv, environ) == 0
    }
}

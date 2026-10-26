import AppKit
import Foundation

/// Opens a fresh Claude Code session in a project folder, in the user's terminal.
public enum SessionLauncher {
    public enum Terminal: Equatable, Sendable { case terminal, iterm, ghostty }

    static let bundleIDs: [(Terminal, String)] = [(.ghostty, "com.mitchellh.ghostty"), (.iterm, "com.googlecode.iterm2")]

    /// Ghostty, then iTerm, then Terminal, by what's installed.
    public static func preferred(installed: (String) -> Bool = { NSWorkspace.shared.urlForApplication(withBundleIdentifier: $0) != nil }) -> Terminal {
        bundleIDs.first { installed($0.1) }?.0 ?? .terminal
    }

    /// `cd '<dir>' && claude`, with the path single-quoted for the shell.
    public static func command(dir: URL) -> String {
        "cd '\(dir.path.replacingOccurrences(of: "'", with: "'\\''"))' && claude"
    }

    static func appleScriptString(_ s: String) -> String {
        "\"" + s.replacingOccurrences(of: "\\", with: "\\\\").replacingOccurrences(of: "\"", with: "\\\"") + "\""
    }

    public static func appleScript(for terminal: Terminal, dir: URL) -> String? {
        let cmd = appleScriptString(command(dir: dir))
        switch terminal {
        case .terminal:
            return "tell application \"Terminal\"\n    activate\n    do script \(cmd)\nend tell"
        case .iterm:
            return "tell application \"iTerm\"\n    activate\n    set w to (create window with default profile)\n    tell current session of w to write text \(cmd)\nend tell"
        case .ghostty:
            return nil
        }
    }

    public static func ghosttyArguments(dir: URL) -> [String] {
        ["-na", "Ghostty", "--args", "--working-directory=\(dir.path)", "-e", "claude"]
    }

    @MainActor
    @discardableResult
    public static func launch(in dir: URL, terminal: Terminal = preferred()) -> Bool {
        if let source = appleScript(for: terminal, dir: dir) {
            var error: NSDictionary?
            NSAppleScript(source: source)?.executeAndReturnError(&error)
            return error == nil
        }
        let open = Process()
        open.executableURL = URL(fileURLWithPath: "/usr/bin/open")
        open.arguments = ghosttyArguments(dir: dir)
        return (try? open.run()) != nil
    }
}

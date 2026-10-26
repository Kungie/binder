import Foundation

/// The basics of this Mac, and which coding agents live on it.
public struct MacObserver: Observer {
    public let name = "mac"
    let shell: String?

    public init(shell: String? = ProcessInfo.processInfo.environment["SHELL"]) {
        self.shell = shell
    }

    public func observe(_ ctx: ObserverContext) -> Observation {
        let out = { (tool: String, args: [String]) -> String? in
            guard let r = ctx.runner.run(tool, args, input: nil, timeout: 3), r.status == 0 else { return nil }
            let s = r.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
            return s.isEmpty ? nil : s
        }
        var lines: [String] = []
        if let version = out("sw_vers", ["-productVersion"]) { lines.append("- macOS: \(version)") }
        if let chip = out("uname", ["-m"]) { lines.append("- chip: \(chip)") }
        if let shell { lines.append("- shell: \((shell as NSString).lastPathComponent)") }
        if let formulae = out("brew", ["list", "--formula", "-1"]) {
            lines.append("- homebrew: \(formulae.split(separator: "\n").count) formulae")
        }
        let fm = FileManager.default
        let claude = fm.fileExists(atPath: ctx.home.appendingPathComponent(".claude").path)
        let codex = fm.fileExists(atPath: ctx.home.appendingPathComponent(".codex").path)
        lines.append("- agents: Claude Code \(claude ? "found" : "not found"), Codex \(codex ? "found" : "not found")")
        return Observation(notes: [NoteUpdate(path: "machines/this-mac.md", title: "This Mac", observed: lines)])
    }
}

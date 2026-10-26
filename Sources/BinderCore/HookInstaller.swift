import Foundation

public enum HookInstallerError: Error, Equatable {
    case malformedSettings
}

/// Adds or removes Binder's two entries in Claude Code's `~/.claude/settings.json`:
/// SessionStart runs `binder card`, Stop runs `binder capture`. Every other setting and hook is kept,
/// a backup is written before the first change, and a file that isn't valid JSON is never touched.
public struct HookInstaller: Sendable {
    public let settingsURL: URL
    public let binderPath: String

    public init(settingsURL: URL, binderPath: String) {
        self.settingsURL = settingsURL
        self.binderPath = binderPath
    }

    public static var defaultSettingsURL: URL {
        FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".claude/settings.json")
    }

    var events: [(event: String, command: String)] {
        [("SessionStart", "\"\(binderPath)\" card"), ("Stop", "\"\(binderPath)\" capture")]
    }

    static func isOurs(_ entry: Any) -> Bool {
        guard let hooks = (entry as? [String: Any])?["hooks"] as? [[String: Any]] else { return false }
        return hooks.contains {
            let command = $0["command"] as? String ?? ""
            return command.contains("binder\" card") || command.contains("binder\" capture")
        }
    }

    public var isInstalled: Bool {
        guard let json = try? load(), let hooks = json["hooks"] as? [String: Any] else { return false }
        return events.allSatisfy { ((hooks[$0.event] as? [Any]) ?? []).contains(where: Self.isOurs) }
    }

    public func install() throws { try change(add: true) }
    public func uninstall() throws { try change(add: false) }

    func load() throws -> [String: Any] {
        guard FileManager.default.fileExists(atPath: settingsURL.path) else { return [:] }
        let data = try Data(contentsOf: settingsURL)
        if data.allSatisfy({ $0 == 0x20 || $0 == 0x0A || $0 == 0x09 || $0 == 0x0D }) { return [:] }
        guard let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw HookInstallerError.malformedSettings
        }
        if let hooks = json["hooks"], !(hooks is [String: Any]) { throw HookInstallerError.malformedSettings }
        return json
    }

    private func change(add: Bool) throws {
        var json = try load()
        var hooks = json["hooks"] as? [String: Any] ?? [:]
        for (event, command) in events {
            var list = (hooks[event] as? [Any] ?? []).filter { !Self.isOurs($0) }
            if add {
                list.append(["matcher": "", "hooks": [["type": "command", "command": command, "timeout": 5]]] as [String: Any])
            }
            if list.isEmpty { hooks.removeValue(forKey: event) } else { hooks[event] = list }
        }
        if hooks.isEmpty { json.removeValue(forKey: "hooks") } else { json["hooks"] = hooks }

        let data = try JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .withoutEscapingSlashes, .sortedKeys])
        let fm = FileManager.default
        if fm.fileExists(atPath: settingsURL.path) {
            guard try Data(contentsOf: settingsURL) != data else { return }
            let backup = settingsURL.appendingPathExtension("binder-backup")
            if !fm.fileExists(atPath: backup.path) { try fm.copyItem(at: settingsURL, to: backup) }
        } else if !add {
            return
        }
        try fm.createDirectory(at: settingsURL.deletingLastPathComponent(), withIntermediateDirectories: true)
        try data.write(to: settingsURL, options: .atomic)
    }
}

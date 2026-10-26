import Foundation

/// The binder folder and every path inside it. All paths come from here, so tests can use a temp root.
public struct Vault: Sendable {
    public let root: URL

    public init(root: URL) {
        self.root = root
    }

    public static var defaultRoot: URL {
        FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Binder", isDirectory: true)
    }

    public func url(_ relative: String) -> URL {
        root.appendingPathComponent(relative)
    }

    public var internalDir: URL { url(".binder") }
    public var inbox: URL { url(".binder/inbox") }
    public var cards: URL { url(".binder/cards") }
    public var sessions: URL { url(".binder/sessions") }
    public var logFile: URL { url(".binder/log") }
    public var stateFile: URL { url(".binder/state.json") }
    public var cursorsFile: URL { url(".binder/cursors.json") }
    public var droppedFile: URL { url(".binder/dropped.txt") }

    static let folders = ["machines", "access", "servers", "projects", "handoffs", ".binder/inbox", ".binder/cards", ".binder/sessions"]

    public func ensureLayout() throws {
        for folder in Self.folders {
            try FileManager.default.createDirectory(at: url(folder), withIntermediateDirectories: true)
        }
    }

    /// Appends one timestamped line to `.binder/log`. Never throws: logging must not break a hook.
    public func log(_ message: String) {
        let line = "\(ISO8601DateFormatter().string(from: Date())) \(message)\n"
        let fm = FileManager.default
        try? fm.createDirectory(at: internalDir, withIntermediateDirectories: true)
        if let handle = try? FileHandle(forWritingTo: logFile) {
            defer { try? handle.close() }
            _ = try? handle.seekToEnd()
            try? handle.write(contentsOf: Data(line.utf8))
        } else {
            try? Data(line.utf8).write(to: logFile)
        }
    }
}

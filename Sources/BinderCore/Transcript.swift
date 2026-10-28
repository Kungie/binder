import Foundation

public struct Usage: Equatable, Sendable {
    public var input: Int
    public var cacheRead: Int
    public var cacheCreation: Int

    public init(input: Int, cacheRead: Int, cacheCreation: Int) {
        self.input = input
        self.cacheRead = cacheRead
        self.cacheCreation = cacheCreation
    }

    /// Everything the model had in context for that turn.
    public var total: Int { input + cacheRead + cacheCreation }
}

/// The parts of a Claude Code transcript Binder cares about.
public enum TranscriptEntry: Equatable, Sendable {
    case user(String)
    case assistant(String)
    case command(String)
    case fileEdit(String)
    case todos([String])
    case usage(Usage, model: String?)
}

public struct TranscriptChunk: Sendable {
    public var entries: [TranscriptEntry]
    /// Where the next read should start: just after the last complete line.
    public var endOffset: UInt64
    public var sessionID: String?
    public var cwd: String?
}

/// Reads `~/.claude/projects/*/<session>.jsonl` from a byte offset onward.
/// A half-written last line is left for the next read; lines that aren't JSON are skipped.
public enum Transcript {
    /// `upTo` stops at that byte, so two readers can split a file without reading the same lines twice.
    public static func read(_ url: URL, from offset: UInt64, upTo end: UInt64? = nil) throws -> TranscriptChunk {
        let handle = try FileHandle(forReadingFrom: url)
        defer { try? handle.close() }
        try handle.seek(toOffset: offset)
        let data: Data
        if let end {
            data = end > offset ? try handle.read(upToCount: Int(end - offset)) ?? Data() : Data()
        } else {
            data = try handle.readToEnd() ?? Data()
        }
        guard let lastNewline = data.lastIndex(of: UInt8(ascii: "\n")) else {
            return TranscriptChunk(entries: [], endOffset: offset, sessionID: nil, cwd: nil)
        }
        let complete = data[data.startIndex...lastNewline]
        var chunk = TranscriptChunk(entries: [], endOffset: offset + UInt64(complete.count), sessionID: nil, cwd: nil)
        for line in complete.split(separator: UInt8(ascii: "\n")) {
            guard let object = try? JSONSerialization.jsonObject(with: Data(line)) as? [String: Any] else { continue }
            if chunk.sessionID == nil { chunk.sessionID = object["sessionId"] as? String }
            if chunk.cwd == nil { chunk.cwd = object["cwd"] as? String }
            chunk.entries += entries(from: object)
        }
        return chunk
    }

    static func entries(from line: [String: Any]) -> [TranscriptEntry] {
        guard let type = line["type"] as? String, type == "user" || type == "assistant",
              (line["isSidechain"] as? Bool) != true, (line["isMeta"] as? Bool) != true,
              let message = line["message"] as? [String: Any] else { return [] }

        var result: [TranscriptEntry] = []
        if let text = message["content"] as? String {
            result.append(type == "user" ? .user(text) : .assistant(text))
        } else if let blocks = message["content"] as? [[String: Any]] {
            for block in blocks {
                switch block["type"] as? String {
                case "text":
                    if let text = block["text"] as? String { result.append(type == "user" ? .user(text) : .assistant(text)) }
                case "tool_use":
                    if let entry = toolUse(block) { result.append(entry) }
                default:
                    continue
                }
            }
        }
        if type == "assistant", let usage = message["usage"] as? [String: Any] {
            let value = { (key: String) in usage[key] as? Int ?? 0 }
            result.append(.usage(Usage(input: value("input_tokens"), cacheRead: value("cache_read_input_tokens"),
                                       cacheCreation: value("cache_creation_input_tokens")),
                                 model: message["model"] as? String))
        }
        return result
    }

    static func toolUse(_ block: [String: Any]) -> TranscriptEntry? {
        let input = block["input"] as? [String: Any] ?? [:]
        switch block["name"] as? String {
        case "Bash":
            return (input["command"] as? String).map(TranscriptEntry.command)
        case "Edit", "Write", "MultiEdit", "NotebookEdit":
            return ((input["file_path"] ?? input["notebook_path"]) as? String).map(TranscriptEntry.fileEdit)
        case "TodoWrite":
            let todos = (input["todos"] as? [[String: Any]] ?? [])
                .filter { ($0["status"] as? String) != "completed" }
                .compactMap { $0["content"] as? String }
            return .todos(todos)
        default:
            return nil
        }
    }
}

/// Where each session's transcript was last read up to, in `.binder/cursors.json`.
public struct Cursors: Sendable {
    public let vault: Vault

    public init(vault: Vault) {
        self.vault = vault
    }

    private func load() -> [String: UInt64] {
        guard let data = try? Data(contentsOf: vault.cursorsFile) else { return [:] }
        return (try? JSONDecoder().decode([String: UInt64].self, from: data)) ?? [:]
    }

    public func offset(for session: String) -> UInt64 {
        load()[session] ?? 0
    }

    public func set(_ offset: UInt64, for session: String) throws {
        var all = load()
        all[session] = offset
        try FileManager.default.createDirectory(at: vault.internalDir, withIntermediateDirectories: true)
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        try encoder.encode(all).write(to: vault.cursorsFile, options: .atomic)
    }
}

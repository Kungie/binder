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
    /// An image or PDF the user attached, base64 as the transcript has it, with the text sent alongside.
    case attachment(mediaType: String, base64: String, near: String)
    /// A Bash command whose result was an error, with the start of that error.
    case failed(command: String, error: String)
}

public struct TranscriptChunk: Sendable {
    public var entries: [TranscriptEntry]
    /// Where the next read should start: just after the last complete line.
    public var endOffset: UInt64
    public var sessionID: String?
    public var cwd: String?
    /// When the first line read was written: files made in the session are newer than this.
    public var firstTimestamp: Date?
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
        var commands: [String: String] = [:]
        for line in complete.split(separator: UInt8(ascii: "\n")) {
            guard let object = try? JSONSerialization.jsonObject(with: Data(line)) as? [String: Any] else { continue }
            if chunk.sessionID == nil { chunk.sessionID = object["sessionId"] as? String }
            if chunk.cwd == nil { chunk.cwd = object["cwd"] as? String }
            if chunk.firstTimestamp == nil, let stamp = object["timestamp"] as? String { chunk.firstTimestamp = Self.stamp(stamp) }
            chunk.entries += entries(from: object, commands: &commands)
        }
        return chunk
    }

    static func entries(from line: [String: Any]) -> [TranscriptEntry] {
        var commands: [String: String] = [:]
        return entries(from: line, commands: &commands)
    }

    /// `commands` remembers Bash commands by tool id, so a failed result later in the chunk can name its command.
    static func entries(from line: [String: Any], commands: inout [String: String]) -> [TranscriptEntry] {
        guard let type = line["type"] as? String, type == "user" || type == "assistant",
              (line["isSidechain"] as? Bool) != true, (line["isMeta"] as? Bool) != true,
              let message = line["message"] as? [String: Any] else { return [] }

        var result: [TranscriptEntry] = []
        if let text = message["content"] as? String {
            result.append(type == "user" ? .user(text) : .assistant(text))
        } else if let blocks = message["content"] as? [[String: Any]] {
            let near = blocks.compactMap { $0["type"] as? String == "text" ? $0["text"] as? String : nil }.joined(separator: " ")
            for block in blocks {
                switch block["type"] as? String {
                case "text":
                    if let text = block["text"] as? String { result.append(type == "user" ? .user(text) : .assistant(text)) }
                case "tool_use":
                    if let entry = toolUse(block) {
                        result.append(entry)
                        if case let .command(command) = entry, let id = block["id"] as? String { commands[id] = command }
                    }
                case "tool_result":
                    guard type == "user", (block["is_error"] as? Bool) == true, let id = block["tool_use_id"] as? String,
                          let command = commands[id] else { continue }
                    let error = (block["content"] as? String)
                        ?? (block["content"] as? [[String: Any]] ?? []).compactMap { $0["text"] as? String }.joined(separator: " ")
                    result.append(.failed(command: command, error: String(SecretFilter.redact(error).prefix(300))))
                case "image", "document":
                    // Only what the user attached: screenshots inside tool results never reach this level.
                    guard type == "user", let source = block["source"] as? [String: Any], source["type"] as? String == "base64",
                          let media = source["media_type"] as? String, let data = source["data"] as? String else { continue }
                    result.append(.attachment(mediaType: media, base64: data, near: String(near.prefix(300))))
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

    static func stamp(_ text: String) -> Date? {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return f.date(from: text) ?? ISO8601DateFormatter().date(from: text)
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

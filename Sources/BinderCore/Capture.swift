import Foundation

/// What Claude Code sends a hook on stdin.
public struct HookInput: Decodable, Equatable, Sendable {
    public var sessionID: String
    public var transcriptPath: String?
    public var cwd: String
    public var hookEventName: String?
    public var source: String?

    public init(sessionID: String, transcriptPath: String?, cwd: String, hookEventName: String?, source: String?) {
        self.sessionID = sessionID
        self.transcriptPath = transcriptPath
        self.cwd = cwd
        self.hookEventName = hookEventName
        self.source = source
    }

    enum CodingKeys: String, CodingKey {
        case sessionID = "session_id", transcriptPath = "transcript_path", cwd
        case hookEventName = "hook_event_name", source
    }
}

/// The latest state of one session, in `.binder/sessions/<id>.json`, for the menu bar.
public struct SessionStatus: Codable, Equatable, Sendable {
    public var session: String
    public var project: String?
    public var cwd: String
    public var reading: ContextReading?
    public var updatedAt: Date
    public var transcriptPath: String?

    public init(session: String, project: String?, cwd: String, reading: ContextReading?, updatedAt: Date, transcriptPath: String? = nil) {
        self.session = session
        self.project = project
        self.cwd = cwd
        self.reading = reading
        self.updatedAt = updatedAt
        self.transcriptPath = transcriptPath
    }

    static func url(_ session: String, _ vault: Vault) -> URL {
        vault.sessions.appendingPathComponent("\(session).json")
    }

    public static func load(session: String, vault: Vault) -> SessionStatus? {
        guard let data = try? Data(contentsOf: url(session, vault)) else { return nil }
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return try? decoder.decode(SessionStatus.self, from: data)
    }

    public func save(_ vault: Vault) throws {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.sortedKeys]
        try FileManager.default.createDirectory(at: vault.sessions, withIntermediateDirectories: true)
        try encoder.encode(self).write(to: Self.url(session, vault), options: .atomic)
    }
}

public struct CaptureResult: Equatable, Sendable {
    public var proposalsAdded = 0
    public var reading: ContextReading?
    /// The byte range read this time, so slow follow-up work (the summarizer) can read the same part later.
    public var startOffset: UInt64 = 0
    public var endOffset: UInt64 = 0
}

/// The Stop hook's work: read what's new in the transcript, turn it into proposals, remember where it stopped.
public enum Capture {
    public static func run(input: HookInput, vault: Vault, runner: CommandRunner, summarizer: Summarizer?) throws -> CaptureResult {
        guard let path = input.transcriptPath else { return CaptureResult() }
        let url = URL(fileURLWithPath: path)
        let cursors = Cursors(vault: vault)
        var offset = cursors.offset(for: input.sessionID)
        let size = (try? FileManager.default.attributesOfItem(atPath: path)[.size] as? UInt64) ?? 0
        if size < offset { offset = 0 } // the file was replaced

        let chunk = try Transcript.read(url, from: offset)
        let project = GitRepo.locate(from: URL(fileURLWithPath: input.cwd))?.name
        var proposals = chunk.entries.flatMap { entry -> [Proposal] in
            guard case let .command(command) = entry else { return [] }
            return CommandRules.proposals(for: command, project: project, session: input.sessionID)
        }
        if BinderState.load(vault).summarizerConsent, let summarizer {
            proposals += summarizer.proposals(from: chunk.entries, project: project)
        }

        let store = ProposalStore(vault: vault)
        var result = CaptureResult(startOffset: offset, endOffset: chunk.endOffset)
        for p in proposals where try store.add(p) { result.proposalsAdded += 1 }
        try cursors.set(chunk.endOffset, for: input.sessionID)

        result.reading = ContextMeter.reading(from: chunk.entries)
            ?? SessionStatus.load(session: input.sessionID, vault: vault)?.reading
        try SessionStatus(session: input.sessionID, project: project, cwd: input.cwd, reading: result.reading,
                          updatedAt: Date(), transcriptPath: input.transcriptPath).save(vault)
        return result
    }
}

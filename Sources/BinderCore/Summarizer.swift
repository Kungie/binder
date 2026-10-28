import Foundation

/// Picks durable decisions out of what was said in a session. Only used after the user agreed.
public protocol Summarizer: Sendable {
    func proposals(from entries: [TranscriptEntry], project: String?) -> [Proposal]
}

/// Asks the user's own Claude Code (`claude -p`) on their own plan, stripped bare: its own short system prompt,
/// no tools, no MCP servers, no plugins or skills, no saved session, run from an empty folder so no CLAUDE.md
/// loads. That keeps a call to about a thousand tokens of overhead instead of Claude Code's full context.
/// Any failure means no proposals and nothing marked outdated.
public struct ClaudeCLISummarizer: Summarizer {
    let runner: CommandRunner
    let session: String?

    public init(runner: CommandRunner, session: String?) {
        self.runner = runner
        self.session = session
    }

    /// A runner for these calls: never re-enters Binder's hooks, and runs from an empty scratch folder.
    public static func runner() -> SystemRunner {
        SystemRunner(extraEnvironment: ["BINDER_NESTED": "1"],
                     directory: FileManager.default.temporaryDirectory.appendingPathComponent("binder-claude", isDirectory: true))
    }

    /// Characters of conversation per call, and how many calls one run may make (the latest windows win).
    public static let limit = 12_000
    public static let maxWindows = 3
    /// The newest decisions shown to the model, per note.
    static let knownLimit = 40

    static func arguments(system: String) -> [String] {
        ["-p", "--model", "haiku", "--output-format", "text",
         "--system-prompt", system,
         "--tools", "", "--strict-mcp-config", "--setting-sources", "",
         "--disable-slash-commands", "--no-session-persistence"]
    }

    // MARK: Review: new things, and what they make untrue

    /// What the binder already says, newest first, as full decision lines.
    public struct Known: Sendable {
        public var project: [String]
        public var me: [String]

        public init(project: [String], me: [String]) {
            self.project = project
            self.me = me
        }

        /// The project's note and me.md, newest first.
        public static func load(vault: Vault, project: String?) -> Known {
            let projectLines = project.flatMap { NoteIO.read(vault.url("projects/\($0).md"))?.decisions } ?? []
            let meLines = NoteIO.read(vault.url("me.md"))?.decisions ?? []
            return Known(project: Array(projectLines.reversed().prefix(knownLimit)), me: Array(meLines.reversed().prefix(knownLimit)))
        }
    }

    public struct Outdated: Equatable, Sendable {
        public var target: String
        public var line: String
    }

    public struct Review: Sendable {
        public var proposals: [Proposal] = []
        public var outdated: [Outdated] = []
    }

    public static let reviewSystem = """
    You keep a developer's memory accurate. You get what the memory already says, as numbered lines, and part of a \
    coding session between the developer (USER) and their coding agent (ASSISTANT).

    Reply with only this JSON and nothing else:
    {"new":[{"kind":"decision|fact|preference|access","scope":"me|project","text":"one short sentence","confidence":"high|medium|low"}],"outdated":["P2","M1"]}

    new: durable things worth knowing in future sessions: decisions, hosting and deploy choices, conventions, \
    preferences the developer stated, access details (which host, which key, where a token is kept, never the token). \
    One specific sentence each, in English, with names, paths and versions. Skip one-off experiments, temporary states, \
    anything obvious from the code, and anything the memory already says.
    scope: "me" only for what is true of the developer across all their projects: how they like to work, their tools \
    and taste. Anything about this project, its code, releases, design or the people on it is "project".
    outdated: ids of memory lines this session makes untrue: replaced, reversed, a plan now finished, a question now \
    decided. Also M lines that are about one project rather than the developer. When unsure, leave a line out.
    Never include secrets, tokens or passwords. With nothing to report, reply {"new":[],"outdated":[]}.
    """

    public static func said(_ entry: TranscriptEntry) -> String? {
        switch entry {
        case let .user(text): "USER: \(text)"
        case let .assistant(text): "ASSISTANT: \(text)"
        default: nil
        }
    }

    /// The talk split into windows of at most `limit` characters, oldest first, keeping the latest `maxWindows`.
    public static func windows(_ entries: [TranscriptEntry]) -> [[TranscriptEntry]] {
        var windows: [[TranscriptEntry]] = []
        var current: [TranscriptEntry] = [], size = 0
        for entry in entries {
            guard let text = said(entry) else { continue }
            let length = min(text.count, limit)
            if size + length > limit, !current.isEmpty {
                windows.append(current)
                current = []
                size = 0
            }
            current.append(entry)
            size += length
        }
        if !current.isEmpty { windows.append(current) }
        return Array(windows.suffix(maxWindows))
    }

    /// One window of talk against what the binder already says.
    public func review(entries: [TranscriptEntry], project: String?, known: Known) -> Review {
        let talk = entries.compactMap(Self.said).map { String($0.prefix(Self.limit)) }
        guard !talk.isEmpty else { return Review() }
        var memory: [String] = []
        for (i, line) in known.project.enumerated() { memory.append("P\(i + 1). \(CardBuilder.decisionText(line))") }
        for (i, line) in known.me.enumerated() { memory.append("M\(i + 1). \(CardBuilder.decisionText(line))") }
        let input = """
        PROJECT: \(project ?? "none")

        MEMORY:
        \(memory.isEmpty ? "(empty)" : memory.joined(separator: "\n"))

        SESSION:
        \(SecretFilter.redact(talk.joined(separator: "\n")).suffix(Self.limit))
        """
        guard let result = runner.run("claude", Self.arguments(system: Self.reviewSystem), input: input, timeout: 90),
              result.status == 0, !result.timedOut, let json = Self.object(in: result.stdout) else { return Review() }

        var review = Review()
        for item in json["new"] as? [[String: Any]] ?? [] {
            guard let text = (item["text"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines), !text.isEmpty else { continue }
            let kind = Proposal.Kind(rawValue: item["kind"] as? String ?? "") ?? .fact
            let confidence = Proposal.Confidence(rawValue: item["confidence"] as? String ?? "") ?? .medium
            let target: String
            if item["scope"] as? String == "me" {
                target = "me.md"
            } else if let project {
                target = "projects/\(project).md"
            } else {
                continue // about a project, but this session isn't in one
            }
            review.proposals.append(Proposal.make(kind: kind, source: .chat, text: SecretFilter.redact(text), evidence: "said in session \(session ?? "?")",
                                                  target: target, project: project, session: session, confidence: confidence))
        }
        for id in json["outdated"] as? [String] ?? [] {
            guard let n = Int(id.dropFirst()), n >= 1 else { continue }
            if id.hasPrefix("P"), n <= known.project.count, let project {
                review.outdated.append(Outdated(target: "projects/\(project).md", line: known.project[n - 1]))
            } else if id.hasPrefix("M"), n <= known.me.count {
                review.outdated.append(Outdated(target: "me.md", line: known.me[n - 1]))
            }
        }
        return review
    }

    /// The `Summarizer` protocol's view: just the new things, without the memory to compare against.
    public func proposals(from entries: [TranscriptEntry], project: String?) -> [Proposal] {
        Self.windows(entries).flatMap { review(entries: $0, project: project, known: Known(project: [], me: [])).proposals }
    }

    // MARK: Tidy: stale lines in a note, without a session

    public static let tidySystem = """
    You keep a developer's memory accurate. You get the lines of one memory file, newest first, numbered L1, L2 and so on. \
    Reply with only this JSON and nothing else: {"outdated":["L5","L9"]}

    List the lines a newer line makes untrue (replaced, reversed, a plan since finished, a question since decided) \
    and lines that say the same thing as a newer line. Keep the newest of any pair. When unsure, leave a line out.
    """

    /// Lines (given newest first) that newer ones make untrue or repeat.
    public func outdated(in lines: [String], aboutPerson: Bool = false) -> [String] {
        guard lines.count >= 2 else { return [] }
        return ask(Self.tidySystem, about: lines, key: "outdated")
    }

    public static let personSystem = """
    You sort the lines of a developer's personal memory file, numbered L1, L2 and so on. The file should hold only     what is true of the developer across everything they do: how they like to work and be talked to, their language,     tools, habits, taste and rules for agents. Reply with only this JSON and nothing else: {"project":["L3","L8"]}

    List every line that is really about one specific project, product, website, app, repository, server or client:     its design, colors, features, releases, files or status. Such lines belong in that project's notes, not here.     Keep general working rules even if they name a tool. When unsure, leave a line out.
    """

    /// Lines in me.md that belong to a project rather than to the person.
    public func notAboutPerson(in lines: [String]) -> [String] {
        lines.isEmpty ? [] : ask(Self.personSystem, about: lines, key: "project")
    }

    /// Numbers the lines L1, L2… and returns the ones the model lists under `key`.
    private func ask(_ system: String, about lines: [String], key: String) -> [String] {
        let numbered = lines.enumerated().map { "L\($0.offset + 1). \(CardBuilder.decisionText($0.element))" }.joined(separator: "\n")
        guard let result = runner.run("claude", Self.arguments(system: system), input: numbered, timeout: 90),
              result.status == 0, !result.timedOut, let json = Self.object(in: result.stdout) else { return [] }
        return (json[key] as? [String] ?? []).compactMap { id in
            guard id.hasPrefix("L"), let n = Int(id.dropFirst()), n >= 1, n <= lines.count else { return nil }
            return lines[n - 1]
        }
    }

    // MARK: Handoff

    static let handoffSystem = """
    You read the end of a long coding session so a fresh session can take over. Reply with only one JSON object \
    and no other text: {"done":["short items"],"left":["short items"],"dontTouch":["files or areas to leave alone"],\
    "next":"the single next action"}. Use [] for empty lists. Never include secrets.
    """

    /// A handoff card written by the model, or nil when it can't run or replies with something unreadable.
    public func handoff(from entries: [TranscriptEntry]) -> HandoffCard? {
        let lines = entries.compactMap { entry -> String? in
            switch entry {
            case let .user(text): "USER: \(text)"
            case let .assistant(text): "ASSISTANT: \(text)"
            case let .fileEdit(path): "EDITED: \(path)"
            case let .todos(todos): "OPEN TODOS: \(todos.joined(separator: "; "))"
            default: nil
            }
        }
        guard !lines.isEmpty else { return nil }
        let condensed = String(SecretFilter.redact(lines.joined(separator: "\n")).suffix(Self.limit))
        guard let result = runner.run("claude", Self.arguments(system: Self.handoffSystem), input: "SESSION:\n" + condensed, timeout: 90),
              result.status == 0, !result.timedOut, let json = Self.object(in: result.stdout)
        else { return nil }
        let list = { (key: String) in (json[key] as? [String] ?? []).map { SecretFilter.redact($0) } }
        return HandoffCard(done: list("done"), left: list("left"), dontTouch: list("dontTouch"),
                           next: SecretFilter.redact(json["next"] as? String ?? ""))
    }

    /// The first `{…}` object in a reply, ignoring any chatter or code fences around it.
    static func object(in reply: String) -> [String: Any]? {
        guard let start = reply.firstIndex(of: "{"), let end = reply.lastIndex(of: "}"), start < end else { return nil }
        return try? JSONSerialization.jsonObject(with: Data(reply[start...end].utf8)) as? [String: Any]
    }
}

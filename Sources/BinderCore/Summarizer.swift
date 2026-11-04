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
        /// The project's topic notes, so the model reuses one instead of inventing a near twin.
        public var topics: [String]
        /// Which note each project line is in: the overview or a topic note. Empty means the overview.
        public var projectPaths: [String]
        /// Lines the developer marked wrong: never to be proposed again, in any wording.
        public var wrong: [String]

        public init(project: [String], me: [String], topics: [String] = [], projectPaths: [String] = [], wrong: [String] = []) {
            self.project = project
            self.me = me
            self.topics = topics
            self.projectPaths = projectPaths
            self.wrong = wrong
        }

        /// The project's overview and topic notes, and me.md, newest first.
        public static func load(vault: Vault, project: String?) -> Known {
            var lines: [(line: String, path: String)] = []
            var topics: [String] = []
            if let project {
                topics = Topic.topics(of: project, vault: vault)
                for path in ["projects/\(project).md"] + topics.map({ Topic.path(project: project, topic: $0) }) {
                    lines += (NoteIO.read(vault.url(path))?.decisions ?? []).map { ($0, path) }
                }
            }
            // Lines start with their day, so sorting them sorts by date; equal days keep note order.
            let newest = lines.enumerated().sorted { a, b in
                let (da, db) = (a.element.line.prefix(10), b.element.line.prefix(10))
                return da != db ? da > db : a.offset > b.offset
            }.map(\.element).prefix(knownLimit)
            let meLines = NoteIO.read(vault.url("me.md"))?.decisions ?? []
            let notes = ["me.md"] + (project.map { p in ["projects/\(p).md"] + topics.map { Topic.path(project: p, topic: $0) } } ?? [])
            return Known(project: newest.map(\.line), me: Array(meLines.reversed().prefix(knownLimit)), topics: topics,
                         projectPaths: newest.map(\.path), wrong: Forgetting.wrongLines(in: notes, vault: vault))
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
    {"new":[{"kind":"decision|fact|preference|access","scope":"me|project","text":"one short sentence","confidence":"high|medium|low",\
    "detail":"2-3 sentences: why, and what it touches","quote":"what was said, rendered in English, at most 200 characters",\
    "topic":"one or two words","also":["server:<name>","hosting:<provider>"],"until":"YYYY-MM-DD"}],"outdated":["P2","M1"]}

    Write every field in English, whatever language the session is in.

    new: durable things worth knowing in future sessions: decisions, hosting and deploy choices, conventions, \
    preferences the developer stated, access details (which host, which key, where a token is kept, never the token). \
    One specific sentence each, in English, with names, paths and versions. Skip one-off experiments, temporary states, \
    anything obvious from the code, and anything the memory already says.
    scope: "me" only for what is true of the developer across all their projects: how they like to work, their tools \
    and taste. Anything about this project, its code, releases, design or the people on it is "project".
    topic: for project items, the part of the project it is about (hosting, api, auth, payments). Reuse one of TOPICS \
    when it fits. Leave it out for something about the whole project.
    also: up to two other notes it belongs in: a server or machine it is about ("server:hetzner-1"), or a hosting \
    provider or service account ("hosting:cloudflare"). Leave it out otherwise.
    FAILED lines are commands that failed. When a failure teaches something lasting (a host needs sudo, a tool needs a \
    flag, a path is wrong), add it as a fact with "source":"failure". Skip one-off typos.
    until: only for a fact that stops being true on a known day ("down until Friday"): that day. Leave it out otherwise.
    NOT TRUE lists things the developer marked wrong. Never propose them again, in any wording.
    outdated: ids of memory lines this session makes untrue: replaced, reversed, a plan now finished, a question now \
    decided. Also M lines that are about one project rather than the developer. When unsure, leave a line out.
    Never include secrets, tokens or passwords. With nothing to report, reply {"new":[],"outdated":[]}.
    """

    /// A temporary fact's last day, `YYYY-MM-DD`: kept only when it's after today and within a year.
    static func until(_ value: Any?, now: Date = .now) -> Date? {
        guard let text = value as? String else { return nil }
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.timeZone = TimeZone(identifier: "UTC")
        f.dateFormat = "yyyy-MM-dd"
        guard let day = f.date(from: text), day > now, day <= now.addingTimeInterval(365 * 86_400) else { return nil }
        return day
    }

    /// A model-written field: trimmed, redacted, cut to `limit` characters; nil when missing or empty.
    static func field(_ value: Any?, limit: Int) -> String? {
        guard let text = (value as? String)?.trimmingCharacters(in: .whitespacesAndNewlines), !text.isEmpty else { return nil }
        return String(SecretFilter.redact(text).prefix(limit))
    }

    public static func said(_ entry: TranscriptEntry) -> String? {
        switch entry {
        case let .user(text): "USER: \(text)"
        case let .assistant(text): "ASSISTANT: \(text)"
        case let .failed(command, error): "FAILED: \(command) · \(error)"
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

    /// The memory block every prompt that adds things shows the model: numbered lines, topics, and what's not true.
    static func context(project: String?, known: Known) -> String {
        var memory: [String] = []
        for (i, line) in known.project.enumerated() { memory.append("P\(i + 1). \(CardBuilder.decisionText(line))") }
        for (i, line) in known.me.enumerated() { memory.append("M\(i + 1). \(CardBuilder.decisionText(line))") }
        let topics = (known.topics.isEmpty ? "" : "\nTOPICS: \(known.topics.joined(separator: ", "))")
            + (known.wrong.isEmpty ? "" : "\n\nNOT TRUE:\n" + known.wrong.map { "- \($0)" }.joined(separator: "\n"))
        return """
        PROJECT: \(project ?? "none")\(topics)

        MEMORY:
        \(memory.isEmpty ? "(empty)" : memory.joined(separator: "\n"))
        """
    }

    /// The `new` items of a reply as proposals: target from scope and topic, extra notes, detail, quote and expiry.
    func parseNew(_ items: [[String: Any]], project: String?, evidence: String, source: Proposal.Source) -> [Proposal] {
        var out: [Proposal] = []
        for item in items {
            guard let text = (item["text"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines), !text.isEmpty else { continue }
            let kind = Proposal.Kind(rawValue: item["kind"] as? String ?? "") ?? .fact
            let confidence = Proposal.Confidence(rawValue: item["confidence"] as? String ?? "") ?? .medium
            let target: String
            if item["scope"] as? String == "me" {
                target = "me.md"
            } else if let project {
                target = (item["topic"] as? String).flatMap(Topic.slug).map { Topic.path(project: project, topic: $0) } ?? "projects/\(project).md"
            } else {
                continue // about a project, but there's no project here
            }
            let also = (item["also"] as? [Any] ?? []).compactMap { ($0 as? String).flatMap(Destination.path) }
                .reduce(into: [String]()) { if !$0.contains($1) && $1 != target && $0.count < 2 { $0.append($1) } }
            let from: Proposal.Source = item["source"] as? String == "failure" ? .failure : source
            var proposal = Proposal.make(kind: kind, source: from, text: SecretFilter.redact(text), evidence: evidence,
                                         target: target, project: project, session: session, confidence: confidence,
                                         detail: Self.field(item["detail"], limit: 600), quote: Self.field(item["quote"], limit: 200), also: also)
            proposal.until = Self.until(item["until"])
            out.append(proposal)
        }
        return out
    }

    /// One window of talk against what the binder already says.
    public func review(entries: [TranscriptEntry], project: String?, known: Known) -> Review {
        let talk = entries.compactMap(Self.said).map { String($0.prefix(Self.limit)) }
        guard !talk.isEmpty else { return Review() }
        let input = Self.context(project: project, known: known) + """


        SESSION:
        \(SecretFilter.redact(talk.joined(separator: "\n")).suffix(Self.limit))
        """
        guard let result = runner.run("claude", Self.arguments(system: Self.reviewSystem), input: input, timeout: 90),
              result.status == 0, !result.timedOut, let json = Self.object(in: result.stdout) else { return Review() }

        var review = Review()
        review.proposals = parseNew(json["new"] as? [[String: Any]] ?? [], project: project, evidence: "said in session \(session ?? "?")", source: .chat)
        for id in json["outdated"] as? [String] ?? [] {
            guard let n = Int(id.dropFirst()), n >= 1 else { continue }
            if id.hasPrefix("P"), n <= known.project.count, let project {
                let target = n <= known.projectPaths.count ? known.projectPaths[n - 1] : "projects/\(project).md"
                review.outdated.append(Outdated(target: target, line: known.project[n - 1]))
            } else if id.hasPrefix("M"), n <= known.me.count {
                review.outdated.append(Outdated(target: "me.md", line: known.me[n - 1]))
            }
        }
        return review
    }

    // MARK: Code: a repo's architecture as memory lines

    public static let codeSystem = """
    You summarize a software project for a developer's binder, so a coding agent starting fresh knows how it fits \
    together. You get what the memory already says, and the repo's file list, README and build and deploy files.

    Reply with only this JSON and nothing else:
    {"new":[{"kind":"fact","scope":"project","text":"one short sentence","confidence":"high|medium|low",\
    "detail":"one or two sentences","topic":"architecture"}]}

    Write every field in English. At most 15 items: the main parts and what each does, how they talk, where things \
    live, how it builds, tests and deploys. Be specific, with names and paths. Use the topic "architecture" unless a \
    line is clearly about hosting, release, api or another part. Skip anything the memory already says. \
    Never include secrets, tokens or passwords.
    """

    /// Architecture lines as catches, or nil when the model can't run or answers with something unreadable.
    public func codeLines(digest: String, project: String, known: Known = Known(project: [], me: [])) -> [Proposal]? {
        let input = Self.context(project: project, known: known) + "\n\nREPO:\n" + digest
        guard let result = runner.run("claude", Self.arguments(system: Self.codeSystem), input: input, timeout: 120),
              result.status == 0, !result.timedOut, let json = Self.object(in: result.stdout) else { return nil }
        let items = (json["new"] as? [[String: Any]] ?? []).prefix(15).map { item -> [String: Any] in
            var item = item
            item["scope"] = "project"
            if (item["topic"] as? String).flatMap(Topic.slug) == nil { item["topic"] = "architecture" }
            return item
        }
        return parseNew(Array(items), project: project, evidence: "summarized from the code", source: .observed)
    }

    // MARK: Import: an agent rules file split into memory lines

    public static let importSystem = """
    You turn a developer's agent instructions file (CLAUDE.md or AGENTS.md) into memory lines for their binder. You get \
    what the memory already says, as numbered lines, and the file.

    Reply with only this JSON and nothing else:
    {"new":[{"kind":"decision|fact|preference|access","scope":"me|project","text":"one short sentence","confidence":"high|medium|low",\
    "detail":"why, if the file says","topic":"one or two words"}]}

    Write every field in English, whatever language the file is in. At most 40 items. One durable rule or fact per \
    item: working style, conventions, stack, hosting, deploy, where things live. Skip anything the memory already says, \
    generic advice, and anything that only explains the file itself. scope "me" is for how the developer works across \
    all projects; anything about this project is "project", with a topic such as hosting, api or release. \
    Never include secrets, tokens or passwords. With nothing to add, reply {"new":[]}.
    """

    /// The file's lines as catches, or nil when the model can't run or answers with something unreadable.
    public func importLines(text: String, fileName: String, project: String?, known: Known) -> [Proposal]? {
        let input = Self.context(project: project, known: known) + "\n\nFILE: \(fileName)\n" + SecretFilter.redact(String(text.prefix(Self.limit)))
        guard let result = runner.run("claude", Self.arguments(system: Self.importSystem), input: input, timeout: 120),
              result.status == 0, !result.timedOut, let json = Self.object(in: result.stdout) else { return nil }
        let items = Array((json["new"] as? [[String: Any]] ?? []).prefix(40))
        return parseNew(items, project: project, evidence: "from \(fileName)", source: .observed)
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

    // MARK: Topics: a project's lines sorted into topic notes

    public static let topicSystem = """
    You sort the memory lines of one software project, numbered L1, L2 and so on, into topics: the part of the project \
    each line is about, such as hosting, api, auth, payments or design. Reuse one of TOPICS when it fits. Topic names are \
    English, one or two words. Leave out lines about the whole project, and lines you are unsure about.
    Reply with only this JSON and nothing else: {"topics":{"hosting":["L2","L5"],"api":["L3"]}}
    """

    /// Topic slug → the lines (exactly as given) the model put under it. Each line goes to one topic at most.
    public func topicsFor(lines: [String], existing: [String]) -> [String: [String]] {
        guard !lines.isEmpty else { return [:] }
        let numbered = lines.enumerated().map { "L\($0.offset + 1). \(CardBuilder.decisionText($0.element))" }.joined(separator: "\n")
        let input = (existing.isEmpty ? "" : "TOPICS: \(existing.joined(separator: ", "))\n\n") + numbered
        guard let result = runner.run("claude", Self.arguments(system: Self.topicSystem), input: input, timeout: 90),
              result.status == 0, !result.timedOut, let json = Self.object(in: result.stdout),
              let topics = json["topics"] as? [String: Any] else { return [:] }
        var sorted: [String: [String]] = [:], taken = Set<String>()
        for (name, ids) in topics.sorted(by: { $0.key < $1.key }) {
            guard let slug = Topic.slug(name) else { continue }
            for case let id as String in ids as? [Any] ?? [] {
                guard id.hasPrefix("L"), let n = Int(id.dropFirst()), n >= 1, n <= lines.count, !taken.contains(lines[n - 1]) else { continue }
                taken.insert(lines[n - 1])
                sorted[slug, default: []].append(lines[n - 1])
            }
        }
        return sorted
    }

    // MARK: Captions for caught files

    public static let captionSystem = """
    You write short captions for images and PDFs a developer kept from coding sessions. For each file you get its id, \
    its name and what was said around it, in any language. Write each caption in English, at most 12 words, saying what \
    the file shows. Never include secrets. Reply with only this JSON and nothing else: {"captions":{"p_1":"Caption"}}
    """

    /// File id → an English caption. Empty when the model can't run or answers with something unreadable.
    public func captions(for files: [(id: String, near: String, name: String)]) -> [String: String] {
        guard !files.isEmpty else { return [:] }
        let input = files.map { "\($0.id) · \($0.name) · \(SecretFilter.redact($0.near))" }.joined(separator: "\n")
        guard let result = runner.run("claude", Self.arguments(system: Self.captionSystem), input: input, timeout: 90),
              result.status == 0, !result.timedOut, let json = Self.object(in: result.stdout),
              let captions = json["captions"] as? [String: Any] else { return [:] }
        var out: [String: String] = [:]
        for (id, value) in captions {
            guard let text = Self.field(value, limit: 120) else { continue }
            out[id] = text.split(separator: " ").prefix(12).joined(separator: " ")
        }
        return out
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

import CryptoKit
import Darwin
import Foundation

/// How often sessions used a note or a line: on how many session cards (exact) and how often an agent
/// acted on it (an estimate from commands and replies).
public struct UseCounts: Codable, Equatable, Sendable {
    public var cards = 0
    public var used = 0
    public var lastCard: Date?
    public var lastUsed: Date?

    public init() {}

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        cards = try c.decodeIfPresent(Int.self, forKey: .cards) ?? 0
        used = try c.decodeIfPresent(Int.self, forKey: .used) ?? 0
        lastCard = try c.decodeIfPresent(Date.self, forKey: .lastCard)
        lastUsed = try c.decodeIfPresent(Date.self, forKey: .lastUsed)
    }

    mutating func card(_ now: Date) {
        cards += 1
        lastCard = now
    }

    mutating func use(_ now: Date) {
        used += 1
        lastUsed = now
    }
}

/// `.binder/usage.json`: counts only. Lines are keyed by a hash of their text, so no text is copied here.
public struct UsageStore: Sendable {
    public let vault: Vault
    /// Sessions whose card was already counted, so a second SessionStart (resume, `/clear`) doesn't count twice.
    static let sessionMemory = 500

    public init(vault: Vault) { self.vault = vault }

    struct File: Codable {
        var notes: [String: UseCounts] = [:]
        var lines: [String: [String: UseCounts]] = [:]
        var cardSessions: [String] = []
        /// Lines kept on the card by hand (Keep on card), by note: they never fade.
        var pins: [String: [String]] = [:]

        init() {}

        init(from decoder: Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            notes = try c.decodeIfPresent([String: UseCounts].self, forKey: .notes) ?? [:]
            lines = try c.decodeIfPresent([String: [String: UseCounts]].self, forKey: .lines) ?? [:]
            cardSessions = try c.decodeIfPresent([String].self, forKey: .cardSessions) ?? []
            pins = try c.decodeIfPresent([String: [String]].self, forKey: .pins) ?? [:]
        }
    }

    var url: URL { vault.url(".binder/usage.json") }

    func load() -> File {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return (try? Data(contentsOf: url)).flatMap { try? decoder.decode(File.self, from: $0) } ?? File()
    }

    /// Read, change and write under a lock: the card hook and the capture hook can run at the same moment.
    func change(_ body: (inout File) -> Void) {
        try? FileManager.default.createDirectory(at: vault.internalDir, withIntermediateDirectories: true)
        let lock = open(vault.url(".binder/usage.lock").path, O_CREAT | O_RDWR, 0o644)
        if lock >= 0 { flock(lock, LOCK_EX) }
        defer { if lock >= 0 { flock(lock, LOCK_UN); close(lock) } }
        var file = load()
        body(&file)
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.sortedKeys]
        try? encoder.encode(file).write(to: url, options: .atomic)
    }

    static func key(_ line: String) -> String {
        let digest = SHA256.hash(data: Data(ProposalStore.normalize(CardBuilder.decisionText(line)).utf8))
        return digest.prefix(12).map { String(format: "%02x", $0) }.joined()
    }

    // MARK: Recording

    public func recordCard(session: String, notes: [String], lines: [String: [String]], now: Date = .now) {
        change { file in
            guard !file.cardSessions.contains(session) else { return }
            file.cardSessions = Array((file.cardSessions + [session]).suffix(Self.sessionMemory))
            for note in Set(notes) { file.notes[note, default: UseCounts()].card(now) }
            for (note, list) in lines {
                for key in Set(list.map(Self.key)) { file.lines[note, default: [:]][key, default: UseCounts()].card(now) }
            }
        }
    }

    /// Commands that name a note's host, address, key, domain or tool, and replies that repeat a decision.
    public func recordUse(commands: [String], replies: [String], now: Date = .now) {
        guard !commands.isEmpty || !replies.isEmpty else { return }
        let handles = UsageHandles.handles(vault: vault)
        var noteHits: [String: Int] = [:], lineHits: [String: [String: Int]] = [:]
        for command in commands {
            for (note, list) in handles where list.contains(where: { UsageHandles.mentions(command, $0) }) { noteHits[note, default: 0] += 1 }
        }
        let said = replies.map(ProposalStore.normalize)
        if !said.isEmpty {
            for note in UsageHandles.notes(vault: vault) {
                for line in NoteIO.read(vault.url(note))?.decisions ?? [] {
                    let text = ProposalStore.normalize(CardBuilder.decisionText(line))
                    guard text.split(separator: " ").count >= 5 else { continue }
                    let hits = said.filter { $0.contains(text) }.count
                    guard hits > 0 else { continue }
                    lineHits[note, default: [:]][Self.key(line), default: 0] += hits
                    noteHits[note, default: 0] += hits
                }
            }
        }
        guard !noteHits.isEmpty else { return }
        change { file in
            for (note, n) in noteHits { for _ in 0..<n { file.notes[note, default: UseCounts()].use(now) } }
            for (note, keys) in lineHits {
                for (key, n) in keys { for _ in 0..<n { file.lines[note, default: [:]][key, default: UseCounts()].use(now) } }
            }
        }
    }

    /// Lines `binder search` handed to an agent count as used.
    public func recordSearch(hits: [SearchHit], now: Date = .now) {
        guard !hits.isEmpty else { return }
        change { file in
            for note in Set(hits.map(\.path)) { file.notes[note, default: UseCounts()].use(now) }
            for hit in hits { file.lines[hit.path, default: [:]][Self.key(hit.line), default: UseCounts()].use(now) }
        }
    }

    // MARK: Pins

    public func pin(line: String, in note: String) {
        change { file in
            let key = Self.key(line)
            if !(file.pins[note] ?? []).contains(key) { file.pins[note, default: []].append(key) }
        }
    }

    public func unpin(line: String, in note: String) {
        change { file in file.pins[note]?.removeAll { $0 == Self.key(line) } }
    }

    public func isPinned(line: String, in note: String) -> Bool { load().pins[note]?.contains(Self.key(line)) == true }

    // MARK: Reading

    public func counts(note: String) -> UseCounts { load().notes[note] ?? UseCounts() }

    public func noteCounts() -> [String: UseCounts] { load().notes }

    public func counts(line: String, in note: String) -> UseCounts { load().lines[note]?[Self.key(line)] ?? UseCounts() }

    public func totals() -> UseCounts {
        load().notes.values.reduce(into: UseCounts()) { total, c in
            total.cards += c.cards
            total.used += c.used
            total.lastCard = [total.lastCard, c.lastCard].compactMap { $0 }.max()
            total.lastUsed = [total.lastUsed, c.lastUsed].compactMap { $0 }.max()
        }
    }
}

/// What in a command points at a note: a host, an address, a key, a domain, a repo or a provider's tool.
public enum UsageHandles {
    static let tools = ["cloudflare": ["wrangler"], "fly": ["fly", "flyctl"], "vercel": ["vercel"], "netlify": ["netlify"]]

    /// Every note Binder keeps, topic notes included.
    static func notes(vault: Vault) -> [String] {
        var paths = ["me.md"]
        let fm = FileManager.default
        for folder in ["machines", "access", "servers", "hosting", "projects"] {
            for name in ((try? fm.contentsOfDirectory(atPath: vault.url(folder).path)) ?? []).sorted() {
                if name.hasSuffix(".md") { paths.append("\(folder)/\(name)"); continue }
                guard folder == "projects" else { continue }
                paths += ((try? fm.contentsOfDirectory(atPath: vault.url("projects/\(name)").path)) ?? []).filter { $0.hasSuffix(".md") }.sorted()
                    .map { "projects/\(name)/\($0)" }
            }
        }
        return paths.filter { fm.fileExists(atPath: vault.url($0).path) }
    }

    public static func handles(vault: Vault) -> [String: [String]] {
        var result: [String: [String]] = [:]
        for path in notes(vault: vault) {
            guard let note = NoteIO.read(vault.url(path)) else { continue }
            let stem = String((path as NSString).lastPathComponent.dropLast(3))
            let text = note.render()
            var list: [String] = []
            switch path.split(separator: "/").first.map(String.init) {
            case "servers"?, "machines"?:
                if stem != "this-mac" { list.append(stem) }
                list += text.matches(of: #/\b\d{1,3}(?:\.\d{1,3}){3}\b/#).map { String($0.output) }
                list += note.observedLines.flatMap { line in line.matches(of: #/\b[a-z0-9-]+(?:\.[a-z0-9-]+)+\.[a-z]{2,}\b/#).map { String($0.output) } }
            case "access"?:
                list += note.observedLines.compactMap { line in line.firstMatch(of: #/^- key ([^:\s]+):/#).map { String($0.1) } }
            case "hosting"?:
                list += tools[stem] ?? [stem]
            case "projects"?:
                for line in note.observedLines {
                    if let domains = line.firstMatch(of: #/^- domains: (.+)$/#) {
                        list += domains.1.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }
                    }
                    if let repo = line.firstMatch(of: #/^- repo: \S*/([^/\s]+)/#) { list.append(String(repo.1)) }
                }
            default:
                break
            }
            let clean = Array(Set(list.filter { $0.count >= 3 })).sorted()
            if !clean.isEmpty { result[path] = clean }
        }
        return result
    }

    /// The handle appears as a whole word: `hetzner-1` matches `ssh root@hetzner-1`, never `hetzner-10`.
    static func mentions(_ command: String, _ handle: String) -> Bool {
        let escaped = NSRegularExpression.escapedPattern(for: handle)
        return command.range(of: #"(?<![\w.-])"# + escaped + #"(?![\w.-])"#, options: [.regularExpression, .caseInsensitive]) != nil
    }
}

/// The SessionStart card for a folder: cached when nothing changed, and counted once per session either way.
public enum SessionCard {
    public static func make(vault: Vault, cwd: String, session: String, helper: String? = nil, now: Date = .now) -> String {
        let repo = CardBuilder.project(for: URL(fileURLWithPath: cwd), vault: vault)
        let built: (card: String, notes: [String], lines: [String: [String]])
        if let repo {
            let cache = CardCache(vault: vault)
            if let cached = cache.cachedIncluded(for: repo) {
                built = cached
            } else {
                built = CardBuilder.buildIncluded(vault: vault, repo: repo, helper: helper)
                try? cache.store(built.card, included: (built.notes, built.lines), for: repo)
            }
            if Handoff.pending(project: repo.name, vault: vault) != nil { try? Handoff.markConsumed(project: repo.name, vault: vault) }
        } else {
            built = CardBuilder.buildIncluded(vault: vault, repo: nil, helper: helper)
        }
        UsageStore(vault: vault).recordCard(session: session, notes: built.notes, lines: built.lines, now: now)
        return built.card
    }
}

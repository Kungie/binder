import Foundation

/// Keeps notes fresh: older lines that repeat a newer one, or that a newer one made untrue, move to
/// `## Superseded`. Nothing is deleted; the card just stops showing them.
public enum Tidy {
    /// Older lines whose text matches a newer one (lines given newest first). No model needed.
    public static func duplicates(_ newestFirst: [String]) -> [String] {
        var seen = Set<String>()
        var repeats: [String] = []
        for line in newestFirst {
            let key = ProposalStore.normalize(CardBuilder.decisionText(line))
            if seen.contains(key) { repeats.append(line) } else { seen.insert(key) }
        }
        return repeats
    }

    /// The notes worth tidying: you, your projects and your servers.
    static func notes(in vault: Vault) -> [String] {
        var paths = ["me.md"]
        for folder in ["projects", "servers", "machines"] {
            let names = (try? FileManager.default.contentsOfDirectory(atPath: vault.url(folder).path)) ?? []
            paths += names.filter { $0.hasSuffix(".md") }.sorted().map { "\(folder)/\($0)" }
        }
        return paths.filter { FileManager.default.fileExists(atPath: vault.url($0).path) }
    }

    /// Tidies every note changed since its last tidy (all of them with `everything`). Duplicates always go;
    /// with a summarizer (the user agreed to use their Claude plan) so do lines newer ones made untrue.
    /// Returns how many lines moved, per note.
    @discardableResult
    public static func run(vault: Vault, summarizer: ClaudeCLISummarizer?, everything: Bool = false, now: Date = .now) -> [String: Int] {
        var record = TidyRecord.load(vault)
        var moved: [String: Int] = [:]
        for path in notes(in: vault) {
            let url = vault.url(path)
            let modified = (try? FileManager.default.attributesOfItem(atPath: url.path)[.modificationDate] as? Date) ?? now
            if !everything, let last = record.tidied[path], last >= modified { continue }
            guard let note = NoteIO.read(url) else { continue }
            let newestFirst = Array(note.decisions.reversed())
            var stale = duplicates(newestFirst)
            if let summarizer, !newestFirst.isEmpty {
                var fresh = newestFirst.filter { !stale.contains($0) }
                stale += summarizer.outdated(in: Array(fresh.prefix(60)))
                if path == "me.md" {
                    // A separate, single question works better than folding it into the one above.
                    fresh = fresh.filter { !stale.contains($0) }
                    stale += summarizer.notAboutPerson(in: Array(fresh.prefix(80)))
                }
            }
            if !stale.isEmpty {
                try? Memory.supersede(stale, in: path, vault: vault, now: now)
                moved[path] = Set(stale).count
            }
            let after = (try? FileManager.default.attributesOfItem(atPath: url.path)[.modificationDate] as? Date) ?? now
            record.tidied[path] = max(after, now)
        }
        try? record.save(vault)
        return moved
    }
}

/// When each note was last tidied, so only notes that changed since get another look.
struct TidyRecord: Codable {
    var tidied: [String: Date] = [:]

    static func url(_ vault: Vault) -> URL { vault.internalDir.appendingPathComponent("tidy.json") }

    // Seconds with fractions: file dates have them, and a rounded record would make every note look changed.
    static func load(_ vault: Vault) -> TidyRecord {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .secondsSince1970
        return (try? Data(contentsOf: url(vault))).flatMap { try? decoder.decode(TidyRecord.self, from: $0) } ?? TidyRecord()
    }

    func save(_ vault: Vault) throws {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .secondsSince1970
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        try FileManager.default.createDirectory(at: vault.internalDir, withIntermediateDirectories: true)
        try encoder.encode(self).write(to: Self.url(vault), options: .atomic)
    }
}

/// Batches the summarizer: instead of every reply, it reads a session's talk once enough has piled up, once
/// a while has passed, or when the session goes quiet (the app checks). Each byte is claimed exactly once.
public struct SummarizeQueue {
    let vault: Vault
    /// Characters of new talk that are worth a call on their own.
    public static let minChars = 4_000
    /// After this long, whatever's waiting goes even if it's short.
    public static let maxWait: TimeInterval = 20 * 60

    public init(vault: Vault) { self.vault = vault }

    struct Entry: Codable {
        var offset: UInt64
        var last: Date
    }

    var url: URL { vault.internalDir.appendingPathComponent("summarize.json") }

    func load() -> [String: Entry] {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return (try? Data(contentsOf: url)).flatMap { try? decoder.decode([String: Entry].self, from: $0) } ?? [:]
    }

    func save(_ entries: [String: Entry]) {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        try? FileManager.default.createDirectory(at: vault.internalDir, withIntermediateDirectories: true)
        try? encoder.encode(entries).write(to: url, options: .atomic)
    }

    /// The byte range to summarize now, or nil to wait. A session Binder hasn't seen before starts at
    /// `startingAt` (where capture first read it), so a resumed old session isn't read from the top.
    /// `force` is the app noticing the session went quiet: anything waiting goes.
    public func claim(session: String, transcript: URL, startingAt: UInt64, now: Date, force: Bool = false) -> Range<UInt64>? {
        var entries = load()
        let size = (try? FileManager.default.attributesOfItem(atPath: transcript.path)[.size] as? UInt64) ?? 0
        var entry = entries[session] ?? Entry(offset: startingAt, last: now)
        if entry.offset > size { entry.offset = 0 } // the file was replaced
        defer { entries[session] = entry; save(entries) }
        guard let chunk = try? Transcript.read(transcript, from: entry.offset), chunk.endOffset > entry.offset else { return nil }
        let waiting = chunk.entries.compactMap(ClaudeCLISummarizer.said).reduce(0) { $0 + $1.count }
        guard waiting > 0 else {
            entry.offset = chunk.endOffset // only tool calls since: nothing to read, move on
            return nil
        }
        guard force || waiting >= Self.minChars || now.timeIntervalSince(entry.last) >= Self.maxWait else { return nil }
        let range = entry.offset..<chunk.endOffset
        entry = Entry(offset: chunk.endOffset, last: now)
        return range
    }

    /// Sessions with talk waiting, for the app's quiet-session sweep.
    public func sessions() -> [String] { Array(load().keys) }
}

import Foundation

/// What a fresh session needs to pick up where a long one stopped.
public struct HandoffCard: Equatable, Sendable {
    public var done: [String]
    public var left: [String]
    public var dontTouch: [String]
    public var next: String

    public init(done: [String], left: [String], dontTouch: [String], next: String) {
        self.done = done
        self.left = left
        self.dontTouch = dontTouch
        self.next = next
    }
}

/// Handoff cards live in `handoffs/<project>/<yyyy-MM-dd-HHmm>.md`, visible in Obsidian like any note.
public enum Handoff {
    static let placeholders = ["nothing done", "nothing left", "nothing noted"]

    /// Without the summarizer: what was edited, the open todos, and the last thing the user asked for.
    public static func fallback(from entries: [TranscriptEntry]) -> HandoffCard {
        var done: [String] = []
        var left: [String] = []
        var next = ""
        for entry in entries {
            switch entry {
            case let .fileEdit(path):
                let line = "Edited \((path as NSString).lastPathComponent)"
                if !done.contains(line) { done.append(line) }
            case let .todos(todos):
                left = todos
            case let .user(text):
                let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
                if !trimmed.isEmpty { next = String(trimmed.prefix(200)) }
            default:
                continue
            }
        }
        return HandoffCard(done: done, left: left, dontTouch: [], next: next)
    }

    public static func write(_ card: HandoffCard, project: String, vault: Vault, now: Date) throws -> URL {
        let dir = vault.url("handoffs/\(project)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let stamp = format("yyyy-MM-dd-HHmm", now)
        var url = dir.appendingPathComponent("\(stamp).md")
        var n = 2
        while FileManager.default.fileExists(atPath: url.path) {
            url = dir.appendingPathComponent("\(stamp)-\(n).md")
            n += 1
        }
        func list(_ items: [String], empty: String) -> String {
            (items.isEmpty ? [empty] : items).map { "- \($0)" }.joined(separator: "\n")
        }
        let body = """
        # Handoff · \(project) · \(format("yyyy-MM-dd HH:mm", now))

        ## Done
        \(list(card.done, empty: "nothing done"))

        ## Left
        \(list(card.left, empty: "nothing left"))

        ## Don't touch
        \(list(card.dontTouch, empty: "nothing noted"))

        ## Next
        \(card.next.isEmpty ? "nothing noted" : card.next)

        """
        let note = Note(body: body).setting("created", ISO8601DateFormatter().string(from: now)).setting("consumed", "false")
        try NoteIO.write(note, to: url)
        return url
    }

    /// The card as a message to paste into a new session, for agents Binder can't start itself
    /// (Claude Code in the desktop app, say). Empty sections are left out.
    public static func prompt(_ card: HandoffCard, project: String) -> String {
        var parts = ["We're picking up work on \(project) from a previous session. Here's where it stopped."]
        for (title, items) in [("Done", card.done), ("Left", card.left), ("Don't touch", card.dontTouch)] where !items.isEmpty {
            parts.append("\(title):\n" + items.map { "- \($0)" }.joined(separator: "\n"))
        }
        if !card.next.isEmpty { parts.append("Next: \(card.next)") }
        parts.append("Start with the next step. Ask me if something here doesn't match what you find.")
        return parts.joined(separator: "\n\n")
    }

    /// The newest card for `project` that no session has picked up yet.
    public static func pending(project: String, vault: Vault) -> HandoffCard? {
        pendingFiles(project: project, vault: vault).last.flatMap { NoteIO.read($0) }.map(parse)
    }

    /// The newest card for `project`, picked up or not.
    public static func latest(project: String, vault: Vault) -> (card: HandoffCard, url: URL)? {
        guard let url = files(project: project, vault: vault).last, let note = NoteIO.read(url) else { return nil }
        return (parse(note), url)
    }

    public static func markConsumed(project: String, vault: Vault) throws {
        for url in pendingFiles(project: project, vault: vault) {
            try NoteIO.update(url, title: "Handoff") { $0.setting("consumed", "true") }
        }
    }

    static func files(project: String, vault: Vault) -> [URL] {
        let dir = vault.url("handoffs/\(project)")
        let names = (try? FileManager.default.contentsOfDirectory(atPath: dir.path)) ?? []
        return names.filter { $0.hasSuffix(".md") }.sorted().map { dir.appendingPathComponent($0) }
    }

    static func pendingFiles(project: String, vault: Vault) -> [URL] {
        files(project: project, vault: vault).filter { NoteIO.read($0)?.value("consumed") == "false" }
    }

    static func parse(_ note: Note) -> HandoffCard {
        var sections: [String: [String]] = [:]
        var current: String?
        for line in note.body.components(separatedBy: "\n") {
            if line.hasPrefix("## ") {
                current = String(line.dropFirst(3))
                sections[current!] = []
            } else if let current, !line.trimmingCharacters(in: .whitespaces).isEmpty {
                sections[current]?.append(line)
            }
        }
        func items(_ name: String) -> [String] {
            (sections[name] ?? []).map { $0.hasPrefix("- ") ? String($0.dropFirst(2)) : $0 }.filter { !placeholders.contains($0) }
        }
        let next = items("Next").joined(separator: " ")
        return HandoffCard(done: items("Done"), left: items("Left"), dontTouch: items("Don't touch"), next: next)
    }

    static func format(_ pattern: String, _ date: Date) -> String {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.dateFormat = pattern
        return f.string(from: date)
    }
}

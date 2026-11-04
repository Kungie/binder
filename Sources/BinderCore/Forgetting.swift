import Foundation

/// What leaves the card on its own: old lines no agent ever used (fading).
public enum Forgetting {
    public static let fadeAfterDays = 90

    /// A decision fades when it's at least 90 days old, no agent ever used it, and nobody pinned it.
    /// You (me.md) and your access notes never fade, and neither does a line without a date.
    public static func fades(_ line: String, in note: String, usage: UsageStore, now: Date = .now) -> Bool {
        Checker(usage: usage, now: now).fades(line, in: note)
    }

    /// The same rule with usage read once, for the card and the main window.
    public struct Checker {
        let file: UsageStore.File
        let now: Date

        public init(usage: UsageStore, now: Date) {
            file = usage.load()
            self.now = now
        }

        public func fades(_ line: String, in note: String) -> Bool {
            guard note != "me.md", !note.hasPrefix("access/"), let day = Forgetting.day(line),
                  now.timeIntervalSince(day) >= Double(fadeAfterDays) * 86_400 else { return false }
            let key = UsageStore.key(line)
            if file.pins[note]?.contains(key) == true { return false }
            return (file.lines[note]?[key]?.used ?? 0) == 0
        }
    }

    /// The user says a line isn't true: it moves to Superseded, marked `wrong <day>` with its detail, and its text is
    /// remembered as dropped for that note, so the same thing is never offered again.
    public static func markWrong(_ line: String, in note: String, vault: Vault, now: Date = .now) throws {
        let url = vault.url(note)
        guard let current = NoteIO.read(url) else { return }
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.dateFormat = "yyyy-MM-dd"
        let next = current.superseding([line], on: f.string(from: now), marker: "wrong")
        guard next != current else { return }
        try NoteIO.write(next, to: url)
        try ProposalStore(vault: vault).rememberDropped(target: note, text: CardBuilder.decisionText(line))
    }

    /// The texts of lines marked wrong in these notes, newest first.
    static func wrongLines(in notes: [String], vault: Vault, limit: Int = 20) -> [String] {
        let marked = notes.flatMap { NoteIO.read(vault.url($0))?.superseded ?? [] }
            .filter { $0.range(of: #" · wrong \d{4}-\d{2}-\d{2}$"#, options: .regularExpression) != nil }
        return Array(marked.reversed().prefix(limit)).map {
            CardBuilder.decisionText($0.replacingOccurrences(of: Note.supersededSuffix, with: "", options: .regularExpression))
        }
    }

    /// The day a decision line starts with, `2026-10-26 · …`.
    static func day(_ line: String) -> Date? {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.timeZone = TimeZone(identifier: "UTC")
        f.dateFormat = "yyyy-MM-dd"
        return line.count >= 10 ? f.date(from: String(line.prefix(10))) : nil
    }
}

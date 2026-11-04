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

    /// The day a decision line starts with, `2026-10-26 · …`.
    static func day(_ line: String) -> Date? {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.timeZone = TimeZone(identifier: "UTC")
        f.dateFormat = "yyyy-MM-dd"
        return line.count >= 10 ? f.date(from: String(line.prefix(10))) : nil
    }
}

import Foundation

/// Something the user writes down by hand: goes straight into the note's Decisions as "written by you".
public enum Memory {
    @discardableResult
    public static func add(_ text: String, to target: String, vault: Vault, now: Date = .now) throws -> Bool {
        let clean = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !clean.isEmpty, !SecretFilter.containsSecret(clean) else { return false }
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.dateFormat = "yyyy-MM-dd"
        let title = (target as NSString).lastPathComponent.replacingOccurrences(of: ".md", with: "")
        try NoteIO.update(vault.url(target), title: title) { $0.appendingDecision("\(f.string(from: now)) · \(clean) · written by you") }
        return true
    }

    /// Moves decisions a later one made untrue into the note's `## Superseded` section.
    public static func supersede(_ lines: [String], in target: String, vault: Vault, now: Date = .now) throws {
        let url = vault.url(target)
        guard !lines.isEmpty, let note = NoteIO.read(url) else { return }
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.dateFormat = "yyyy-MM-dd"
        let updated = note.superseding(lines, on: f.string(from: now))
        if updated != note { try NoteIO.write(updated, to: url) }
    }

    /// Puts a superseded line back.
    public static func restore(_ supersededLine: String, in target: String, vault: Vault) throws {
        let url = vault.url(target)
        guard let note = NoteIO.read(url) else { return }
        try NoteIO.write(note.restoring(supersededLine), to: url)
    }
}

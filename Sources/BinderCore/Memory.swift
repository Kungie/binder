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
}

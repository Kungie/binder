import Foundation

/// Editing a note by hand inside Binder: everything but the observed block, which the observers own.
extension Note {
    /// The body without the observed block (and the blank line before it). Frontmatter isn't part of it.
    public var editableText: String {
        var all = body.components(separatedBy: "\n")
        func isMarker(_ line: String, _ marker: String) -> Bool { line.trimmingCharacters(in: .whitespaces) == marker }
        guard let start = all.firstIndex(where: { isMarker($0, Note.observedStart) }),
              let end = all[(start + 1)...].firstIndex(where: { isMarker($0, Note.observedEnd) }) else { return body }
        var from = start
        if from > 0, all[from - 1].trimmingCharacters(in: .whitespaces).isEmpty { from -= 1 }
        all.removeSubrange(from...end)
        return all.joined(separator: "\n")
    }

    /// The note with `text` as its body and this note's observed block put back under the title, byte for byte.
    public func replacingEditable(_ text: String) -> Note {
        let hasBlock = body.components(separatedBy: "\n").contains { $0.trimmingCharacters(in: .whitespaces) == Note.observedStart }
        let edited = Note(frontmatter: frontmatter, body: text)
        return hasBlock ? edited.replacingObserved(observedLines) : edited
    }
}

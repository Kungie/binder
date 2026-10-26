import Foundation

public enum NoteIO {
    public static func read(_ url: URL) -> Note? {
        guard let text = try? String(contentsOf: url, encoding: .utf8) else { return nil }
        return Note.parse(text)
    }

    /// Atomic: the note is written to a temp file and renamed over the old one.
    public static func write(_ note: Note, to url: URL) throws {
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try Data(note.render().utf8).write(to: url, options: .atomic)
    }

    /// Reads the note fresh (or starts a new one with `title`), applies `change` and writes it back.
    public static func update(_ url: URL, title: String, _ change: (Note) -> Note) throws {
        let current = read(url) ?? Note.new(title: title)
        let next = change(current)
        guard next != current || !FileManager.default.fileExists(atPath: url.path) else { return }
        try write(next, to: url)
    }
}

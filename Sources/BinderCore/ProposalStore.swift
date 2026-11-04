import CryptoKit
import Foundation

public enum ProposalStoreError: Error { case notFound(String) }

/// The inbox: one JSON file per pending proposal in `.binder/inbox`.
public struct ProposalStore: Sendable {
    public let vault: Vault

    public init(vault: Vault) {
        self.vault = vault
    }

    /// False when the proposal holds a secret, repeats something pending or already kept, or was dropped before.
    @discardableResult
    public func add(_ p: Proposal) throws -> Bool {
        guard !SecretFilter.containsSecret(p.text), !SecretFilter.containsSecret(p.evidence) else { return false }
        if p.kind == .file {
            // A file is the same file by its bytes, whatever its caption says.
            guard let hash = p.fileHash, !pending().contains(where: { $0.fileHash == hash }), !isDropped(hash) else { return false }
            try FileManager.default.createDirectory(at: vault.inbox, withIntermediateDirectories: true)
            try Self.encoder.encode(p).write(to: file(p.id), options: .atomic)
            return true
        }
        let text = Self.normalize(p.text)
        guard !text.isEmpty else { return false }
        if pending().contains(where: { $0.target == p.target && Self.normalize($0.text) == text }) { return false }
        let kept = ([p.target] + p.also).allSatisfy { path in
            NoteIO.read(vault.url(path))?.decisions.contains { Self.normalize($0).contains(text) } == true
        }
        if kept { return false }
        if droppedHashes().contains(Self.hash(p)) { return false }
        try FileManager.default.createDirectory(at: vault.inbox, withIntermediateDirectories: true)
        try Self.encoder.encode(p).write(to: file(p.id), options: .atomic)
        return true
    }

    public func pending() -> [Proposal] {
        let files = (try? FileManager.default.contentsOfDirectory(at: vault.inbox, includingPropertiesForKeys: nil)) ?? []
        return files.filter { $0.pathExtension == "json" }
            .compactMap { try? Self.decoder.decode(Proposal.self, from: Data(contentsOf: $0)) }
            .sorted { $0.createdAt < $1.createdAt }
    }

    /// Appends `<yyyy-MM-dd> · <text> · <provenance>` (and the detail under it) to the target note's Decisions,
    /// and `… · for [[<project>]] · …` to each extra note, then clears the inbox entry. A first line in a topic note
    /// also links the topic from the project's overview.
    public func keep(_ id: String, editedText: String? = nil, now: Date = .now) throws {
        let p = try load(id)
        let text = editedText?.trimmingCharacters(in: .whitespacesAndNewlines) ?? p.text
        let day = Self.day.string(from: now)
        if p.kind == .file { return try keepFile(p, caption: text, day: day) }
        // A note that already has the line keeps its one copy: Keep can be offered again for the other targets.
        let normalized = Self.normalize(text)
        func has(_ note: Note) -> Bool { note.decisions.contains { Self.normalize($0).contains(normalized) } }
        try NoteIO.update(vault.url(p.target), title: Self.title(for: p.target)) {
            has($0) ? $0 : $0.appendingDecision("\(day) · \(text) · \(p.provenance)", detail: p.detail)
        }
        if let (project, topic) = Self.topic(of: p.target) { try Topic.link(project: project, topic: topic, vault: vault, now: now) }
        let forProject = p.project.map { " · for [[\($0)]]" } ?? ""
        for path in p.also {
            try NoteIO.update(vault.url(path), title: Self.title(for: path)) {
                has($0) ? $0 : $0.appendingDecision("\(day) · \(text)\(forProject) · \(p.provenance)")
            }
        }
        try FileManager.default.removeItem(at: file(id))
    }

    /// Moves the waiting file to `files/<project or me>/<day>-<caption>.<ext>` and embeds it in the note.
    private func keepFile(_ p: Proposal, caption: String, day: String) throws {
        guard let name = p.file else { throw ProposalStoreError.notFound(p.id) }
        let fm = FileManager.default
        let folder = "files/\(p.project ?? "me")", ext = (name as NSString).pathExtension
        let stem = "\(day)-\(Topic.slug(caption) ?? "file")"
        var path = "\(folder)/\(stem).\(ext)", n = 2
        while fm.fileExists(atPath: vault.url(path).path) {
            path = "\(folder)/\(stem)-\(n).\(ext)"
            n += 1
        }
        try fm.createDirectory(at: vault.url(folder), withIntermediateDirectories: true)
        try fm.moveItem(at: vault.inboxFiles.appendingPathComponent(name), to: vault.url(path))
        if let hash = p.fileHash { FileCatch.recordKept(hash, vault: vault) }
        let provenance = p.source == .command ? "made in a session" : "kept from the chat"
        try NoteIO.update(vault.url(p.target), title: Self.title(for: p.target)) { $0.appendingDecision("\(day) · ![[\(path)]] \(caption) · \(provenance)") }
        try fm.removeItem(at: file(p.id))
    }

    /// Rewrites a waiting proposal, for a caption written later.
    public func replace(_ p: Proposal) throws {
        guard FileManager.default.fileExists(atPath: file(p.id).path) else { throw ProposalStoreError.notFound(p.id) }
        try Self.encoder.encode(p).write(to: file(p.id), options: .atomic)
    }

    /// A new note's title: its file name, or for a machine named after its address (`10-0-7-50`), the address.
    static func title(for path: String) -> String {
        let stem = (path as NSString).lastPathComponent.replacingOccurrences(of: ".md", with: "")
        return stem.wholeMatch(of: #/\d{1,3}(-\d{1,3}){3}/#) != nil ? stem.replacingOccurrences(of: "-", with: ".") : stem
    }

    /// `projects/<project>/<topic>.md` → (project, topic).
    static func topic(of path: String) -> (String, String)? {
        let parts = path.split(separator: "/", omittingEmptySubsequences: false)
        guard parts.count == 3, parts[0] == "projects", parts[2].hasSuffix(".md") else { return nil }
        return (String(parts[1]), String(parts[2].dropLast(3)))
    }

    /// Remembers a line as dropped for a note, as Drop does, without an inbox entry (Wrong in the main window).
    public func rememberDropped(target: String, text: String) throws {
        let digest = SHA256.hash(data: Data((target + "\n" + Self.normalize(text)).utf8))
        try appendDropped(digest.map { String(format: "%02x", $0) }.joined())
    }

    private func appendDropped(_ hash: String) throws {
        let line = hash + "\n"
        try FileManager.default.createDirectory(at: vault.internalDir, withIntermediateDirectories: true)
        if let handle = try? FileHandle(forWritingTo: vault.droppedFile) {
            defer { try? handle.close() }
            _ = try handle.seekToEnd()
            try handle.write(contentsOf: Data(line.utf8))
        } else {
            try Data(line.utf8).write(to: vault.droppedFile)
        }
    }

    /// Deletes the entry and remembers it, so the same thing is never offered again.
    public func drop(_ id: String) throws {
        let p = try load(id)
        if p.kind == .file, let name = p.file { try? FileManager.default.removeItem(at: vault.inboxFiles.appendingPathComponent(name)) }
        try appendDropped(p.kind == .file ? p.fileHash ?? Self.hash(p) : Self.hash(p))
        try FileManager.default.removeItem(at: file(id))
    }

    /// Lower case, letters, digits and single spaces only: `"Let's try"` and `"Let’s try."` compare equal.
    public static func normalize(_ s: String) -> String {
        let kept = s.lowercased().unicodeScalars.map { CharacterSet.alphanumerics.contains($0) ? Character($0) : " " }
        return String(kept).split(separator: " ").joined(separator: " ")
    }

    // MARK: Private

    private func file(_ id: String) -> URL { vault.inbox.appendingPathComponent("\(id).json") }

    private func load(_ id: String) throws -> Proposal {
        guard let data = try? Data(contentsOf: file(id)), let p = try? Self.decoder.decode(Proposal.self, from: data)
        else { throw ProposalStoreError.notFound(id) }
        return p
    }

    func isDropped(_ hash: String) -> Bool { droppedHashes().contains(hash) }

    private func droppedHashes() -> Set<String> {
        guard let text = try? String(contentsOf: vault.droppedFile, encoding: .utf8) else { return [] }
        return Set(text.split(separator: "\n").map(String.init))
    }

    private static func hash(_ p: Proposal) -> String {
        let digest = SHA256.hash(data: Data((p.target + "\n" + normalize(p.text)).utf8))
        return digest.map { String(format: "%02x", $0) }.joined()
    }

    private static let encoder: JSONEncoder = {
        let e = JSONEncoder()
        e.dateEncodingStrategy = .iso8601
        e.outputFormatting = [.prettyPrinted, .sortedKeys]
        return e
    }()

    private static let decoder: JSONDecoder = {
        let d = JSONDecoder()
        d.dateDecodingStrategy = .iso8601
        return d
    }()

    private static let day: DateFormatter = {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.dateFormat = "yyyy-MM-dd"
        return f
    }()
}

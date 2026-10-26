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
        let text = Self.normalize(p.text)
        guard !text.isEmpty else { return false }
        if pending().contains(where: { $0.target == p.target && Self.normalize($0.text) == text }) { return false }
        if let note = NoteIO.read(vault.url(p.target)),
           note.decisions.contains(where: { Self.normalize($0).contains(text) }) { return false }
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

    /// Appends `<yyyy-MM-dd> · <text> · <provenance>` to the target note's Decisions and clears the inbox entry.
    public func keep(_ id: String, editedText: String? = nil, now: Date = .now) throws {
        let p = try load(id)
        let text = editedText?.trimmingCharacters(in: .whitespacesAndNewlines) ?? p.text
        let line = "\(Self.day.string(from: now)) · \(text) · \(p.provenance)"
        let title = (p.target as NSString).lastPathComponent.replacingOccurrences(of: ".md", with: "")
        try NoteIO.update(vault.url(p.target), title: title) { $0.appendingDecision(line) }
        try FileManager.default.removeItem(at: file(id))
    }

    /// Deletes the entry and remembers it, so the same thing is never offered again.
    public func drop(_ id: String) throws {
        let p = try load(id)
        let line = Self.hash(p) + "\n"
        try FileManager.default.createDirectory(at: vault.internalDir, withIntermediateDirectories: true)
        if let handle = try? FileHandle(forWritingTo: vault.droppedFile) {
            defer { try? handle.close() }
            _ = try handle.seekToEnd()
            try handle.write(contentsOf: Data(line.utf8))
        } else {
            try Data(line.utf8).write(to: vault.droppedFile)
        }
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

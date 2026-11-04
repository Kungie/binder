import CryptoKit
import Foundation
import ImageIO

/// Images and PDFs from a session: the ones you attached, and the ones the agent wrote to disk.
/// Each waits in `.binder/inbox/files` as a catch card. Nothing reaches the binder without Keep.
public enum FileCatch {
    public enum Candidate: Equatable, Sendable {
        case attached(mediaType: String, base64: String, near: String)
        case made(path: String)
    }

    /// File cards one session may offer.
    public static let perSession = 6
    static let maxBytes = 20_000_000
    static let minPixels = 64
    static let extensions = ["png", "jpg", "jpeg", "gif", "webp", "heic", "pdf"]
    static let mediaExtensions = ["image/png": "png", "image/jpeg": "jpg", "image/gif": "gif", "image/webp": "webp",
                                  "image/heic": "heic", "application/pdf": "pdf"]

    /// Newest first. A made file must exist and be modified at or after `since`; without `since` none count.
    public static func candidates(entries: [TranscriptEntry], cwd: String, since: Date?) -> [Candidate] {
        var found: [Candidate] = [], seen = Set<String>()
        let base = URL(fileURLWithPath: cwd)
        func made(_ raw: String) {
            let token = raw.trimmingCharacters(in: CharacterSet(charactersIn: "\"'`(),;"))
            guard let since, extensions.contains((token as NSString).pathExtension.lowercased()) else { return }
            let url = token.hasPrefix("/") ? URL(fileURLWithPath: token) : base.appendingPathComponent(token)
            let path = url.standardizedFileURL.path
            guard !seen.contains(path), let modified = (try? FileManager.default.attributesOfItem(atPath: path))?[.modificationDate] as? Date,
                  modified >= since else { return }
            seen.insert(path)
            found.append(.made(path: path))
        }
        for entry in entries {
            switch entry {
            case let .attachment(media, data, near): found.append(.attached(mediaType: media, base64: data, near: near))
            case let .fileEdit(path): made(path)
            case let .command(command): command.split(whereSeparator: { $0 == " " || $0 == "\n" || $0 == "=" }).forEach { made(String($0)) }
            default: continue
            }
        }
        return found.reversed()
    }

    /// Writes the waiting copies and returns their proposals, at most `perSession - offeredSoFar` of them.
    public static func stage(_ candidates: [Candidate], vault: Vault, project: String?, session: String, offeredSoFar: Int) throws -> [Proposal] {
        var room = perSession - offeredSoFar
        guard room > 0 else { return [] }
        let store = ProposalStore(vault: vault)
        var known = Set(store.pending().compactMap(\.fileHash)).union(keptHashes(vault))
        let target = project.map { "projects/\($0).md" } ?? "me.md"
        var staged: [Proposal] = []
        for candidate in candidates where room > 0 {
            guard let (data, ext, name, evidence, source, near) = load(candidate, vault: vault) else { continue }
            let hash = sha256(data)
            guard !known.contains(hash), !store.isDropped(hash), !tooSmall(data, ext: ext) else { continue }
            known.insert(hash)
            var p = Proposal.make(kind: .file, source: source, text: caption(name: name, ext: ext), evidence: evidence, target: target,
                                  project: project, session: session, confidence: .medium)
            p.fileHash = hash
            p.file = "\(p.id).\(ext)"
            let clean = near.map { SecretFilter.redact($0).trimmingCharacters(in: .whitespacesAndNewlines) } ?? ""
            p.context = clean.isEmpty ? nil : clean
            try FileManager.default.createDirectory(at: vault.inboxFiles, withIntermediateDirectories: true)
            try data.write(to: vault.inboxFiles.appendingPathComponent(p.file!), options: .atomic)
            staged.append(p)
            room -= 1
        }
        return staged
    }

    /// The bytes, extension, a name for the caption, the evidence, the source and the text sent with it.
    /// Size is checked before anything is decoded or read.
    static func load(_ c: Candidate, vault: Vault) -> (Data, String, String?, String, Proposal.Source, String?)? {
        switch c {
        case let .attached(media, base64, near):
            guard let ext = mediaExtensions[media.lowercased()], base64.count / 4 * 3 <= maxBytes,
                  let data = Data(base64Encoded: base64, options: .ignoreUnknownCharacters) else { return nil }
            return (data, ext, nil, "attached in the session", .chat, near)
        case let .made(path):
            let url = URL(fileURLWithPath: path)
            guard let size = (try? FileManager.default.attributesOfItem(atPath: path))?[.size] as? Int, size <= maxBytes,
                  let data = try? Data(contentsOf: url) else { return nil }
            let ext = url.pathExtension.lowercased() == "jpeg" ? "jpg" : url.pathExtension.lowercased()
            return (data, ext, url.deletingPathExtension().lastPathComponent, "made in the session: \(url.lastPathComponent)", .command, nil)
        }
    }

    /// An image under 64 × 64 is an icon or a spacer, not something worth keeping.
    static func tooSmall(_ data: Data, ext: String) -> Bool {
        guard ext != "pdf", let source = CGImageSourceCreateWithData(data as CFData, nil),
              let props = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let w = props[kCGImagePropertyPixelWidth] as? Int, let h = props[kCGImagePropertyPixelHeight] as? Int else { return false }
        return w < minPixels || h < minPixels
    }

    /// `new-chart` → `New chart`; with no name, what kind of file it is.
    static func caption(name: String?, ext: String) -> String {
        let words = (name ?? "").replacingOccurrences(of: "_", with: " ").replacingOccurrences(of: "-", with: " ")
            .split(separator: " ").joined(separator: " ")
        guard !words.isEmpty else { return ext == "pdf" ? "PDF from the session" : "Image from the session" }
        return words.prefix(1).uppercased() + words.dropFirst()
    }

    public static func sha256(_ data: Data) -> String { SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined() }

    /// Hashes of the files already kept under `files/`.
    static func keptHashes(_ vault: Vault) -> Set<String> {
        guard let walker = FileManager.default.enumerator(at: vault.url("files"), includingPropertiesForKeys: nil) else { return [] }
        var hashes = Set<String>()
        for case let url as URL in walker where extensions.contains(url.pathExtension.lowercased()) {
            if let data = try? Data(contentsOf: url) { hashes.insert(sha256(data)) }
        }
        return hashes
    }
}

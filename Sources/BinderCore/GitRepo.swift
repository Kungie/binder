import CryptoKit
import Foundation

/// A git repository found on disk, read without starting git.
public struct GitRepo: Equatable, Sendable {
    public let root: URL
    public let remote: String?

    public init(root: URL, remote: String?) {
        self.root = root
        self.remote = remote
    }

    public var name: String { root.lastPathComponent }

    /// Stable across clones: hashed from the normalized remote, or from the path when there is none.
    public var projectID: String {
        let source = remote.map(Self.normalize(remote:)) ?? root.standardizedFileURL.path
        let digest = SHA256.hash(data: Data(source.utf8))
        return digest.map { String(format: "%02x", $0) }.joined().prefix(12).description
    }

    /// Walks up from `dir` to the folder that holds `.git`.
    public static func locate(from dir: URL) -> GitRepo? {
        var current = dir.standardizedFileURL
        let fm = FileManager.default
        while true {
            let git = current.appendingPathComponent(".git")
            if fm.fileExists(atPath: git.path) {
                return GitRepo(root: current, remote: originURL(gitDir: gitDirectory(git)))
            }
            // At "/", deletingLastPathComponent gives "/..", so compare standardized paths.
            let parent = current.deletingLastPathComponent().standardizedFileURL
            if current.path == "/" || parent.path == current.path { return nil }
            current = parent
        }
    }

    /// `.git` is a folder, or (in worktrees and submodules) a file saying `gitdir: <path>`.
    static func gitDirectory(_ git: URL) -> URL {
        var isDir: ObjCBool = false
        FileManager.default.fileExists(atPath: git.path, isDirectory: &isDir)
        if isDir.boolValue { return git }
        guard let text = try? String(contentsOf: git, encoding: .utf8),
              let line = text.split(separator: "\n").first(where: { $0.hasPrefix("gitdir:") })
        else { return git }
        let path = line.dropFirst("gitdir:".count).trimmingCharacters(in: .whitespaces)
        return URL(fileURLWithPath: path, relativeTo: git.deletingLastPathComponent()).standardizedFileURL
    }

    /// The `url` of `[remote "origin"]` in the repo's config.
    static func originURL(gitDir: URL) -> String? {
        guard let config = try? String(contentsOf: gitDir.appendingPathComponent("config"), encoding: .utf8) else { return nil }
        var inOrigin = false
        for raw in config.split(separator: "\n") {
            let line = raw.trimmingCharacters(in: .whitespaces)
            if line.hasPrefix("[") {
                inOrigin = line.replacingOccurrences(of: " ", with: "") == "[remote\"origin\"]"
            } else if inOrigin, line.hasPrefix("url") {
                let parts = line.split(separator: "=", maxSplits: 1)
                if parts.count == 2 { return parts[1].trimmingCharacters(in: .whitespaces) }
            }
        }
        return nil
    }

    /// `git@github.com:Kungie/dawn.git`, `https://github.com/Kungie/Dawn` and friends → `github.com/kungie/dawn`.
    public static func normalize(remote: String) -> String {
        var s = remote.trimmingCharacters(in: .whitespaces).lowercased()
        if let scheme = s.range(of: "://") { s = String(s[scheme.upperBound...]) }
        if let at = s.firstIndex(of: "@"), at < (s.firstIndex(of: "/") ?? s.endIndex) {
            s = String(s[s.index(after: at)...])
        }
        if let colon = s.firstIndex(of: ":"), colon < (s.firstIndex(of: "/") ?? s.endIndex) {
            s.replaceSubrange(colon...colon, with: "/")
        }
        while s.hasSuffix("/") { s.removeLast() }
        if s.hasSuffix(".git") { s.removeLast(4) }
        return s
    }
}

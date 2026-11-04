import CryptoKit
import Foundation

/// A rules file to import and where its lines go.
public struct ImportSource: Equatable, Sendable {
    public var file: URL
    public var target: String
    public var project: String?
}

/// Brings what's already written in `CLAUDE.md` and `AGENTS.md` into the binder, as catches to keep or drop.
/// Each version of a file is offered once.
public enum Importer {
    static let projectFiles = ["CLAUDE.md", "AGENTS.md", ".claude/CLAUDE.md"]

    /// `~/.claude/CLAUDE.md` for you, and each repo's rules files under the project folders for that project.
    public static func sources(home: URL, projectRoots: [URL]) -> [ImportSource] {
        let fm = FileManager.default
        var out: [ImportSource] = []
        let global = home.appendingPathComponent(".claude/CLAUDE.md")
        if fm.fileExists(atPath: global.path) { out.append(ImportSource(file: global, target: "me.md", project: nil)) }
        for root in projectRoots {
            for repo in ProjectsObserver.repos(under: root) {
                let name = repo.lastPathComponent
                for file in projectFiles where fm.fileExists(atPath: repo.appendingPathComponent(file).path) {
                    out.append(ImportSource(file: repo.appendingPathComponent(file), target: "projects/\(name).md", project: name))
                }
            }
        }
        return out
    }

    /// Offers each new or changed file's lines as catches. Returns how many were added to the inbox.
    /// A file is remembered as imported only once the model answered.
    @discardableResult
    public static func run(vault: Vault, sources: [ImportSource], summarizer: ClaudeCLISummarizer) throws -> Int {
        var record = ImportRecord.load(vault)
        let store = ProposalStore(vault: vault)
        var added = 0
        for source in sources {
            guard let data = try? Data(contentsOf: source.file), let text = String(data: data, encoding: .utf8) else { continue }
            let hash = SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
            guard record.files[source.file.path] != hash else { continue }
            let known = ClaudeCLISummarizer.Known.load(vault: vault, project: source.project)
            guard let lines = summarizer.importLines(text: text, fileName: source.file.lastPathComponent, project: source.project, known: known) else { continue }
            for p in lines where try store.add(p) { added += 1 }
            record.files[source.file.path] = hash
            try record.save(vault)
        }
        return added
    }
}

/// `.binder/imported.json`: each imported file's path and the SHA-256 it had.
struct ImportRecord: Codable {
    var files: [String: String] = [:]

    static func url(_ vault: Vault) -> URL { vault.url(".binder/imported.json") }

    static func load(_ vault: Vault) -> ImportRecord {
        (try? Data(contentsOf: url(vault))).flatMap { try? JSONDecoder().decode(ImportRecord.self, from: $0) } ?? ImportRecord()
    }

    func save(_ vault: Vault) throws {
        try FileManager.default.createDirectory(at: vault.internalDir, withIntermediateDirectories: true)
        try JSONEncoder().encode(self).write(to: Self.url(vault), options: .atomic)
    }
}

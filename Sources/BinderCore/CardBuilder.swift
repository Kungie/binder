import Foundation

/// The text an agent reads at the start of a session: who you are, this project, how to reach its servers,
/// and where the last session stopped. Plain text, at most `budget` characters, secrets redacted.
public enum CardBuilder {
    public static let header = "BINDER CARD · VERIFIED, USE IT. If something you need is missing, ask once and say what you'd write down."
    /// About a thousand tokens.
    public static let budget = 3_600
    static let meLimit = 14
    static let codeHosts = Set(SSHObserver.codeHosts)

    /// The project for a session's folder: its git repo, or, outside git, a folder that has its own note.
    public static func project(for folder: URL, vault: Vault) -> GitRepo? {
        if let repo = GitRepo.locate(from: folder) { return repo }
        let name = folder.lastPathComponent
        return FileManager.default.fileExists(atPath: vault.url("projects/\(name).md").path) ? GitRepo(root: folder, remote: nil) : nil
    }

    public static func build(vault: Vault, repo: GitRepo?) -> String {
        var me = meLines(vault)
        var projectLines: [String] = []
        var decisions: [String] = []
        var access: [String] = []
        var handoff: [String] = []

        if let repo {
            let note = NoteIO.read(vault.url("projects/\(repo.name).md"))
            if let note {
                projectLines = note.observedLines + freeLines(note)
                // Date and text only: where a line came from matters in the note, not to the agent.
                decisions = note.decisions.reversed().map { line in
                    let date = line.components(separatedBy: " · ").first ?? ""
                    let text = decisionText(line)
                    return text == line ? "- \(line)" : "- \(date) · \(text)"
                }
            } else {
                projectLines = ["- nothing in the binder yet"]
            }
            access = accessLines(vault, linkedFrom: note)
            if let card = Handoff.pending(project: repo.name, vault: vault) {
                func join(_ items: [String]) -> String { items.isEmpty ? "nothing" : items.joined(separator: "; ") }
                handoff = ["Done: \(join(card.done))", "Left: \(join(card.left))", "Don't touch: \(join(card.dontTouch))",
                           "Next: \(card.next.isEmpty ? "nothing noted" : card.next)"]
            }
        }

        func render() -> String {
            var parts = [header]
            if !me.isEmpty { parts.append("ME:\n" + me.joined(separator: "\n")) }
            if let repo {
                var block = "PROJECT \(repo.name):\n" + projectLines.joined(separator: "\n")
                if !decisions.isEmpty { block += "\nDecisions, newest first:\n" + decisions.joined(separator: "\n") }
                parts.append(block)
            }
            if !access.isEmpty { parts.append("SERVERS AND ACCESS:\n" + access.joined(separator: "\n")) }
            if !handoff.isEmpty { parts.append("PICK UP FROM LAST TIME:\n" + handoff.joined(separator: "\n")) }
            return SecretFilter.redact(parts.joined(separator: "\n\n"))
        }

        var card = render()
        while card.count > budget {
            if decisions.count > 1 { decisions.removeLast() }
            else if !access.isEmpty { access.removeLast() }
            else if me.count > 3 { me.removeLast() }
            else { return String(card.prefix(budget)) }
            card = render()
        }
        return card
    }

    static func meLines(_ vault: Vault) -> [String] {
        guard let note = NoteIO.read(vault.url("me.md")) else { return [] }
        let kept = note.decisions.reversed().map { "- " + decisionText($0) }
        return Array((note.observedLines + freeLines(note) + kept).prefix(meLimit))
    }

    /// The user's own lines: not headings, not the observed block, not Decisions or Superseded.
    public static func freeLines(_ note: Note) -> [String] {
        var result: [String] = []
        var inObserved = false, inDecisions = false
        for line in note.body.components(separatedBy: "\n") {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            if trimmed == Note.observedStart { inObserved = true; continue }
            if trimmed == Note.observedEnd { inObserved = false; continue }
            if trimmed.hasPrefix("#") { inDecisions = trimmed == Note.decisionsHeading || trimmed == Note.supersededHeading; continue }
            guard !inObserved, !inDecisions, !trimmed.isEmpty else { continue }
            result.append(trimmed)
        }
        return result
    }

    /// `2026-10-17 · Licence is AGPL-3.0 · kept from the chat` → `Licence is AGPL-3.0`.
    public static func decisionText(_ decision: String) -> String {
        let parts = decision.components(separatedBy: " · ")
        return parts.count >= 3 ? parts[1..<(parts.count - 1)].joined(separator: " · ") : decision
    }

    static func accessLines(_ vault: Vault, linkedFrom note: Note?) -> [String] {
        var lines: [String] = []
        let linked = note.map { links(in: $0.render()) } ?? []
        for name in linked {
            guard let server = NoteIO.read(vault.url("servers/\(name).md")) else { continue }
            let facts = server.observedLines.compactMap { line -> String? in
                let item = line.hasPrefix("- ") ? String(line.dropFirst(2)) : line
                guard !item.hasPrefix("ssh:") else { return nil }
                return item.replacingOccurrences(of: ": ", with: " ", options: [], range: item.range(of: ": "))
            }
            if !facts.isEmpty { lines.append("- \(name): \(facts.joined(separator: ", "))") }
        }
        if let ssh = NoteIO.read(vault.url("access/ssh.md")) {
            let aliases = ssh.observedLines.compactMap { line -> String? in
                guard line.hasPrefix("- "), !line.hasPrefix("- key "), let colon = line.firstIndex(of: ":") else { return nil }
                let alias = String(line[line.index(line.startIndex, offsetBy: 2)..<colon])
                return codeHosts.contains(alias) ? nil : alias
            }
            if !aliases.isEmpty { lines.append("- ssh hosts already set up: \(aliases.joined(separator: ", ")). Use them, don't create new keys.") }
        }
        if let accounts = NoteIO.read(vault.url("access/accounts.md")) { lines += accounts.observedLines }
        return lines
    }

    public static func links(in text: String) -> [String] {
        var seen: [String] = []
        for match in text.matches(of: #/\[\[([^\]|#]+)(?:[|#][^\]]*)?\]\]/#) where !seen.contains(String(match.1)) {
            seen.append(String(match.1))
        }
        return seen
    }

    /// Every file and folder a project's card reads, for cache freshness.
    static func sources(vault: Vault, repo: GitRepo) -> [URL] {
        var urls = [vault.url("me.md"), vault.url("projects/\(repo.name).md"), vault.url("servers"), vault.url("access"),
                    vault.url("handoffs/\(repo.name)")]
        for dir in ["servers", "access", "handoffs/\(repo.name)"] {
            let names = (try? FileManager.default.contentsOfDirectory(atPath: vault.url(dir).path)) ?? []
            urls += names.map { vault.url("\(dir)/\($0)") }
        }
        return urls
    }
}

/// `.binder/cards/<project-id>.md`: a card is reused while it's newer than everything it was built from.
public struct CardCache: Sendable {
    public let vault: Vault

    public init(vault: Vault) {
        self.vault = vault
    }

    func file(_ repo: GitRepo) -> URL { vault.cards.appendingPathComponent("\(repo.projectID).md") }

    public func cached(for repo: GitRepo) -> String? {
        guard let cachedAt = modified(file(repo)), let text = try? String(contentsOf: file(repo), encoding: .utf8) else { return nil }
        let newest = CardBuilder.sources(vault: vault, repo: repo).compactMap(modified).max() ?? .distantPast
        return newest <= cachedAt ? text : nil
    }

    public func store(_ card: String, for repo: GitRepo) throws {
        try FileManager.default.createDirectory(at: vault.cards, withIntermediateDirectories: true)
        try Data(card.utf8).write(to: file(repo), options: .atomic)
    }

    private func modified(_ url: URL) -> Date? {
        (try? FileManager.default.attributesOfItem(atPath: url.path))?[.modificationDate] as? Date
    }
}

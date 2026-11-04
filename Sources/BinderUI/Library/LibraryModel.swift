import AppKit
import BinderCore
import Foundation

public enum SectionKind: String, CaseIterable, Sendable {
    case inbox, graph, me, machines, access, servers, hosting, projects, handoffs, settings

    public var label: String { rawValue.uppercased() }
}

public struct NoteRef: Hashable, Identifiable, Sendable {
    public var title: String
    public var path: String
    public var id: String { path }

    public init(title: String, path: String) {
        self.title = title
        self.path = path
    }
}

public struct LibrarySection: Identifiable, Sendable {
    public var kind: SectionKind
    public var items: [NoteRef]
    public var id: String { kind.rawValue }
}

public struct Fact: Equatable, Sendable {
    public var key: String
    public var value: String
}

public struct Decision: Equatable, Sendable {
    public var date: String
    public var text: String
    public var provenance: String
    /// The line exactly as the note has it, for Delete.
    public var line: String = ""
    /// What's written under it.
    public var details: [String] = []
    public var usage = UseCounts()
}

public struct NoteDetail: Sendable {
    public var ref: NoteRef
    public var title: String
    public var verified: String?
    public var source: String?
    public var facts: [Fact]
    public var decisions: [Decision]
    public var freeText: [String]
    public var links: [String]
    /// Lines moved out of Decisions because later ones made them untrue, newest first.
    public var superseded: [String] = []
    /// How often sessions used this note.
    public var usage = UseCounts()
    /// Old lines no agent used, off the card, oldest first.
    public var fading: [Decision] = []
    /// A project overview's topic notes, A to Z.
    public var topics: [NoteRef] = []
    public var lastHandoff: HandoffCard?
    public var cardPreview: String?
    public var cardTokens: Int
}

/// The main window's view of the vault: every note by folder, one note in detail, and the settings.
@MainActor
public final class LibraryModel: ObservableObject {
    @Published public private(set) var sections: [LibrarySection] = []
    @Published public var selectedKind: SectionKind = .projects
    @Published public var selected: NoteRef?
    @Published public private(set) var state: BinderState
    @Published public private(set) var vault: Vault
    public let home: URL
    /// Where Delete note sends a file: the macOS Trash, so it can be recovered. Tests swap it out.
    var trash: (URL) throws -> Void = { try FileManager.default.trashItem(at: $0, resultingItemURL: nil) }

    public init(vault: Vault, home: URL) {
        self.vault = vault
        self.home = home
        self.state = BinderState.load(vault)
        reload()
        selected = sections.first { $0.kind == .projects }?.items.first ?? sections.first { $0.kind == .me }?.items.first
        if let selected, let kind = sections.first(where: { $0.items.contains(selected) })?.kind { selectedKind = kind }
    }

    public func reload() {
        state = BinderState.load(vault)
        sections = SectionKind.allCases.map { LibrarySection(kind: $0, items: items(for: $0)) }
    }

    func items(for kind: SectionKind) -> [NoteRef] {
        switch kind {
        case .inbox, .graph, .settings:
            return []
        case .me:
            return FileManager.default.fileExists(atPath: vault.url("me.md").path) ? [NoteRef(title: "Me", path: "me.md")] : []
        case .handoffs:
            let projects = (try? FileManager.default.contentsOfDirectory(atPath: vault.url("handoffs").path)) ?? []
            return projects.sorted().flatMap { project -> [NoteRef] in
                let names = (try? FileManager.default.contentsOfDirectory(atPath: vault.url("handoffs/\(project)").path)) ?? []
                return names.filter { $0.hasSuffix(".md") }.sorted().reversed().map {
                    NoteRef(title: "\(project) · \($0.dropLast(3))", path: "handoffs/\(project)/\($0)")
                }
            }
        default:
            let names = (try? FileManager.default.contentsOfDirectory(atPath: vault.url(kind.rawValue).path)) ?? []
            // By the title the sidebar shows, A to Z, whatever the case or the file name.
            return names.filter { $0.hasSuffix(".md") }.map { name in
                NoteRef(title: title(of: "\(kind.rawValue)/\(name)") ?? String(name.dropLast(3)), path: "\(kind.rawValue)/\(name)")
            }
            .sorted { $0.title.localizedStandardCompare($1.title) == .orderedAscending }
        }
    }

    func title(of path: String) -> String? {
        NoteIO.read(vault.url(path))?.body.components(separatedBy: "\n").first { $0.hasPrefix("# ") }.map { String($0.dropFirst(2)) }
    }

    public func note(_ ref: NoteRef) -> NoteDetail {
        let note = NoteIO.read(vault.url(ref.path)) ?? Note.new(title: ref.title)
        let facts = note.observedLines.compactMap { line -> Fact? in
            guard line.hasPrefix("- "), let colon = line.range(of: ": ") else { return nil }
            return Fact(key: String(line[line.index(line.startIndex, offsetBy: 2)..<colon.lowerBound]), value: String(line[colon.upperBound...]))
        }
        let decisions = note.decisionItems.reversed().map { item -> Decision in
            let line = item.line, parts = line.components(separatedBy: " · ")
            guard parts.count >= 3 else { return Decision(date: "", text: line, provenance: "", line: line, details: item.details) }
            return Decision(date: parts[0], text: parts[1..<(parts.count - 1)].joined(separator: " · "), provenance: parts.last!,
                            line: line, details: item.details)
        }
        var detail = NoteDetail(ref: ref, title: title(of: ref.path) ?? ref.title, verified: note.value("verified"), source: note.value("source"),
                                facts: facts, decisions: decisions, freeText: CardBuilder.freeLines(note),
                                links: CardBuilder.links(in: note.render()), lastHandoff: nil, cardPreview: nil, cardTokens: 0)
        detail.superseded = note.superseded.reversed()
        let usage = UsageStore(vault: vault)
        detail.usage = usage.counts(note: ref.path)
        for i in detail.decisions.indices { detail.decisions[i].usage = usage.counts(line: detail.decisions[i].line, in: ref.path) }
        let fading = Forgetting.Checker(usage: usage, now: Date())
        detail.fading = detail.decisions.filter { fading.fades($0.line, in: ref.path) }.reversed()
        detail.decisions.removeAll { fading.fades($0.line, in: ref.path) }
        if ref.path.hasPrefix("projects/"), ref.path.split(separator: "/").count == 2 {
            let name = String(ref.path.dropFirst("projects/".count).dropLast(3))
            detail.topics = Topic.topics(of: name, vault: vault).map { topic in
                let path = Topic.path(project: name, topic: topic)
                return NoteRef(title: title(of: path) ?? topic, path: path)
            }
            detail.lastHandoff = Handoff.latest(project: name, vault: vault)?.card
            let card = CardBuilder.build(vault: vault, repo: GitRepo(root: URL(fileURLWithPath: "/\(name)"), remote: nil))
            detail.cardPreview = card
            detail.cardTokens = (card.count + 3) / 4
        }
        return detail
    }

    @discardableResult
    public func addMemory(_ text: String, to ref: NoteRef) -> Bool {
        let added = (try? Memory.add(text, to: ref.path, vault: vault)) ?? false
        if added { objectWillChange.send() }
        return added
    }

    // MARK: Editing

    public enum SaveResult: Equatable, Sendable { case saved, changedOnDisk }
    public enum DeleteError: Error { case meStays }

    /// The note's editable text, and its file date, so Save can tell whether something else changed it since.
    public func beginEdit(_ ref: NoteRef) -> (text: String, opened: Date) {
        let note = NoteIO.read(vault.url(ref.path)) ?? Note.new(title: ref.title)
        return (note.editableText, Self.modified(vault.url(ref.path)) ?? .distantPast)
    }

    /// Writes the edit around the note's current observed block. Refuses when the file changed after `opened`, unless forced.
    public func save(_ text: String, to ref: NoteRef, opened: Date, force: Bool = false) -> SaveResult {
        let url = vault.url(ref.path)
        if !force, let now = Self.modified(url), now > opened { return .changedOnDisk }
        let current = NoteIO.read(url) ?? Note.new(title: ref.title)
        try? NoteIO.write(current.replacingEditable(text), to: url)
        reload()
        objectWillChange.send()
        return .saved
    }

    /// Takes one decision, and what's written under it, out of the note.
    public func delete(line: String, in ref: NoteRef) {
        let url = vault.url(ref.path)
        guard let note = NoteIO.read(url) else { return }
        let next = note.removingDecision(line)
        if next != note { try? NoteIO.write(next, to: url) }
        // A kept file goes with its line.
        if let file = line.firstMatch(of: #/!\[\[(files/[^\]]+)\]\]/#).map({ vault.url(String($0.1)) }),
           FileManager.default.fileExists(atPath: file.path) { try? trash(file) }
        objectWillChange.send()
    }

    /// Moves the note to the Trash. me.md stays: it's who you are.
    public func deleteNote(_ ref: NoteRef) throws {
        guard ref.path != "me.md" else { throw DeleteError.meStays }
        try trash(vault.url(ref.path))
        if selected == ref { selected = nil }
        reload()
    }

    static func modified(_ url: URL) -> Date? {
        (try? FileManager.default.attributesOfItem(atPath: url.path))?[.modificationDate] as? Date
    }

    /// Notes whose content matches, by the sidebar path they show under (a topic's hit shows on its project),
    /// with the best line for each.
    public func matches(_ query: String) -> [String: String] {
        guard !query.trimmingCharacters(in: .whitespaces).isEmpty else { return [:] }
        var best: [String: String] = [:]
        for hit in Search.run(query, vault: vault, limit: 50) {
            let parts = hit.path.split(separator: "/")
            let row = parts.count == 3 && parts[0] == "projects" ? "projects/\(parts[1]).md" : hit.path
            if best[row] == nil { best[row] = hit.text }
        }
        return best
    }

    /// The project an overview note is for: `projects/dawn.md` → `dawn`. Nil for anything else, topic notes included.
    static func overviewProject(_ ref: NoteRef) -> String? {
        let parts = ref.path.split(separator: "/")
        guard parts.count == 2, parts[0] == "projects", parts[1].hasSuffix(".md") else { return nil }
        return String(parts[1].dropLast(3))
    }

    /// Sorts a project overview's decisions into topic notes with the user's own Claude plan.
    /// Nil when the user hasn't agreed to Binder reading with their plan.
    public func splitByTopic(_ ref: NoteRef) async -> TopicSplit.Result? {
        guard state.summarizerConsent, let project = Self.overviewProject(ref) else { return nil }
        let vault = vault
        let result = await Task.detached {
            try? TopicSplit.run(project: project, vault: vault, summarizer: ClaudeCLISummarizer(runner: ClaudeCLISummarizer.runner(), session: nil))
        }.value
        reload()
        objectWillChange.send()
        return result ?? TopicSplit.Result(lines: 0, topics: 0)
    }

    /// Keeps a fading line on the card for good.
    public func pin(_ line: String, in ref: NoteRef) {
        UsageStore(vault: vault).pin(line: line, in: ref.path)
        objectWillChange.send()
    }

    /// Puts a superseded line back among the note's decisions.
    public func restore(_ supersededLine: String, in ref: NoteRef) {
        try? Memory.restore(supersededLine, in: ref.path, vault: vault)
        objectWillChange.send()
    }

    public func update(_ change: (inout BinderState) -> Void) {
        var next = BinderState.load(vault)
        change(&next)
        try? next.save(vault)
        state = next
    }

    /// Moves the binder into `folder` (an Obsidian vault, say) and repoints the CLI.
    public func moveVault(into folder: URL) throws {
        let destination = folder.appendingPathComponent("Binder")
        try FileManager.default.moveItem(at: vault.root, to: destination)
        try VaultLocation.save(destination, home: home)
        vault = Vault(root: destination)
        reload()
    }

    // MARK: Opening files elsewhere

    public static var obsidianInstalled: Bool {
        NSWorkspace.shared.urlForApplication(withBundleIdentifier: "md.obsidian") != nil
    }

    public func openInObsidian(_ ref: NoteRef) {
        var components = URLComponents(string: "obsidian://open")!
        components.queryItems = [URLQueryItem(name: "path", value: vault.url(ref.path).path)]
        if let url = components.url { NSWorkspace.shared.open(url) }
    }

    public func openInEditor(_ ref: NoteRef) {
        NSWorkspace.shared.open(vault.url(ref.path))
    }

    public func revealInFinder(_ ref: NoteRef?) {
        NSWorkspace.shared.activateFileViewerSelecting([ref.map { vault.url($0.path) } ?? vault.root])
    }
}

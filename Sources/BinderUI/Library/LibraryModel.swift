import AppKit
import BinderCore
import Foundation

public enum SectionKind: String, CaseIterable, Sendable {
    case inbox, graph, me, machines, access, servers, projects, handoffs, settings

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
        let decisions = note.decisions.reversed().map { line -> Decision in
            let parts = line.components(separatedBy: " · ")
            guard parts.count >= 3 else { return Decision(date: "", text: line, provenance: "") }
            return Decision(date: parts[0], text: parts[1..<(parts.count - 1)].joined(separator: " · "), provenance: parts.last!)
        }
        var detail = NoteDetail(ref: ref, title: title(of: ref.path) ?? ref.title, verified: note.value("verified"), source: note.value("source"),
                                facts: facts, decisions: decisions, freeText: CardBuilder.freeLines(note),
                                links: CardBuilder.links(in: note.render()), lastHandoff: nil, cardPreview: nil, cardTokens: 0)
        if ref.path.hasPrefix("projects/") {
            let name = String(ref.path.dropFirst("projects/".count).dropLast(3))
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

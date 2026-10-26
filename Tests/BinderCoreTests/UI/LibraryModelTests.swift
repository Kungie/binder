import Foundation
import Testing
@testable import BinderCore
@testable import BinderUI

@MainActor
@Suite struct LibraryModelTests {
    func model() throws -> (LibraryModel, Vault) {
        let vault = try fixtureVault()
        return (LibraryModel(vault: vault, home: makeTempDir("home")), vault)
    }

    @Test func sectionsListNotesByFolder() throws {
        let (m, _) = try model()
        let byKind = Dictionary(uniqueKeysWithValues: m.sections.map { ($0.kind, $0.items.map(\.path)) })
        #expect(m.sections.map(\.kind) == SectionKind.allCases)
        #expect(byKind[.me] == ["me.md"])
        #expect(byKind[.projects] == ["projects/dawn.md"])
        #expect(byKind[.servers] == ["servers/hetzner-1.md"])
        #expect(byKind[.access] == ["access/accounts.md", "access/ssh.md"])
        #expect(m.sections.first { $0.kind == .projects }?.items.first?.title == "dawn")
    }

    @Test func projectsSortAlphabeticallyByTitleIgnoringCase() throws {
        let (_, vault) = try model()
        for (file, title) in [("Zeta.md", "Zeta"), ("b-site.md", "avenox"), ("api.md", "Beacon"), ("x.md", "cockpit")] {
            try NoteIO.write(Note(body: "# \(title)\n"), to: vault.url("projects/\(file)"))
        }
        let m = LibraryModel(vault: vault, home: makeTempDir("home"))
        let titles = m.sections.first { $0.kind == .projects }?.items.map(\.title)
        #expect(titles == ["avenox", "Beacon", "cockpit", "dawn", "Zeta"])
    }

    @Test func noteDetailParsesObservedAndDecisions() throws {
        let (m, _) = try model()
        let d = m.note(NoteRef(title: "dawn", path: "projects/dawn.md"))
        #expect(d.title == "dawn")
        #expect(d.verified == "2026-10-26T09:14:00Z")
        #expect(d.facts.first == Fact(key: "repo", value: "github.com/kungie/dawn"))
        #expect(d.decisions.first == Decision(date: "2026-10-26", text: "The dawn site deploys to Cloudflare Pages.", provenance: "kept from a command"))
        #expect(d.freeText == ["- site: Cloudflare Pages, see [[site]]", "- server: [[hetzner-1]]"])
        #expect(d.links == ["site", "hetzner-1"])
    }

    @Test func projectNoteHasCardPreview() throws {
        let (m, vault) = try model()
        _ = try Handoff.write(HandoffCard(done: ["A"], left: ["B"], dontTouch: [], next: "C"), project: "dawn", vault: vault, now: Date())
        let d = m.note(NoteRef(title: "dawn", path: "projects/dawn.md"))
        #expect(d.cardPreview?.hasPrefix(CardBuilder.header) == true)
        #expect(d.cardTokens > 50)
        #expect(d.lastHandoff?.next == "C")
        #expect(m.note(NoteRef(title: "Me", path: "me.md")).cardPreview == nil)
    }

    @Test func addMemoryWritesWrittenByYou() throws {
        let (m, vault) = try model()
        #expect(m.addMemory("Keep the panel 350 wide.", to: NoteRef(title: "dawn", path: "projects/dawn.md")))
        #expect(NoteIO.read(vault.url("projects/dawn.md"))?.decisions.last?.hasSuffix("Keep the panel 350 wide. · written by you") == true)
        #expect(!m.addMemory("  ", to: NoteRef(title: "dawn", path: "projects/dawn.md")))
    }

    @Test func inboxCountMatchesPending() throws {
        let (m, vault) = try model()
        try ProposalStore(vault: vault).add(Proposal.make(kind: .fact, source: .command, text: "One.", evidence: "x", target: "projects/dawn.md", project: "dawn", session: nil))
        m.reload()
        #expect(m.inboxCount == 1)
    }

    @Test func settingsChangesAreSaved() throws {
        let (m, vault) = try model()
        m.update { $0.handoffThreshold = 0.8; $0.observeIntervalHours = 24 }
        let state = BinderState.load(vault)
        #expect(state.handoffThreshold == 0.8 && state.observeIntervalHours == 24)
    }
}

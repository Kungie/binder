import Foundation
import Testing
@testable import BinderCore

private func day(_ y: Int, _ m: Int, _ d: Int, hour: Int = 12) -> Date {
    Calendar.current.date(from: DateComponents(year: y, month: m, day: d, hour: hour))!
}

private func deployProposal(_ text: String = "The dawn site deploys to Cloudflare Pages.", at date: Date = day(2026, 10, 26)) -> Proposal {
    Proposal.make(kind: .decision, source: .command, text: text, evidence: "wrangler pages deploy",
                  target: "projects/dawn.md", project: "dawn", session: "s1", confidence: .high, now: date)
}

@Suite struct ProposalStoreTests {
    let vault = Vault(root: makeTempDir())
    var store: ProposalStore { ProposalStore(vault: vault) }

    @Test func addWritesInboxFile() throws {
        let p = deployProposal()
        #expect(try store.add(p))
        #expect(FileManager.default.fileExists(atPath: vault.inbox.appendingPathComponent("\(p.id).json").path))
        #expect(p.id.hasPrefix("p_") && p.id.count == 10)
        #expect(store.pending() == [p])
    }

    @Test func addRejectsSecret() throws {
        let token = "gh" + "p_" + String(repeating: "a1B2c3D4e5", count: 4)
        #expect(try !store.add(deployProposal("Use \(token) for pushes.")))
        var evidence = deployProposal()
        evidence.evidence = "export GITHUB_TOKEN=\(token) && wrangler pages deploy"
        #expect(try !store.add(evidence))
        #expect(store.pending().isEmpty)
    }

    @Test func addRejectsDuplicateOfPending() throws {
        #expect(try store.add(deployProposal()))
        #expect(try !store.add(deployProposal("the dawn site deploys to cloudflare pages")))
        #expect(store.pending().count == 1)
    }

    @Test func addRejectsDuplicateOfKeptDecision() throws {
        let p = deployProposal()
        _ = try store.add(p)
        try store.keep(p.id, now: day(2026, 10, 26))
        #expect(try !store.add(deployProposal()))
    }

    @Test func droppedIsNeverOfferedAgain() throws {
        let p = deployProposal("Let's try Tailwind for a minute.")
        _ = try store.add(p)
        try store.drop(p.id)
        #expect(store.pending().isEmpty)
        #expect(try !store.add(deployProposal("Let’s try Tailwind for a minute")))
    }

    @Test func keepAppendsDecisionWithProvenance() throws {
        let p = deployProposal()
        _ = try store.add(p)
        try store.keep(p.id, now: day(2026, 10, 26))
        let note = try #require(NoteIO.read(vault.url("projects/dawn.md")))
        #expect(note.decisions == ["2026-10-26 · The dawn site deploys to Cloudflare Pages. · kept from a command"])
        #expect(note.render().hasPrefix("# dawn\n"))
        #expect(store.pending().isEmpty)
    }

    @Test func keepUsesEditedText() throws {
        var chat = deployProposal("Release notes go in docs.")
        chat.source = .chat
        _ = try store.add(chat)
        try store.keep(chat.id, editedText: "Release notes live in docs/releases, one file per version.", now: day(2026, 10, 27))
        let note = try #require(NoteIO.read(vault.url("projects/dawn.md")))
        #expect(note.decisions == ["2026-10-27 · Release notes live in docs/releases, one file per version. · kept from the chat"])
    }

    @Test func pendingIsSortedOldestFirst() throws {
        let newer = deployProposal("Newer.", at: day(2026, 10, 26, hour: 15))
        let older = deployProposal("Older.", at: day(2026, 10, 26, hour: 9))
        _ = try store.add(newer)
        _ = try store.add(older)
        #expect(store.pending().map(\.text) == ["Older.", "Newer."])
    }
}

import Foundation
import Testing
@testable import BinderCore

private struct StubObserver: Observer {
    let name = "stub"
    var lines: [String]
    var proposal: Proposal?

    func observe(_ ctx: ObserverContext) -> Observation {
        Observation(notes: [NoteUpdate(path: "projects/dawn.md", title: "dawn", observed: lines)],
                    proposals: proposal.map { [$0] } ?? [], filesRead: 3)
    }
}

@Suite struct ObserveRunTests {
    let vault = Vault(root: makeTempDir("vault"))
    let ctx = ObserverContext(home: makeTempDir(), projectRoots: [], runner: FakeRunner(),
                              now: ISO8601DateFormatter().date(from: "2026-10-26T09:14:00Z")!)

    @Test func observeRunWritesNotesAndKeepsUserText() throws {
        _ = try ObserveRun.run(vault: vault, ctx: ctx, observers: [StubObserver(lines: ["- repo: github.com/kungie/dawn"])])
        let url = vault.url("projects/dawn.md")
        let first = try #require(read(url))
        #expect(first.hasPrefix("---\nverified: 2026-10-26T09:14:00Z\nsource: observed\n---\n# dawn\n"))
        write(first + "\nKeep the panel 340 wide.\n", to: url)
        let summary = try ObserveRun.run(vault: vault, ctx: ctx, observers: [StubObserver(lines: ["- repo: github.com/kungie/dawn", "- branch: main"])])
        let note = try #require(NoteIO.read(url))
        #expect(note.observedLines == ["- repo: github.com/kungie/dawn", "- branch: main"])
        #expect(note.render().hasSuffix("\nKeep the panel 340 wide.\n"))
        #expect(summary.notesWritten == 1 && summary.filesRead == 3)
        #expect(BinderState.load(vault).lastObserve == ctx.now)
    }

    @Test func observeRunIsIdempotent() throws {
        let p = Proposal.make(kind: .fact, source: .observed, text: "proxmox at 192.168.1.20 is part of your homelab.",
                              evidence: "~/.ssh/config", target: "servers/proxmox.md", project: nil, session: nil, agent: "binder")
        let observers = [StubObserver(lines: ["- a"], proposal: p)]
        let one = try ObserveRun.run(vault: vault, ctx: ctx, observers: observers)
        let text = read(vault.url("projects/dawn.md"))
        let two = try ObserveRun.run(vault: vault, ctx: ctx, observers: observers)
        #expect(one.proposalsAdded == 1 && two.proposalsAdded == 0)
        #expect(read(vault.url("projects/dawn.md")) == text)
        #expect(ProposalStore(vault: vault).pending().count == 1)
    }

    @Test func userSetSourceIsKept() throws {
        write("---\nsource: you\n---\n# dawn\n", to: vault.url("projects/dawn.md"))
        _ = try ObserveRun.run(vault: vault, ctx: ctx, observers: [StubObserver(lines: ["- a"])])
        #expect(NoteIO.read(vault.url("projects/dawn.md"))?.value("source") == "you")
    }
}

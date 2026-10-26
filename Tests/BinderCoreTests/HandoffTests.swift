import Foundation
import Testing
@testable import BinderCore

@Suite struct HandoffTests {
    @Test func fallbackHandoffFromFixture() throws {
        let entries = try Transcript.read(fixture("transcript.jsonl"), from: 0).entries
        let card = Handoff.fallback(from: entries)
        #expect(card.done == ["Edited README.md"])
        #expect(card.left == ["Write release notes"])
        #expect(card.dontTouch.isEmpty)
        #expect(card.next == "Next: write the release notes for 0.3.")
    }

    @Test func writesReadableFile() throws {
        let vault = Vault(root: makeTempDir())
        let now = Calendar.current.date(from: DateComponents(year: 2026, month: 10, day: 26, hour: 9, minute: 14))!
        let url = try Handoff.write(HandoffCard(done: ["A"], left: [], dontTouch: [], next: "B"), project: "dawn", vault: vault, now: now)
        #expect(url.path.hasSuffix("handoffs/dawn/2026-10-26-0914.md"))
        let text = try #require(read(url))
        #expect(text.contains("consumed: false"))
        #expect(text.contains("# Handoff · dawn · 2026-10-26 09:14\n\n## Done\n- A\n\n## Left\n- nothing left\n\n## Don't touch\n- nothing noted\n\n## Next\nB\n"))
    }

    @Test func pendingThenConsumed() throws {
        let vault = Vault(root: makeTempDir())
        #expect(Handoff.pending(project: "dawn", vault: vault) == nil)
        let card = HandoffCard(done: ["A", "B"], left: ["C"], dontTouch: ["D"], next: "E")
        _ = try Handoff.write(card, project: "dawn", vault: vault, now: Date())
        #expect(Handoff.pending(project: "dawn", vault: vault) == card)
        try Handoff.markConsumed(project: "dawn", vault: vault)
        #expect(Handoff.pending(project: "dawn", vault: vault) == nil)
        #expect(Handoff.pending(project: "site", vault: vault) == nil)
    }
}

import Foundation
import Testing
@testable import BinderCore
@testable import BinderUI

@MainActor
@Suite struct UsageDisplayTests {
    let now = ISO8601DateFormatter().date(from: "2026-11-04T15:00:00Z")!

    func counts(_ cards: Int, _ used: Int, last: Date?) -> UseCounts {
        var c = UseCounts()
        c.cards = cards
        c.used = used
        c.lastCard = last
        return c
    }

    @Test func usageLineWording() {
        #expect(NoteView.usageLine(UseCounts(), now: now) == nil)
        #expect(NoteView.usageLine(counts(1, 1, last: now), now: now) == "On 1 session card · used once · last today")
        #expect(NoteView.usageLine(counts(40, 12, last: now.addingTimeInterval(-3600)), now: now) == "On 40 session cards · used 12 times · last today")
        #expect(NoteView.usageLine(counts(3, 0, last: now.addingTimeInterval(-86_400)), now: now) == "On 3 session cards · last yesterday")
        let tenDays = now.addingTimeInterval(-10 * 86_400)
        let f = DateFormatter()
        f.dateFormat = "d MMM"
        #expect(NoteView.usageLine(counts(5, 2, last: tenDays), now: now) == "On 5 session cards · used 2 times · last \(f.string(from: tenDays))")
        #expect(NoteView.lineUsage(counts(31, 4, last: now)) == "On 31 cards · used 4 times")
    }

    @Test func busierNotesDrawBigger() {
        #expect(GraphLayout.useScale(cards: 0) == 1)
        #expect(GraphLayout.useScale(cards: 40) > 1.25)
        #expect(GraphLayout.useScale(cards: 100_000) == 1.35)
    }
}

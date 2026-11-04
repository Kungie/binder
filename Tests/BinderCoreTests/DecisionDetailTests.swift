import Testing
@testable import BinderCore

@Suite struct DecisionDetailTests {
    let base = Note(body: "# dawn\n")

    var twoDecisions: Note { base.appendingDecision("A", detail: "why A").appendingDecision("B") }

    @Test func appendKeepsNewDecisionAfterPreviousDetail() {
        let lines = twoDecisions.body.components(separatedBy: "\n").filter { !$0.isEmpty }
        #expect(lines == ["# dawn", "## Decisions", "- A", "  - why A", "- B"])
    }

    @Test func decisionItemsCarryDetails() {
        #expect(twoDecisions.decisionItems == [DecisionItem(line: "A", details: ["why A"]), DecisionItem(line: "B", details: [])])
    }

    @Test func supersedeMovesDetailToo() {
        let note = twoDecisions.superseding(["A"], on: "2026-11-04")
        #expect(note.decisions == ["B"])
        #expect(!note.body.components(separatedBy: "## Superseded")[0].contains("why A"))
        let superseded = note.body.components(separatedBy: "## Superseded\n")[1].components(separatedBy: "\n").filter { !$0.isEmpty }
        #expect(superseded == ["- A · superseded 2026-11-04", "  - why A"])
    }

    @Test func restoreBringsDetailBack() {
        let note = twoDecisions.superseding(["A"], on: "2026-11-04").restoring("A · superseded 2026-11-04")
        #expect(note.superseded.isEmpty)
        #expect(note.decisionItems.first { $0.line == "A" }?.details == ["why A"])
    }

    @Test func removingDecisionDropsOnlyItsLines() {
        let note = twoDecisions.appendingDecision("C", detail: "why C").removingDecision("A")
        #expect(note.decisionItems == [DecisionItem(line: "B", details: []), DecisionItem(line: "C", details: ["why C"])])
        #expect(!note.body.contains("why A"))
    }

    @Test func handWrittenSubBulletsTravel() {
        let note = Note(body: "# dawn\n\n## Decisions\n- A\n    * sub\n- B\n").superseding(["A"], on: "2026-11-04")
        #expect(note.decisions == ["B"])
        #expect(note.body.contains("- A · superseded 2026-11-04\n    * sub"))
    }

    @Test func decisionsListStillOmitsDetails() {
        #expect(twoDecisions.decisions == ["A", "B"])
    }
}

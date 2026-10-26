import Testing
@testable import BinderCore

private let sample = """
---
verified: 2026-10-26T09:14:00+02:00
source: [observed, you]
---
# dawn

<!-- binder:observed start -->
- repo: github.com/Kungie/dawn
- site: Cloudflare Pages, see [[site]]
<!-- binder:observed end -->

## Decisions
- 2026-10-17 · Licence is AGPL-3.0 · kept from the chat

Keep the panel 340 wide.

"""

@Suite struct NoteTests {
    @Test func parseAndRenderRoundTrips() {
        let note = Note.parse(sample)
        #expect(note.render() == sample)
        #expect(note.frontmatter.map(\.key) == ["verified", "source"])
        #expect(note.frontmatter.first?.value == "2026-10-26T09:14:00+02:00")
    }

    @Test func replacingObservedOnlyTouchesTheBlock() {
        let note = Note.parse(sample).replacingObserved(["- repo: github.com/kungie/dawn", "- branch: main"])
        #expect(note.observedLines == ["- repo: github.com/kungie/dawn", "- branch: main"])
        #expect(note.decisions == ["2026-10-17 · Licence is AGPL-3.0 · kept from the chat"])
        #expect(note.render().contains("Keep the panel 340 wide."))
    }

    @Test func userTextOutsideMarkersIsPreserved() {
        let text = "# x\n\nabove  \n\n\(Note.observedStart)\n- old\n\(Note.observedEnd)\n\nbelow   \n\n\ntail"
        let rendered = Note.parse(text).replacingObserved(["- new one", "- new two"]).render()
        let prefix = "# x\n\nabove  \n\n\(Note.observedStart)\n"
        let suffix = "\(Note.observedEnd)\n\nbelow   \n\n\ntail"
        #expect(rendered == prefix + "- new one\n- new two\n" + suffix)
    }

    @Test func missingMarkersAreInsertedAfterTitle() {
        let note = Note.parse("# servers\nMy notes here.\n").replacingObserved(["- ssh: hetzner-1"])
        #expect(note.render() == "# servers\n\n\(Note.observedStart)\n- ssh: hetzner-1\n\(Note.observedEnd)\nMy notes here.\n")
        let untitled = Note.parse("just text\n").replacingObserved(["- a"])
        #expect(untitled.render() == "\(Note.observedStart)\n- a\n\(Note.observedEnd)\njust text\n")
    }

    @Test func duplicateMarkersCollapseToFirstPair() {
        let s = Note.observedStart, e = Note.observedEnd
        let text = "# x\n\(s)\n- a\n\(e)\nmiddle\n\(s)\n- b\n\(e)\nend\n"
        let rendered = Note.parse(text).replacingObserved(["- fresh"]).render()
        #expect(rendered == "# x\n\(s)\n- fresh\n\(e)\nmiddle\n- b\nend\n")
    }

    @Test func appendingDecisionCreatesSection() {
        let fresh = Note.new(title: "site").appendingDecision("2026-10-26 · Deploys to Cloudflare Pages · kept from a command")
        #expect(fresh.decisions == ["2026-10-26 · Deploys to Cloudflare Pages · kept from a command"])
        #expect(fresh.render() == "# site\n\n## Decisions\n- 2026-10-26 · Deploys to Cloudflare Pages · kept from a command\n")
        let existing = Note.parse(sample).appendingDecision("2026-10-26 · New one · written by you")
        #expect(existing.decisions.count == 2)
        #expect(existing.decisions.last == "2026-10-26 · New one · written by you")
        #expect(existing.render().contains("· kept from the chat\n- 2026-10-26 · New one · written by you\n\nKeep the panel"))
    }

    @Test func noteWithoutFrontmatterParses() {
        let note = Note.parse("# me\nEren\n")
        #expect(note.frontmatter.isEmpty)
        #expect(note.render() == "# me\nEren\n")
        let set = note.setting("verified", "now")
        #expect(set.render() == "---\nverified: now\n---\n# me\nEren\n")
        #expect(set.setting("verified", "later").frontmatter.map(\.value) == ["later"])
    }
}

import Testing
@testable import BinderCore

@Suite struct NoteEditingTests {
    let original = Note.parse("""
    ---
    verified: 2026-11-04T10:00:00Z
    source: observed, you
    ---
    # dawn

    <!-- binder:observed start -->
    - repo: github.com/kungie/dawn
    - deploy:   Cloudflare Pages  
    <!-- binder:observed end -->

    Builds on [[this-mac]].

    ## Decisions
    - 2026-11-04 · Ships free · written by you

    """)

    @Test func editableTextLeavesOutTheObservedBlock() {
        #expect(!original.editableText.contains("binder:observed"))
        #expect(!original.editableText.contains("repo: github.com"))
        #expect(original.editableText.contains("Builds on [[this-mac]]."))
        #expect(!original.editableText.contains("verified:"))
    }

    @Test func editKeepsObservedBlockByteForByte() {
        let edited = original.replacingEditable(original.editableText.replacingOccurrences(of: "Builds on", with: "Built on"))
        #expect(edited.observedLines == original.observedLines)
        #expect(edited.body.contains("<!-- binder:observed start -->\n- repo: github.com/kungie/dawn\n- deploy:   Cloudflare Pages  \n<!-- binder:observed end -->"))
        #expect(edited.body.contains("Built on [[this-mac]]."))
    }

    @Test func unchangedEditRoundTrips() {
        #expect(original.replacingEditable(original.editableText) == original)
    }

    @Test func editKeepsFrontmatter() {
        let edited = original.replacingEditable("# dawn\n\nNew text.\n")
        #expect(edited.frontmatter == original.frontmatter)
        #expect(edited.observedLines == original.observedLines)
    }

    @Test func noteWithoutObservedBlockEditsAsIs() {
        let plain = Note(body: "# Me\n\nHello.\n")
        #expect(plain.editableText == "# Me\n\nHello.\n")
        #expect(plain.replacingEditable("# Me\n\nBye.\n").body == "# Me\n\nBye.\n")
    }
}

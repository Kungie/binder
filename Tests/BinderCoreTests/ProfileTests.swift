import Foundation
import Testing
@testable import BinderCore

private let eren = Profile(name: "Eren", role: "One-person studio. Small instruments and Mac apps.", timeZone: "Europe/Berlin",
                           language: "Turkish in chat, English in files", workingStyle: ["Short answers", "Ask before pushing"],
                           taste: ["Swift", "SwiftUI"], avoid: ["Electron"], nowAndNext: "Shipping Binder v1.")

@Suite struct ProfileTests {
    @Test func meNoteRendersSections() {
        let note = MeNote.render(eren, into: nil)
        #expect(note.render() == """
        # Me

        - name: Eren
        - does: One-person studio. Small instruments and Mac apps.
        - time zone: Europe/Berlin
        - language: Turkish in chat, English in files

        ## How to work with me
        - Short answers
        - Ask before pushing

        ## Taste
        - likes: Swift, SwiftUI
        - rather not: Electron

        ## Now and next
        Shipping Binder v1.

        """)
    }

    @Test func meNoteKeepsObservedAndDecisions() {
        let existing = Note.parse("""
        ---
        source: you
        ---
        # Me

        \(Note.observedStart)
        - time zone seen: Europe/Berlin
        \(Note.observedEnd)
        - name: Old Name
        Loves coffee.

        ## Taste
        - likes: Python

        ## Decisions
        - 2026-10-26 · Short answers · kept from the chat

        """)
        let note = MeNote.render(eren, into: existing)
        let text = note.render()
        #expect(note.value("source") == "you")
        #expect(note.observedLines == ["- time zone seen: Europe/Berlin"])
        #expect(note.decisions == ["2026-10-26 · Short answers · kept from the chat"])
        #expect(text.contains("Loves coffee."))
        #expect(!text.contains("Old Name") && !text.contains("- likes: Python"))
        #expect(text.contains("- likes: Swift, SwiftUI"))
        #expect(text.range(of: "## Now and next")!.lowerBound < text.range(of: "## Decisions")!.lowerBound)
    }

    @Test func profileRoundTrips() {
        #expect(MeNote.profile(from: MeNote.render(eren, into: nil)) == eren)
        #expect(MeNote.profile(from: Note.new(title: "Me")) == Profile())
    }

    @Test func tasteRanksLanguagesByProjects() throws {
        let vault = Vault(root: makeTempDir())
        for (name, langs) in [("a", "Swift, Python"), ("b", "Swift, HTML"), ("c", "TypeScript, HTML, Swift")] {
            try NoteIO.write(Note.new(title: name).replacingObserved(["- repo: x", "- languages: \(langs)"]), to: vault.url("projects/\(name).md"))
        }
        #expect(TasteDetector.languages(vault: vault) == ["Swift", "HTML", "Python", "TypeScript"])
        #expect(TasteDetector.languages(vault: vault, limit: 2) == ["Swift", "HTML"])
    }
}

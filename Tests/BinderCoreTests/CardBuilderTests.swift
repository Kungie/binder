import Foundation
import Testing
@testable import BinderCore

/// A copy of Fixtures/vault in a temp folder, so tests can change it.
func fixtureVault() throws -> Vault {
    let root = makeTempDir("vault").appendingPathComponent("Binder")
    try FileManager.default.copyItem(at: fixture("vault"), to: root)
    return Vault(root: root)
}

let dawnRepo = GitRepo(root: URL(fileURLWithPath: "/Users/x/Desktop/dawn"), remote: "git@github.com:Kungie/dawn.git")

@Suite struct CardBuilderTests {
    @Test func cardMatchesGolden() throws {
        let golden = try String(contentsOf: fixture("card-dawn.txt"), encoding: .utf8)
        let card = CardBuilder.build(vault: try fixtureVault(), repo: dawnRepo)
        #expect(card + "\n" == golden)
    }

    @Test func cardRespectsBudget() throws {
        let vault = try fixtureVault()
        try NoteIO.update(vault.url("projects/dawn.md"), title: "dawn") { note in
            (1...90).reduce(note) { n, i in n.appendingDecision("2026-11-\(String(format: "%02d", min(i, 28))) · Decision number \(i) with enough words to take up some room · kept from the chat") }
        }
        let card = CardBuilder.build(vault: vault, repo: dawnRepo)
        #expect(card.count <= CardBuilder.budget)
        #expect(card.contains("Decision number 90 "))
        #expect(!card.contains("kept from the chat"))
        #expect(!card.contains("Decision number 1 "))
        #expect(card.hasPrefix(CardBuilder.header))
    }

    @Test func cardWithoutRepoHasHeaderAndMe() throws {
        let card = CardBuilder.build(vault: try fixtureVault(), repo: nil)
        #expect(card == CardBuilder.header + "\n\nME:\n- name: Eren\n- language: Turkish in chat, English in files\n- style: short answers, ask before pushing\nOne-person studio. Small instruments and Mac apps.")
    }

    @Test func cardNeverContainsSecrets() throws {
        let vault = try fixtureVault()
        let token = "gh" + "p_" + String(repeating: "a1B2c3D4e5", count: 4)
        write(try #require(read(vault.url("projects/dawn.md"))) + "- 2026-10-27 · Push with \(token) · written by you\n", to: vault.url("projects/dawn.md"))
        let card = CardBuilder.build(vault: vault, repo: dawnRepo)
        #expect(!card.contains(token))
        #expect(card.contains("[REDACTED:github]"))
    }

    @Test func cacheInvalidatesWhenNoteChanges() throws {
        let vault = try fixtureVault()
        let cache = CardCache(vault: vault)
        #expect(cache.cached(for: dawnRepo) == nil)
        try cache.store("CARD", for: dawnRepo)
        #expect(cache.cached(for: dawnRepo) == "CARD")
        let note = vault.url("projects/dawn.md")
        try FileManager.default.setAttributes([.modificationDate: Date().addingTimeInterval(5)], ofItemAtPath: note.path)
        #expect(cache.cached(for: dawnRepo) == nil)
    }

    @Test func cardIncludesPendingHandoff() throws {
        let vault = try fixtureVault()
        let card = HandoffCard(done: ["Panel redesign merged"], left: ["Notarization"], dontTouch: ["Resources/Fonts"], next: "Release notes for 0.3.")
        _ = try Handoff.write(card, project: "dawn", vault: vault, now: Date())
        let text = CardBuilder.build(vault: vault, repo: dawnRepo)
        #expect(text.hasSuffix("PICK UP FROM LAST TIME:\nDone: Panel redesign merged\nLeft: Notarization\nDon't touch: Resources/Fonts\nNext: Release notes for 0.3."))
    }
}

import Foundation
import Testing
@testable import BinderCore
@testable import BinderUI

@MainActor
@Suite struct FadingDisplayTests {
    @Test func pinBringsItBack() throws {
        let vault = Vault(root: makeTempDir("fade-ui"))
        let old = "2025-01-01 · Uses Netlify for previews · kept from the chat"
        try NoteIO.write(Note(body: "# dawn\n\n## Decisions\n- \(old)\n- 2099-01-01 · Future line · kept from the chat\n"), to: vault.url("projects/dawn.md"))
        let m = LibraryModel(vault: vault, home: makeTempDir("home"))
        let ref = NoteRef(title: "dawn", path: "projects/dawn.md")
        #expect(m.note(ref).fading.map(\.line) == [old])
        #expect(!m.note(ref).decisions.contains { $0.line == old })
        m.pin(old, in: ref)
        #expect(m.note(ref).fading.isEmpty)
        #expect(m.note(ref).decisions.contains { $0.line == old })
    }
}

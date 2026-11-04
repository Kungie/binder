import Foundation
import Testing
@testable import BinderCore
@testable import BinderUI

@MainActor
@Suite struct LibraryEditTests {
    func model() throws -> (LibraryModel, Vault) {
        let vault = try fixtureVault()
        let m = LibraryModel(vault: vault, home: makeTempDir("home"))
        m.trash = { try FileManager.default.removeItem(at: $0) }
        return (m, vault)
    }

    let dawn = NoteRef(title: "dawn", path: "projects/dawn.md")

    @Test func saveWritesTheEdit() throws {
        let (m, vault) = try model()
        let (text, opened) = m.beginEdit(dawn)
        #expect(m.save(text + "\nA new line of my own.\n", to: dawn, opened: opened) == .saved)
        #expect(try String(contentsOf: vault.url("projects/dawn.md"), encoding: .utf8).contains("A new line of my own."))
    }

    @Test func saveDetectsOutsideChange() throws {
        let (m, vault) = try model()
        let (text, opened) = m.beginEdit(dawn)
        try FileManager.default.setAttributes([.modificationDate: opened.addingTimeInterval(30)], ofItemAtPath: vault.url("projects/dawn.md").path)
        #expect(m.save(text + "\nMine.\n", to: dawn, opened: opened) == .changedOnDisk)
        #expect(try !String(contentsOf: vault.url("projects/dawn.md"), encoding: .utf8).contains("Mine."))
        #expect(m.save(text + "\nMine.\n", to: dawn, opened: opened, force: true) == .saved)
    }

    @Test func deleteLineRemovesItsDetail() throws {
        let (m, vault) = try model()
        try NoteIO.update(vault.url("projects/dawn.md"), title: "dawn") { $0.appendingDecision("2026-11-04 · Gone soon · kept from the chat", detail: "Its detail.") }
        m.delete(line: "2026-11-04 · Gone soon · kept from the chat", in: dawn)
        let body = try String(contentsOf: vault.url("projects/dawn.md"), encoding: .utf8)
        #expect(!body.contains("Gone soon") && !body.contains("Its detail."))
        #expect(m.note(dawn).decisions.allSatisfy { !$0.text.contains("Gone soon") })
    }

    @Test func decisionsCarryTheirDetailAndRawLine() throws {
        let (m, vault) = try model()
        try NoteIO.update(vault.url("projects/dawn.md"), title: "dawn") { $0.appendingDecision("2026-11-04 · With a reason · kept from the chat", detail: "The reason.") }
        let d = try #require(m.note(dawn).decisions.first { $0.text == "With a reason" })
        #expect(d.line == "2026-11-04 · With a reason · kept from the chat" && d.details == ["The reason."])
    }

    @Test func deleteNoteGoesToTrashAndRefusesMe() throws {
        let (m, vault) = try model()
        try m.deleteNote(dawn)
        #expect(!FileManager.default.fileExists(atPath: vault.url("projects/dawn.md").path))
        #expect(m.sections.first { $0.kind == .projects }?.items.contains(dawn) == false)
        #expect(throws: (any Error).self) { try m.deleteNote(NoteRef(title: "Me", path: "me.md")) }
        #expect(FileManager.default.fileExists(atPath: vault.url("me.md").path))
    }
}

import Foundation
import Testing
@testable import BinderCore

@Suite struct VaultTests {
    @Test func ensureLayoutCreatesFolders() throws {
        let vault = Vault(root: makeTempDir().appendingPathComponent("Binder"))
        try vault.ensureLayout()
        for folder in ["machines", "access", "servers", "projects", "handoffs", ".binder/inbox", ".binder/cards", ".binder/sessions"] {
            var isDir: ObjCBool = false
            #expect(FileManager.default.fileExists(atPath: vault.url(folder).path, isDirectory: &isDir) && isDir.boolValue, "\(folder)")
        }
        #expect(vault.inbox == vault.url(".binder/inbox"))
        #expect(vault.stateFile.lastPathComponent == "state.json")
    }

    @Test func noteIOWritesAtomicallyAndCreatesParents() throws {
        let root = makeTempDir()
        let url = root.appendingPathComponent("projects/deep/dawn.md")
        try NoteIO.write(Note.new(title: "dawn"), to: url)
        #expect(read(url) == "# dawn\n")
        #expect(NoteIO.read(url)?.render() == "# dawn\n")
        #expect(NoteIO.read(root.appendingPathComponent("missing.md")) == nil)
    }

    @Test func updateCreatesNoteWithTitle() throws {
        let url = makeTempDir().appendingPathComponent("servers/hetzner-1.md")
        try NoteIO.update(url, title: "hetzner-1") { $0.replacingObserved(["- ssh: hetzner-1"]) }
        #expect(NoteIO.read(url)?.observedLines == ["- ssh: hetzner-1"])
        try NoteIO.update(url, title: "ignored") { $0.appendingDecision("kept") }
        let note = try #require(NoteIO.read(url))
        #expect(note.render().hasPrefix("# hetzner-1\n"))
        #expect(note.decisions == ["kept"])
    }

    @Test func logNeverThrowsWhenFolderMissing() {
        let vault = Vault(root: makeTempDir().appendingPathComponent("nowhere/at/all"))
        vault.log("first line")
        vault.log("second line")
        let text = read(vault.logFile) ?? ""
        #expect(text.contains("first line") && text.contains("second line"))
    }

    @Test func stateDefaultsWhenFileBroken() throws {
        let vault = Vault(root: makeTempDir())
        #expect(BinderState.load(vault).summarizerConsent == false)
        write("{ not json", to: vault.stateFile)
        let state = BinderState.load(vault)
        #expect(state.handoffThreshold == 0.70)
        #expect(state.projectRoots.contains("~/Desktop"))
        var changed = state
        changed.summarizerConsent = true
        try changed.save(vault)
        #expect(BinderState.load(vault).summarizerConsent == true)
    }

    @Test func thresholdIsClamped() {
        var state = BinderState()
        state.handoffThreshold = 0.95
        #expect(state.clampedThreshold == 0.90)
        state.handoffThreshold = 0.2
        #expect(state.clampedThreshold == 0.50)
        state.handoffThreshold = 0.7
        #expect(state.clampedThreshold == 0.70)
    }
}

import Foundation
import Testing
@testable import BinderCore

@Suite struct MemoryTests {
    let vault = Vault(root: makeTempDir())
    let day = Calendar.current.date(from: DateComponents(year: 2026, month: 10, day: 26, hour: 12))!

    @Test func memoryAddsWrittenByYou() throws {
        #expect(try Memory.add("Keep the panel 350 wide.", to: "projects/dawn.md", vault: vault, now: day))
        #expect(NoteIO.read(vault.url("projects/dawn.md"))?.decisions == ["2026-10-26 · Keep the panel 350 wide. · written by you"])
        #expect(try !Memory.add("   ", to: "projects/dawn.md", vault: vault, now: day))
    }

    @Test func memoryRefusesSecret() throws {
        let token = "gh" + "p_" + String(repeating: "a1B2c3D4e5", count: 4)
        #expect(try !Memory.add("Push with \(token)", to: "projects/site.md", vault: vault, now: day))
        #expect(NoteIO.read(vault.url("projects/site.md")) == nil)
    }

    @Test func onboardedDefaultsFalse() throws {
        #expect(BinderState().onboarded == false)
        write(#"{"summarizerConsent": true}"#, to: vault.stateFile)
        let state = BinderState.load(vault)
        #expect(state.summarizerConsent == true)
        #expect(state.onboarded == false)
        #expect(state.handoffThreshold == 0.70)
    }

    @Test func sessionStatusKeepsTranscriptPath() throws {
        let repo = makeTempDir().appendingPathComponent("dawn")
        write("[core]\n", to: repo.appendingPathComponent(".git/config"))
        let input = HookInput(sessionID: "s9", transcriptPath: fixture("transcript.jsonl").path, cwd: repo.path, hookEventName: "Stop", source: nil)
        _ = try Capture.run(input: input, vault: vault, runner: FakeRunner(), summarizer: nil)
        #expect(SessionStatus.load(session: "s9", vault: vault)?.transcriptPath == fixture("transcript.jsonl").path)
    }
}

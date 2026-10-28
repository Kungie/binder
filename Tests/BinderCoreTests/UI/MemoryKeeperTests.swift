import Foundation
import Testing
@testable import BinderCore
@testable import BinderUI

@MainActor
@Suite(.serialized) struct MemoryKeeperTests {
    let vault = Vault(root: makeTempDir("keeper"))

    final class Spawns: @unchecked Sendable {
        var calls: [[String]] = []
    }

    /// A session whose transcript has `talk` waiting after what the queue already took.
    func session(_ id: String, updated: Date, talk: String) throws {
        try vault.ensureLayout()
        let url = makeTempDir("t").appendingPathComponent("\(id).jsonl")
        let first: [String: Any] = ["type": "user", "message": ["role": "user", "content": "hello"]]
        write(String(decoding: try JSONSerialization.data(withJSONObject: first), as: UTF8.self) + "\n", to: url)
        // The Stop hook has seen it: the queue knows the session and took nothing yet.
        _ = SummarizeQueue(vault: vault).claim(session: id, transcript: url, startingAt: 0, now: updated)
        let more: [String: Any] = ["type": "user", "message": ["role": "user", "content": talk]]
        let handle = try FileHandle(forWritingTo: url)
        try handle.seekToEnd()
        try handle.write(contentsOf: JSONSerialization.data(withJSONObject: more) + Data("\n".utf8))
        try handle.close()
        try SessionStatus(session: id, project: "dawn", cwd: "/x/dawn", reading: nil, updatedAt: updated, transcriptPath: url.path).save(vault)
    }

    func keeper(_ spawns: Spawns) -> MemoryKeeper {
        MemoryKeeper(vault: { [vault] in vault }, helper: "/Applications/Binder.app/Contents/Helpers/binder",
                     spawn: { path, args in spawns.calls.append([path] + args) })
    }

    func consent(_ on: Bool) throws {
        var state = BinderState.load(vault)
        state.summarizerConsent = on
        try state.save(vault)
    }

    @Test func aQuietSessionsLeftoverTalkGetsSummarized() throws {
        try consent(true)
        let now = Date()
        try session("quiet", updated: now.addingTimeInterval(-15 * 60), talk: "Let's keep release notes in docs/releases.")
        try session("busy", updated: now.addingTimeInterval(-60), talk: "Still going.")
        let spawns = Spawns()
        keeper(spawns).sweep(now: now)
        #expect(spawns.calls.count == 1)
        #expect(spawns.calls.first?[1] == "summarize")
        #expect(spawns.calls.first?[2] == "quiet")
        #expect(spawns.calls.first?.last == "/x/dawn")
        keeper(spawns).sweep(now: now)
        #expect(spawns.calls.count == 1)
    }

    @Test func nothingIsReadWithoutConsent() throws {
        try consent(false)
        let now = Date()
        try session("quiet", updated: now.addingTimeInterval(-15 * 60), talk: "Something said.")
        let spawns = Spawns()
        keeper(spawns).sweep(now: now)
        #expect(spawns.calls.isEmpty)
    }

    @Test func oldSessionsAreLeftAlone() throws {
        try consent(true)
        let now = Date()
        try session("ancient", updated: now.addingTimeInterval(-3 * 86400), talk: "Long ago.")
        let spawns = Spawns()
        keeper(spawns).sweep(now: now)
        #expect(spawns.calls.isEmpty)
    }
}

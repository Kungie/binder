import Foundation
import Testing
@testable import BinderCore

@Suite struct WrongTests {
    let now = ISO8601DateFormatter().date(from: "2026-11-05T10:00:00Z")!
    let line = "2026-11-01 · The API runs on Fly.io · kept from the chat"

    func vault() throws -> Vault {
        let vault = Vault(root: makeTempDir("wrong"))
        try NoteIO.update(vault.url("projects/dawn.md"), title: "dawn") { $0.appendingDecision(line, detail: "Moved there in October.") }
        return vault
    }

    @Test func wrongSupersedesAndMarks() throws {
        let v = try vault()
        try Forgetting.markWrong(line, in: "projects/dawn.md", vault: v, now: now)
        let note = try #require(NoteIO.read(v.url("projects/dawn.md")))
        #expect(note.decisions.isEmpty)
        #expect(note.superseded == ["\(line) · wrong 2026-11-05"])
    }

    @Test func wrongBlocksTheSameTextComingBack() throws {
        let v = try vault()
        try Forgetting.markWrong(line, in: "projects/dawn.md", vault: v, now: now)
        let again = Proposal.make(kind: .fact, source: .chat, text: "The API runs on Fly.io", evidence: "said", target: "projects/dawn.md", project: "dawn", session: "s")
        #expect(try !ProposalStore(vault: v).add(again))
    }

    @Test func reviewListsNotTrue() throws {
        let v = try vault()
        try Forgetting.markWrong(line, in: "projects/dawn.md", vault: v, now: now)
        let known = ClaudeCLISummarizer.Known.load(vault: v, project: "dawn")
        #expect(known.wrong == ["The API runs on Fly.io"])
        let spy = SpyRunner(reply: #"{"new":[],"outdated":[]}"#)
        _ = ClaudeCLISummarizer(runner: spy, session: "s").review(entries: [.user("x")], project: "dawn", known: known)
        #expect(spy.calls.first?.input.contains("NOT TRUE:\n- The API runs on Fly.io") == true)
        #expect(ClaudeCLISummarizer.reviewSystem.contains("NOT TRUE"))
    }

    @Test func wrongKeepsDetailAndFile() throws {
        let v = try vault()
        let fileLine = "2026-11-01 · ![[files/dawn/2026-11-01-shot.png]] Shot · kept from the chat"
        try FileManager.default.createDirectory(at: v.url("files/dawn"), withIntermediateDirectories: true)
        try Data("x".utf8).write(to: v.url("files/dawn/2026-11-01-shot.png"))
        try NoteIO.update(v.url("projects/dawn.md"), title: "dawn") { $0.appendingDecision(fileLine) }
        try Forgetting.markWrong(line, in: "projects/dawn.md", vault: v, now: now)
        try Forgetting.markWrong(fileLine, in: "projects/dawn.md", vault: v, now: now)
        let body = try #require(NoteIO.read(v.url("projects/dawn.md"))?.body)
        #expect(body.contains("\(line) · wrong 2026-11-05\n  - Moved there in October."))
        #expect(FileManager.default.fileExists(atPath: v.url("files/dawn/2026-11-01-shot.png").path))
    }

    @Test func restoreUndoesWrong() throws {
        let v = try vault()
        try Forgetting.markWrong(line, in: "projects/dawn.md", vault: v, now: now)
        try Memory.restore("\(line) · wrong 2026-11-05", in: "projects/dawn.md", vault: v)
        let note = try #require(NoteIO.read(v.url("projects/dawn.md")))
        #expect(note.decisionItems == [DecisionItem(line: line, details: ["Moved there in October."])])
    }
}

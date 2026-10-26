import Foundation
import Testing
@testable import BinderCore

private final class SpySummarizer: Summarizer, @unchecked Sendable {
    var calls = 0
    func proposals(from entries: [TranscriptEntry], project: String?) -> [Proposal] {
        calls += 1
        return [Proposal.make(kind: .decision, source: .chat, text: "Release notes live in docs/releases.",
                              evidence: "session", target: "projects/\(project ?? "x").md", project: project, session: "sess-1")]
    }
}

@Suite struct CaptureTests {
    let vault = Vault(root: makeTempDir("vault"))
    let repo: URL = {
        let root = makeTempDir("projects").appendingPathComponent("dawn")
        write("[core]\n", to: root.appendingPathComponent(".git/config"))
        return root
    }()

    var input: HookInput {
        HookInput(sessionID: "sess-1", transcriptPath: fixture("transcript.jsonl").path, cwd: repo.path, hookEventName: "Stop", source: nil)
    }

    @Test func captureAddsProposalsFromFixture() throws {
        let result = try Capture.run(input: input, vault: vault, runner: FakeRunner(), summarizer: nil)
        #expect(result.proposalsAdded == 1)
        #expect(ProposalStore(vault: vault).pending().map(\.text) == ["dawn deploys to Cloudflare Pages."])
    }

    @Test func secondCaptureAddsNothing() throws {
        _ = try Capture.run(input: input, vault: vault, runner: FakeRunner(), summarizer: nil)
        let again = try Capture.run(input: input, vault: vault, runner: FakeRunner(), summarizer: nil)
        #expect(again.proposalsAdded == 0)
        #expect(ProposalStore(vault: vault).pending().count == 1)
    }

    @Test func summarizerNotCalledWithoutConsent() throws {
        let spy = SpySummarizer()
        _ = try Capture.run(input: input, vault: vault, runner: FakeRunner(), summarizer: spy)
        #expect(spy.calls == 0)
    }

    @Test func summarizerCalledWithConsent() throws {
        var state = BinderState()
        state.summarizerConsent = true
        try state.save(vault)
        let spy = SpySummarizer()
        let result = try Capture.run(input: input, vault: vault, runner: FakeRunner(), summarizer: spy)
        #expect(spy.calls == 1)
        #expect(result.proposalsAdded == 2)
    }

    @Test func captureWritesSessionReading() throws {
        let result = try Capture.run(input: input, vault: vault, runner: FakeRunner(), summarizer: nil)
        #expect(result.reading == ContextReading(used: 120_000, window: 200_000))
        let status = try #require(SessionStatus.load(session: "sess-1", vault: vault))
        #expect(status.project == "dawn" && status.reading == result.reading && status.cwd == repo.path)
    }

    @Test func cliSummarizerParsesModelJSON() throws {
        let reply = "Here you go:\n[{\"kind\":\"decision\",\"text\":\"Release notes live in docs/releases.\",\"confidence\":\"high\"},{\"kind\":\"preference\",\"text\":\"Short answers.\"}]\n"
        let runner = FakeRunner(results: ["claude -p --model haiku --output-format text": ok(reply)])
        let summarizer = ClaudeCLISummarizer(runner: runner, session: "sess-1")
        let proposals = summarizer.proposals(from: [.user("Put release notes in docs/releases please")], project: "dawn")
        #expect(proposals.map(\.text) == ["Release notes live in docs/releases.", "Short answers."])
        #expect(proposals.map(\.target) == ["projects/dawn.md", "me.md"])
        #expect(proposals.allSatisfy { $0.source == .chat })
        #expect(ClaudeCLISummarizer(runner: FakeRunner(), session: nil).proposals(from: [.user("x")], project: nil).isEmpty)
    }
}

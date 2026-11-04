import Foundation
import Testing
@testable import BinderCore

@Suite struct TopicSplitTests {
    func vault() throws -> Vault {
        let vault = Vault(root: makeTempDir("split"))
        try NoteIO.write(Note(body: """
        # dawn

        ## Decisions
        - 2026-10-01 · Runs on Fly.io · kept from the chat
          - Cheaper than Heroku.
        - 2026-10-02 · The API is REST, versioned under /v1 · kept from the chat
        - 2026-10-03 · Licence is AGPL-3.0 · written by you
        """), to: vault.url("projects/dawn.md"))
        return vault
    }

    func split(_ vault: Vault, reply: String) throws -> TopicSplit.Result {
        try TopicSplit.run(project: "dawn", vault: vault, summarizer: ClaudeCLISummarizer(runner: SpyRunner(reply: reply), session: nil))
    }

    @Test func splitMovesOnlyNamedLines() throws {
        let vault = try vault()
        // Lines go to the model newest first: L1 licence, L2 API, L3 Fly.io.
        let result = try split(vault, reply: #"{"topics":{"hosting":["L3"],"API":["L2"]}}"#)
        #expect(result == TopicSplit.Result(lines: 2, topics: 2))
        let overview = try #require(NoteIO.read(vault.url("projects/dawn.md")))
        #expect(overview.decisions.contains("2026-10-03 · Licence is AGPL-3.0 · written by you"))
        #expect(!overview.decisions.contains { $0.contains("Fly.io") || $0.contains("REST") })
        #expect(overview.render().contains("[[dawn/hosting]]") && overview.render().contains("[[dawn/api]]"))
        #expect(NoteIO.read(vault.url("projects/dawn/api.md"))?.decisions == ["2026-10-02 · The API is REST, versioned under /v1 · kept from the chat"])
    }

    @Test func splitKeepsDetails() throws {
        let vault = try vault()
        _ = try split(vault, reply: #"{"topics":{"hosting":["L3"]}}"#)
        #expect(NoteIO.read(vault.url("projects/dawn/hosting.md"))?.decisionItems.first?.details == ["Cheaper than Heroku."])
        #expect(NoteIO.read(vault.url("projects/dawn.md"))?.body.contains("Cheaper than Heroku.") == false)
    }

    @Test func splitIgnoresUnknownIds() throws {
        let vault = try vault()
        let result = try split(vault, reply: #"{"topics":{"hosting":["L9","banana",3],"../x":["L3"]}}"#)
        #expect(result.lines == 1)
        #expect(NoteIO.read(vault.url("projects/dawn/x.md"))?.decisions.count == 1)
    }

    @Test func splitWithBadReplyMovesNothing() throws {
        let vault = try vault()
        let before = try String(contentsOf: vault.url("projects/dawn.md"), encoding: .utf8)
        #expect(try split(vault, reply: "sorry, no").lines == 0)
        #expect(try String(contentsOf: vault.url("projects/dawn.md"), encoding: .utf8) == before)
    }

    @Test func topicLinksAreNotSentToTheModel() throws {
        let vault = try vault()
        _ = try split(vault, reply: #"{"topics":{"hosting":["L3"]}}"#)
        let spy = SpyRunner(reply: #"{"topics":{}}"#)
        _ = try TopicSplit.run(project: "dawn", vault: vault, summarizer: ClaudeCLISummarizer(runner: spy, session: nil))
        #expect(spy.calls.first?.input.contains("Topic:") == false)
        #expect(spy.calls.first?.input.contains("TOPICS: hosting") == true)
    }
}

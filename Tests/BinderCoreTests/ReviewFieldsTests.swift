import Foundation
import Testing
@testable import BinderCore

@Suite struct ReviewFieldsTests {
    let known = ClaudeCLISummarizer.Known(project: [], me: [], topics: ["hosting"])

    func review(_ reply: String) -> ClaudeCLISummarizer.Review {
        ClaudeCLISummarizer(runner: SpyRunner(reply: reply), session: "s").review(entries: [.user("x")], project: "dawn", known: known)
    }

    @Test func reviewReadsNewFields() throws {
        let r = review(#"{"new":[{"kind":"fact","scope":"project","text":"The API runs in Docker on hetzner-1.","confidence":"high","detail":"It moved off Fly.io to save money. Compose file is in deploy/.","quote":"Let's run the API on hetzner-1 with Docker.","topic":"Hosting","also":["server:hetzner-1"]}],"outdated":[]}"#)
        let p = try #require(r.proposals.first)
        #expect(p.target == "projects/dawn/hosting.md")
        #expect(p.also == ["servers/hetzner-1.md"])
        #expect(p.detail == "It moved off Fly.io to save money. Compose file is in deploy/.")
        #expect(p.quote == "Let's run the API on hetzner-1 with Docker.")
    }

    @Test func reviewWithoutNewFieldsStillParses() throws {
        let p = try #require(review(#"{"new":[{"kind":"fact","scope":"project","text":"Deploys with wrangler.","confidence":"high"}],"outdated":[]}"#).proposals.first)
        #expect(p.target == "projects/dawn.md")
        #expect(p.detail == nil && p.quote == nil && p.also.isEmpty)
    }

    @Test func reviewPromptAsksForEnglish() {
        #expect(ClaudeCLISummarizer.reviewSystem.contains("in English, whatever language"))
        let spy = SpyRunner(reply: #"{"new":[],"outdated":[]}"#)
        _ = ClaudeCLISummarizer(runner: spy, session: "s").review(entries: [.user("x")], project: "dawn", known: known)
        #expect(spy.calls.first?.input.contains("TOPICS: hosting") == true)
    }

    @Test func reviewIgnoresUnsafeTopicAndAlso() throws {
        let r = review(#"{"new":[{"kind":"fact","scope":"project","text":"One.","topic":"../../etc","also":["servers/../me",4,"ftp:x","server:../../x"]},{"kind":"fact","scope":"project","text":"Two.","topic":""}],"outdated":[]}"#)
        #expect(r.proposals.map(\.target) == ["projects/dawn/etc.md", "projects/dawn.md"])
        #expect(r.proposals[0].also == ["servers/x.md"])
        #expect(!r.proposals.contains { $0.target.contains("..") || $0.also.contains { $0.contains("..") } })
    }

    @Test func oldInboxFileDecodes() throws {
        let json = #"{"id":"p_1","createdAt":"2026-10-28T10:00:00Z","agent":"claude-code","kind":"fact","source":"chat","evidence":"e","text":"t","target":"projects/dawn.md","confidence":"high"}"#
        let d = JSONDecoder()
        d.dateDecodingStrategy = .iso8601
        let p = try d.decode(Proposal.self, from: Data(json.utf8))
        #expect(p.also.isEmpty && p.file == nil && p.detail == nil)
    }

    @Test func slugRules() {
        #expect(Topic.slug("Payments API") == "payments-api")
        #expect(Topic.slug("  ") == nil)
        #expect(Topic.slug(String(repeating: "a", count: 40))?.count == 24)
        #expect(Destination.path(for: "hosting:Cloudflare") == "hosting/cloudflare.md")
        #expect(Destination.path(for: "ftp:x") == nil)
    }

    @Test func detailTextFallsBackToEvidenceForCommands() {
        let p = Proposal.make(kind: .decision, source: .command, text: "t", evidence: "wrangler pages deploy", target: "projects/dawn.md", project: "dawn", session: nil)
        #expect(p.detailText == "wrangler pages deploy")
        let chat = Proposal.make(kind: .decision, source: .chat, text: "t", evidence: "said in session s", target: "projects/dawn.md", project: "dawn", session: nil)
        #expect(chat.detailText == nil)
    }
}

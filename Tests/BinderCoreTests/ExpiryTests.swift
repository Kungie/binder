import Foundation
import Testing
@testable import BinderCore

@Suite struct ExpiryTests {
    func day(_ offset: Int, from base: Date = Date()) -> String {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.timeZone = TimeZone(identifier: "UTC")
        f.dateFormat = "yyyy-MM-dd"
        return f.string(from: base.addingTimeInterval(Double(offset) * 86_400))
    }

    func reviewed(until: String) -> Proposal? {
        let reply = #"{"new":[{"kind":"fact","scope":"project","text":"The API is down for maintenance.","until":"\#(until)"}],"outdated":[]}"#
        return ClaudeCLISummarizer(runner: SpyRunner(reply: reply), session: "s")
            .review(entries: [.user("x")], project: "dawn", known: .init(project: [], me: [])).proposals.first
    }

    @Test func untilMustBeAFutureDate() {
        #expect(reviewed(until: day(5))?.until != nil)
        #expect(reviewed(until: day(-3))?.until == nil)
        #expect(reviewed(until: day(400))?.until == nil)
        #expect(reviewed(until: "Friday")?.until == nil)
        #expect(ClaudeCLISummarizer.reviewSystem.contains("until"))
    }

    @Test func keepRecordsExpiry() throws {
        let vault = Vault(root: makeTempDir("expiry"))
        var p = Proposal.make(kind: .fact, source: .chat, text: "The API is down", evidence: "said", target: "projects/dawn.md",
                              project: "dawn", session: "s", also: ["servers/hetzner-1.md"])
        p.until = Date().addingTimeInterval(3 * 86_400)
        let store = ProposalStore(vault: vault)
        #expect(try store.add(p))
        try store.keep(p.id)
        let expiry = ExpiryStore(vault: vault)
        let line = try #require(NoteIO.read(vault.url("projects/dawn.md"))?.decisions.first)
        let serverLine = try #require(NoteIO.read(vault.url("servers/hetzner-1.md"))?.decisions.first)
        #expect(expiry.until(line: line, in: "projects/dawn.md") != nil)
        #expect(expiry.until(line: serverLine, in: "servers/hetzner-1.md") != nil)
    }

    @Test func tidyMovesExpiredLines() throws {
        let vault = Vault(root: makeTempDir("expiry-tidy"))
        try Memory.add("The API is down", to: "projects/dawn.md", vault: vault)
        let line = try #require(NoteIO.read(vault.url("projects/dawn.md"))?.decisions.first)
        ExpiryStore(vault: vault).set(Date().addingTimeInterval(-86_400), line: line, in: "projects/dawn.md")
        _ = Tidy.run(vault: vault, summarizer: nil, everything: true)
        let note = try #require(NoteIO.read(vault.url("projects/dawn.md")))
        #expect(note.decisions.isEmpty)
        #expect(note.superseded.first?.contains(" · expired ") == true)
        #expect(ExpiryStore(vault: vault).until(line: line, in: "projects/dawn.md") == nil)
    }

    @Test func cardDropsExpiredLines() throws {
        let vault = Vault(root: makeTempDir("expiry-card"))
        try Memory.add("The API is down", to: "projects/dawn.md", vault: vault)
        try Memory.add("Ships free", to: "projects/dawn.md", vault: vault)
        let line = try #require(NoteIO.read(vault.url("projects/dawn.md"))?.decisions.first)
        ExpiryStore(vault: vault).set(Date().addingTimeInterval(-60), line: line, in: "projects/dawn.md")
        let card = CardBuilder.build(vault: vault, repo: GitRepo(root: URL(fileURLWithPath: "/x/dawn"), remote: nil))
        #expect(!card.contains("The API is down"))
        #expect(card.contains("Ships free"))
    }
}

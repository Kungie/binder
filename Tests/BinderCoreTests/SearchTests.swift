import Foundation
import Testing
@testable import BinderCore

@Suite struct SearchTests {
    func vault() throws -> Vault {
        let vault = Vault(root: makeTempDir("search"))
        try Memory.add("The site deploys to Cloudflare Pages.", to: "projects/dawn.md", vault: vault)
        try Memory.add("Cloudflare tokens live in 1Password.", to: "projects/site.md", vault: vault)
        try Memory.add("Pages are built with Astro and deploy from main.", to: "projects/site.md", vault: vault)
        try NoteIO.write(Note.new(title: "hetzner-1").replacingObserved(["- os: Ubuntu 24.04", "- runs: Docker"]), to: vault.url("servers/hetzner-1.md"))
        try NoteIO.update(vault.url("projects/dawn.md"), title: "dawn") {
            $0.appendingDecision("2026-10-01 · Old host was Netlify · kept from the chat").superseding(["2026-10-01 · Old host was Netlify · kept from the chat"], on: "2026-10-20")
        }
        return vault
    }

    @Test func exactPhraseRanksFirst() throws {
        let hits = Search.run("cloudflare pages", vault: try vault())
        #expect(hits.first?.line.contains("Cloudflare Pages") == true)
        #expect(hits.count >= 2)
    }

    @Test func projectBoost() throws {
        let hits = Search.run("cloudflare", vault: try vault(), project: "site")
        #expect(hits.first?.path == "projects/site.md")
    }

    @Test func supersededOnlyWithAll() throws {
        let v = try vault()
        #expect(Search.run("netlify", vault: v).isEmpty)
        #expect(Search.run("netlify", vault: v, includeSuperseded: true).first?.path == "projects/dawn.md")
    }

    @Test func observedLinesAreFound() throws {
        #expect(Search.run("ubuntu", vault: try vault()).first?.path == "servers/hetzner-1.md")
    }

    @Test func renderStaysUnderBudget() throws {
        let v = Vault(root: makeTempDir("search-big"))
        for i in 0..<500 { try Memory.add("Deploy step \(i) runs on the build server with docker", to: "projects/p\(i).md", vault: v) }
        let text = Search.render(Search.run("deploy docker", vault: v, limit: 200))
        #expect(text.count <= 1_500)
        #expect(text.hasPrefix("projects/"))
    }

    @Test func secretsRedacted() throws {
        let v = Vault(root: makeTempDir("search-secret"))
        try NoteIO.write(Note(body: "# x\n\nThe deploy token is ghp_abcdefghijklmnopqrstuvwxyz0123456789 for now.\n"), to: v.url("projects/x.md"))
        let text = Search.render(Search.run("deploy token", vault: v))
        #expect(!text.contains("ghp_abcdefghijklmnopqrstuvwxyz0123456789"))
    }

    @Test func searchRecordsUse() throws {
        let v = try vault()
        let hits = Search.run("astro", vault: v)
        UsageStore(vault: v).recordSearch(hits: hits)
        let line = try #require(NoteIO.read(v.url("projects/site.md"))?.decisions.first { $0.contains("Astro") })
        #expect(UsageStore(vault: v).counts(line: line, in: "projects/site.md").used == 1)
        #expect(UsageStore(vault: v).counts(note: "projects/site.md").used == 1)
    }

    @Test func oddQueriesAreSafe() throws {
        let v = try vault()
        #expect(Search.run("(.*", vault: v).isEmpty)
        #expect(Search.run("zzzz", vault: v).isEmpty)
        #expect(Search.run("", vault: v).isEmpty)
        #expect(Search.render([]) == "Nothing in the binder matches.")
    }
}

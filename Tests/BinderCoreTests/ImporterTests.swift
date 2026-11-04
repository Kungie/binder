import Foundation
import Testing
@testable import BinderCore

@Suite struct ImporterTests {
    let home = makeTempDir("import-home")
    var roots: [URL] { [home.appendingPathComponent("code")] }
    let vault = Vault(root: makeTempDir("import-vault"))

    func setUp() throws {
        write("# Me\n\nAnswer short. Ask before pushing.\n", to: home.appendingPathComponent(".claude/CLAUDE.md"))
        let repo = home.appendingPathComponent("code/dawn")
        try FileManager.default.createDirectory(at: repo.appendingPathComponent(".git"), withIntermediateDirectories: true)
        write("# dawn\n\nDeploy with wrangler.\n", to: repo.appendingPathComponent("CLAUDE.md"))
        write("Use pnpm.\n", to: repo.appendingPathComponent("AGENTS.md"))
        try vault.ensureLayout()
    }

    let reply = #"{"new":[{"kind":"preference","scope":"me","text":"Wants short answers.","confidence":"high"},{"kind":"decision","scope":"project","text":"dawn deploys with wrangler.","topic":"hosting","confidence":"high"}]}"#

    @Test func findsMeAndProjectFiles() throws {
        try setUp()
        let sources = Importer.sources(home: home, projectRoots: roots)
        #expect(sources.map(\.file.lastPathComponent).sorted() == ["AGENTS.md", "CLAUDE.md", "CLAUDE.md"])
        #expect(sources.first { $0.project == nil }?.target == "me.md")
        #expect(sources.filter { $0.project == "dawn" }.allSatisfy { $0.target == "projects/dawn.md" })
    }

    @Test func importOffersCardsAndSkipsUnchangedFiles() throws {
        try setUp()
        let sources = Importer.sources(home: home, projectRoots: roots).filter { $0.file.lastPathComponent == "CLAUDE.md" && $0.project == "dawn" }
        let spy = SpyRunner(reply: reply)
        let added = try Importer.run(vault: vault, sources: sources, summarizer: ClaudeCLISummarizer(runner: spy, session: nil))
        #expect(added == 2)
        let pending = ProposalStore(vault: vault).pending()
        #expect(Set(pending.map(\.target)) == ["me.md", "projects/dawn/hosting.md"])
        #expect(pending.allSatisfy { $0.evidence == "from CLAUDE.md" })
        #expect(try Importer.run(vault: vault, sources: sources, summarizer: ClaudeCLISummarizer(runner: spy, session: nil)) == 0)
        #expect(spy.calls.count == 1)
        write("# dawn\n\nDeploy with wrangler. Now also Fly.\n", to: home.appendingPathComponent("code/dawn/CLAUDE.md"))
        _ = try Importer.run(vault: vault, sources: sources, summarizer: ClaudeCLISummarizer(runner: spy, session: nil))
        #expect(spy.calls.count == 2)
    }

    @Test func importCapsAFileAt12k() throws {
        try setUp()
        write(String(repeating: "Line of rules. ", count: 3_000), to: home.appendingPathComponent(".claude/CLAUDE.md"))
        let spy = SpyRunner(reply: #"{"new":[]}"#)
        _ = try Importer.run(vault: vault, sources: Importer.sources(home: home, projectRoots: []), summarizer: ClaudeCLISummarizer(runner: spy, session: nil))
        #expect((spy.calls.first?.input.count ?? 0) < 14_000)
    }

    @Test func fortyLineCap() throws {
        try setUp()
        let items = (0..<60).map { #"{"kind":"fact","scope":"me","text":"Rule number \#($0) for agents."}"# }.joined(separator: ",")
        let added = try Importer.run(vault: vault, sources: Importer.sources(home: home, projectRoots: []),
                                     summarizer: ClaudeCLISummarizer(runner: SpyRunner(reply: #"{"new":[\#(items)]}"#), session: nil))
        #expect(added == 40)
    }

    @Test func badReplyRecordsNothing() throws {
        try setUp()
        let sources = Importer.sources(home: home, projectRoots: [])
        #expect(try Importer.run(vault: vault, sources: sources, summarizer: ClaudeCLISummarizer(runner: SpyRunner(reply: "no"), session: nil)) == 0)
        let spy = SpyRunner(reply: reply)
        _ = try Importer.run(vault: vault, sources: sources, summarizer: ClaudeCLISummarizer(runner: spy, session: nil))
        #expect(spy.calls.count == 1)
    }

    @Test func importPromptAsksForEnglish() {
        #expect(ClaudeCLISummarizer.importSystem.contains("English"))
    }

    @Test func dedupeAgainstKept() throws {
        try setUp()
        try Memory.add("Wants short answers.", to: "me.md", vault: vault)
        _ = try Importer.run(vault: vault, sources: Importer.sources(home: home, projectRoots: []),
                             summarizer: ClaudeCLISummarizer(runner: SpyRunner(reply: reply), session: nil))
        #expect(ProposalStore(vault: vault).pending().isEmpty)
    }
}

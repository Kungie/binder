import Foundation
import Testing
@testable import BinderCore

/// Findings from the 1.1 branch review, each pinned by a test.
@Suite struct FinalFixesTests {
    // 1. Obsidian indents with a tab.
    @Test func tabIndentedDetailStaysWithItsDecision() {
        let note = Note(body: "# dawn\n\n## Decisions\n- A\n\t- tab detail of A\n")
        let appended = note.appendingDecision("B")
        #expect(appended.body.contains("- A\n\t- tab detail of A\n- B"))
        let superseded = note.superseding(["A"], on: "2026-11-04")
        #expect(superseded.body.contains("- A · superseded 2026-11-04\n\t- tab detail of A"))
        #expect(!superseded.body.components(separatedBy: "## Superseded")[0].contains("tab detail"))
    }

    // 2. A Keep that lands while the model sorts topics survives the split.
    @Test func splitKeepsAChangeMadeWhileTheModelThinks() throws {
        let vault = Vault(root: makeTempDir("split-race"))
        try NoteIO.write(Note(body: "# dawn\n\n## Decisions\n- 2026-10-01 · Runs on Fly.io · kept from the chat\n"), to: vault.url("projects/dawn.md"))
        let runner = MidSplitRunner(reply: #"{"topics":{"hosting":["L1"]}}"#) {
            try? Memory.add("Kept while sorting", to: "projects/dawn.md", vault: vault)
        }
        let result = try TopicSplit.run(project: "dawn", vault: vault, summarizer: ClaudeCLISummarizer(runner: runner, session: nil))
        #expect(result.lines == 1)
        let overview = try #require(NoteIO.read(vault.url("projects/dawn.md")))
        #expect(overview.decisions.contains { $0.contains("Kept while sorting") })
        #expect(!overview.decisions.contains { $0.contains("Fly.io") })
        #expect(NoteIO.read(vault.url("projects/dawn/hosting.md"))?.decisions.count == 1)
    }

    // 3. Kept files are remembered by hash at Keep, so staging never rereads files/.
    @Test func keptFileHashIsRecordedAtKeep() throws {
        let vault = Vault(root: makeTempDir("kept-hash"))
        try vault.ensureLayout()
        let png = pngData(100, 100, seed: 4)
        let staged = try FileCatch.stage([.attached(mediaType: "image/png", base64: png.base64EncodedString(), near: "")], vault: vault, project: "dawn", session: "s", offeredSoFar: 0)
        let p = try #require(staged.first)
        #expect(try ProposalStore(vault: vault).add(p))
        try ProposalStore(vault: vault).keep(p.id)
        #expect(FileCatch.keptHashes(vault).contains(FileCatch.sha256(png)))
        #expect(try FileCatch.stage([.attached(mediaType: "image/png", base64: png.base64EncodedString(), near: "")], vault: vault, project: "dawn", session: "s", offeredSoFar: 0).isEmpty)
        #expect(try FileCatch.stage([], vault: vault, project: "dawn", session: "s", offeredSoFar: 0).isEmpty)
    }

    // 4. The card's file paths work from any folder.
    @Test func cardFilePathsAreAbsolute() throws {
        let vault = Vault(root: makeTempDir("abs-files"))
        try NoteIO.write(Note(body: "# dawn\n\n## Decisions\n- 2026-11-04 · ![[files/dawn/2026-11-04-shot.png]] Shot · kept from the chat\n"), to: vault.url("projects/dawn.md"))
        let card = CardBuilder.build(vault: vault, repo: GitRepo(root: URL(fileURLWithPath: "/x/dawn"), remote: nil))
        // The temp folder's random name can look like a token to the secret filter, so check the shape, not the root.
        let line = try #require(card.components(separatedBy: "\n").first { $0.hasSuffix("files/dawn/2026-11-04-shot.png: Shot") })
        #expect(line.hasPrefix("- /"))
        #expect(CardBuilder.fileLine("2026-11-04 · ![[files/dawn/a.png]] A · kept from the chat", vault: Vault(root: URL(fileURLWithPath: "/Users/sam/Binder")))
                == "- /Users/sam/Binder/files/dawn/a.png: A")
    }

    // 5. Topic and hosting notes are tidied, and the review sees topic lines.
    @Test func topicAndHostingNotesAreTidied() throws {
        let vault = Vault(root: makeTempDir("tidy-topics"))
        for path in ["projects/dawn/hosting.md", "hosting/cloudflare.md"] {
            try NoteIO.write(Note(body: "# x\n\n## Decisions\n- 2026-10-01 · Same line · kept from the chat\n- 2026-10-02 · Same line · kept from the chat\n"), to: vault.url(path))
        }
        let moved = Tidy.run(vault: vault, summarizer: nil)
        #expect(moved["projects/dawn/hosting.md"] == 1 && moved["hosting/cloudflare.md"] == 1)
    }

    @Test func reviewSeesTopicLinesAndCanRetireThem() throws {
        let vault = Vault(root: makeTempDir("known-topics"))
        try Memory.add("Runs on Fly.io", to: "projects/dawn/hosting.md", vault: vault)
        let known = ClaudeCLISummarizer.Known.load(vault: vault, project: "dawn")
        #expect(known.project.contains { $0.contains("Runs on Fly.io") })
        let spy = SpyRunner(reply: #"{"new":[],"outdated":["P1"]}"#)
        let review = ClaudeCLISummarizer(runner: spy, session: "s").review(entries: [.user("we left Fly")], project: "dawn", known: known)
        #expect(review.outdated.first?.target == "projects/dawn/hosting.md")
    }

    // 6. Keep never writes a second copy into a note that already has the line.
    @Test func keepSkipsTargetsThatAlreadyHaveTheLine() throws {
        let vault = Vault(root: makeTempDir("keep-dupe"))
        try Memory.add("dawn deploys to Cloudflare Pages.", to: "projects/dawn.md", vault: vault)
        let p = try #require(CommandRules.proposals(for: "wrangler pages deploy", project: "dawn", session: "s").first)
        let store = ProposalStore(vault: vault)
        #expect(try store.add(p))
        try store.keep(p.id)
        #expect(NoteIO.read(vault.url("projects/dawn.md"))?.decisions.count == 1)
        #expect(NoteIO.read(vault.url("hosting/cloudflare.md"))?.decisions.count == 1)
    }

    // 7. Identical lines are separate lines.
    @Test func restoreAndDeleteTouchOneCopyOnly() {
        let twice = Note(body: "# x\n\n## Superseded\n- A · superseded 2026-11-04\n  - first\n- A · superseded 2026-11-04\n  - second\n")
        let restored = twice.restoring("A · superseded 2026-11-04")
        #expect(restored.decisions == ["A"] && restored.superseded == ["A · superseded 2026-11-04"])
        #expect(restored.body.contains("second"))
        let two = Note(body: "# x\n\n## Decisions\n- A\n- A\n")
        #expect(two.removingDecision("A").decisions == ["A"])
    }
}

/// Answers like `SpyRunner`, after doing something to the vault, as a Keep would while the model is busy.
final class MidSplitRunner: CommandRunner, @unchecked Sendable {
    let reply: String
    let meanwhile: () -> Void
    init(reply: String, meanwhile: @escaping () -> Void) { self.reply = reply; self.meanwhile = meanwhile }
    func run(_ tool: String, _ args: [String], input: String?, timeout: TimeInterval) -> CommandResult? {
        meanwhile()
        return ok(reply)
    }
}

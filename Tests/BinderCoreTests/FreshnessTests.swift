import Foundation
import Testing
@testable import BinderCore

/// Records what the summarizer asked `claude` and answers with a canned reply.
final class SpyRunner: CommandRunner, @unchecked Sendable {
    var reply: String
    var calls: [(args: [String], input: String)] = []

    init(reply: String) { self.reply = reply }

    func run(_ tool: String, _ args: [String], input: String?, timeout: TimeInterval) -> CommandResult? {
        calls.append((args, input ?? ""))
        return tool == "claude" ? ok(reply) : nil
    }
}

@Suite struct ReviewTests {
    let known = ClaudeCLISummarizer.Known(
        project: ["2026-10-28 · Releases are signed with Ed25519 · kept from the chat",
                  "2026-10-27 · The updater was proposed and not yet approved · kept from the chat"],
        me: ["2026-10-26 · When redesigning Nashi screens, keep Orkun's copy · kept from the chat",
             "2026-10-26 · Short answers · written by you"])

    @Test func runsABareClaudeWithItsOwnShortSystemPrompt() {
        let spy = SpyRunner(reply: #"{"new":[],"outdated":[]}"#)
        _ = ClaudeCLISummarizer(runner: spy, session: "s").review(entries: [.user("ship it")], project: "binder", known: known)
        let args = spy.calls.first?.args ?? []
        #expect(args.starts(with: ["-p", "--model", "haiku"]))
        for flag in ["--system-prompt", "--strict-mcp-config", "--disable-slash-commands", "--no-session-persistence"] {
            #expect(args.contains(flag), "\(flag)")
        }
        #expect(args.firstIndex(of: "--tools").map { args[$0 + 1] } == "")
        #expect(args.firstIndex(of: "--setting-sources").map { args[$0 + 1] } == "")
        #expect(args.firstIndex(of: "--system-prompt").map { args[$0 + 1] } == ClaudeCLISummarizer.reviewSystem)
    }

    @Test func showsTheModelWhatTheBinderAlreadySays() {
        let spy = SpyRunner(reply: #"{"new":[],"outdated":[]}"#)
        _ = ClaudeCLISummarizer(runner: spy, session: "s").review(entries: [.user("ship it"), .assistant("Shipped 1.0.4.")], project: "binder", known: known)
        let input = spy.calls.first?.input ?? ""
        #expect(input.contains("PROJECT: binder"))
        #expect(input.contains("P1. Releases are signed with Ed25519"))
        #expect(input.contains("P2. The updater was proposed and not yet approved"))
        #expect(input.contains("M2. Short answers"))
        #expect(input.contains("USER: ship it\nASSISTANT: Shipped 1.0.4."))
        #expect(!input.contains("kept from the chat"))
    }

    @Test func filesEachCatchWhereItBelongsAndNamesWhatItReplaces() {
        let reply = """
        Here you go: {"new":[{"kind":"preference","scope":"project","text":"Keep the approved copy when redesigning screens.","confidence":"high"},
        {"kind":"preference","scope":"me","text":"Wants short answers in Turkish.","confidence":"high"},
        {"kind":"decision","scope":"project","text":"The in-app updater shipped in 1.0.4.","confidence":"high"}],
        "outdated":["P2","M1","P9","banana"]}
        """
        let review = ClaudeCLISummarizer(runner: SpyRunner(reply: reply), session: "s").review(entries: [.user("x")], project: "binder", known: known)
        #expect(review.proposals.map(\.target) == ["projects/binder.md", "me.md", "projects/binder.md"])
        #expect(review.proposals.map(\.text).last == "The in-app updater shipped in 1.0.4.")
        #expect(review.outdated == [ClaudeCLISummarizer.Outdated(target: "projects/binder.md", line: known.project[1]),
                                    ClaudeCLISummarizer.Outdated(target: "me.md", line: known.me[0])])
    }

    @Test func withoutAProjectOnlyThingsAboutYouAreKept() {
        let reply = #"{"new":[{"kind":"fact","scope":"project","text":"Deploys with wrangler.","confidence":"high"},{"kind":"preference","scope":"me","text":"Likes VT323.","confidence":"medium"}],"outdated":[]}"#
        let review = ClaudeCLISummarizer(runner: SpyRunner(reply: reply), session: "s").review(entries: [.user("x")], project: nil, known: known)
        #expect(review.proposals.map(\.target) == ["me.md"])
    }

    @Test func aBrokenReplyChangesNothing() {
        let review = ClaudeCLISummarizer(runner: SpyRunner(reply: "sorry, no"), session: "s").review(entries: [.user("x")], project: "binder", known: known)
        #expect(review.proposals.isEmpty && review.outdated.isEmpty)
    }

    @Test func longSessionsGoInWindowsKeepingTheLatest() {
        let entries: [TranscriptEntry] = (0..<40).map { .user(String(repeating: "\($0 % 10)", count: 1_000)) }
        let windows = ClaudeCLISummarizer.windows(entries)
        #expect(windows.count == ClaudeCLISummarizer.maxWindows)
        #expect(windows.allSatisfy { w in w.map { ClaudeCLISummarizer.said($0)?.count ?? 0 }.reduce(0, +) <= ClaudeCLISummarizer.limit })
        #expect(windows.last?.last == entries.last)
        #expect(ClaudeCLISummarizer.windows([.command("ls")]).isEmpty)
    }

    @Test func tidyFlagsLinesNewerOnesMakeUntrue() {
        let reply = #"{"outdated":["L2","L7"]}"#
        let lines = known.project
        let spy = SpyRunner(reply: reply)
        let flagged = ClaudeCLISummarizer(runner: spy, session: nil).outdated(in: lines, aboutPerson: false)
        #expect(flagged == [lines[1]])
        #expect(spy.calls.first?.input.contains("L1. Releases are signed with Ed25519") == true)
    }
}

@Suite struct SupersededTests {
    let note = Note(body: """
    # binder

    Notes the user wrote.

    ## Decisions
    - 2026-10-26 · Old plan · kept from the chat
    - 2026-10-28 · New plan · kept from the chat
    """)

    @Test func supersededLinesLeaveDecisionsButStayInTheNote() {
        let moved = note.superseding(["2026-10-26 · Old plan · kept from the chat"], on: "2026-10-29")
        #expect(moved.decisions == ["2026-10-28 · New plan · kept from the chat"])
        #expect(moved.superseded == ["2026-10-26 · Old plan · kept from the chat · superseded 2026-10-29"])
        #expect(moved.render().contains("## Superseded"))
        #expect(CardBuilder.freeLines(moved) == ["Notes the user wrote."])
    }

    @Test func aSupersededLineCanComeBack() {
        let moved = note.superseding(["2026-10-26 · Old plan · kept from the chat"], on: "2026-10-29")
        let back = moved.restoring("2026-10-26 · Old plan · kept from the chat · superseded 2026-10-29")
        #expect(back.decisions.contains("2026-10-26 · Old plan · kept from the chat"))
        #expect(back.superseded.isEmpty)
    }

    @Test func linesThatArentThereAreIgnored() {
        #expect(note.superseding(["2026-01-01 · Never written · x"], on: "2026-10-29").render() == note.render())
    }

    @Test func memorySupersedesInTheVault() throws {
        let vault = Vault(root: makeTempDir())
        try NoteIO.write(note, to: vault.url("projects/binder.md"))
        try Memory.supersede(["2026-10-26 · Old plan · kept from the chat"], in: "projects/binder.md", vault: vault,
                             now: ISO8601DateFormatter().date(from: "2026-10-29T10:00:00Z")!)
        #expect(NoteIO.read(vault.url("projects/binder.md"))?.decisions == ["2026-10-28 · New plan · kept from the chat"])
    }

    @Test func olderDuplicatesAreSpottedWithoutAModel() {
        let newestFirst = ["2026-10-28 · Ships as a free app. · kept from the chat",
                           "2026-10-27 · Licence is AGPL · written by you",
                           "2026-10-26 · Ships as a free app · kept from a command"]
        #expect(Tidy.duplicates(newestFirst) == ["2026-10-26 · Ships as a free app · kept from a command"])
    }
}

@Suite struct SummarizeQueueTests {
    let vault = Vault(root: makeTempDir("queue"))

    func transcript(_ texts: [String]) -> URL {
        let url = makeTempDir("t").appendingPathComponent("s.jsonl")
        let lines = texts.map { text in
            let obj: [String: Any] = ["type": "user", "message": ["role": "user", "content": text]]
            return String(decoding: try! JSONSerialization.data(withJSONObject: obj), as: UTF8.self)
        }
        write(lines.joined(separator: "\n") + "\n", to: url)
        return url
    }

    func append(_ text: String, to url: URL) throws {
        let obj: [String: Any] = ["type": "user", "message": ["role": "user", "content": text]]
        let handle = try FileHandle(forWritingTo: url)
        try handle.seekToEnd()
        try handle.write(contentsOf: JSONSerialization.data(withJSONObject: obj) + Data("\n".utf8))
        try handle.close()
    }

    @Test func waitsForEnoughNewTalk() throws {
        let url = transcript(["short"])
        let queue = SummarizeQueue(vault: vault)
        let now = Date()
        #expect(queue.claim(session: "s", transcript: url, startingAt: 0, now: now) == nil)
        try append(String(repeating: "a", count: SummarizeQueue.minChars), to: url)
        let range = try #require(queue.claim(session: "s", transcript: url, startingAt: 0, now: now))
        #expect(range.lowerBound == 0)
        #expect(range.upperBound == (try FileManager.default.attributesOfItem(atPath: url.path)[.size] as? UInt64))
        #expect(queue.claim(session: "s", transcript: url, startingAt: 0, now: now) == nil)
    }

    @Test func aLittleTalkGoesAfterAWhileOrWhenTheSessionRests() throws {
        let url = transcript(["first"])
        let queue = SummarizeQueue(vault: vault)
        let now = Date()
        try append(String(repeating: "b", count: SummarizeQueue.minChars), to: url)
        #expect(queue.claim(session: "s", transcript: url, startingAt: 0, now: now) != nil)
        try append("one more thing", to: url)
        #expect(queue.claim(session: "s", transcript: url, startingAt: 0, now: now.addingTimeInterval(60)) == nil)
        #expect(queue.claim(session: "s", transcript: url, startingAt: 0, now: now.addingTimeInterval(SummarizeQueue.maxWait + 1)) != nil)
        try append("and another", to: url)
        #expect(queue.claim(session: "s", transcript: url, startingAt: 0, now: now.addingTimeInterval(SummarizeQueue.maxWait + 60), force: true) != nil)
    }

    @Test func aNewSessionStartsWhereCaptureFirstSawIt() throws {
        let url = transcript([String(repeating: "x", count: 20_000)])
        let size = try FileManager.default.attributesOfItem(atPath: url.path)[.size] as! UInt64
        let queue = SummarizeQueue(vault: vault)
        #expect(queue.claim(session: "fresh", transcript: url, startingAt: size, now: Date(), force: true) == nil)
    }

    @Test func readsOnlyTheClaimedBytes() throws {
        let url = transcript(["one", "two"])
        let middle = try FileManager.default.attributesOfItem(atPath: url.path)[.size] as! UInt64
        try append("three", to: url)
        let chunk = try Transcript.read(url, from: 0, upTo: middle)
        #expect(chunk.entries == [.user("one"), .user("two")])
        #expect(chunk.endOffset == middle)
    }
}

@Suite struct CardBudgetTests {
    @Test func theCardHoldsAboutAThousandTokens() {
        #expect(CardBuilder.budget == 3_600)
        let vault = Vault(root: makeTempDir())
        var note = Note(body: "# binder\n")
        for i in 0..<200 { note = note.appendingDecision("2026-10-\(10 + i % 18) · Decision number \(i) about the release flow · kept from the chat") }
        try! NoteIO.write(note, to: vault.url("projects/binder.md"))
        let card = CardBuilder.build(vault: vault, repo: GitRepo(root: URL(fileURLWithPath: "/x/binder"), remote: nil))
        #expect(card.count <= 3_600)
        #expect(card.count > 3_000)
        #expect(card.contains("Decision number 199"))
        #expect(!card.contains("Decision number 0 "))
    }

    @Test func aFolderWithItsOwnNoteGetsItsProjectEvenWithoutGit() {
        let vault = Vault(root: makeTempDir())
        try! NoteIO.write(Note(body: "# Vektor Labs\n\n## Decisions\n- 2026-10-28 · The site isn't a git repo · kept from the chat\n"), to: vault.url("projects/Vektor Labs.md"))
        let folder = makeTempDir().appendingPathComponent("Vektor Labs")
        try! FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        let repo = CardBuilder.project(for: folder, vault: vault)
        #expect(repo?.name == "Vektor Labs")
        #expect(CardBuilder.project(for: makeTempDir().appendingPathComponent("nothing"), vault: vault) == nil)
    }
}

@Suite struct TidyRunTests {
    func vault() throws -> Vault {
        let vault = Vault(root: makeTempDir("tidy"))
        try NoteIO.write(Note(body: """
        # binder

        ## Decisions
        - 2026-10-26 · Ships as a free app · kept from a command
        - 2026-10-27 · The updater was proposed and not yet approved · kept from the chat
        - 2026-10-28 · Ships as a free app. · kept from the chat
        - 2026-10-28 · The in-app updater shipped in 1.0.4 · kept from the chat
        """), to: vault.url("projects/binder.md"))
        return vault
    }

    @Test func movesRepeatsAndWhatTheModelFlags() throws {
        let vault = try vault()
        // Newest first, after the repeat is gone: L1 updater shipped, L2 free app, L3 updater proposed.
        let spy = SpyRunner(reply: #"{"outdated":["L3"]}"#)
        let moved = Tidy.run(vault: vault, summarizer: ClaudeCLISummarizer(runner: spy, session: nil))
        #expect(moved == ["projects/binder.md": 2])
        let note = try #require(NoteIO.read(vault.url("projects/binder.md")))
        #expect(note.decisions.map(CardBuilder.decisionText) == ["Ships as a free app.", "The in-app updater shipped in 1.0.4"])
        #expect(note.superseded.count == 2)
    }

    @Test func leavesUnchangedNotesAlone() throws {
        let vault = try vault()
        let spy = SpyRunner(reply: #"{"outdated":[]}"#)
        Tidy.run(vault: vault, summarizer: ClaudeCLISummarizer(runner: spy, session: nil))
        let calls = spy.calls.count
        Tidy.run(vault: vault, summarizer: ClaudeCLISummarizer(runner: spy, session: nil))
        #expect(spy.calls.count == calls)
        Tidy.run(vault: vault, summarizer: ClaudeCLISummarizer(runner: spy, session: nil), everything: true)
        #expect(spy.calls.count == calls + 1)
    }

    @Test func withoutTheModelOnlyRepeatsGo() throws {
        let vault = try vault()
        #expect(Tidy.run(vault: vault, summarizer: nil) == ["projects/binder.md": 1])
    }
}

@Suite struct PersonFilterTests {
    @Test func asksASingleQuestionAboutWhatIsntAboutThePerson() {
        let lines = ["2026-10-28 · Writes to the assistant in Turkish · kept from the chat",
                     "2026-10-27 · The site uses a night-blue background (#0b0d13) · kept from the chat",
                     "2026-10-26 · Commits and PRs must not include co-author lines · written by you"]
        let spy = SpyRunner(reply: #"{"project":["L2"]}"#)
        let flagged = ClaudeCLISummarizer(runner: spy, session: nil).notAboutPerson(in: lines)
        #expect(flagged == [lines[1]])
        #expect(spy.calls.first?.args.firstIndex(of: "--system-prompt").map { spy.calls[0].args[$0 + 1] } == ClaudeCLISummarizer.personSystem)
    }

    @Test func tidyingMeAlsoClearsOutProjectLines() throws {
        let vault = Vault(root: makeTempDir("person"))
        try NoteIO.write(Note(body: """
        # Me

        ## Decisions
        - 2026-10-26 · The site uses a night-blue background · kept from the chat
        - 2026-10-27 · Writes in Turkish · kept from the chat
        """), to: vault.url("me.md"))
        // First call: outdated (none). Second: not about the person.
        final class TwoReplies: CommandRunner, @unchecked Sendable {
            var replies = [#"{"outdated":[]}"#, #"{"project":["L2"]}"#]
            func run(_ tool: String, _ args: [String], input: String?, timeout: TimeInterval) -> CommandResult? { ok(replies.removeFirst()) }
        }
        let moved = Tidy.run(vault: vault, summarizer: ClaudeCLISummarizer(runner: TwoReplies(), session: nil))
        #expect(moved == ["me.md": 1])
        #expect(NoteIO.read(vault.url("me.md"))?.decisions.map(CardBuilder.decisionText) == ["Writes in Turkish"])
    }
}

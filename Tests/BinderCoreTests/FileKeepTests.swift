import Foundation
import Testing
@testable import BinderCore

@Suite struct FileKeepTests {
    let vault: Vault = {
        let v = Vault(root: makeTempDir("filekeep"))
        try? v.ensureLayout()
        return v
    }()
    var store: ProposalStore { ProposalStore(vault: vault) }
    let day = ISO8601DateFormatter().date(from: "2026-11-04T10:00:00Z")!

    /// One attached image, staged and in the inbox.
    func staged(project: String? = "dawn", seed: Int = 0, near: String = "") throws -> Proposal {
        let entry = TranscriptEntry.attachment(mediaType: "image/png", base64: pngData(100, 100, seed: seed).base64EncodedString(), near: near)
        let p = try #require(try FileCatch.stage(FileCatch.candidates(entries: [entry], cwd: "/x", since: nil), vault: vault,
                                                 project: project, session: "s", offeredSoFar: 0).first)
        #expect(try store.add(p))
        return p
    }

    @Test func keepMovesFileAndWritesLine() throws {
        let p = try staged()
        try store.keep(p.id, editedText: "Architecture", now: day)
        let kept = vault.url("files/dawn/2026-11-04-architecture.png")
        #expect(FileManager.default.fileExists(atPath: kept.path))
        #expect(!FileManager.default.fileExists(atPath: vault.inboxFiles.appendingPathComponent(p.file!).path))
        #expect(NoteIO.read(vault.url("projects/dawn.md"))?.decisions == ["2026-11-04 · ![[files/dawn/2026-11-04-architecture.png]] Architecture · kept from the chat"])
    }

    @Test func keepFileIntoOddProject() throws {
        let p = try staged(project: "Bib.")
        try store.keep(p.id, now: day)
        let line = try #require(NoteIO.read(vault.url("projects/Bib..md"))?.decisions.first)
        let link = try #require(line.firstMatch(of: #/!\[\[([^\]]+)\]\]/#)).1
        #expect(link.hasPrefix("files/Bib./"))
        #expect(FileManager.default.fileExists(atPath: vault.url(String(link)).path))
    }

    @Test func nameClashGetsSuffix() throws {
        let a = try staged(seed: 1), b = try staged(seed: 2)
        try store.keep(a.id, editedText: "Chart", now: day)
        try store.keep(b.id, editedText: "Chart", now: day)
        #expect(FileManager.default.fileExists(atPath: vault.url("files/dawn/2026-11-04-chart.png").path))
        #expect(FileManager.default.fileExists(atPath: vault.url("files/dawn/2026-11-04-chart-2.png").path))
    }

    @Test func dropDeletesWaitingFileAndRemembersHash() throws {
        let p = try staged(seed: 3)
        try store.drop(p.id)
        #expect(!FileManager.default.fileExists(atPath: vault.inboxFiles.appendingPathComponent(p.file!).path))
        let again = try FileCatch.stage(FileCatch.candidates(entries: [.attachment(mediaType: "image/png", base64: pngData(100, 100, seed: 3).base64EncodedString(), near: "")],
                                                             cwd: "/x", since: nil), vault: vault, project: "dawn", session: "s", offeredSoFar: 0)
        #expect(again.isEmpty)
    }

    @Test func cardListsNewestFiveFiles() throws {
        for i in 0..<7 {
            let p = try staged(seed: 10 + i)
            try store.keep(p.id, editedText: "Shot \(i)", now: day.addingTimeInterval(Double(i) * 86_400))
        }
        let card = CardBuilder.build(vault: vault, repo: GitRepo(root: URL(fileURLWithPath: "/x/dawn"), remote: nil))
        let block = try #require(card.components(separatedBy: "FILES:\n").last)
        let lines = block.components(separatedBy: "\n").filter { $0.hasPrefix("- files/") }
        #expect(lines.count == 5)
        #expect(lines.first == "- files/dawn/2026-11-10-shot-6.png: Shot 6")
    }

    @Test func captionsReplaceFileNameWhenModelAnswers() throws {
        let p = try staged(seed: 20, near: "bu mimari diyagram")
        let spy = SpyRunner(reply: #"{"captions":{"\#(p.id)":"Architecture diagram of the API"}}"#)
        #expect(FileCatch.captionPending(vault: vault, summarizer: ClaudeCLISummarizer(runner: spy, session: nil)) == 1)
        let now = try #require(store.pending().first { $0.id == p.id })
        #expect(now.text == "Architecture diagram of the API" && now.context == nil)
        #expect(spy.calls.first?.input.contains("bu mimari diyagram") == true)
        #expect(ClaudeCLISummarizer.captionSystem.contains("English"))
    }

    @Test func captionsLeaveTextWithBadReply() throws {
        let p = try staged(seed: 21, near: "something")
        #expect(FileCatch.captionPending(vault: vault, summarizer: ClaudeCLISummarizer(runner: SpyRunner(reply: "no"), session: nil)) == 0)
        #expect(store.pending().first { $0.id == p.id }?.text == "Image from the session")
    }
}

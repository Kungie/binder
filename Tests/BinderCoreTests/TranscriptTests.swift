import Foundation
import Testing
@testable import BinderCore

private func copyOfFixture() throws -> URL {
    let url = makeTempDir().appendingPathComponent("session.jsonl")
    try FileManager.default.copyItem(at: fixture("transcript.jsonl"), to: url)
    return url
}

private func append(_ text: String, to url: URL) throws {
    let handle = try FileHandle(forWritingTo: url)
    defer { try? handle.close() }
    _ = try handle.seekToEnd()
    try handle.write(contentsOf: Data(text.utf8))
}

@Suite struct TranscriptTests {
    @Test func parsesAllEntryKinds() throws {
        let chunk = try Transcript.read(fixture("transcript.jsonl"), from: 0)
        #expect(chunk.sessionID == "sess-1")
        #expect(chunk.cwd == "/Users/x/Desktop/dawn")
        #expect(chunk.entries == [
            .user("Deploy the site to Cloudflare Pages please"),
            .assistant("Deploying now."),
            .command("wrangler pages deploy ./site --project-name dawn"),
            .usage(Usage(input: 2, cacheRead: 90000, cacheCreation: 2000), model: "claude-opus-5-5"),
            .fileEdit("/Users/x/Desktop/dawn/README.md"),
            .usage(Usage(input: 2, cacheRead: 100000, cacheCreation: 500), model: "claude-opus-5-5"),
            .todos(["Write release notes"]),
            .usage(Usage(input: 2, cacheRead: 119000, cacheCreation: 998), model: "claude-opus-5-5"),
            .user("Next: write the release notes for 0.3.")
        ])
    }

    @Test func readsFromOffsetOnly() throws {
        let url = try copyOfFixture()
        let first = try Transcript.read(url, from: 0)
        try append(#"{"type":"user","message":{"role":"user","content":"one more thing"}}"# + "\n", to: url)
        let second = try Transcript.read(url, from: first.endOffset)
        #expect(second.entries == [.user("one more thing")])
        #expect(second.endOffset > first.endOffset)
    }

    @Test func ignoresPartialLastLine() throws {
        let url = try copyOfFixture()
        let size = try #require(try FileManager.default.attributesOfItem(atPath: url.path)[.size] as? UInt64)
        try append(#"{"type":"user","message":{"role":"us"#, to: url)
        let partial = try Transcript.read(url, from: 0)
        #expect(partial.endOffset == size)
        #expect(partial.entries.last == .user("Next: write the release notes for 0.3."))
        try append(#"er","content":"finished line"}}"# + "\n", to: url)
        #expect(try Transcript.read(url, from: partial.endOffset).entries == [.user("finished line")])
    }

    @Test func skipsGarbageLines() throws {
        let url = makeTempDir().appendingPathComponent("garbage.jsonl")
        write("{broken\n[1,2,3]\n\"just a string\"\n\n", to: url)
        let chunk = try Transcript.read(url, from: 0)
        #expect(chunk.entries.isEmpty)
        #expect(chunk.endOffset == 33)
    }

    @Test func meterUses200kWindowByDefault() throws {
        let reading = try #require(ContextMeter.reading(from: try Transcript.read(fixture("transcript.jsonl"), from: 0).entries))
        #expect(reading.used == 120_000 && reading.window == 200_000)
        #expect(abs(reading.fraction - 0.60) < 0.0001)
        let big = try #require(ContextMeter.reading(from: [.usage(Usage(input: 2, cacheRead: 340_000, cacheCreation: 1_468), model: "claude-opus-5-5")]))
        #expect(big.used == 341_470 && big.window == 1_000_000)
        #expect(ContextMeter.reading(from: [.user("hi")]) == nil)
    }

    @Test func meterUses1MForBracketModel() throws {
        let reading = try #require(ContextMeter.reading(from: [.usage(Usage(input: 0, cacheRead: 120_000, cacheCreation: 0), model: "claude-opus-5-5[1m]")]))
        #expect(reading.window == 1_000_000)
        #expect(abs(reading.fraction - 0.12) < 0.0001)
    }

    @Test func cursorsPersist() throws {
        let vault = Vault(root: makeTempDir())
        #expect(Cursors(vault: vault).offset(for: "s1") == 0)
        try Cursors(vault: vault).set(123, for: "s1")
        try Cursors(vault: vault).set(7, for: "s2")
        #expect(Cursors(vault: vault).offset(for: "s1") == 123)
        #expect(Cursors(vault: vault).offset(for: "s2") == 7)
    }
}

import Foundation
import Testing
@testable import BinderCore

@Suite struct UsageStoreTests {
    let line = "2026-10-26 · The API runs in Docker on hetzner-1 · kept from the chat"

    @Test func cardCountsOncePerSession() {
        let store = UsageStore(vault: Vault(root: makeTempDir("usage")))
        for session in ["s1", "s1", "s2"] {
            store.recordCard(session: session, notes: ["projects/dawn.md"], lines: ["projects/dawn.md": [line]], now: Date())
        }
        #expect(store.counts(note: "projects/dawn.md").cards == 2)
        #expect(store.counts(line: line, in: "projects/dawn.md").cards == 2)
        #expect(store.counts(note: "me.md") == UseCounts())
    }

    @Test func cachedCardStillRecordsOnce() throws {
        let vault = try fixtureVault()
        let folder = makeTempDir("work").appendingPathComponent("dawn")
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        let first = SessionCard.make(vault: vault, cwd: folder.path, session: "s")
        let second = SessionCard.make(vault: vault, cwd: folder.path, session: "s")
        #expect(first == second && first.contains("PROJECT dawn:"))
        let store = UsageStore(vault: vault)
        #expect(store.counts(note: "projects/dawn.md").cards == 1)
        _ = SessionCard.make(vault: vault, cwd: folder.path, session: "t")
        #expect(store.counts(note: "projects/dawn.md").cards == 2)
        #expect(store.counts(note: "me.md").cards == 2)
    }

    func handlesVault() throws -> Vault {
        let vault = Vault(root: makeTempDir("handles"))
        try NoteIO.write(Note.new(title: "hetzner-1").replacingObserved(["- os: Ubuntu 24.04"]), to: vault.url("servers/hetzner-1.md"))
        try Memory.add("The home server, a Raspberry Pi at 10.0.7.20.", to: "machines/raspberry-pi.md", vault: vault)
        try Memory.add("Pages project dawn-site.", to: "hosting/cloudflare.md", vault: vault)
        try NoteIO.write(Note.new(title: "dawn").replacingObserved(["- repo: github.com/kungie/dawn · main"]), to: vault.url("projects/dawn.md"))
        try NoteIO.update(vault.url("projects/dawn.md"), title: "dawn") { $0.appendingDecision(line) }
        return vault
    }

    @Test func handlesMatchRealCommands() throws {
        let vault = try handlesVault()
        let store = UsageStore(vault: vault)
        store.recordUse(commands: ["ssh hetzner-1", "ssh root@10.0.7.20 uptime", "npx wrangler pages deploy dist", "ssh hetzner-10 ls"], replies: [], now: Date())
        #expect(store.counts(note: "servers/hetzner-1.md").used == 1)
        #expect(store.counts(note: "machines/raspberry-pi.md").used == 1)
        #expect(store.counts(note: "hosting/cloudflare.md").used == 1)
        #expect(store.counts(note: "projects/dawn.md").used == 0)
    }

    @Test func replyRepeatingALineCountsForIt() throws {
        let vault = try handlesVault()
        let store = UsageStore(vault: vault)
        store.recordUse(commands: [], replies: ["Since the API runs in Docker on hetzner-1, I restarted the container."], now: Date())
        #expect(store.counts(line: line, in: "projects/dawn.md").used == 1)
        #expect(store.counts(note: "projects/dawn.md").used == 1)
    }

    @Test func missingOrCorruptUsageLoadsEmpty() throws {
        let vault = Vault(root: makeTempDir("usage-bad"))
        #expect(UsageStore(vault: vault).totals() == UseCounts())
        try FileManager.default.createDirectory(at: vault.internalDir, withIntermediateDirectories: true)
        try Data("not json".utf8).write(to: vault.url(".binder/usage.json"))
        #expect(UsageStore(vault: vault).totals() == UseCounts())
    }

    @Test func usageStoresNoText() throws {
        let vault = try handlesVault()
        let store = UsageStore(vault: vault)
        store.recordUse(commands: ["ssh hetzner-1 banana"], replies: ["Since the API runs in Docker on hetzner-1, banana."], now: Date())
        store.recordCard(session: "s", notes: ["projects/dawn.md"], lines: ["projects/dawn.md": [line]], now: Date())
        let json = try String(contentsOf: vault.url(".binder/usage.json"), encoding: .utf8)
        #expect(!json.contains("banana") && !json.contains("Docker"))
    }
}

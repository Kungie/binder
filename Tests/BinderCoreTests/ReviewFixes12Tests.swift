import Foundation
import Testing
@testable import BinderCore

/// Findings from the 1.2 branch review, each pinned by a test.
@Suite struct ReviewFixes12Tests {
    func dawnFolder() throws -> String {
        let folder = makeTempDir("work").appendingPathComponent("dawn")
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        return folder.path
    }

    // 1. A pin or an end date reaches the next card even while the card is cached.
    @Test func pinReachesACachedCard() throws {
        let vault = Vault(root: makeTempDir("cache-pin"))
        try NoteIO.write(Note(body: "# dawn\n\n## Decisions\n- 2025-01-01 · Uses Netlify for previews · kept from the chat\n- 2099-01-01 · Ships free · written by you\n"), to: vault.url("projects/dawn.md"))
        let cwd = try dawnFolder()
        #expect(!SessionCard.make(vault: vault, cwd: cwd, session: "a").contains("Netlify"))
        UsageStore(vault: vault).pin(line: "2025-01-01 · Uses Netlify for previews · kept from the chat", in: "projects/dawn.md")
        #expect(SessionCard.make(vault: vault, cwd: cwd, session: "b").contains("Netlify"))
    }

    @Test func expiryReachesACachedCard() throws {
        let vault = Vault(root: makeTempDir("cache-expiry"))
        try Memory.add("The API is down", to: "projects/dawn.md", vault: vault)
        let cwd = try dawnFolder()
        #expect(SessionCard.make(vault: vault, cwd: cwd, session: "a").contains("The API is down"))
        let line = try #require(NoteIO.read(vault.url("projects/dawn.md"))?.decisions.first)
        ExpiryStore(vault: vault).set(Date().addingTimeInterval(-3 * 86_400), line: line, in: "projects/dawn.md")
        #expect(!SessionCard.make(vault: vault, cwd: cwd, session: "b").contains("The API is down"))
    }

    @Test func aCardFromAnEarlierDayIsRebuilt() throws {
        let vault = Vault(root: makeTempDir("cache-day"))
        try Memory.add("Ships free", to: "projects/dawn.md", vault: vault)
        let repo = GitRepo(root: URL(fileURLWithPath: "/x/dawn"), remote: nil)
        let cache = CardCache(vault: vault)
        try cache.store("OLD", for: repo)
        let yesterday = Date().addingTimeInterval(-86_400 * 1.5)
        try FileManager.default.setAttributes([.modificationDate: yesterday], ofItemAtPath: vault.url("projects/dawn.md").path)
        try FileManager.default.setAttributes([.modificationDate: yesterday.addingTimeInterval(60)], ofItemAtPath: cache.file(repo).path)
        #expect(cache.cached(for: repo) == nil)
    }

    // 2. Kept files never fade.
    @Test func fileLinesNeverFade() throws {
        let usage = UsageStore(vault: Vault(root: makeTempDir("fade-file")))
        #expect(!Forgetting.fades("2025-01-01 · ![[files/dawn/2025-01-01-shot.png]] Shot · kept from the chat", in: "projects/dawn.md", usage: usage))
    }

    // 3. The model knows what day it is.
    @Test func reviewInputNamesToday() {
        let spy = SpyRunner(reply: #"{"new":[],"outdated":[]}"#)
        _ = ClaudeCLISummarizer(runner: spy, session: "s").review(entries: [.user("x")], project: "dawn", known: .init(project: [], me: []))
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.dateFormat = "yyyy-MM-dd (EEEE)"
        #expect(spy.calls.first?.input.contains("TODAY: \(f.string(from: Date()))") == true)
    }

    // 6. A negative limit is no limit, not a crash.
    @Test func negativeLimitIsSafe() throws {
        let vault = Vault(root: makeTempDir("limit"))
        try Memory.add("Deploys with wrangler", to: "projects/dawn.md", vault: vault)
        #expect(Search.run("wrangler", vault: vault, limit: -1).isEmpty)
    }

    // 8. "Down until today" lasts through today, then goes.
    @Test func untilTodayLastsTheDay() throws {
        let today = ClaudeCLISummarizer.until(ExpiryStore.format.string(from: Date()))
        #expect(today != nil)
        let vault = Vault(root: makeTempDir("until-today"))
        try Memory.add("The API is down", to: "projects/dawn.md", vault: vault)
        let line = try #require(NoteIO.read(vault.url("projects/dawn.md"))?.decisions.first)
        ExpiryStore(vault: vault).set(today!, line: line, in: "projects/dawn.md")
        #expect(ExpiryStore.Checker(store: ExpiryStore(vault: vault), now: Date()).expired(line, in: "projects/dawn.md") == false)
        #expect(ExpiryStore.Checker(store: ExpiryStore(vault: vault), now: Date().addingTimeInterval(2 * 86_400)).expired(line, in: "projects/dawn.md"))
    }

    // 10. A huge error is cut before it's scanned.
    @Test func hugeErrorIsCutFirst() throws {
        let url = makeTempDir("err").appendingPathComponent("s.jsonl")
        let big = String(repeating: "x", count: 200_000)
        let lines = [
            #"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"make"}}]}}"#,
            #"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","is_error":true,"content":"\#(big)"}]}}"#
        ]
        try Data((lines.joined(separator: "\n") + "\n").utf8).write(to: url)
        let start = Date()
        let entries = try Transcript.read(url, from: 0).entries
        #expect(Date().timeIntervalSince(start) < 1)
        #expect(entries.contains { if case let .failed(_, error) = $0 { error.count == 300 } else { false } })
    }

    // 12. A 1.1 install (card and capture, no PreCompact) is seen as needing an upgrade, not as missing.
    @Test func oldInstallNeedsUpgrade() throws {
        let url = makeTempDir("hooks").appendingPathComponent("settings.json")
        let path = "/Applications/Binder.app/Contents/Helpers/binder"
        let old = ["hooks": ["SessionStart": [["matcher": "", "hooks": [["type": "command", "command": "\"\(path)\" card"]]]],
                             "Stop": [["matcher": "", "hooks": [["type": "command", "command": "\"\(path)\" capture"]]]]]]
        try JSONSerialization.data(withJSONObject: old).write(to: url)
        let installer = HookInstaller(settingsURL: url, binderPath: path)
        #expect(installer.needsUpgrade)
        try installer.upgradeIfNeeded()
        #expect(installer.isInstalled && !installer.needsUpgrade)
        let fresh = HookInstaller(settingsURL: makeTempDir("hooks2").appendingPathComponent("settings.json"), binderPath: path)
        #expect(!fresh.needsUpgrade)
        try fresh.upgradeIfNeeded()
        #expect(!fresh.isInstalled)
    }
}

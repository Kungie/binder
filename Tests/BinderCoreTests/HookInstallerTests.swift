import Foundation
import Testing
@testable import BinderCore

private let binderPath = "/Applications/Binder.app/Contents/MacOS/binder"

private func settings(_ url: URL) throws -> [String: Any] {
    try #require(try JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
}

private func commands(_ json: [String: Any], _ event: String) -> [String] {
    let entries = (json["hooks"] as? [String: Any])?[event] as? [[String: Any]] ?? []
    return entries.flatMap { ($0["hooks"] as? [[String: Any]] ?? []).compactMap { $0["command"] as? String } }
}

@Suite struct HookInstallerTests {
    let dir = makeTempDir("claude")
    var url: URL { dir.appendingPathComponent("settings.json") }
    var installer: HookInstaller { HookInstaller(settingsURL: url, binderPath: binderPath) }

    @Test func installAddsBothEvents() throws {
        try installer.install()
        let json = try settings(url)
        #expect(commands(json, "SessionStart") == ["\"\(binderPath)\" card"])
        #expect(commands(json, "Stop") == ["\"\(binderPath)\" capture"])
        let entry = try #require(((json["hooks"] as? [String: Any])?["Stop"] as? [[String: Any]])?.first)
        #expect(entry["matcher"] as? String == "")
        #expect(((entry["hooks"] as? [[String: Any]])?.first?["timeout"] as? Int) == 5)
        #expect(installer.isInstalled)
    }

    @Test func installIsIdempotent() throws {
        try installer.install()
        let first = try Data(contentsOf: url)
        try installer.install()
        #expect(try Data(contentsOf: url) == first)
        #expect(commands(try settings(url), "Stop").count == 1)
    }

    @Test func installKeepsForeignHooks() throws {
        try FileManager.default.copyItem(at: fixture("settings-with-hooks.json"), to: url)
        try installer.install()
        var json = try settings(url)
        #expect(json["model"] as? String == "opus")
        #expect(json["cleanupPeriodDays"] as? Int == 365)
        #expect(commands(json, "Notification") == ["\"/Applications/Dawn.app/Contents/MacOS/Dawn\" hook"])
        #expect(commands(json, "Stop") == ["\"/Applications/Dawn.app/Contents/MacOS/Dawn\" hook stop", "\"\(binderPath)\" capture"])
        try installer.uninstall()
        json = try settings(url)
        #expect(commands(json, "Stop") == ["\"/Applications/Dawn.app/Contents/MacOS/Dawn\" hook stop"])
        #expect(commands(json, "SessionStart").isEmpty)
        #expect((json["permissions"] as? [String: Any])?["allow"] as? [String] == ["Bash(git status)"])
    }

    @Test func installRefusesMalformedSettings() throws {
        write("{ \"model\": \"opus\", oops }", to: url)
        let before = try Data(contentsOf: url)
        #expect(throws: HookInstallerError.malformedSettings) { try installer.install() }
        #expect(try Data(contentsOf: url) == before)
        #expect(!installer.isInstalled)
    }

    @Test func uninstallRemovesOnlyOurs() throws {
        try installer.install()
        try installer.uninstall()
        let json = try settings(url)
        #expect(json["hooks"] == nil)
        #expect(!installer.isInstalled)
        try installer.uninstall() // twice is fine
    }

    @Test func backupWrittenOnce() throws {
        try FileManager.default.copyItem(at: fixture("settings-with-hooks.json"), to: url)
        let original = try Data(contentsOf: url)
        try installer.install()
        try installer.uninstall()
        try installer.install()
        let backup = dir.appendingPathComponent("settings.json.binder-backup")
        #expect(try Data(contentsOf: backup) == original)
    }

    @Test func doctorFlagsMissingVault() {
        let vault = Vault(root: makeTempDir().appendingPathComponent("no-binder-here"))
        let lines = Doctor.report(vault: vault, installer: installer, runner: FakeRunner())
        #expect(lines.first?.ok == false)
        #expect(lines.first?.text.contains("No binder at") == true)
        #expect(lines.contains { !$0.ok && $0.text.contains("hooks") })
        #expect(lines.contains { !$0.ok && $0.text.contains("claude") })
    }
}

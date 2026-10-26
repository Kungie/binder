import Foundation
import Testing
@testable import BinderCore

/// The built `binder`: it sits in the products folder, a few levels above the test bundle's resources.
let binderBinary: URL = {
    var dir = Bundle.module.bundleURL
    while dir.path != "/" {
        let candidate = dir.appendingPathComponent("binder")
        if FileManager.default.isExecutableFile(atPath: candidate.path) { return candidate }
        dir = dir.deletingLastPathComponent()
    }
    fatalError("binder executable not found next to the tests")
}()

/// Runs the built `binder` with a temp HOME and vault.
private func runBinder(_ args: [String], stdin: String = "", home: URL, vault: URL) throws -> (status: Int32, stdout: String) {
    let process = Process()
    process.executableURL = binderBinary
    process.arguments = args
    var env = ProcessInfo.processInfo.environment
    env["BINDER_HOME"] = home.path
    env["BINDER_VAULT"] = vault.path
    env.removeValue(forKey: "BINDER_NESTED")
    process.environment = env
    let input = Pipe(), output = Pipe()
    process.standardInput = input
    process.standardOutput = output
    process.standardError = FileHandle.nullDevice
    try process.run()
    input.fileHandleForWriting.write(Data(stdin.utf8))
    try input.fileHandleForWriting.close()
    let data = output.fileHandleForReading.readDataToEndOfFile()
    process.waitUntilExit()
    return (process.terminationStatus, String(decoding: data, as: UTF8.self))
}

private func makeDawnRepo() -> URL {
    let repo = makeTempDir("projects").appendingPathComponent("dawn")
    write("[remote \"origin\"]\n\turl = git@github.com:Kungie/dawn.git\n", to: repo.appendingPathComponent(".git/config"))
    return repo
}

@Suite(.serialized) struct CLITests {
    let home = makeTempDir("home")

    @Test func cardPrintsHookJSONForFixtureRepo() throws {
        let vault = try fixtureVault()
        let repo = makeDawnRepo()
        let stdin = #"{"session_id":"x","cwd":"\#(repo.path)","hook_event_name":"SessionStart","source":"startup"}"#
        let result = try runBinder(["card"], stdin: stdin, home: home, vault: vault.root)
        #expect(result.status == 0)
        let json = try #require(try JSONSerialization.jsonObject(with: Data(result.stdout.utf8)) as? [String: Any])
        let context = try #require((json["hookSpecificOutput"] as? [String: Any])?["additionalContext"] as? String)
        #expect(context.hasPrefix(CardBuilder.header))
        #expect(context.contains("PROJECT dawn:"))
    }

    @Test func cardWithGarbageStdinExitsCleanly() throws {
        let result = try runBinder(["card"], stdin: "garbage {", home: home, vault: try fixtureVault().root)
        #expect(result.status == 0)
        #expect(result.stdout.isEmpty)
    }

    @Test func captureWithoutVaultExitsCleanly() throws {
        let missing = makeTempDir().appendingPathComponent("no-binder")
        let stdin = #"{"session_id":"s","transcript_path":"\#(fixture("transcript.jsonl").path)","cwd":"/tmp","hook_event_name":"Stop"}"#
        let result = try runBinder(["capture"], stdin: stdin, home: home, vault: missing)
        #expect(result.status == 0)
        #expect(result.stdout.isEmpty)
        #expect(!FileManager.default.fileExists(atPath: missing.path))
    }

    @Test func observePrintsSummary() throws {
        let vault = makeTempDir().appendingPathComponent("Binder")
        let result = try runBinder(["observe"], home: home, vault: vault)
        #expect(result.status == 0)
        #expect(result.stdout.wholeMatch(of: #/Read \d+ files?\. Wrote \d+ notes?\. \d+ things? to review\.\n/#) != nil, "\(result.stdout)")
        #expect(FileManager.default.fileExists(atPath: vault.appendingPathComponent("machines/this-mac.md").path))
    }

    @Test func installThenUninstallRoundTrip() throws {
        let vault = makeTempDir().appendingPathComponent("Binder")
        let settings = home.appendingPathComponent(".claude/settings.json")
        let installed = try runBinder(["install"], home: home, vault: vault)
        #expect(installed.status == 0)
        #expect(installed.stdout == "Hooks added to \(settings.path).\n")
        let text = try #require(read(settings))
        #expect(text.contains("binder\\\" card") || text.contains("binder\" card"))
        let removed = try runBinder(["uninstall"], home: home, vault: vault)
        #expect(removed.stdout == "Hooks removed.\n")
        #expect(read(settings)?.contains("capture") == false)
    }

    @Test func nestedSessionsSkipHooks() throws {
        let vault = try fixtureVault()
        let process = Process()
        process.executableURL = binderBinary
        process.arguments = ["card"]
        process.environment = ["BINDER_NESTED": "1", "BINDER_VAULT": vault.root.path, "BINDER_HOME": home.path]
        let output = Pipe()
        process.standardOutput = output
        process.standardInput = FileHandle.nullDevice
        try process.run()
        process.waitUntilExit()
        #expect(process.terminationStatus == 0)
        #expect(output.fileHandleForReading.readDataToEndOfFile().isEmpty)
    }
}

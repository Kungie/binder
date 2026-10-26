import Foundation
import Testing
@testable import BinderCore

@Suite struct VaultLocationTests {
    let home = makeTempDir("home")

    @Test func resolveOrder() throws {
        #expect(VaultLocation.resolve(env: [:], home: home).path == home.appendingPathComponent("Binder").path)
        let pointed = makeTempDir("obsidian").appendingPathComponent("Vault/Binder")
        try VaultLocation.save(pointed, home: home)
        #expect(VaultLocation.resolve(env: [:], home: home).path == pointed.path)
        #expect(VaultLocation.resolve(env: ["BINDER_VAULT": "/tmp/override"], home: home).path == "/tmp/override")
    }

    @Test func pointerRoundTrips() throws {
        #expect(VaultLocation.load(home: home) == nil)
        let root = URL(fileURLWithPath: "/Users/x/Documents/Notes/Binder")
        try VaultLocation.save(root, home: home)
        #expect(VaultLocation.load(home: home)?.path == root.path)
        #expect(VaultLocation.configURL(home: home).path.hasSuffix("Library/Application Support/Binder/config.json"))
    }

    @Test func cliPathsFollowVaultPointer() throws {
        let pointed = makeTempDir("obsidian").appendingPathComponent("Binder")
        try FileManager.default.createDirectory(at: pointed, withIntermediateDirectories: true)
        try VaultLocation.save(pointed, home: home)
        let process = Process()
        process.executableURL = binderBinary
        process.arguments = ["doctor"]
        var env = ProcessInfo.processInfo.environment
        env.removeValue(forKey: "BINDER_VAULT")
        env["BINDER_HOME"] = home.path
        process.environment = env
        let out = Pipe()
        process.standardOutput = out
        process.standardError = FileHandle.nullDevice
        try process.run()
        let text = String(decoding: out.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
        process.waitUntilExit()
        #expect(text.contains("Binder folder: \(pointed.path)"), "\(text)")
    }
}

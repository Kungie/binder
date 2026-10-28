import CryptoKit
import Foundation
import Testing
@testable import BinderCore

@Suite struct UpdateInstallerTests {
    let root = makeTempDir("update")
    let key = Curve25519.Signing.PrivateKey()

    /// A minimal app bundle with this id, version and executable text.
    func makeApp(at url: URL, id: String = Update.bundleID, version: String, body: String) throws {
        let plist: [String: Any] = ["CFBundleIdentifier": id, "CFBundleShortVersionString": version, "CFBundleExecutable": "Binder"]
        let data = try PropertyListSerialization.data(fromPropertyList: plist, format: .xml, options: 0)
        write(String(decoding: data, as: UTF8.self), to: url.appendingPathComponent("Contents/Info.plist"))
        write(body, to: url.appendingPathComponent("Contents/MacOS/Binder"))
    }

    func zip(_ app: URL) throws -> Data {
        let out = root.appendingPathComponent("Binder.zip")
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/usr/bin/ditto")
        p.arguments = ["-c", "-k", "--keepParent", app.path, out.path]
        try p.run()
        p.waitUntilExit()
        return try Data(contentsOf: out)
    }

    func signature(_ data: Data, with key: Curve25519.Signing.PrivateKey) throws -> Data {
        Data(try key.signature(for: data).base64EncodedString().utf8)
    }

    func installed(_ app: URL) -> (version: String?, body: String?) {
        let plist = NSDictionary(contentsOf: app.appendingPathComponent("Contents/Info.plist"))
        return (plist?["CFBundleShortVersionString"] as? String, try? String(contentsOf: app.appendingPathComponent("Contents/MacOS/Binder"), encoding: .utf8))
    }

    /// The current app in a fake Applications folder, the new one zipped and signed.
    func setUp(newID: String = Update.bundleID, newVersion: String = "1.0.4") throws -> (app: URL, zip: Data) {
        let app = root.appendingPathComponent("Applications/Binder.app")
        try makeApp(at: app, version: "1.0.3", body: "old")
        let staged = root.appendingPathComponent("build/Binder.app")
        try makeApp(at: staged, id: newID, version: newVersion, body: "new")
        return (app, try zip(staged))
    }

    @Test func installsASignedUpdateInPlace() throws {
        let (app, zip) = try setUp()
        try UpdateInstaller.install(zip: zip, signature: signature(zip, with: key), expectedVersion: "1.0.4",
                                    over: app, publicKey: key.publicKey.rawRepresentation)
        #expect(installed(app) == ("1.0.4", "new"))
        let leftovers = try FileManager.default.contentsOfDirectory(atPath: app.deletingLastPathComponent().path)
        #expect(leftovers == ["Binder.app"])
    }

    @Test func leavesYourBinderSettingsAndHooksAlone() throws {
        let (app, zip) = try setUp()
        let home = root.appendingPathComponent("home")
        let files = [
            "Binder/me.md": "# Me\n\n## Decisions\n- 2026-10-26 · Short answers · written by you\n",
            "Binder/projects/dawn.md": "# dawn\n",
            "Binder/.binder/state.json": "{\"onboarded\": true, \"summarizerConsent\": true}",
            "Binder/.binder/inbox/p_1.json": "{}",
            "Library/Application Support/Binder/config.json": "{\"vault\": \"~/Binder\"}",
            ".claude/settings.json": "{\"hooks\": {}}"
        ]
        for (path, text) in files { write(text, to: home.appendingPathComponent(path)) }
        try UpdateInstaller.install(zip: zip, signature: signature(zip, with: key), expectedVersion: "1.0.4",
                                    over: app, publicKey: key.publicKey.rawRepresentation)
        for (path, text) in files {
            #expect(try String(contentsOf: home.appendingPathComponent(path), encoding: .utf8) == text, "\(path)")
        }
    }

    @Test func refusesAZipSignedWithAnotherKey() throws {
        let (app, zip) = try setUp()
        #expect(throws: UpdateInstaller.Failure.badSignature) {
            try UpdateInstaller.install(zip: zip, signature: signature(zip, with: Curve25519.Signing.PrivateKey()), expectedVersion: "1.0.4",
                                        over: app, publicKey: key.publicKey.rawRepresentation)
        }
        #expect(installed(app) == ("1.0.3", "old"))
    }

    @Test func refusesAnotherApp() throws {
        let (app, zip) = try setUp(newID: "com.example.notbinder")
        #expect(throws: UpdateInstaller.Failure.notBinder) {
            try UpdateInstaller.install(zip: zip, signature: signature(zip, with: key), expectedVersion: "1.0.4",
                                        over: app, publicKey: key.publicKey.rawRepresentation)
        }
        #expect(installed(app) == ("1.0.3", "old"))
    }

    @Test func refusesAVersionOtherThanTheReleaseSaid() throws {
        let (app, zip) = try setUp(newVersion: "1.0.2")
        #expect(throws: UpdateInstaller.Failure.wrongVersion("1.0.2")) {
            try UpdateInstaller.install(zip: zip, signature: signature(zip, with: key), expectedVersion: "1.0.4",
                                        over: app, publicKey: key.publicKey.rawRepresentation)
        }
        #expect(installed(app) == ("1.0.3", "old"))
    }

    @Test func refusesSomethingThatIsntAZip() throws {
        let (app, _) = try setUp()
        let junk = Data("not a zip".utf8)
        #expect(throws: UpdateInstaller.Failure.notBinder) {
            try UpdateInstaller.install(zip: junk, signature: signature(junk, with: key), expectedVersion: "1.0.4",
                                        over: app, publicKey: key.publicKey.rawRepresentation)
        }
        #expect(installed(app) == ("1.0.3", "old"))
    }
}

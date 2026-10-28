import CryptoKit
import Foundation
import Testing
@testable import BinderCore
@testable import BinderUI

@MainActor
@Suite(.serialized) struct UpdaterTests {
    let vault = Vault(root: makeTempDir("vault"))
    let key = Curve25519.Signing.PrivateKey()

    /// Every URL asked for. The zip and its signature download at the same time, so appends take a lock.
    final class Fetches: @unchecked Sendable {
        private let lock = NSLock()
        private var list: [URL] = []
        var urls: [URL] { lock.withLock { list } }
        func append(_ url: URL) { lock.withLock { list.append(url) } }
    }

    nonisolated static func unsigned(_ version: String) -> Data {
        Data("""
        {"tag_name": "v\(version)", "body": "", "assets": [{"name": "Binder.zip", "browser_download_url": "https://example.com/Binder.zip"}]}
        """.utf8)
    }

    nonisolated static func release(_ version: String) -> Data {
        Data("""
        {"tag_name": "v\(version)", "body": "Notes.", "assets": [
          {"name": "Binder.zip", "browser_download_url": "https://example.com/Binder.zip"},
          {"name": "Binder.zip.sig", "browser_download_url": "https://example.com/Binder.zip.sig"}]}
        """.utf8)
    }

    func updater(current: String = "1.0.3", answer: @escaping @Sendable (URL) throws -> Data, fetches: Fetches = Fetches(),
                 app: URL? = nil, relaunched: (() -> Void)? = nil) -> Updater {
        try? vault.ensureLayout()
        return Updater(current: current, vault: { [vault] in vault }, app: app ?? makeTempDir("apps").appendingPathComponent("Binder.app"),
                       publicKey: key.publicKey.rawRepresentation,
                       fetch: { url in fetches.append(url); return try answer(url) },
                       relaunch: relaunched ?? {})
    }

    @Test func findsANewerRelease() async {
        let u = updater(answer: { _ in Self.release("1.0.4") })
        await u.check(manual: true)
        #expect(u.status == .available(version: "1.0.4"))
        #expect(BinderState.load(vault).lastUpdateCheck != nil)
    }

    @Test func saysUpToDateWhenItIs() async {
        let u = updater(current: "1.0.4", answer: { _ in Self.release("1.0.4") })
        await u.check(manual: true)
        guard case .upToDate = u.status else { Issue.record("\(u.status)"); return }
    }

    @Test func anUnsignedReleaseOfThisVersionIsUpToDate() async {
        let u = updater(current: "1.0.3", answer: { _ in Self.unsigned("1.0.3") })
        await u.check(manual: true)
        guard case .upToDate = u.status else { Issue.record("\(u.status)"); return }
    }

    @Test func aNewerUnsignedReleaseIsNeverInstalled() async {
        let quiet = updater(answer: { _ in Self.unsigned("1.0.4") })
        await quiet.check(manual: false)
        #expect(quiet.status == .idle)
        let asked = updater(answer: { _ in Self.unsigned("1.0.4") })
        await asked.check(manual: true)
        #expect(asked.status == .failed("Binder 1.0.4 is out, but it can't update itself to it. Download it from GitHub."))
        await asked.install()
        #expect(asked.status == .failed("Binder 1.0.4 is out, but it can't update itself to it. Download it from GitHub."))
    }

    @Test func dailyCheckRunsOnlyWhenDueAndOn() async throws {
        let fetches = Fetches()
        let u = updater(answer: { _ in Self.release("1.0.4") }, fetches: fetches)
        var state = BinderState.load(vault)
        state.lastUpdateCheck = Date().addingTimeInterval(-3600)
        try state.save(vault)
        await u.checkIfDue()
        #expect(fetches.urls.isEmpty)
        state.lastUpdateCheck = Date().addingTimeInterval(-30 * 3600)
        try state.save(vault)
        await u.checkIfDue()
        #expect(fetches.urls == [Update.latestURL])
        state.checkForUpdates = false
        state.lastUpdateCheck = nil
        try state.save(vault)
        await u.checkIfDue()
        #expect(fetches.urls.count == 1)
    }

    @Test func aFailedCheckOnlyShowsWhenYouAsked() async {
        struct Offline: Error {}
        let quiet = updater(answer: { _ in throw Offline() })
        await quiet.check(manual: false)
        #expect(quiet.status == .idle)
        let asked = updater(answer: { _ in throw Offline() })
        await asked.check(manual: true)
        #expect(asked.status == .failed("Couldn't reach GitHub. Try again in a bit."))
    }

    @Test func updateDownloadsInstallsAndRelaunches() async throws {
        let root = makeTempDir("install")
        let app = root.appendingPathComponent("Applications/Binder.app")
        for (url, version) in [(app, "1.0.3"), (root.appendingPathComponent("build/Binder.app"), "1.0.4")] {
            let plist: [String: Any] = ["CFBundleIdentifier": Update.bundleID, "CFBundleShortVersionString": version]
            write(String(decoding: try PropertyListSerialization.data(fromPropertyList: plist, format: .xml, options: 0), as: UTF8.self),
                  to: url.appendingPathComponent("Contents/Info.plist"))
        }
        let zipURL = root.appendingPathComponent("Binder.zip")
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/usr/bin/ditto")
        p.arguments = ["-c", "-k", "--keepParent", root.appendingPathComponent("build/Binder.app").path, zipURL.path]
        try p.run()
        p.waitUntilExit()
        let zip = try Data(contentsOf: zipURL)
        let sig = Data(try key.signature(for: zip).base64EncodedString().utf8)
        var relaunched = false
        let u = updater(answer: { url in
            switch url.lastPathComponent {
            case "Binder.zip": return zip
            case "Binder.zip.sig": return sig
            default: return Self.release("1.0.4")
            }
        }, app: app, relaunched: { relaunched = true })
        await u.check(manual: false)
        await u.install()
        #expect(relaunched)
        #expect(NSDictionary(contentsOf: app.appendingPathComponent("Contents/Info.plist"))?["CFBundleShortVersionString"] as? String == "1.0.4")
    }

    @Test func aBadDownloadLeavesTheAppAndSaysSo() async throws {
        let u = updater(answer: { url in
            url.lastPathComponent == "latest" ? Self.release("1.0.4") : Data("tampered".utf8)
        })
        await u.check(manual: true)
        await u.install()
        #expect(u.status == .failed("The download didn't match Binder's signature, so I left Binder as it is."))
    }
}

import CryptoKit
import Foundation
import Testing
@testable import BinderCore

@Suite struct UpdateTests {
    @Test func versionsCompareByNumberNotText() {
        #expect(Update.isNewer("v1.0.4", than: "1.0.3"))
        #expect(Update.isNewer("1.0.10", than: "1.0.9"))
        #expect(Update.isNewer("1.1", than: "1.0.9"))
        #expect(Update.isNewer("2.0.0", than: "1.9.9"))
        #expect(!Update.isNewer("v1.0.3", than: "1.0.3"))
        #expect(!Update.isNewer("1.0.2", than: "1.0.3"))
        #expect(!Update.isNewer("nightly", than: "1.0.3"))
        #expect(!Update.isNewer("1.0.4", than: "1.0.4-dev"))
    }

    static let releaseJSON = """
    {"tag_name": "v1.0.4", "name": "Binder 1.0.4", "body": "- **Faster.** It's faster.",
     "assets": [
       {"name": "Binder.dmg", "browser_download_url": "https://github.com/Kungie/binder/releases/download/v1.0.4/Binder.dmg"},
       {"name": "Binder.zip", "browser_download_url": "https://github.com/Kungie/binder/releases/download/v1.0.4/Binder.zip"},
       {"name": "Binder.zip.sig", "browser_download_url": "https://github.com/Kungie/binder/releases/download/v1.0.4/Binder.zip.sig"}
     ]}
    """

    @Test func readsTheLatestReleaseFromGitHub() throws {
        let release = try #require(Update.Release.parse(Data(Self.releaseJSON.utf8)))
        #expect(release.version == "1.0.4")
        #expect(release.zip.absoluteString.hasSuffix("/v1.0.4/Binder.zip"))
        #expect(release.signature?.absoluteString.hasSuffix("/v1.0.4/Binder.zip.sig") == true)
        #expect(release.notes == "- **Faster.** It's faster.")
    }

    @Test func aReleaseWithoutASignatureCantBeInstalled() throws {
        let unsigned = Self.releaseJSON.replacingOccurrences(of: "Binder.zip.sig", with: "Binder.zip.txt")
        let release = try #require(Update.Release.parse(Data(unsigned.utf8)))
        #expect(release.version == "1.0.4")
        #expect(release.signature == nil)
        #expect(Update.Release.parse(Data("not json".utf8)) == nil)
    }

    @Test func onlyFilesSignedWithBindersKeyPass() throws {
        let key = Curve25519.Signing.PrivateKey()
        let zip = Data("the new Binder".utf8)
        let signature = Data(try key.signature(for: zip).base64EncodedString().utf8)
        #expect(Update.verify(zip, signature: signature, publicKey: key.publicKey.rawRepresentation))
        #expect(!Update.verify(Data("something else".utf8), signature: signature, publicKey: key.publicKey.rawRepresentation))
        #expect(!Update.verify(zip, signature: signature, publicKey: Curve25519.Signing.PrivateKey().publicKey.rawRepresentation))
        #expect(!Update.verify(zip, signature: Data("garbage".utf8), publicKey: key.publicKey.rawRepresentation))
    }

    @Test func theShippedPublicKeyIsAnEd25519Key() throws {
        let raw = try #require(Data(base64Encoded: Update.publicKey))
        #expect(raw.count == 32)
        #expect(throws: Never.self) { try Curve25519.Signing.PublicKey(rawRepresentation: raw) }
    }

    @Test func checksOnceADayWhenOn() {
        let now = Date()
        #expect(Update.isDue(lastCheck: nil, enabled: true, now: now))
        #expect(!Update.isDue(lastCheck: now.addingTimeInterval(-23 * 3600), enabled: true, now: now))
        #expect(Update.isDue(lastCheck: now.addingTimeInterval(-25 * 3600), enabled: true, now: now))
        #expect(!Update.isDue(lastCheck: nil, enabled: false, now: now))
        // A clock that went backwards doesn't block checks forever.
        #expect(Update.isDue(lastCheck: now.addingTimeInterval(3 * 86400), enabled: true, now: now))
    }

    @Test func olderStateFilesKeepTheirSettingsAndCheckForUpdates() throws {
        let vault = Vault(root: makeTempDir("state"))
        try FileManager.default.createDirectory(at: vault.internalDir, withIntermediateDirectories: true)
        try Data("""
        {"summarizerConsent": true, "handoffThreshold": 0.8, "onboarded": true, "observeIntervalHours": 24, "projectRoots": ["~/Code"]}
        """.utf8).write(to: vault.stateFile)
        var state = BinderState.load(vault)
        #expect(state.summarizerConsent && state.onboarded)
        #expect(state.handoffThreshold == 0.8 && state.observeIntervalHours == 24 && state.projectRoots == ["~/Code"])
        #expect(state.checkForUpdates)
        #expect(state.lastUpdateCheck == nil)
        state.checkForUpdates = false
        state.lastUpdateCheck = Date(timeIntervalSince1970: 1_793_000_000)
        try state.save(vault)
        let again = BinderState.load(vault)
        #expect(!again.checkForUpdates)
        #expect(again.lastUpdateCheck == Date(timeIntervalSince1970: 1_793_000_000))
        #expect(again.summarizerConsent && again.projectRoots == ["~/Code"])
    }
}

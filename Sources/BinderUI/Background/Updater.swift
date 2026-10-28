import BinderCore
import Foundation

/// Keeps Binder up to date: looks for a new release once a day (if that's on) and whenever you ask,
/// and on Update downloads, checks and installs it, then restarts Binder. Your binder and settings stay put.
@MainActor
public final class Updater: ObservableObject {
    public enum Status: Equatable {
        case idle
        case checking
        case upToDate(Date)
        case available(version: String)
        case installing(version: String)
        case failed(String)
    }

    @Published public private(set) var status: Status = .idle
    public let current: String
    private var release: Update.Release?
    private let vault: () -> Vault
    private let app: URL
    private let publicKey: Data
    private let fetch: @Sendable (URL) async throws -> Data
    private let relaunch: () -> Void
    private var timer: Timer?

    public init(current: String, vault: @escaping () -> Vault, app: URL, publicKey: Data = Data(base64Encoded: Update.publicKey)!,
                fetch: @escaping @Sendable (URL) async throws -> Data = Updater.download, relaunch: @escaping () -> Void) {
        self.current = current
        self.vault = vault
        self.app = app
        self.publicKey = publicKey
        self.fetch = fetch
        self.relaunch = relaunch
    }

    /// GitHub over HTTPS, with the User-Agent its API asks for.
    public static let download: @Sendable (URL) async throws -> Data = { url in
        var request = URLRequest(url: url, timeoutInterval: 60)
        request.setValue("Binder", forHTTPHeaderField: "User-Agent")
        request.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
        let (data, response) = try await URLSession.shared.data(for: request)
        guard (response as? HTTPURLResponse)?.statusCode == 200 else { throw URLError(.badServerResponse) }
        return data
    }

    /// A first look a little after launch, then every hour whether a day has passed.
    public func start() {
        Task { [weak self] in
            try? await Task.sleep(nanoseconds: 20_000_000_000)
            await self?.checkIfDue()
        }
        timer = Timer.scheduledTimer(withTimeInterval: 3600, repeats: true) { [weak self] _ in
            Task { @MainActor in await self?.checkIfDue() }
        }
    }

    public func checkIfDue() async {
        let state = BinderState.load(vault())
        guard Update.isDue(lastCheck: state.lastUpdateCheck, enabled: state.checkForUpdates, now: Date()) else { return }
        await check(manual: false)
    }

    /// `manual` is the Check for updates button: it shows progress and failures. The daily check stays quiet.
    public func check(manual: Bool) async {
        if case .installing = status { return }
        if manual { status = .checking }
        do {
            let data = try await fetch(Update.latestURL)
            var state = BinderState.load(vault())
            state.lastUpdateCheck = Date()
            try? state.save(vault())
            guard let latest = Update.Release.parse(data) else {
                if manual { status = .failed("GitHub's answer didn't make sense. Try again in a bit.") }
                return
            }
            if Update.isNewer(latest.version, than: current), latest.signature == nil {
                // Newer, but not signed for in-app updates: never install it, and only say so when asked.
                release = nil
                if manual { status = .failed("Binder \(latest.version) is out, but it can't update itself to it. Download it from GitHub.") }
            } else if Update.isNewer(latest.version, than: current) {
                release = latest
                status = .available(version: latest.version)
            } else {
                release = nil
                status = .upToDate(Date())
            }
        } catch {
            if manual { status = .failed("Couldn't reach GitHub. Try again in a bit.") }
        }
    }

    public func install() async {
        guard let release, let signatureURL = release.signature, case .available = status else { return }
        status = .installing(version: release.version)
        do {
            async let zip = fetch(release.zip)
            async let signature = fetch(signatureURL)
            let (zipData, sigData) = try await (zip, signature)
            let app = self.app, key = publicKey, version = release.version
            try await Task.detached {
                try UpdateInstaller.install(zip: zipData, signature: sigData, expectedVersion: version, over: app, publicKey: key)
            }.value
            relaunch()
        } catch let failure as UpdateInstaller.Failure {
            status = .failed(Self.message(failure))
        } catch {
            status = .failed("Couldn't download the update. Try again in a bit.")
        }
    }

    static func message(_ failure: UpdateInstaller.Failure) -> String {
        switch failure {
        case .badSignature: "The download didn't match Binder's signature, so I left Binder as it is."
        case .notBinder: "The download wasn't a Binder app, so I left Binder as it is."
        case let .wrongVersion(v): "The download was version \(v), not the one GitHub announced, so I left Binder as it is."
        case .cannotWrite: "I can't replace myself where I am. Move Binder to Applications, open it from there and try again."
        }
    }
}

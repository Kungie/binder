import BinderCore
import Foundation

/// Looks around again: at launch, every few hours, and 30 s after something it watches changes.
@MainActor
public final class ObserveScheduler {
    public static let debounce: TimeInterval = 30

    let vault: () -> Vault
    let home: URL
    var onRunning: (Bool) -> Void
    var onFinished: () -> Void
    private var timer: Timer?
    private var watchers: [DirectoryWatcher] = []
    private var lastEvent: Date?
    private var running = false

    public init(vault: @escaping () -> Vault, home: URL, onRunning: @escaping (Bool) -> Void, onFinished: @escaping () -> Void) {
        self.vault = vault
        self.home = home
        self.onRunning = onRunning
        self.onFinished = onFinished
    }

    /// The whole decision: never ran, the interval passed, or a change settled for `debounce` seconds since the last run.
    nonisolated public static func shouldRun(now: Date, lastRun: Date?, interval: TimeInterval, lastEvent: Date?, debounce: TimeInterval = debounce) -> Bool {
        guard let lastRun else { return true }
        if now.timeIntervalSince(lastRun) >= interval { return true }
        if let lastEvent, lastEvent > lastRun, now.timeIntervalSince(lastEvent) >= debounce { return true }
        return false
    }

    public func start() {
        let state = BinderState.load(vault())
        var watched = [home.appendingPathComponent(".ssh")]
        watched += state.projectRoots.map { $0.hasPrefix("~/") ? home.appendingPathComponent(String($0.dropFirst(2))) : URL(fileURLWithPath: $0) }
        watchers = watched.compactMap { url in
            DirectoryWatcher(url: url, debounce: 1) { [weak self] in MainActor.assumeIsolated { self?.lastEvent = Date() } }
        }
        timer = Timer.scheduledTimer(withTimeInterval: 15, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.tick() }
        }
        tick()
    }

    public func stop() {
        timer?.invalidate()
        watchers.forEach { $0.stop() }
        watchers = []
    }

    func tick() {
        let state = BinderState.load(vault())
        guard !running, Self.shouldRun(now: Date(), lastRun: state.lastObserve, interval: TimeInterval(state.observeIntervalHours) * 3600, lastEvent: lastEvent)
        else { return }
        runNow()
    }

    public func runNow() {
        guard !running else { return }
        running = true
        onRunning(true)
        let vault = vault(), home = home
        let state = BinderState.load(vault)
        let roots = state.projectRoots.map { $0.hasPrefix("~/") ? home.appendingPathComponent(String($0.dropFirst(2))) : URL(fileURLWithPath: $0) }
        Task.detached {
            let ctx = ObserverContext(home: home, projectRoots: roots, runner: SystemRunner())
            _ = try? ObserveRun.run(vault: vault, ctx: ctx, observers: ObserveRun.defaultObservers())
            await MainActor.run {
                self.running = false
                self.lastEvent = nil
                self.onRunning(false)
                self.onFinished()
            }
        }
    }
}

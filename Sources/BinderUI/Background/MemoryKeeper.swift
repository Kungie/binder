import BinderCore
import Foundation

/// Keeps the binder fresh in the background, with the user's consent to read sessions:
/// - every few minutes, a session that went quiet gets whatever talk the Stop hook left waiting summarized,
///   so a short last stretch isn't lost to batching;
/// - every few hours, notes changed since their last tidy lose repeated and outdated lines to Superseded.
@MainActor
public final class MemoryKeeper {
    /// A session this quiet is done for now.
    static let quiet: TimeInterval = 10 * 60
    /// Sessions older than this aren't worth reading any more.
    static let tooOld: TimeInterval = 2 * 86400
    static let sweepEvery: TimeInterval = 5 * 60
    static let tidyEvery: TimeInterval = 6 * 3600

    private let vault: () -> Vault
    private let helper: String
    private let spawn: (String, [String]) -> Void
    private var timer: Timer?
    private var lastTidy: Date?

    public init(vault: @escaping () -> Vault, helper: String,
                spawn: @escaping (String, [String]) -> Void = { path, args in _ = HookIO.spawnDetached(path, args) }) {
        self.vault = vault
        self.helper = helper
        self.spawn = spawn
    }

    public func start() {
        timer = Timer.scheduledTimer(withTimeInterval: Self.sweepEvery, repeats: true) { [weak self] _ in
            Task { @MainActor in self?.tick() }
        }
        Task { [weak self] in
            try? await Task.sleep(nanoseconds: 60_000_000_000)
            self?.tick()
        }
    }

    func tick(now: Date = Date()) {
        sweep(now: now)
        if lastTidy.map({ now.timeIntervalSince($0) >= Self.tidyEvery }) ?? true {
            lastTidy = now
            let vault = vault()
            let consent = BinderState.load(vault).summarizerConsent
            Task.detached(priority: .utility) {
                Tidy.run(vault: vault, summarizer: consent ? ClaudeCLISummarizer(runner: ClaudeCLISummarizer.runner(), session: nil) : nil)
            }
        }
    }

    /// Hands every quiet session's waiting talk to the summarizer, one detached `binder summarize` each.
    func sweep(now: Date = Date()) {
        let vault = vault()
        guard BinderState.load(vault).summarizerConsent else { return }
        let queue = SummarizeQueue(vault: vault)
        let known = Set(queue.sessions())
        let files = (try? FileManager.default.contentsOfDirectory(at: vault.sessions, includingPropertiesForKeys: nil)) ?? []
        for file in files where file.pathExtension == "json" {
            guard let status = SessionStatus.load(session: file.deletingPathExtension().lastPathComponent, vault: vault),
                  known.contains(status.session), let path = status.transcriptPath,
                  now.timeIntervalSince(status.updatedAt) >= Self.quiet, now.timeIntervalSince(status.updatedAt) < Self.tooOld,
                  let range = queue.claim(session: status.session, transcript: URL(fileURLWithPath: path), startingAt: 0, now: now, force: true)
            else { continue }
            spawn(helper, ["summarize", status.session, path, String(range.lowerBound), String(range.upperBound), status.cwd])
        }
    }
}

import BinderCore
import Foundation

/// Where a proposal card is in its little life on the panel.
public enum CardPhase: Equatable, Sendable {
    case idle
    case absorbing
    case shredding
    case editing(String)

    public static let absorbDuration = 0.7
    public static let shredDuration = 1.1
    public static let keepAllGap = 0.22
}

/// Where "Hand off now" is for the session it was pressed in.
public enum HandoffState: Equatable, Sendable {
    case writing(session: String)
    /// Written, and the prompt for a new session is on the clipboard.
    case copied(session: String)
    case failed(session: String)

    public var session: String {
        switch self {
        case let .writing(s), let .copied(s), let .failed(s): s
        }
    }
}

/// The running app's view of the vault: what's waiting for review, the session being watched, and how much Binder remembers.
@MainActor
public final class AppModel: ObservableObject {
    /// Shrinks every animation delay; tests set it small.
    public nonisolated(unsafe) static var timeScale: Double = 1
    /// A session older than this isn't "this session" any more.
    public static let sessionFreshness: TimeInterval = 2 * 3600

    @Published public private(set) var proposals: [Proposal] = []
    @Published public private(set) var session: SessionStatus?
    @Published public private(set) var memories = 0
    @Published public private(set) var state: BinderState
    @Published public var phases: [String: CardPhase] = [:]
    /// Cleared by Done, or by itself once a new session takes over as "this session".
    @Published public var handoff: HandoffState?

    @Published public private(set) var vault: Vault
    public let home: URL
    let now: () -> Date
    private var watchers: [DirectoryWatcher] = []

    public init(vault: Vault, home: URL, now: @escaping () -> Date = Date.init) {
        self.vault = vault
        self.home = home
        self.now = now
        self.state = BinderState.load(vault)
    }

    // MARK: Reading

    public func refresh() {
        state = BinderState.load(vault)
        proposals = ProposalStore(vault: vault).pending()
        phases = phases.filter { id, _ in proposals.contains { $0.id == id } }
        session = Self.latestSession(vault: vault, now: now())
        if let handoff, session?.session != handoff.session { self.handoff = nil }
        memories = Self.countMemories(vault: vault)
    }

    static func latestSession(vault: Vault, now: Date) -> SessionStatus? {
        let files = (try? FileManager.default.contentsOfDirectory(at: vault.sessions, includingPropertiesForKeys: nil)) ?? []
        return files.filter { $0.pathExtension == "json" }
            .compactMap { SessionStatus.load(session: $0.deletingPathExtension().lastPathComponent, vault: vault) }
            .filter { now.timeIntervalSince($0.updatedAt) < sessionFreshness }
            .max { $0.updatedAt < $1.updatedAt }
    }

    /// Every decision line in the vault: what Binder remembers.
    static func countMemories(vault: Vault) -> Int {
        var count = NoteIO.read(vault.url("me.md"))?.decisions.count ?? 0
        for folder in ["machines", "access", "servers", "projects"] {
            let names = (try? FileManager.default.contentsOfDirectory(atPath: vault.url(folder).path)) ?? []
            for name in names where name.hasSuffix(".md") {
                count += NoteIO.read(vault.url("\(folder)/\(name)"))?.decisions.count ?? 0
            }
        }
        return count
    }

    public var pendingCount: Int {
        proposals.filter { phases[$0.id] != .absorbing && phases[$0.id] != .shredding }.count
    }

    public var meterFraction: Double? { session?.reading?.fraction }

    public var handoffSuggested: Bool {
        guard let fraction = meterFraction else { return false }
        return fraction >= state.clampedThreshold
    }

    // MARK: Acting

    public func keep(_ id: String, editedText: String? = nil) {
        guard proposals.contains(where: { $0.id == id }), phases[id] != .absorbing, phases[id] != .shredding else { return }
        var text = editedText
        if text == nil, case let .editing(draft)? = phases[id] { text = draft }
        phases[id] = .absorbing
        after(CardPhase.absorbDuration) { [weak self] in
            guard let self else { return }
            try? ProposalStore(vault: vault).keep(id, editedText: text)
            phases[id] = nil
            refresh()
        }
    }

    public func drop(_ id: String) {
        guard proposals.contains(where: { $0.id == id }), phases[id] != .absorbing, phases[id] != .shredding else { return }
        phases[id] = .shredding
        after(CardPhase.shredDuration) { [weak self] in
            guard let self else { return }
            try? ProposalStore(vault: vault).drop(id)
            phases[id] = nil
            refresh()
        }
    }

    public func keepAll() {
        let ids = proposals.filter { phases[$0.id] == nil || phases[$0.id] == .idle }.map(\.id)
        for (i, id) in ids.enumerated() {
            if i == 0 { keep(id) } else { after(Double(i) * CardPhase.keepAllGap) { [weak self] in self?.keep(id) } }
        }
    }

    public func edit(_ id: String) {
        guard let p = proposals.first(where: { $0.id == id }) else { return }
        phases[id] = .editing(p.text)
    }

    public func cancelEdit(_ id: String) {
        if case .editing? = phases[id] { phases[id] = nil }
    }

    private func after(_ seconds: Double, _ body: @escaping @MainActor () -> Void) {
        Task { @MainActor in
            try? await Task.sleep(nanoseconds: UInt64(seconds * Self.timeScale * 1_000_000_000))
            body()
        }
    }

    // MARK: Watching

    public func startWatching() {
        try? vault.ensureLayout()
        watchers = [vault.inbox, vault.sessions].compactMap { url in
            DirectoryWatcher(url: url) { [weak self] in
                MainActor.assumeIsolated { self?.refresh() }
            }
        }
    }

    /// The binder moved (Settings, or into an Obsidian vault): read and watch the new place.
    public func relocate(to newVault: Vault) {
        let watching = !watchers.isEmpty
        stopWatching()
        vault = newVault
        phases = [:]
        refresh()
        if watching { startWatching() }
    }

    public func stopWatching() {
        watchers.forEach { $0.stop() }
        watchers = []
    }
}

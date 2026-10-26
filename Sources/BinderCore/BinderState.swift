import Foundation

/// Settings and bookkeeping kept in `.binder/state.json`.
public struct BinderState: Codable, Equatable, Sendable {
    public var summarizerConsent = false
    public var handoffThreshold = 0.70
    public var projectRoots = ["~/Desktop", "~/Documents", "~/Developer", "~/Projects", "~/code"]
    public var lastObserve: Date?
    public var onboarded = false

    public init() {}

    enum CodingKeys: String, CodingKey { case summarizerConsent, handoffThreshold, projectRoots, lastObserve, onboarded }

    /// Every key is optional, so a file written by an older version keeps the settings it has.
    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        let d = BinderState()
        summarizerConsent = try c.decodeIfPresent(Bool.self, forKey: .summarizerConsent) ?? d.summarizerConsent
        handoffThreshold = try c.decodeIfPresent(Double.self, forKey: .handoffThreshold) ?? d.handoffThreshold
        projectRoots = try c.decodeIfPresent([String].self, forKey: .projectRoots) ?? d.projectRoots
        lastObserve = try c.decodeIfPresent(Date.self, forKey: .lastObserve)
        onboarded = try c.decodeIfPresent(Bool.self, forKey: .onboarded) ?? d.onboarded
    }

    public var clampedThreshold: Double { min(max(handoffThreshold, 0.50), 0.90) }

    /// Defaults when the file is missing or unreadable.
    public static func load(_ vault: Vault) -> BinderState {
        guard let data = try? Data(contentsOf: vault.stateFile) else { return BinderState() }
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return (try? decoder.decode(BinderState.self, from: data)) ?? BinderState()
    }

    public func save(_ vault: Vault) throws {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        try FileManager.default.createDirectory(at: vault.internalDir, withIntermediateDirectories: true)
        try encoder.encode(self).write(to: vault.stateFile, options: .atomic)
    }
}

import Foundation

/// Settings and bookkeeping kept in `.binder/state.json`.
public struct BinderState: Codable, Equatable, Sendable {
    public var summarizerConsent = false
    public var handoffThreshold = 0.70
    public var projectRoots = ["~/Desktop", "~/Documents", "~/Developer", "~/Projects", "~/code"]
    public var lastObserve: Date?

    public init() {}

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

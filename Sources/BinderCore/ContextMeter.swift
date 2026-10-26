import Foundation

public struct ContextReading: Codable, Equatable, Sendable {
    public var used: Int
    public var window: Int

    public init(used: Int, window: Int) {
        self.used = used
        self.window = window
    }

    public var fraction: Double { window > 0 ? Double(used) / Double(window) : 0 }
}

/// How full a session's context is, from the last turn's token usage. An estimate: the transcript
/// doesn't name the window, so it's 200k unless the model id says `[1m]` or the usage is already past 200k.
public enum ContextMeter {
    public static let standardWindow = 200_000
    public static let largeWindow = 1_000_000

    public static func reading(from entries: [TranscriptEntry]) -> ContextReading? {
        for entry in entries.reversed() {
            guard case let .usage(usage, model) = entry else { continue }
            let large = (model?.contains("[1m]") ?? false) || usage.total > standardWindow
            return ContextReading(used: usage.total, window: large ? largeWindow : standardWindow)
        }
        return nil
    }
}

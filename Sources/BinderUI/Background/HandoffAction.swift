import BinderCore
import Foundation

/// "Hand off now": read the end of the session and write a handoff card for the next one.
public enum HandoffAction {
    static let tail: UInt64 = 2_000_000

    public static func run(session: SessionStatus, vault: Vault, consent: Bool, runner: CommandRunner) -> URL? {
        guard let path = session.transcriptPath else { return nil }
        let size = (try? FileManager.default.attributesOfItem(atPath: path)[.size] as? UInt64) ?? 0
        guard let chunk = try? Transcript.read(URL(fileURLWithPath: path), from: size > tail ? size - tail : 0) else { return nil }
        var card: HandoffCard?
        if consent { card = ClaudeCLISummarizer(runner: runner, session: session.session).handoff(from: chunk.entries) }
        let project = session.project ?? URL(fileURLWithPath: session.cwd).lastPathComponent
        return try? Handoff.write(card ?? Handoff.fallback(from: chunk.entries), project: project, vault: vault, now: Date())
    }
}

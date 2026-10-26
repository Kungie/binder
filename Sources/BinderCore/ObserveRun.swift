import Foundation

public struct ObserveSummary: Equatable, Sendable {
    public var notesWritten = 0
    public var proposalsAdded = 0
    public var filesRead = 0
}

/// Runs the observers and writes what they saw: observed blocks into notes, guesses into the inbox.
public enum ObserveRun {
    public static func defaultObservers() -> [Observer] {
        [MacObserver(), SSHObserver(), AccountsObserver(), ProjectsObserver(), HomelabObserver()]
    }

    public static func run(vault: Vault, ctx: ObserverContext, observers: [Observer]) throws -> ObserveSummary {
        try vault.ensureLayout()
        let store = ProposalStore(vault: vault)
        let verified = ISO8601DateFormatter().string(from: ctx.now)
        var summary = ObserveSummary()
        for observer in observers {
            let obs = observer.observe(ctx)
            summary.filesRead += obs.filesRead
            for update in obs.notes {
                try NoteIO.update(vault.url(update.path), title: update.title) { note in
                    var next = note.replacingObserved(update.observed).setting("verified", verified)
                    if next.value("source") == nil { next = next.setting("source", "observed") }
                    return next
                }
                summary.notesWritten += 1
            }
            for proposal in obs.proposals where try store.add(proposal) {
                summary.proposalsAdded += 1
            }
        }
        var state = BinderState.load(vault)
        state.lastObserve = ctx.now
        try state.save(vault)
        return summary
    }
}

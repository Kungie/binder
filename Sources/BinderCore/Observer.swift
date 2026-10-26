import Foundation

public struct ObserverContext: Sendable {
    public var home: URL
    public var projectRoots: [URL]
    public var runner: CommandRunner
    public var now: Date
    public var allowLAN: Bool

    public init(home: URL, projectRoots: [URL], runner: CommandRunner, now: Date = .now, allowLAN: Bool = false) {
        self.home = home
        self.projectRoots = projectRoots
        self.runner = runner
        self.now = now
        self.allowLAN = allowLAN
    }
}

/// The observed block a note should have, in full. Observers rewrite it from scratch every run.
public struct NoteUpdate: Equatable, Sendable {
    public var path: String
    public var title: String
    public var observed: [String]

    public init(path: String, title: String, observed: [String]) {
        self.path = path
        self.title = title
        self.observed = observed
    }
}

public struct Observation: Sendable {
    public var notes: [NoteUpdate] = []
    public var proposals: [Proposal] = []
    public var filesRead = 0

    public init(notes: [NoteUpdate] = [], proposals: [Proposal] = [], filesRead: Int = 0) {
        self.notes = notes
        self.proposals = proposals
        self.filesRead = filesRead
    }
}

/// Reads one corner of the machine. Read-only: observers never write anywhere themselves.
public protocol Observer: Sendable {
    var name: String { get }
    func observe(_ ctx: ObserverContext) -> Observation
}

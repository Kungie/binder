import Foundation
@testable import BinderCore

/// Answers commands from a table keyed by `"tool arg1 arg2"`. Anything missing counts as not installed.
struct FakeRunner: CommandRunner {
    var results: [String: CommandResult] = [:]

    func run(_ tool: String, _ args: [String], input: String?, timeout: TimeInterval) -> CommandResult? {
        results[([tool] + args).joined(separator: " ")]
    }
}

func ok(_ stdout: String, stderr: String = "") -> CommandResult {
    CommandResult(status: 0, stdout: stdout, stderr: stderr, timedOut: false)
}

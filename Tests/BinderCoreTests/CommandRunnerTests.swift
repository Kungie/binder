import Foundation
import Testing
@testable import BinderCore

@Suite struct CommandRunnerTests {
    let runner = SystemRunner()

    @Test func systemRunnerRunsEcho() throws {
        let result = try #require(runner.run("echo", ["hello", "binder"], input: nil, timeout: 5))
        #expect(result.status == 0)
        #expect(result.stdout == "hello binder\n")
        #expect(!result.timedOut)
    }

    @Test func systemRunnerReturnsNilForMissingTool() {
        #expect(runner.run("definitely-not-a-binder-tool", [], input: nil, timeout: 1) == nil)
    }

    @Test func systemRunnerKillsOnTimeout() throws {
        let start = Date()
        let result = try #require(runner.run("sleep", ["5"], input: nil, timeout: 0.3))
        #expect(result.timedOut)
        #expect(Date().timeIntervalSince(start) < 1.5)
    }

    @Test func systemRunnerPassesStdin() throws {
        let result = try #require(runner.run("cat", [], input: "from stdin\n", timeout: 5))
        #expect(result.stdout == "from stdin\n")
    }
}

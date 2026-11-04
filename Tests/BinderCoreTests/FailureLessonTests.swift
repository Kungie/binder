import Foundation
import Testing
@testable import BinderCore

@Suite struct FailureLessonTests {
    func jsonl(_ objects: [[String: Any]]) throws -> URL {
        let url = makeTempDir("fail").appendingPathComponent("s.jsonl")
        let text = try objects.map { String(data: try JSONSerialization.data(withJSONObject: $0), encoding: .utf8)! }.joined(separator: "\n") + "\n"
        try Data(text.utf8).write(to: url)
        return url
    }

    @Test func failedResultPairsWithCommand() throws {
        let url = try jsonl([
            ["type": "assistant", "message": ["role": "assistant", "content": [["type": "tool_use", "id": "t1", "name": "Bash", "input": ["command": "ssh hetzner-1 docker compose up -d"]]]]],
            ["type": "user", "message": ["role": "user", "content": [["type": "tool_result", "tool_use_id": "t1", "is_error": true,
                                                                       "content": [["type": "text", "text": "permission denied while trying to connect to the Docker daemon socket"]]]]]],
            ["type": "assistant", "message": ["role": "assistant", "content": [["type": "tool_use", "id": "t2", "name": "Bash", "input": ["command": "ls"]]]]],
            ["type": "user", "message": ["role": "user", "content": [["type": "tool_result", "tool_use_id": "t2", "content": "a b"]]]]
        ])
        let failed = try Transcript.read(url, from: 0).entries.compactMap { entry -> String? in
            if case let .failed(command, error) = entry { return "\(command) | \(error)" } else { return nil }
        }
        #expect(failed == ["ssh hetzner-1 docker compose up -d | permission denied while trying to connect to the Docker daemon socket"])
    }

    @Test func reviewInputListsFailures() {
        let spy = SpyRunner(reply: #"{"new":[{"kind":"fact","scope":"project","text":"On hetzner-1, docker compose needs sudo.","source":"failure"}],"outdated":[]}"#)
        let review = ClaudeCLISummarizer(runner: spy, session: "s").review(entries: [.failed(command: "docker compose up", error: "permission denied")],
                                                                          project: "dawn", known: .init(project: [], me: []))
        #expect(spy.calls.first?.input.contains("FAILED: docker compose up · permission denied") == true)
        #expect(review.proposals.first?.source == .failure)
        #expect(ClaudeCLISummarizer.reviewSystem.contains("FAILED"))
    }

    @Test func failureSourceRoundTrips() throws {
        let p = Proposal.make(kind: .fact, source: .failure, text: "t", evidence: "e", target: "projects/dawn.md", project: "dawn", session: nil)
        let e = JSONEncoder(), d = JSONDecoder()
        e.dateEncodingStrategy = .iso8601; d.dateDecodingStrategy = .iso8601
        #expect(try d.decode(Proposal.self, from: e.encode(p)).source == .failure)
        #expect(p.provenance == "learned from a failure")
    }
}

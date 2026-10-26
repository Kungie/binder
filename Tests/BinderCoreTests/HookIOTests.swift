import Foundation
import Testing
@testable import BinderCore

@Suite struct HookIOTests {
    @Test func hookInputParsesSessionStart() throws {
        let json = #"{"session_id":"abc","transcript_path":"/t.jsonl","cwd":"/Users/x/dawn","hook_event_name":"SessionStart","source":"clear"}"#
        let input = try #require(HookIO.readInput(Data(json.utf8)))
        #expect(input == HookInput(sessionID: "abc", transcriptPath: "/t.jsonl", cwd: "/Users/x/dawn", hookEventName: "SessionStart", source: "clear"))
    }

    @Test func hookInputNilOnGarbage() {
        #expect(HookIO.readInput(Data("not json".utf8)) == nil)
        #expect(HookIO.readInput(Data()) == nil)
        #expect(HookIO.readInput(Data(#"{"cwd":"/x"}"#.utf8)) == nil)
    }

    @Test func sessionStartOutputShape() throws {
        let data = HookIO.sessionStartOutput("BINDER CARD · line \"quoted\"")
        let json = try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any])
        let output = try #require(json["hookSpecificOutput"] as? [String: Any])
        #expect(output["hookEventName"] as? String == "SessionStart")
        #expect(output["additionalContext"] as? String == "BINDER CARD · line \"quoted\"")
    }

    @Test func cliSummarizerParsesHandoffJSON() throws {
        let reply = #"Sure. {"done":["Panel redesign merged"],"left":["Notarization"],"dontTouch":["Resources/Fonts"],"next":"Release notes for 0.3."}"#
        let runner = FakeRunner(results: ["claude -p --model haiku --output-format text": ok(reply)])
        let card = try #require(ClaudeCLISummarizer(runner: runner, session: "s").handoff(from: [.user("wrap up")]))
        #expect(card == HandoffCard(done: ["Panel redesign merged"], left: ["Notarization"], dontTouch: ["Resources/Fonts"], next: "Release notes for 0.3."))
        #expect(ClaudeCLISummarizer(runner: FakeRunner(), session: "s").handoff(from: [.user("x")]) == nil)
    }

    @Test func claudeProjectFolderEncoding() {
        #expect(HookIO.claudeProjectFolder(for: "/Users/x/Desktop/My Projects/binder") == "-Users-x-Desktop-My-Projects-binder")
        #expect(HookIO.claudeProjectFolder(for: "/Users/x/my.site") == "-Users-x-my-site")
    }
}

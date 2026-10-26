import Foundation
import Testing
@testable import BinderCore
@testable import BinderUI

@Suite struct BackgroundTests {
    let t0 = Date(timeIntervalSince1970: 1_800_000_000)

    @Test func schedulerRunsOnIntervalAndDebouncesEvents() {
        let hour: TimeInterval = 3600
        #expect(ObserveScheduler.shouldRun(now: t0, lastRun: nil, interval: 6 * hour, lastEvent: nil))
        #expect(!ObserveScheduler.shouldRun(now: t0.addingTimeInterval(hour), lastRun: t0, interval: 6 * hour, lastEvent: nil))
        #expect(ObserveScheduler.shouldRun(now: t0.addingTimeInterval(6 * hour), lastRun: t0, interval: 6 * hour, lastEvent: nil))
        let event = t0.addingTimeInterval(100)
        #expect(!ObserveScheduler.shouldRun(now: event.addingTimeInterval(10), lastRun: t0, interval: 6 * hour, lastEvent: event))
        #expect(ObserveScheduler.shouldRun(now: event.addingTimeInterval(30), lastRun: t0, interval: 6 * hour, lastEvent: event))
        #expect(!ObserveScheduler.shouldRun(now: event.addingTimeInterval(60), lastRun: event.addingTimeInterval(40), interval: 6 * hour, lastEvent: event))
    }

    func status(_ id: String, used: Int) -> SessionStatus {
        SessionStatus(session: id, project: "dawn", cwd: "/x/dawn", reading: ContextReading(used: used, window: 200_000), updatedAt: t0)
    }

    @Test func notifiesOncePerSession() {
        let notifier = HandoffNotifier(store: InMemoryNotified())
        #expect(notifier.check(status("a", used: 150_000), threshold: 0.7))
        #expect(!notifier.check(status("a", used: 160_000), threshold: 0.7))
        #expect(notifier.check(status("b", used: 150_000), threshold: 0.7))
    }

    @Test func noNotificationBelowThreshold() {
        let notifier = HandoffNotifier(store: InMemoryNotified())
        #expect(!notifier.check(status("a", used: 100_000), threshold: 0.7))
        var noReading = status("a", used: 0)
        noReading.reading = nil
        #expect(!notifier.check(noReading, threshold: 0.7))
        #expect(notifier.check(status("a", used: 141_000), threshold: 0.7))
    }

    @Test func handoffActionWritesFallbackWithoutConsent() throws {
        let vault = Vault(root: makeTempDir())
        let session = SessionStatus(session: "s", project: "dawn", cwd: "/x/dawn", reading: nil, updatedAt: Date(),
                                    transcriptPath: fixture("transcript.jsonl").path)
        let url = try #require(HandoffAction.run(session: session, vault: vault, consent: false, runner: FakeRunner()))
        #expect(url.path.contains("handoffs/dawn/"))
        #expect(Handoff.pending(project: "dawn", vault: vault)?.next == "Next: write the release notes for 0.3.")
        var noTranscript = session
        noTranscript.transcriptPath = nil
        #expect(HandoffAction.run(session: noTranscript, vault: vault, consent: false, runner: FakeRunner()) == nil)
    }

    @Test func launcherEscapesQuotesInPath() {
        #expect(SessionLauncher.command(dir: URL(fileURLWithPath: "/Users/x/Desktop/My Projects/it's")) == "cd '/Users/x/Desktop/My Projects/it'\\''s' && claude")
    }

    @Test func terminalScriptsAreWellFormed() throws {
        let dir = URL(fileURLWithPath: "/Users/x/My \"Proj\"")
        let terminal = try #require(SessionLauncher.appleScript(for: .terminal, dir: dir))
        #expect(terminal.contains("tell application \"Terminal\""))
        #expect(terminal.contains("do script \"cd '/Users/x/My \\\"Proj\\\"' && claude\""))
        let iterm = try #require(SessionLauncher.appleScript(for: .iterm, dir: dir))
        #expect(iterm.contains("tell application \"iTerm\"") && iterm.contains("write text"))
        #expect(SessionLauncher.appleScript(for: .ghostty, dir: dir) == nil)
        #expect(SessionLauncher.ghosttyArguments(dir: dir) == ["-na", "Ghostty", "--args", "--working-directory=/Users/x/My \"Proj\"", "-e", "claude"])
        #expect(SessionLauncher.preferred { $0 == "com.googlecode.iterm2" } == .iterm)
        #expect(SessionLauncher.preferred { _ in false } == .terminal)
    }
}

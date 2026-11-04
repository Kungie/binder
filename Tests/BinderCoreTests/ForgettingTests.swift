import Foundation
import Testing
@testable import BinderCore

@Suite struct ForgettingTests {
    let now = ISO8601DateFormatter().date(from: "2026-11-05T10:00:00Z")!
    let old = "2026-06-01 · Uses Netlify for previews · kept from the chat"
    let fresh = "2026-10-30 · Uses Fly.io · kept from the chat"

    func vault() throws -> Vault {
        let vault = Vault(root: makeTempDir("fade"))
        for path in ["projects/dawn.md", "me.md", "access/ssh.md", "projects/dawn/hosting.md"] {
            try NoteIO.write(Note(body: "# x\n\n## Decisions\n- \(old)\n- \(fresh)\n- Undated line\n"), to: vault.url(path))
        }
        return vault
    }

    @Test func oldUnusedLineFades() throws {
        let v = try vault()
        let usage = UsageStore(vault: v)
        #expect(Forgetting.fades(old, in: "projects/dawn.md", usage: usage, now: now))
        #expect(Forgetting.fades(old, in: "projects/dawn/hosting.md", usage: usage, now: now))
        #expect(!Forgetting.fades(fresh, in: "projects/dawn.md", usage: usage, now: now))
    }

    @Test func usedOrPinnedLineStays() throws {
        let v = try vault()
        let usage = UsageStore(vault: v)
        usage.recordSearch(hits: [SearchHit(path: "projects/dawn.md", line: old, text: "", score: 1)])
        #expect(!Forgetting.fades(old, in: "projects/dawn.md", usage: usage, now: now))
        usage.pin(line: old, in: "projects/dawn/hosting.md")
        #expect(usage.isPinned(line: old, in: "projects/dawn/hosting.md"))
        #expect(!Forgetting.fades(old, in: "projects/dawn/hosting.md", usage: usage, now: now))
        usage.unpin(line: old, in: "projects/dawn/hosting.md")
        #expect(Forgetting.fades(old, in: "projects/dawn/hosting.md", usage: usage, now: now))
    }

    @Test func fadingSparesMeAccessAndUndated() throws {
        let usage = UsageStore(vault: try vault())
        #expect(!Forgetting.fades(old, in: "me.md", usage: usage, now: now))
        #expect(!Forgetting.fades(old, in: "access/ssh.md", usage: usage, now: now))
        #expect(!Forgetting.fades("Undated line", in: "projects/dawn.md", usage: usage, now: now))
    }

    @Test func cardLeavesFadedLinesOut() throws {
        let v = try vault()
        let card = CardBuilder.buildIncluded(vault: v, repo: GitRepo(root: URL(fileURLWithPath: "/x/dawn"), remote: nil), now: now).card
        // me.md has the same old line and never fades, so look only at the project and its topics.
        let project = try #require(card.components(separatedBy: "PROJECT dawn:").last)
        #expect(!project.contains("Netlify"))
        #expect(project.contains("Fly.io"))
        #expect(card.components(separatedBy: "PROJECT dawn:").first?.contains("Netlify") == true)
    }
}

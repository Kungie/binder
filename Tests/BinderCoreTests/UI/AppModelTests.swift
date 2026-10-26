import Foundation
import Testing
@testable import BinderCore
@testable import BinderUI

private func proposal(_ text: String, target: String = "projects/dawn.md") -> Proposal {
    Proposal.make(kind: .decision, source: .command, text: text, evidence: "wrangler pages deploy", target: target,
                  project: "dawn", session: "s1", confidence: .high)
}

/// Waits (yielding the main actor) until `condition` holds, up to `seconds`.
@MainActor
func eventually(_ seconds: Double = 3, _ condition: () -> Bool) async -> Bool {
    let end = Date().addingTimeInterval(seconds)
    while Date() < end {
        if condition() { return true }
        try? await Task.sleep(nanoseconds: 20_000_000)
    }
    return condition()
}

@MainActor
@Suite(.serialized) struct AppModelTests {
    let vault = Vault(root: makeTempDir("vault"))
    let home = makeTempDir("home")

    init() {
        AppModel.timeScale = 0.02
        try? vault.ensureLayout()
    }

    @Test func refreshLoadsPendingAndSession() throws {
        try ProposalStore(vault: vault).add(proposal("The dawn site deploys to Cloudflare Pages."))
        try SessionStatus(session: "s1", project: "dawn", cwd: "/x/dawn", reading: ContextReading(used: 142_000, window: 200_000), updatedAt: Date()).save(vault)
        try Memory.add("Licence is AGPL-3.0", to: "projects/dawn.md", vault: vault)
        let model = AppModel(vault: vault, home: home)
        model.refresh()
        #expect(model.proposals.map(\.text) == ["The dawn site deploys to Cloudflare Pages."])
        #expect(model.session?.project == "dawn")
        #expect(model.memories == 1)
        #expect(model.pendingCount == 1)
    }

    @Test func keepAppendsDecisionAfterAbsorb() async throws {
        let p = proposal("The dawn site deploys to Cloudflare Pages.")
        try ProposalStore(vault: vault).add(p)
        let model = AppModel(vault: vault, home: home)
        model.refresh()
        model.keep(p.id)
        #expect(model.phases[p.id] == .absorbing)
        #expect(model.pendingCount == 0)
        #expect(await eventually { model.proposals.isEmpty })
        #expect(NoteIO.read(vault.url("projects/dawn.md"))?.decisions.first?.contains("The dawn site deploys to Cloudflare Pages.") == true)
        #expect(model.memories == 1)
        #expect(model.phases[p.id] == nil)
    }

    @Test func dropRemembersAfterShred() async throws {
        let p = proposal("Let's try Tailwind for a minute.")
        try ProposalStore(vault: vault).add(p)
        let model = AppModel(vault: vault, home: home)
        model.refresh()
        model.drop(p.id)
        #expect(model.phases[p.id] == .shredding)
        #expect(await eventually { model.proposals.isEmpty })
        #expect(try !ProposalStore(vault: vault).add(proposal("Let's try Tailwind for a minute.")))
    }

    @Test func keepAllStaggers() async throws {
        for text in ["One.", "Two.", "Three."] { try ProposalStore(vault: vault).add(proposal(text)) }
        let model = AppModel(vault: vault, home: home)
        model.refresh()
        model.keepAll()
        #expect(model.phases.values.filter { $0 == .absorbing }.count == 1)
        #expect(await eventually { model.proposals.isEmpty })
        #expect(NoteIO.read(vault.url("projects/dawn.md"))?.decisions.count == 3)
    }

    @Test func editThenKeepUsesNewText() async throws {
        let p = proposal("Release notes go in docs.")
        try ProposalStore(vault: vault).add(p)
        let model = AppModel(vault: vault, home: home)
        model.refresh()
        model.edit(p.id)
        #expect(model.phases[p.id] == .editing("Release notes go in docs."))
        model.phases[p.id] = .editing("Release notes live in docs/releases.")
        model.keep(p.id)
        #expect(await eventually { model.proposals.isEmpty })
        #expect(NoteIO.read(vault.url("projects/dawn.md"))?.decisions.first?.contains("docs/releases") == true)
    }

    @Test func keepOfVanishedProposalIsIgnored() async throws {
        let p = proposal("Gone before the animation ends.")
        try ProposalStore(vault: vault).add(p)
        let model = AppModel(vault: vault, home: home)
        model.refresh()
        model.keep(p.id)
        try FileManager.default.removeItem(at: vault.inbox.appendingPathComponent("\(p.id).json"))
        #expect(await eventually { model.phases[p.id] == nil })
        #expect(model.proposals.isEmpty)
        #expect(NoteIO.read(vault.url("projects/dawn.md")) == nil)
        model.keep("p_nothere")
        #expect(model.phases["p_nothere"] == nil)
    }

    @Test func inboxChangeRefreshes() async throws {
        let model = AppModel(vault: vault, home: home)
        model.refresh()
        model.startWatching()
        defer { model.stopWatching() }
        try ProposalStore(vault: vault).add(proposal("Arrived while the panel was open."))
        #expect(await eventually { model.proposals.count == 1 })
    }

    @Test func staleSessionIsHidden() throws {
        try SessionStatus(session: "old", project: "dawn", cwd: "/x", reading: ContextReading(used: 150_000, window: 200_000),
                          updatedAt: Date().addingTimeInterval(-3 * 3600)).save(vault)
        let model = AppModel(vault: vault, home: home)
        model.refresh()
        #expect(model.session == nil)
        #expect(!model.handoffSuggested)
    }

    @Test func sessionWithoutReadingShowsNoMeter() throws {
        try SessionStatus(session: "new", project: "dawn", cwd: "/x", reading: nil, updatedAt: Date()).save(vault)
        let model = AppModel(vault: vault, home: home)
        model.refresh()
        #expect(model.session?.session == "new")
        #expect(model.meterFraction == nil)
        #expect(!model.handoffSuggested)
    }

    @Test func handoffSuggestedAtThreshold() throws {
        try SessionStatus(session: "s", project: "dawn", cwd: "/x", reading: ContextReading(used: 139_000, window: 200_000), updatedAt: Date()).save(vault)
        let model = AppModel(vault: vault, home: home)
        model.refresh()
        #expect(!model.handoffSuggested)
        try SessionStatus(session: "s", project: "dawn", cwd: "/x", reading: ContextReading(used: 140_000, window: 200_000), updatedAt: Date()).save(vault)
        model.refresh()
        #expect(model.handoffSuggested)
    }
}

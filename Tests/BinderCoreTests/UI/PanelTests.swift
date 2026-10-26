import AppKit
import SwiftUI
import Testing
@testable import BinderCore
@testable import BinderUI

@MainActor
@Suite(.serialized) struct PanelTests {
    @Test func iconStatesAreTemplates18pt() {
        for state in [MenuState.idle, .review(3), .handoffSoon, .observing] {
            let image = MenuBarIcon.image(state: state)
            #expect(image.isTemplate, "\(state)")
            #expect(image.size == NSSize(width: 18, height: 18), "\(state)")
        }
        #expect(MenuBarIcon.title(state: .review(3))?.string == "3")
        #expect(MenuBarIcon.title(state: .idle) == nil)
    }

    @Test func iconWithoutBrainIsStillATemplate() {
        let image = MenuBarIcon.image(state: .review(2), brain: false)
        #expect(image.isTemplate)
        #expect(image.size == MenuBarIcon.size)
    }

    @Test func brainLightsWhileThereIsSomethingToReview() {
        #expect(BrainGlow.decide(previous: nil, current: 0) == (lit: false, sparkle: false))
        #expect(BrainGlow.decide(previous: 2, current: 0) == (lit: false, sparkle: false))
        #expect(BrainGlow.decide(previous: 3, current: 2) == (lit: true, sparkle: false))
    }

    @Test func brainSparklesOnlyWhenNewContextArrives() {
        #expect(BrainGlow.decide(previous: nil, current: 4) == (lit: true, sparkle: false))
        #expect(BrainGlow.decide(previous: 0, current: 1) == (lit: true, sparkle: true))
        #expect(BrainGlow.decide(previous: 2, current: 3) == (lit: true, sparkle: true))
        #expect(BrainGlow.decide(previous: 3, current: 3) == (lit: true, sparkle: false))
    }

    @Test func titlebarHeightDefaultsToZero() {
        #expect(EnvironmentValues().titlebarHeight == 0)
    }

    @Test func tagTextForEachSource() {
        var p = Proposal.make(kind: .decision, source: .command, text: "x", evidence: "wrangler pages deploy", target: "projects/a.md", project: "a", session: nil)
        #expect(ProposalCard.tag(for: p) == "FROM A COMMAND · wrangler pages deploy")
        p.source = .observed
        p.evidence = "~/.ssh/config"
        #expect(ProposalCard.tag(for: p) == "SEEN ON THIS MAC · ~/.ssh/config")
        p.source = .chat
        #expect(ProposalCard.tag(for: p) == "FROM THE CHAT")
        p.confidence = .low
        #expect(ProposalCard.tag(for: p) == "FROM THE CHAT · PROBABLY A ONE-OFF")
    }

    @Test func shredDelaysMatchDesign() {
        let expected: [Double] = [0.12, 0.295, 0.19, 0.365, 0.26, 0.155, 0.33, 0.225]
        for (got, want) in zip(Shred.delays, expected) { #expect(abs(got - want) < 0.0001) }
        #expect(Shred.rotations == [-4, 7, -10, 4, -7, 10, -4, 7])
    }

    /// The panel's natural height with `count` catches waiting.
    func panelHeight(_ count: Int) throws -> CGFloat {
        let vault = Vault(root: makeTempDir("vault"))
        try vault.ensureLayout()
        for i in 0..<count {
            try ProposalStore(vault: vault).add(Proposal.make(kind: .decision, source: .command, text: "Catch number \(i) from this session.",
                                                              evidence: "wrangler pages deploy", target: "projects/dawn.md", project: "dawn", session: "s1"))
        }
        let model = AppModel(vault: vault, home: makeTempDir("home"))
        model.refresh()
        let host = NSHostingController(rootView: PanelContainer(model: model))
        host.view.layoutSubtreeIfNeeded()
        host.view.layoutSubtreeIfNeeded()
        return host.view.fittingSize.height
    }

    @Test func moreThanTenCatchesScrollInsteadOfGrowing() throws {
        let ten = try panelHeight(10), fifteen = try panelHeight(15), five = try panelHeight(5)
        #expect(five < ten)
        #expect(fifteen < ten * 1.15, "15 catches: \(fifteen) pt, 10 catches: \(ten) pt")
    }

    @Test func panelSnapshotsRender() throws {
        for kind in [DemoVault.Kind.review, .empty, .handoff, .edit] {
            let model = DemoVault.model(kind)
            let image = try #require(Snapshot.image(PanelContainer(model: model), size: CGSize(width: PanelView.width, height: 760)), "\(kind)")
            let rep = try #require(NSBitmapImageRep(data: image.tiffRepresentation!))
            #expect(Snapshot.hasInk(rep), "\(kind)")
        }
    }
}

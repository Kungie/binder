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
        #expect(MenuBarIcon.title(state: .review(3))?.string == " 3")
        #expect(MenuBarIcon.title(state: .idle) == nil)
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

    @Test func panelSnapshotsRender() throws {
        for kind in [DemoVault.Kind.review, .empty, .handoff, .edit] {
            let model = DemoVault.model(kind)
            let image = try #require(Snapshot.image(PanelContainer(model: model), size: CGSize(width: PanelView.width, height: 760)), "\(kind)")
            let rep = try #require(NSBitmapImageRep(data: image.tiffRepresentation!))
            #expect(Snapshot.hasInk(rep), "\(kind)")
        }
    }
}

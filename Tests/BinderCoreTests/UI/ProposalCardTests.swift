import Foundation
import Testing
@testable import BinderCore
@testable import BinderUI

@Suite struct ProposalCardTests {
    func proposal(_ target: String, also: [String] = []) -> Proposal {
        Proposal.make(kind: .fact, source: .chat, text: "t", evidence: "e", target: target, project: "dawn", session: "s", also: also)
    }

    @Test func destinationLabels() {
        #expect(ProposalCard.destination(for: proposal("projects/dawn.md")) == "→ dawn")
        #expect(ProposalCard.destination(for: proposal("projects/dawn/hosting.md")) == "→ dawn · hosting")
        #expect(ProposalCard.destination(for: proposal("me.md")) == "→ me")
        #expect(ProposalCard.destination(for: proposal("servers/hetzner-1.md")) == "→ hetzner-1")
        #expect(ProposalCard.destination(for: proposal("projects/dawn/hosting.md", also: ["servers/hetzner-1.md", "hosting/cloudflare.md"]))
                == "→ dawn · hosting + hetzner-1 + cloudflare")
    }

    @Test func stateWithoutDetailedKeyIsSummary() throws {
        let state = try JSONDecoder().decode(BinderState.self, from: Data(#"{"summarizerConsent":true}"#.utf8))
        #expect(state.detailedCatches == false)
        var on = BinderState()
        on.detailedCatches = true
        let round = try JSONDecoder().decode(BinderState.self, from: JSONEncoder().encode(on))
        #expect(round.detailedCatches)
    }
}

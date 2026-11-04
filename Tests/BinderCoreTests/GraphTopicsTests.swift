import Foundation
import Testing
@testable import BinderCore

@Suite struct GraphTopicsTests {
    func vault() -> Vault {
        let root = makeTempDir("graph-topics")
        write("# Me\n", to: root.appendingPathComponent("me.md"))
        write("# dawn\n\nSee [[dawn/hosting]] and [[Dawn/API]].\n", to: root.appendingPathComponent("projects/dawn.md"))
        write("# hosting\n\n## Decisions\n- 2026-11-04 · Runs on Fly.io · kept from the chat\n", to: root.appendingPathComponent("projects/dawn/hosting.md"))
        write("# api\n", to: root.appendingPathComponent("projects/dawn/api.md"))
        write("# cloudflare\n", to: root.appendingPathComponent("hosting/cloudflare.md"))
        return Vault(root: root)
    }

    @Test func topicNodesHangOffTheirProject() {
        let g = VaultGraph.build(vault: vault())
        #expect(g.node("projects/dawn/hosting.md")?.kind == .note)
        #expect(g.node("projects/dawn/hosting.md")?.label == "hosting")
        #expect(g.edge("projects/dawn.md", "projects/dawn/hosting.md") != nil)
        #expect(g.edge("folder:projects", "projects/dawn/hosting.md") == nil)
    }

    @Test func topicWikilinkResolves() {
        let g = VaultGraph.build(vault: vault())
        #expect(!g.nodes.contains { $0.kind == .missing })
        #expect(g.edge("projects/dawn.md", "projects/dawn/api.md") != nil)
    }

    @Test func hostingFolderNode() {
        let g = VaultGraph.build(vault: vault())
        #expect(g.edge("me", "folder:hosting")?.kind == .structure)
        #expect(g.edge("folder:hosting", "hosting/cloudflare.md")?.kind == .structure)
    }
}

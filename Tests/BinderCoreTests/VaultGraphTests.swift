import Foundation
import Testing
@testable import BinderCore

@Suite struct VaultGraphTests {
    /// Me, one machine, three projects (two share Swift and GitHub Actions), a handoff for dawn, and no servers.
    func vault() -> Vault {
        let root = makeTempDir("graph")
        write("# Me\n\n## Decisions\n- 2026-10-26 · Short answers · written by you\n", to: root.appendingPathComponent("me.md"))
        write("# This Mac\n", to: root.appendingPathComponent("machines/this-mac.md"))
        write("""
        # dawn

        <!-- binder:observed start -->
        - repo: github.com/kungie/dawn
        - languages: Swift, Python
        - deploy: GitHub Actions
        <!-- binder:observed end -->

        Builds on [[this-mac]], site notes in [[site]].
        """, to: root.appendingPathComponent("projects/dawn.md"))
        write("""
        # Binder

        <!-- binder:observed start -->
        - languages: Swift
        - deploy: GitHub Actions
        <!-- binder:observed end -->
        """, to: root.appendingPathComponent("projects/binder.md"))
        write("# gut\n\n<!-- binder:observed start -->\n- languages: Rust\n<!-- binder:observed end -->\n", to: root.appendingPathComponent("projects/gut.md"))
        write("# Handoff · dawn\n", to: root.appendingPathComponent("handoffs/dawn/2026-10-26-1200.md"))
        try! FileManager.default.createDirectory(at: root.appendingPathComponent("servers"), withIntermediateDirectories: true)
        return Vault(root: root)
    }

    @Test func meFoldersAndNotesFormTheSpine() {
        let g = VaultGraph.build(vault: vault())
        #expect(g.node("me")?.path == "me.md")
        #expect(g.node("me")?.kind == .me)
        #expect(g.edge("me", "folder:projects")?.kind == .structure)
        #expect(g.edge("folder:projects", "projects/dawn.md")?.kind == .structure)
        #expect(g.edge("folder:machines", "machines/this-mac.md")?.kind == .structure)
        #expect(g.node("folder:servers") == nil)
        #expect(g.node("projects/binder.md")?.label == "Binder")
        #expect(g.node("folder:projects")?.label == "PROJECTS")
    }

    @Test func wikilinksConnectNotesAndKeepMissingOnesAsGhosts() {
        let g = VaultGraph.build(vault: vault())
        #expect(g.edge("projects/dawn.md", "machines/this-mac.md")?.kind == .link)
        #expect(g.node("missing:site")?.kind == .missing)
        #expect(g.edge("projects/dawn.md", "missing:site")?.kind == .link)
    }

    @Test func handoffsHangOffTheirProject() {
        let g = VaultGraph.build(vault: vault())
        let handoff = "handoffs/dawn/2026-10-26-1200.md"
        #expect(g.node(handoff)?.label == "dawn · 2026-10-26-1200")
        #expect(g.edge(handoff, "projects/dawn.md")?.kind == .handoff)
        #expect(g.edge("folder:handoffs", handoff)?.kind == .structure)
    }

    @Test func sharedFactsBecomeFacetsThatLinkProjects() {
        let g = VaultGraph.build(vault: vault())
        #expect(g.node("facet:swift")?.label == "Swift")
        #expect(g.edge("facet:swift", "projects/dawn.md")?.kind == .facet)
        #expect(g.edge("facet:swift", "projects/binder.md")?.kind == .facet)
        #expect(g.edge("facet:github actions", "projects/binder.md")?.kind == .facet)
        #expect(g.node("facet:rust") == nil)
        #expect(g.node("facet:python") == nil)
    }

    @Test func edgesAreUndirectedAndUnique() {
        let g = VaultGraph.build(vault: vault())
        #expect(g.edge("projects/dawn.md", "folder:projects") != nil)
        let keys = g.edges.map { [$0.a, $0.b] }
        #expect(Set(keys).count == keys.count)
        #expect(g.edges.allSatisfy { $0.a < $0.b })
        #expect(g.edges.allSatisfy { g.node($0.a) != nil && g.node($0.b) != nil })
    }

    @Test func emptyVaultIsJustMe() {
        let g = VaultGraph.build(vault: Vault(root: makeTempDir("empty")))
        #expect(g.nodes.map(\.id) == ["me"])
        #expect(g.edges.isEmpty)
    }
}

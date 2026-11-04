import Foundation
import Testing
@testable import BinderCore
@testable import BinderUI

@MainActor
@Suite struct GraphTidyTests {
    /// Six projects, two with eight topics each, like a binder after Split by topic.
    func graph() -> VaultGraph {
        let root = makeTempDir("tidy-graph")
        write("# Me\n", to: root.appendingPathComponent("me.md"))
        for p in ["a", "b", "c", "d", "e", "f"] { write("# \(p)\n", to: root.appendingPathComponent("projects/\(p).md")) }
        for p in ["a", "b"] {
            for t in ["api", "auth", "data", "design", "docs", "hosting", "release", "ui"] {
                write("# \(t)\n", to: root.appendingPathComponent("projects/\(p)/\(t).md"))
            }
        }
        return VaultGraph.build(vault: Vault(root: root))
    }

    func distance(_ l: GraphLayout, _ a: String, _ b: String) -> Double {
        let p = l.position(a)!, q = l.position(b)!
        return hypot(p.x - q.x, p.y - q.y)
    }

    @Test func topicsStartNextToTheirProject() {
        let l = GraphLayout(graph: graph())
        #expect(distance(l, "projects/a/api.md", "projects/a.md") < 90)
        #expect(distance(l, "projects/b/ui.md", "projects/b.md") < 90)
    }

    @Test func topicsStayAroundTheirProjectOnceSettled() {
        let l = GraphLayout(graph: graph())
        for _ in 0..<700 { l.step() }
        for t in ["api", "auth", "data", "design", "docs", "hosting", "release", "ui"] {
            let own = distance(l, "projects/a/\(t).md", "projects/a.md")
            let other = distance(l, "projects/a/\(t).md", "projects/b.md")
            #expect(own < other, "\(t)")
            #expect(own < 120, "\(t)")
        }
    }

    @Test func nothingSitsOnTopOfAnything() {
        let l = GraphLayout(graph: graph())
        for _ in 0..<700 { l.step() }
        let ids = l.nodes.map(\.id)
        var closest = Double.infinity
        for i in ids.indices { for j in ids.indices where j > i { closest = min(closest, distance(l, ids[i], ids[j])) } }
        #expect(closest > 22)
    }

    @Test func topicLabelsWaitForAHoverOrAZoom() {
        #expect(GraphLayout.isTopic("projects/a/api.md"))
        #expect(!GraphLayout.isTopic("projects/a.md"))
        #expect(GraphView.showsTopicLabel(scale: 1, highlighted: false) == false)
        #expect(GraphView.showsTopicLabel(scale: 1.3, highlighted: false) == false)
        #expect(GraphView.showsTopicLabel(scale: 2, highlighted: false))
        #expect(GraphView.showsTopicLabel(scale: 1, highlighted: true))
    }
}

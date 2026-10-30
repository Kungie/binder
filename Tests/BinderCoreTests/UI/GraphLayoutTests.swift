import Foundation
import Testing
@testable import BinderCore
@testable import BinderUI

@MainActor
@Suite struct GraphLayoutTests {
    /// Me, two folders with three notes each, and one link across.
    func graph() -> VaultGraph {
        let root = makeTempDir("layout")
        write("# Me\n", to: root.appendingPathComponent("me.md"))
        for name in ["a", "b", "c"] { write("# \(name)\n", to: root.appendingPathComponent("projects/\(name).md")) }
        for name in ["x", "y"] { write("# \(name)\n", to: root.appendingPathComponent("servers/\(name).md")) }
        write("# z\n\nRuns on [[a]].\n", to: root.appendingPathComponent("servers/z.md"))
        return VaultGraph.build(vault: Vault(root: root))
    }

    func distance(_ l: GraphLayout, _ a: String, _ b: String) -> Double {
        let p = l.position(a)!, q = l.position(b)!
        return hypot(p.x - q.x, p.y - q.y)
    }

    @Test func startsDeterministic() {
        let one = GraphLayout(graph: graph()), two = GraphLayout(graph: graph())
        #expect(one.position("projects/b.md") == two.position("projects/b.md"))
        #expect(one.position("me") == .zero)
    }

    @Test func settlesWithConnectedNotesCloserThanStrangers() {
        let l = GraphLayout(graph: graph())
        for _ in 0..<600 { l.step() }
        #expect(l.settled)
        #expect(l.nodes.allSatisfy { $0.position.x.isFinite && $0.position.y.isFinite })
        #expect(distance(l, "folder:projects", "projects/b.md") < distance(l, "projects/b.md", "servers/x.md"))
        #expect(distance(l, "projects/a.md", "servers/z.md") < distance(l, "projects/b.md", "servers/x.md"))
        #expect(l.nodes.allSatisfy { hypot($0.position.x, $0.position.y) < 1200 })
    }

    @Test func noTwoNodesEndOnTopOfEachOther() {
        let l = GraphLayout(graph: graph())
        for _ in 0..<600 { l.step() }
        for (i, a) in l.nodes.enumerated() {
            for b in l.nodes[(i + 1)...] {
                #expect(hypot(a.position.x - b.position.x, a.position.y - b.position.y) > 12, "\(a.id) and \(b.id)")
            }
        }
    }

    @Test func stopsMovingOnceItCools() {
        let l = GraphLayout(graph: graph())
        #expect(l.moving)
        for _ in 0..<600 { l.step() }
        #expect(!l.moving)
    }

    @Test func anyTouchWakesASettledLayout() {
        func cooled() -> GraphLayout {
            let l = GraphLayout(graph: graph())
            for _ in 0..<600 { l.step() }
            return l
        }
        let held = cooled()
        held.hold("projects/a.md", at: .zero)
        #expect(held.moving)
        held.release("projects/a.md")
        #expect(held.moving)

        let reheated = cooled()
        reheated.reheat()
        #expect(reheated.moving)

        let grown = cooled()
        let root = makeTempDir("layout-grown")
        write("# Me\n", to: root.appendingPathComponent("me.md"))
        write("# new\n", to: root.appendingPathComponent("projects/new.md"))
        grown.update(VaultGraph.build(vault: Vault(root: root)))
        #expect(grown.moving)
    }

    @Test func aDraggedNodeStaysWhereItIsHeld() {
        let l = GraphLayout(graph: graph())
        for _ in 0..<100 { l.step() }
        l.hold("projects/a.md", at: CGPoint(x: 300, y: -200))
        for _ in 0..<50 { l.step() }
        #expect(l.position("projects/a.md") == CGPoint(x: 300, y: -200))
        #expect(!l.settled)
        l.release("projects/a.md")
        l.step()
        #expect(l.position("projects/a.md") != CGPoint(x: 300, y: -200))
    }

    @Test func updateKeepsPositionsOfNotesThatStay() {
        let l = GraphLayout(graph: graph())
        for _ in 0..<200 { l.step() }
        let before = l.position("servers/x.md")
        l.update(graph())
        #expect(l.position("servers/x.md") == before)
    }

    @Test func hitTestFindsTheNearestNodeWithinReach() {
        let l = GraphLayout(graph: graph())
        let p = l.position("projects/c.md")!
        #expect(l.node(near: CGPoint(x: p.x + 3, y: p.y - 2), within: 10)?.id == "projects/c.md")
        #expect(l.node(near: CGPoint(x: 5000, y: 5000), within: 10) == nil)
    }
}

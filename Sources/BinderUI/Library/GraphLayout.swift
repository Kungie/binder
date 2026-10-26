import BinderCore
import CoreGraphics
import Foundation

/// A small force-directed layout for the graph: nodes push each other apart, links pull like springs,
/// a weak pull keeps it all centred, and the whole thing cools down and stops, like d3-force.
/// World coordinates, with you at the origin.
@MainActor
final class GraphLayout: ObservableObject {
    struct Node {
        var id: String
        var label: String
        var kind: GraphNode.Kind
        var path: String?
        var position: CGPoint
        var velocity = CGVector.zero
        var degree = 0
    }

    struct Link {
        var a: Int
        var b: Int
        var kind: GraphEdge.Kind
    }

    private(set) var nodes: [Node] = []
    private(set) var links: [Link] = []
    private(set) var graph = VaultGraph()
    private var index: [String: Int] = [:]
    private var held: [String: CGPoint] = [:]
    /// How hot the simulation is; it stops moving below `restAlpha`.
    private(set) var alpha: Double = 1
    static let restAlpha = 0.01
    static let cooling = 0.99

    init(graph: VaultGraph) { update(graph) }

    var settled: Bool { alpha < Self.restAlpha && held.isEmpty }

    func position(_ id: String) -> CGPoint? { index[id].map { nodes[$0].position } }

    func node(_ id: String) -> Node? { index[id].map { nodes[$0] } }

    /// Swaps in a fresh graph, keeping where every surviving node was.
    func update(_ graph: VaultGraph) {
        let old = Dictionary(uniqueKeysWithValues: nodes.map { ($0.id, $0) })
        self.graph = graph
        let seeds = Self.seed(graph)
        nodes = graph.nodes.map { n in
            var node = Node(id: n.id, label: n.label, kind: n.kind, path: n.path, position: old[n.id]?.position ?? seeds[n.id] ?? .zero)
            node.velocity = old[n.id]?.velocity ?? .zero
            return node
        }
        index = Dictionary(uniqueKeysWithValues: nodes.enumerated().map { ($1.id, $0) })
        links = graph.edges.compactMap { e in
            guard let a = index[e.a], let b = index[e.b] else { return nil }
            return Link(a: a, b: b, kind: e.kind)
        }
        for link in links {
            nodes[link.a].degree += 1
            nodes[link.b].degree += 1
        }
        held = held.filter { index[$0.key] != nil }
        if old.count != nodes.count || old.keys.contains(where: { index[$0] == nil }) { alpha = max(alpha, 0.6) }
    }

    /// Where everything starts: you in the middle, folders in a ring, their notes fanned out behind them,
    /// facets and missing notes on an outer ring. Deterministic, so snapshots and tests repeat.
    static func seed(_ graph: VaultGraph) -> [String: CGPoint] {
        var out: [String: CGPoint] = ["me": .zero]
        let folders = graph.nodes.filter { $0.kind == .folder }
        for (i, folder) in folders.enumerated() {
            let angle = Double(i) / Double(max(folders.count, 1)) * 2 * .pi - .pi / 2
            out[folder.id] = CGPoint(x: cos(angle) * 160, y: sin(angle) * 160)
            let children = graph.edges.filter { $0.kind == .structure && ($0.a == folder.id || $0.b == folder.id) }
                .map { $0.a == folder.id ? $0.b : $0.a }.filter { $0 != "me" }
            for (j, child) in children.enumerated() {
                let spread = Double(j) - Double(children.count - 1) / 2
                let a = angle + spread * min(0.22, 2.4 / Double(max(children.count, 1)))
                let r = 160 + 90 + Double(j % 3) * 22
                out[child] = CGPoint(x: cos(a) * r, y: sin(a) * r)
            }
        }
        let loose = graph.nodes.filter { out[$0.id] == nil }
        for (i, node) in loose.enumerated() {
            let angle = Double(i) / Double(max(loose.count, 1)) * 2 * .pi + 0.3
            out[node.id] = CGPoint(x: cos(angle) * 300, y: sin(angle) * 300)
        }
        return out
    }

    static func charge(_ kind: GraphNode.Kind) -> Double {
        switch kind {
        case .me: 2600
        case .folder: 1100
        case .note: 650
        case .facet: 420
        case .missing: 320
        }
    }

    static func rest(_ link: Link, _ nodes: [Node]) -> Double {
        switch link.kind {
        case .structure: nodes[link.a].kind == .me || nodes[link.b].kind == .me ? 150 : 72
        case .link: 95
        case .handoff: 60
        case .facet: 170
        }
    }

    /// How hard each kind of link pulls. Shared facts pull gently, so they tie projects together without
    /// dragging them all into one knot.
    static func stiffness(_ kind: GraphEdge.Kind) -> Double {
        switch kind {
        case .structure: 0.09
        case .link: 0.07
        case .handoff: 0.08
        case .facet: 0.018
        }
    }

    /// How close two nodes may get before they shove each other apart, whatever the temperature.
    static let personalSpace: Double = 30

    func hold(_ id: String, at point: CGPoint) {
        guard let i = index[id] else { return }
        held[id] = point
        nodes[i].position = point
        nodes[i].velocity = .zero
        alpha = max(alpha, 0.3)
    }

    func release(_ id: String) {
        held[id] = nil
        alpha = max(alpha, 0.3)
    }

    /// Wakes the layout up again (after a drag, or new notes).
    func reheat(_ to: Double = 0.4) { alpha = max(alpha, to) }

    func step() {
        guard !settled, !nodes.isEmpty else { return }
        let n = nodes.count
        var force = [CGVector](repeating: .zero, count: n)
        // Everything pushes everything else away, harder up close.
        for i in 0..<n {
            for j in (i + 1)..<n {
                var dx = nodes[j].position.x - nodes[i].position.x
                var dy = nodes[j].position.y - nodes[i].position.y
                if dx == 0 && dy == 0 { dx = Double(j - i) * 0.37; dy = Double(i + 1) * 0.29 }
                let d2 = max(dx * dx + dy * dy, 36)
                let d = sqrt(d2)
                let push = (Self.charge(nodes[i].kind) + Self.charge(nodes[j].kind)) / 2 / d2 * alpha
                force[i].dx -= dx / d * push; force[i].dy -= dy / d * push
                force[j].dx += dx / d * push; force[j].dy += dy / d * push
                if d < Self.personalSpace {
                    let shove = (Self.personalSpace - d) / d * 0.25
                    force[i].dx -= dx * shove; force[i].dy -= dy * shove
                    force[j].dx += dx * shove; force[j].dy += dy * shove
                }
            }
        }
        // Links pull to their resting length.
        for link in links {
            let p = nodes[link.a].position, q = nodes[link.b].position
            let dx = q.x - p.x, dy = q.y - p.y
            let d = max(sqrt(dx * dx + dy * dy), 0.01)
            let pull = (d - Self.rest(link, nodes)) / d * Self.stiffness(link.kind) * alpha
            force[link.a].dx += dx * pull; force[link.a].dy += dy * pull
            force[link.b].dx -= dx * pull; force[link.b].dy -= dy * pull
        }
        for i in 0..<n {
            if let point = held[nodes[i].id] {
                nodes[i].position = point
                nodes[i].velocity = .zero
                continue
            }
            if nodes[i].kind == .me {
                // You stay put in the middle; everything else arranges itself around you.
                nodes[i].position = .zero
                continue
            }
            let p = nodes[i].position
            var v = nodes[i].velocity
            v.dx = (v.dx + force[i].dx - p.x * 0.006 * alpha) * 0.6
            v.dy = (v.dy + force[i].dy - p.y * 0.006 * alpha) * 0.6
            nodes[i].velocity = v
            nodes[i].position = CGPoint(x: p.x + v.dx, y: p.y + v.dy)
        }
        alpha *= Self.cooling
        if !held.isEmpty { alpha = max(alpha, 0.3) }
    }

    /// The nearest node to `point` (world coordinates) within `reach`.
    func node(near point: CGPoint, within reach: Double) -> Node? {
        var best: (Node, Double)?
        for node in nodes {
            let d = hypot(node.position.x - point.x, node.position.y - point.y)
            if d <= reach + radius(node), d < (best?.1 ?? .infinity) { best = (node, d) }
        }
        return best?.0
    }

    /// How big a node draws, in world units.
    func radius(_ node: Node) -> Double {
        switch node.kind {
        case .me: 15
        case .folder: 8
        case .note: 3.5 + min(Double(node.degree), 8) * 0.55
        case .facet: 3.5
        case .missing: 3
        }
    }

    /// The box around every node, for fitting the view.
    var bounds: CGRect {
        guard let first = nodes.first else { return .zero }
        var r = CGRect(origin: first.position, size: .zero)
        for node in nodes { r = r.union(CGRect(origin: node.position, size: .zero)) }
        return r
    }
}

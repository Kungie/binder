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
    /// How many topic notes each project overview has.
    private var topicCounts: [String: Int] = [:]
    /// How hot the simulation is; it stops moving below `restAlpha`.
    private(set) var alpha: Double = 1
    static let restAlpha = 0.01
    static let cooling = 0.99
    /// False once nothing moves, so the view can stop drawing.
    @Published private(set) var moving = true

    init(graph: VaultGraph) { update(graph) }

    var settled: Bool { alpha < Self.restAlpha && held.isEmpty }

    private func syncMoving() {
        if moving == settled { moving = !settled }
    }

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
        topicCounts = Dictionary(grouping: nodes.map(\.id).filter(Self.isTopic), by: Self.parent(ofTopic:)).mapValues(\.count)
        if old.count != nodes.count || old.keys.contains(where: { index[$0] == nil }) { alpha = max(alpha, 0.6) }
        syncMoving()
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
        // Topic notes fan out from their project, on the side away from you, instead of starting on the far ring.
        let topics = Dictionary(grouping: graph.nodes.filter { isTopic($0.id) }, by: { parent(ofTopic: $0.id) })
        for (project, children) in topics {
            guard let p = out[project] else { continue }
            let base = atan2(p.y, p.x)
            for (j, child) in children.sorted(by: { $0.id < $1.id }).enumerated() {
                let spread = Double(j) - Double(children.count - 1) / 2
                let a = base + spread * min(0.45, 2.6 / Double(children.count))
                let r = 55 + Double(j % 2) * 16
                out[child.id] = CGPoint(x: p.x + cos(a) * r, y: p.y + sin(a) * r)
            }
        }
        let loose = graph.nodes.filter { out[$0.id] == nil }
        for (i, node) in loose.enumerated() {
            let angle = Double(i) / Double(max(loose.count, 1)) * 2 * .pi + 0.3
            out[node.id] = CGPoint(x: cos(angle) * 300, y: sin(angle) * 300)
        }
        return out
    }

    /// A project's topic note, `projects/<project>/<topic>.md`.
    static func isTopic(_ id: String) -> Bool { id.hasPrefix("projects/") && id.split(separator: "/").count == 3 }

    /// The overview a topic note hangs off.
    static func parent(ofTopic id: String) -> String {
        let parts = id.split(separator: "/")
        return "projects/\(parts[1]).md"
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

    /// A topic and its project: a short, stiff link, so topics stay in a tight bunch around their project.
    static func isTopicLink(_ link: Link, _ nodes: [Node]) -> Bool {
        link.kind == .structure && (isTopic(nodes[link.a].id) || isTopic(nodes[link.b].id))
    }

    static func rest(_ link: Link, _ nodes: [Node]) -> Double {
        if isTopicLink(link, nodes) { return 50 }
        return switch link.kind {
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
        syncMoving()
        objectWillChange.send() // the drag draws with the pointer, not with the next step
    }

    func release(_ id: String) {
        held[id] = nil
        alpha = max(alpha, 0.3)
        syncMoving()
    }

    /// Wakes the layout up again (after a drag, or new notes).
    func reheat(_ to: Double = 0.4) {
        alpha = max(alpha, to)
        syncMoving()
    }

    func step() {
        guard !settled, !nodes.isEmpty else { return }
        let n = nodes.count
        var force = [CGVector](repeating: .zero, count: n)
        // Topics push less than notes; a project pushes more for each topic it has, so two busy projects keep
        // room for their bunches instead of overlapping.
        let charges = nodes.map { node in
            Self.isTopic(node.id) ? 380 : Self.charge(node.kind) + Double(topicCounts[node.id] ?? 0) * 260
        }
        // Everything pushes everything else away, harder up close.
        for i in 0..<n {
            for j in (i + 1)..<n {
                var dx = nodes[j].position.x - nodes[i].position.x
                var dy = nodes[j].position.y - nodes[i].position.y
                if dx == 0 && dy == 0 { dx = Double(j - i) * 0.37; dy = Double(i + 1) * 0.29 }
                let d2 = max(dx * dx + dy * dy, 36)
                let d = sqrt(d2)
                let push = (charges[i] + charges[j]) / 2 / d2 * alpha
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
            let stiffness = Self.isTopicLink(link, nodes) ? 0.16 : Self.stiffness(link.kind)
            let pull = (d - Self.rest(link, nodes)) / d * stiffness * alpha
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
        syncMoving()
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
    /// How many session cards each note was on, by path: busier notes draw a little bigger.
    var cards: [String: Int] = [:]

    static func useScale(cards: Int) -> Double { 1 + min(0.35, log1p(Double(cards)) / 12) }

    func radius(_ node: Node) -> Double {
        switch node.kind {
        case .me: 15
        case .folder: 8
        case .note: (3.5 + min(Double(node.degree), 8) * 0.55) * Self.useScale(cards: node.path.flatMap { cards[$0] } ?? 0)
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

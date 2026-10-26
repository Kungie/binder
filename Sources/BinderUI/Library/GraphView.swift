import AppKit
import BinderCore
import SwiftUI

/// The binder as a network on CRT glass: you in the middle as Binder's brain, folders around you, notes as
/// amber neurons, shared facts tying projects together, and now and then a signal running down a link.
/// Hover lights a note's neighbours, click opens it, drag moves a node or the whole view, pinch zooms.
struct GraphView: View {
    var open: (GraphLayout.Node) -> Void
    @StateObject private var layout: GraphLayout
    @StateObject private var camera = Local(Camera())
    @StateObject private var hovered = Local<String?>(nil)
    @StateObject private var drag = Local<DragState?>(nil)
    @StateObject private var zoomBase = Local<CGFloat?>(nil)
    @Environment(\.isSnapshot) private var isSnapshot
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    private let vault: Vault

    struct Camera {
        var pan: CGSize = .zero
        var scale: CGFloat = 1
        var fitted = false
    }

    enum DragState {
        case node(String)
        case pan(base: CGSize)
    }

    init(vault: Vault, open: @escaping (GraphLayout.Node) -> Void) {
        self.vault = vault
        self.open = open
        _layout = StateObject(wrappedValue: GraphLayout(graph: VaultGraph.build(vault: vault)))
    }

    var body: some View {
        GeometryReader { geo in
            ZStack(alignment: .topLeading) {
                ZStack {
                    Phosphor.black
                    RadialGradient(colors: [Phosphor.amber.opacity(0.13), .clear], center: .center, startRadius: 0, endRadius: max(geo.size.width, geo.size.height) * 0.55)
                }
                TimelineView(.animation(paused: isSnapshot)) { timeline in
                    Canvas { ctx, size in
                        if !isSnapshot { layout.step() }
                        draw(&ctx, size: size, time: isSnapshot ? 2.4 : timeline.date.timeIntervalSinceReferenceDate)
                    } symbols: {
                        MascotHead(size: 46, rimWidth: 7).tag(0)
                    }
                }
                .contentShape(Rectangle())
                .gesture(dragGesture(size: geo.size))
                .simultaneousGesture(zoomGesture)
                .onContinuousHover { phase in
                    switch phase {
                    case let .active(point):
                        let hit = layout.node(near: world(point, size: geo.size), within: 6 / camera.value.scale)?.id
                        if hit != hovered.value {
                            hovered.value = hit
                            (hit == nil ? NSCursor.arrow : NSCursor.pointingHand).set()
                        }
                    case .ended:
                        hovered.value = nil
                        NSCursor.arrow.set()
                    }
                }
                Scanlines(opacity: 0.32)
                hud(size: geo.size)
            }
            .clipped()
            .onAppear {
                layout.update(VaultGraph.build(vault: vault))
                guard !camera.value.fitted else { return }
                // Open nearly settled, then let it finish moving.
                for _ in 0..<(isSnapshot ? 700 : 260) { layout.step() }
                fit(geo.size)
            }
        }
    }

    // MARK: Coordinates

    func screen(_ p: CGPoint, size: CGSize) -> CGPoint {
        CGPoint(x: size.width / 2 + camera.value.pan.width + p.x * camera.value.scale,
                y: size.height / 2 + camera.value.pan.height + p.y * camera.value.scale)
    }

    func world(_ p: CGPoint, size: CGSize) -> CGPoint {
        CGPoint(x: (p.x - size.width / 2 - camera.value.pan.width) / camera.value.scale,
                y: (p.y - size.height / 2 - camera.value.pan.height) / camera.value.scale)
    }

    func fit(_ size: CGSize) {
        let b = layout.bounds
        guard size.width > 0, size.height > 0 else { return }
        let scale = min(size.width / max(b.width + 220, 1), size.height / max(b.height + 200, 1), 1.5)
        camera.value = Camera(pan: CGSize(width: -b.midX * scale, height: -b.midY * scale), scale: max(scale, 0.25), fitted: true)
    }

    // MARK: Gestures

    func dragGesture(size: CGSize) -> some Gesture {
        DragGesture(minimumDistance: 0)
            .onChanged { value in
                if drag.value == nil {
                    if let hit = layout.node(near: world(value.startLocation, size: size), within: 6 / camera.value.scale), hit.kind != .me {
                        drag.value = .node(hit.id)
                    } else {
                        drag.value = .pan(base: camera.value.pan)
                    }
                }
                switch drag.value {
                case let .node(id):
                    if hypot(value.translation.width, value.translation.height) > 3 { layout.hold(id, at: world(value.location, size: size)) }
                case let .pan(base):
                    camera.value.pan = CGSize(width: base.width + value.translation.width, height: base.height + value.translation.height)
                case nil:
                    break
                }
            }
            .onEnded { value in
                if case let .node(id) = drag.value { layout.release(id) }
                drag.value = nil
                // A press that barely moved is a click.
                if hypot(value.translation.width, value.translation.height) <= 3,
                   let hit = layout.node(near: world(value.startLocation, size: size), within: 6 / camera.value.scale) {
                    open(hit)
                }
            }
    }

    var zoomGesture: some Gesture {
        MagnifyGesture()
            .onChanged { value in
                let base = zoomBase.value ?? camera.value.scale
                zoomBase.value = base
                let scale = min(max(base * value.magnification, 0.25), 3)
                let ratio = scale / camera.value.scale
                camera.value.pan = CGSize(width: camera.value.pan.width * ratio, height: camera.value.pan.height * ratio)
                camera.value.scale = scale
            }
            .onEnded { _ in zoomBase.value = nil }
    }

    // MARK: Drawing

    func draw(_ ctx: inout GraphicsContext, size: CGSize, time: Double) {
        let nodes = layout.nodes
        let focus = hovered.value
        let near = focus.map { layout.graph.neighbours(of: $0).union([$0]) } ?? []
        let scale = camera.value.scale
        func lit(_ id: String) -> Bool { focus == nil || near.contains(id) }

        // Links: faint amber threads; the hovered note's light up.
        for link in layout.links {
            let a = nodes[link.a], b = nodes[link.b]
            let touches = focus != nil && (a.id == focus || b.id == focus)
            var path = Path()
            path.move(to: screen(a.position, size: size))
            path.addLine(to: screen(b.position, size: size))
            let base: Double = switch link.kind {
            case .structure: 0.22
            case .link: 0.34
            case .handoff: 0.26
            case .facet: 0.13
            }
            let opacity = touches ? 0.85 : (focus == nil ? base : base * 0.3)
            let style = link.kind == .facet ? StrokeStyle(lineWidth: 1, dash: [2, 4]) : StrokeStyle(lineWidth: touches ? 1.6 : 1)
            ctx.stroke(path, with: .color(Phosphor.amber.opacity(opacity)), style: style)
        }

        // Signals: every link fires now and then, a bright dot running from one end to the other.
        if !reduceMotion {
            for (k, link) in layout.links.enumerated() where link.kind != .facet {
                let h = Self.hash(k)
                let period = 4 + Double(h % 60) / 10
                let local = (time + Double(h % 997) / 97).truncatingRemainder(dividingBy: period)
                let run = 1.1
                guard local < run else { continue }
                let t = local / run
                let (from, to) = h % 2 == 0 ? (nodes[link.a], nodes[link.b]) : (nodes[link.b], nodes[link.a])
                let p = screen(CGPoint(x: from.position.x + (to.position.x - from.position.x) * t,
                                       y: from.position.y + (to.position.y - from.position.y) * t), size: size)
                let fade = sin(t * .pi) * (lit(from.id) || lit(to.id) ? 1 : 0.3)
                ctx.fill(Path(ellipseIn: CGRect(x: p.x - 7, y: p.y - 7, width: 14, height: 14)),
                         with: .radialGradient(Gradient(colors: [Phosphor.amber.opacity(0.55 * fade), .clear]), center: p, startRadius: 0, endRadius: 7))
                ctx.fill(Path(ellipseIn: CGRect(x: p.x - 1.6, y: p.y - 1.6, width: 3.2, height: 3.2)), with: .color(Phosphor.cream.opacity(fade)))
            }
        }

        // Nodes, then their labels on top.
        for (i, node) in nodes.enumerated() {
            let p = screen(node.position, size: size)
            let r = layout.radius(node) * min(max(scale, 0.6), 1.6)
            let on = lit(node.id)
            let dim = on ? 1.0 : 0.25
            let breathe = reduceMotion || isSnapshot ? 1 : 0.8 + 0.2 * sin(time * 1.3 + Double(Self.hash(i) % 628) / 100)
            switch node.kind {
            case .me:
                ctx.fill(Path(ellipseIn: CGRect(x: p.x - r * 4, y: p.y - r * 4, width: r * 8, height: r * 8)),
                         with: .radialGradient(Gradient(colors: [Phosphor.amber.opacity(0.4 * breathe), .clear]), center: p, startRadius: 0, endRadius: r * 4))
                if let head = ctx.resolveSymbol(id: 0) {
                    let s = min(max(scale, 0.6), 1.5)
                    var c = ctx
                    c.translateBy(x: p.x, y: p.y)
                    c.scaleBy(x: s, y: s)
                    c.draw(head, at: .zero)
                }
            case .folder:
                let circle = Path(ellipseIn: CGRect(x: p.x - r, y: p.y - r, width: r * 2, height: r * 2))
                ctx.fill(circle, with: .color(Phosphor.black))
                ctx.stroke(circle, with: .color(Phosphor.cream.opacity(0.9 * dim)), lineWidth: 1.6)
                ctx.fill(Path(ellipseIn: CGRect(x: p.x - r * 0.35, y: p.y - r * 0.35, width: r * 0.7, height: r * 0.7)), with: .color(Phosphor.amber.opacity(dim)))
            case .note:
                ctx.fill(Path(ellipseIn: CGRect(x: p.x - r * 3, y: p.y - r * 3, width: r * 6, height: r * 6)),
                         with: .radialGradient(Gradient(colors: [Phosphor.amber.opacity(0.35 * dim * breathe), .clear]), center: p, startRadius: 0, endRadius: r * 3))
                ctx.fill(Path(ellipseIn: CGRect(x: p.x - r, y: p.y - r, width: r * 2, height: r * 2)), with: .color(Phosphor.amber.opacity(dim)))
                if node.id == focus {
                    ctx.stroke(Path(ellipseIn: CGRect(x: p.x - r - 3, y: p.y - r - 3, width: r * 2 + 6, height: r * 2 + 6)), with: .color(Phosphor.cream), lineWidth: 1.2)
                }
            case .facet:
                let circle = Path(ellipseIn: CGRect(x: p.x - r, y: p.y - r, width: r * 2, height: r * 2))
                ctx.fill(circle, with: .color(Phosphor.black))
                ctx.stroke(circle, with: .color(Phosphor.amber.opacity(0.75 * dim)), lineWidth: 1.2)
            case .missing:
                ctx.stroke(Path(ellipseIn: CGRect(x: p.x - r, y: p.y - r, width: r * 2, height: r * 2)),
                           with: .color(Phosphor.dim.opacity(dim)), style: StrokeStyle(lineWidth: 1, dash: [1.5, 2]))
            }
        }

        // Labels by importance; one that would land on a label already drawn is left out, unless it's lit.
        func rank(_ node: GraphLayout.Node) -> Int {
            if node.id == focus { return 0 }
            if focus != nil && near.contains(node.id) { return 1 }
            switch node.kind {
            case .me, .folder: return 2
            case .note: return 3
            default: return 4
            }
        }
        var placed: [CGRect] = []
        let ordered = nodes.sorted { (rank($0), -$0.degree) < (rank($1), -$1.degree) }
        for node in ordered {
            let p = screen(node.position, size: size)
            let r = layout.radius(node) * min(max(scale, 0.6), 1.6)
            let on = lit(node.id)
            let highlighted = focus != nil && near.contains(node.id)
            let width = Double(node.label.count) * (node.kind == .folder ? 8.5 : 8) + 10
            let box: CGRect = switch node.kind {
            case .me, .folder: CGRect(x: p.x - width / 2, y: p.y + r + 2, width: width, height: 16)
            default: CGRect(x: p.x + r + 3, y: p.y - 8, width: width, height: 16)
            }
            if !highlighted, placed.contains(where: { $0.intersects(box) }) { continue }
            placed.append(box)
            switch node.kind {
            case .me:
                ctx.draw(Text("YOU").font(Fonts.mono(10)).tracking(1.2).foregroundColor(Phosphor.cream.opacity(on ? 0.9 : 0.3)),
                         at: CGPoint(x: p.x, y: p.y + 24 * min(max(scale, 0.6), 1.5)), anchor: .top)
            case .folder:
                ctx.draw(Text(node.label).font(Fonts.mono(10)).tracking(1.2).foregroundColor(Phosphor.cream.opacity(on ? 0.95 : 0.3)),
                         at: CGPoint(x: p.x, y: p.y + r + 5), anchor: .top)
            case .note:
                guard scale >= 0.55 || highlighted else { continue }
                ctx.draw(Text(node.label).font(Fonts.vt(highlighted && node.id == focus ? 19 : 16))
                    .foregroundColor((node.id == focus ? Phosphor.cream : Phosphor.cream2).opacity(on ? 0.85 : 0.22)),
                         at: CGPoint(x: p.x + r + 5, y: p.y), anchor: .leading)
            case .facet, .missing:
                guard scale >= 0.8 || highlighted else { continue }
                ctx.draw(Text(node.kind == .missing ? node.label + "?" : node.label).font(Fonts.vt(14))
                    .foregroundColor(Phosphor.dim.opacity(on ? 0.9 : 0.25)),
                         at: CGPoint(x: p.x + r + 4, y: p.y), anchor: .leading)
            }
        }
    }

    static func hash(_ k: Int) -> Int { Int((UInt64(k &+ 1) &* 2_654_435_761) % 1_000_003) }

    // MARK: Overlay

    func hud(size: CGSize) -> some View {
        let notes = layout.nodes.filter { $0.kind == .note }.count
        let shared = layout.nodes.filter { $0.kind == .facet }.count
        return VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 4) {
                    SectionLabel("Graph")
                    Text("\(notes) NOTES · \(layout.links.count) LINKS · \(shared) SHARED")
                        .font(Fonts.vt(19)).foregroundColor(Phosphor.amber)
                    if let id = hovered.value, let node = layout.node(id) {
                        Text(detail(node)).font(Fonts.vt(17)).foregroundColor(Phosphor.cream2)
                    }
                }
                Spacer()
                Button("Fit") {
                    layout.reheat(0.2)
                    fit(size)
                }
                .buttonStyle(GhostButtonStyle(height: 28))
            }
            Spacer()
            HStack(alignment: .bottom) {
                HStack(spacing: 14) {
                    legend(Circle().fill(Phosphor.amber), "NOTE")
                    legend(Circle().stroke(Phosphor.cream, lineWidth: 1.4), "FOLDER")
                    legend(Circle().stroke(Phosphor.amber, lineWidth: 1.2), "SHARED")
                    legend(Circle().stroke(Phosphor.dim, style: StrokeStyle(lineWidth: 1, dash: [1.5, 2])), "MISSING")
                }
                Spacer()
                Text("DRAG TO MOVE · PINCH TO ZOOM · CLICK TO OPEN").font(Fonts.vt(16)).foregroundColor(Phosphor.dim)
            }
        }
        .padding(24)
        .allowsHitTesting(true)
    }

    func legend(_ shape: some View, _ label: String) -> some View {
        HStack(spacing: 6) {
            shape.frame(width: 8, height: 8)
            Text(label).font(Fonts.mono(9.5)).tracking(1).foregroundColor(Phosphor.dim)
        }
    }

    func detail(_ node: GraphLayout.Node) -> String {
        let links = layout.graph.neighbours(of: node.id).count
        switch node.kind {
        case .me: return "YOU · \(links) FOLDERS"
        case .folder: return "\(node.label) · \(links - 1) NOTES"
        case .note: return "\(node.label.uppercased()) · \(node.path ?? "") · \(links) LINKS"
        case .facet: return "SHARED: \(node.label.uppercased()) · \(links) NOTES"
        case .missing: return "[[\(node.label)]] · NO NOTE YET"
        }
    }
}

import AppKit
import BinderCore
import SwiftUI

/// What the panel can ask the app to do.
public struct PanelActions {
    public var handOff: () -> Void = {}
    public var openSession: () -> Void = {}
    public var openBinder: () -> Void = {}
    public var addMemory: () -> Void = {}

    public init(handOff: @escaping () -> Void = {}, openSession: @escaping () -> Void = {},
                openBinder: @escaping () -> Void = {}, addMemory: @escaping () -> Void = {}) {
        self.handOff = handOff
        self.openSession = openSession
        self.openBinder = openBinder
        self.addMemory = addMemory
    }
}

struct BrainFrameKey: PreferenceKey {
    static let defaultValue: CGRect = .zero
    static func reduce(value: inout CGRect, nextValue: () -> CGRect) { value = nextValue() == .zero ? value : nextValue() }
}

struct HeightKey: PreferenceKey {
    static let defaultValue: CGFloat = 0
    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) { value = max(value, nextValue()) }
}

struct ListHeightKey: PreferenceKey {
    static let defaultValue: CGFloat = 0
    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) { value = max(value, nextValue()) }
}

struct CardFramesKey: PreferenceKey {
    static let defaultValue: [String: CGRect] = [:]
    static func reduce(value: inout [String: CGRect], nextValue: () -> [String: CGRect]) { value.merge(nextValue()) { $1 } }
}

/// The menu bar panel: its main job is approving what this session caught.
public struct PanelView: View {
    public static let width: CGFloat = 350
    /// More catches than this and the list scrolls, showing about this many at once.
    public static let visibleCards = 4

    @ObservedObject var model: AppModel
    var actions: PanelActions
    @StateObject private var brainFrame = Local(CGRect.zero)
    @StateObject private var cardFrames = Local([String: CGRect]())
    @StateObject private var gulp = Local(false)
    @StateObject private var snoozed = Local<String?>(nil)
    @StateObject private var panelHeight = Local(CGFloat(0))
    @StateObject private var scrollHeight = Local(CGFloat(0))
    @Environment(\.isSnapshot) private var isSnapshot

    public init(model: AppModel, actions: PanelActions = PanelActions()) {
        self.model = model
        self.actions = actions
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            header
            if let session = model.session { sessionCard(session) }
            if !model.proposals.isEmpty {
                SectionLabel("Caught this session")
                if model.proposals.count > Self.visibleCards {
                    Scroll { cards }
                        .frame(height: listHeight)
                        .background(GeometryReader { g in Color.clear.preference(key: ListHeightKey.self, value: g.size.height) })
                } else {
                    cards
                }
            } else {
                empty
            }
            if model.pendingCount > 0 {
                Button("Keep all \(model.pendingCount)") { model.keepAll() }
                    .buttonStyle(PrimaryButtonStyle(size: 13, height: 36, fullWidth: true))
                    .padding(.trailing, 3)
            }
            footer
        }
        .padding(16)
        .frame(width: Self.width, alignment: .leading)
        .background(GeometryReader { g in Color.clear.preference(key: HeightKey.self, value: g.size.height) })
        .coordinateSpace(name: "panel")
        .onPreferenceChange(HeightKey.self) { panelHeight.value = $0 }
        .onPreferenceChange(ListHeightKey.self) { scrollHeight.value = $0 }
        .onPreferenceChange(BrainFrameKey.self) { brainFrame.value = $0 }
        .onPreferenceChange(CardFramesKey.self) { cardFrames.value = $0 }
        .onChange(of: model.memories) { old, new in
            guard new > old else { return }
            withAnimation(.easeOut(duration: 0.17)) { gulp.value = true }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.17) { withAnimation(.easeIn(duration: 0.33)) { gulp.value = false } }
        }
    }

    // MARK: Parts

    private var cards: some View {
        VStack(spacing: 8) {
            ForEach(model.proposals) { p in card(p) }
        }
    }

    /// The scrolling list's height: the first four cards, or less if the screen under the menu bar can't fit them
    /// along with the rest of the panel.
    private var listHeight: CGFloat {
        let heights = model.proposals.prefix(Self.visibleCards).compactMap { cardFrames.value[$0.id]?.height }
        let shown = heights.isEmpty ? CGFloat(Self.visibleCards) * 96 : heights.reduce(0, +) + 8 * CGFloat(heights.count - 1)
        guard !isSnapshot, let screen = NSScreen.main?.visibleFrame.height else { return shown }
        let rest = scrollHeight.value > 0 ? panelHeight.value - scrollHeight.value : 0
        return max(180, min(shown, screen - 12 - rest))
    }

    private var header: some View {
        HStack(spacing: 12) {
            MascotHead(size: 64, rimWidth: 7)
                .scaleEffect(gulp.value ? 1.22 : 1)
                .background(GeometryReader { g in Color.clear.preference(key: BrainFrameKey.self, value: g.frame(in: .named("panel"))) })
                .accessibilityLabel("Binder's brain, where kept items go")
            VStack(alignment: .leading, spacing: 2) {
                Text("BINDER").font(Fonts.archivo(15, weight: 800)).tracking(0.6).foregroundColor(Phosphor.cream)
                Text("\(model.memories) \(model.memories == 1 ? "MEMORY" : "MEMORIES") · \(model.pendingCount) TO REVIEW")
                    .font(Fonts.vt(18))
                    .foregroundColor(Phosphor.dim)
            }
        }
    }

    private func sessionCard(_ session: SessionStatus) -> some View {
        ScreenPanel(glow: UnitPoint(x: 0.25, y: 0.3), padding: 14) {
            VStack(alignment: .leading, spacing: 8) {
                SectionLabel("This session · \(session.project ?? "no project") · Claude Code")
                if let fraction = model.meterFraction {
                    HStack(alignment: .firstTextBaseline, spacing: 10) {
                        Text("\(Int((fraction * 100).rounded()))%")
                            .font(Fonts.vt(56))
                            .foregroundColor(Phosphor.amber)
                            .shadow(color: Phosphor.amber.opacity(0.45), radius: 7)
                        Text(model.handoffSuggested ? "of context used. Getting long." : "of context used.")
                            .font(Fonts.grotesk(13))
                            .foregroundColor(Phosphor.cream2)
                    }
                    SteppedBar(fraction: fraction)
                    Text("ESTIMATE").font(Fonts.mono(9.5)).tracking(1).foregroundColor(Phosphor.dim)
                }
                if let handoff = model.handoff, handoff.session == session.session {
                    handoffRow(handoff)
                } else if model.handoffSuggested, snoozed.value != session.session {
                    HStack(spacing: 8) {
                        Button("Hand off now") { actions.handOff() }
                        .buttonStyle(PrimaryButtonStyle(size: 12, height: 30))
                        Button("Not yet") { snoozed.value = session.session }.buttonStyle(GhostButtonStyle(height: 30))
                    }
                }
            }
        }
    }

    /// After "Hand off now": writing, then copied to the clipboard, until Done or a new session takes over.
    @ViewBuilder
    private func handoffRow(_ handoff: HandoffState) -> some View {
        switch handoff {
        case .writing:
            Text("WRITING THE HANDOFF…").font(Fonts.vt(19)).foregroundColor(Phosphor.amber)
        case .copied:
            VStack(alignment: .leading, spacing: 8) {
                Text("Handoff copied. Paste it into a new Claude Code session.")
                    .font(Fonts.grotesk(13)).foregroundColor(Phosphor.cream2).fixedSize(horizontal: false, vertical: true)
                HStack(spacing: 8) {
                    Button("Open in Terminal") { actions.openSession() }.buttonStyle(PrimaryButtonStyle(size: 12, height: 30))
                    Button("Done") { model.handoff = nil }.buttonStyle(GhostButtonStyle(height: 30))
                }
            }
        case .failed:
            HStack(spacing: 8) {
                Text("Couldn't read this session to write a handoff.")
                    .font(Fonts.grotesk(13)).foregroundColor(Phosphor.cream2).fixedSize(horizontal: false, vertical: true)
                Button("Done") { model.handoff = nil }.buttonStyle(GhostButtonStyle(height: 30))
            }
        }
    }

    private func card(_ p: Proposal) -> some View {
        let phase = model.phases[p.id] ?? .idle
        let frame = cardFrames.value[p.id] ?? .zero
        let target = CGSize(width: brainFrame.value.midX - frame.minX - frame.width * 0.12, height: brainFrame.value.midY - frame.minY)
        return Group {
            if phase == .shredding {
                ShredView(card: ProposalCard(proposal: p, model: model).allowsHitTesting(false))
                    .frame(height: max(frame.height, 88))
            } else {
                ProposalCard(proposal: p, model: model)
                    .modifier(Absorb(active: phase == .absorbing, target: target))
                    .allowsHitTesting(phase != .absorbing)
            }
        }
        .zIndex(phase == .absorbing ? 1 : 0)
        .background(GeometryReader { g in Color.clear.preference(key: CardFramesKey.self, value: [p.id: g.frame(in: .named("panel"))]) })
    }

    private var empty: some View {
        VStack(spacing: 8) {
            Text("ALL FILED.").font(Fonts.vt(26)).foregroundColor(Phosphor.amber)
            Text("Nothing to review. I'll call you when there is.")
                .font(Fonts.grotesk(13))
                .foregroundColor(Phosphor.cream2)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 24)
    }

    private var footer: some View {
        HStack {
            Button("Open Binder") { actions.openBinder() }
                .buttonStyle(LinkButtonStyle(color: Phosphor.cream))
            Spacer()
            Button("Add a memory") { actions.addMemory() }
                .buttonStyle(LinkButtonStyle(color: Phosphor.cream2))
        }
    }
}

/// Where the notch points, in panel coordinates. The status item sets it so the notch sits under the icon.
public final class PanelGeometry: ObservableObject {
    @Published public var arrowX: CGFloat

    public init(arrowX: CGFloat = PanelView.width / 2) { self.arrowX = arrowX }
}

/// The panel with its frame (raised black, a hairline, as tall as its content) and, on top, an amber pixel
/// notch pointing at the menu bar icon, with amber pixels bleeding into the top edge under it. As in Dawn.
public struct PanelContainer: View {
    @ObservedObject var model: AppModel
    @ObservedObject var geometry: PanelGeometry
    var actions: PanelActions

    public static let notchHeight: CGFloat = PixelNotch.rows * PixelNotch.pixel

    public init(model: AppModel, geometry: PanelGeometry = PanelGeometry(), actions: PanelActions = PanelActions()) {
        self.model = model
        self.geometry = geometry
        self.actions = actions
    }

    public var body: some View {
        VStack(spacing: 0) {
            PixelNotch()
                .frame(width: PixelNotch.width, height: Self.notchHeight)
                .padding(.leading, max(0, geometry.arrowX - PixelNotch.width / 2))
                .frame(maxWidth: .infinity, alignment: .leading)
            PanelView(model: model, actions: actions)
                .background(Phosphor.black2)
                .overlay(Rectangle().strokeBorder(Phosphor.line, lineWidth: 1))
                .overlay(alignment: .top) { PixelBleed(centerX: geometry.arrowX).frame(height: PixelNotch.pixel * 3) }
        }
        .frame(width: PanelView.width)
        .fixedSize(horizontal: false, vertical: true)
        .environment(\.colorScheme, .dark)
    }
}

/// A stepped triangle of 2 pt pixels, 1-3-5-7-9 wide.
struct PixelNotch: View {
    static let pixel: CGFloat = 2
    static let rows: CGFloat = 5
    static var width: CGFloat { (rows * 2 - 1) * pixel }

    var body: some View {
        Canvas { ctx, size in
            let p = Self.pixel
            let cx = size.width / 2
            for r in 0..<Int(Self.rows) {
                let count = 2 * r + 1
                for k in 0..<count {
                    let x = cx - CGFloat(count) * p / 2 + CGFloat(k) * p
                    // The tip is bright; the base row alternates with amber-deep where it meets the panel.
                    let color = (r == Int(Self.rows) - 1 && k % 2 == 1) ? Phosphor.amberDeep : Phosphor.amber
                    ctx.fill(Path(CGRect(x: x, y: CGFloat(r) * p, width: p, height: p)), with: .color(color))
                }
            }
        }
        .accessibilityHidden(true)
    }
}

/// Amber pixels along the panel's top edge, densest under the notch, fading out with a 4 × 4 Bayer dither.
struct PixelBleed: View {
    let centerX: CGFloat
    static let bayer: [[Double]] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]]
    /// How far each row reaches from the notch.
    static let reach: [CGFloat] = [90, 56, 28]

    /// Whether the pixel at `column` in `row` is lit, and if so whether it's the bright amber.
    static func pixel(row: Int, column: Int, centerX: CGFloat) -> (on: Bool, bright: Bool) {
        let p = PixelNotch.pixel
        let d = abs(CGFloat(column) * p + p / 2 - centerX)
        let density = max(0, 1 - d / reach[row])
        let threshold = (bayer[row % 4][column % 4] + 0.5) / 16
        return (density > threshold, density > 0.6)
    }

    var body: some View {
        Canvas { ctx, size in
            let p = PixelNotch.pixel
            for row in 0..<Self.reach.count {
                for column in 0..<Int(size.width / p) {
                    let px = Self.pixel(row: row, column: column, centerX: centerX)
                    guard px.on else { continue }
                    ctx.fill(Path(CGRect(x: CGFloat(column) * p, y: CGFloat(row) * p, width: p, height: p)),
                             with: .color(px.bright ? Phosphor.amber : Phosphor.amberDeep))
                }
            }
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

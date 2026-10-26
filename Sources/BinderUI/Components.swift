import SwiftUI

/// Amber fill, ink text, a hard amber-deep shadow that the button sinks into when pressed.
public struct PrimaryButtonStyle: ButtonStyle {
    var size: CGFloat
    var height: CGFloat
    var fullWidth: Bool

    public init(size: CGFloat = 13, height: CGFloat = 36, fullWidth: Bool = false) {
        self.size = size
        self.height = height
        self.fullWidth = fullWidth
    }

    public func makeBody(configuration: Configuration) -> some View {
        let pressed = configuration.isPressed
        return configuration.label
            .font(Fonts.archivo(size, weight: 800))
            .foregroundColor(Phosphor.ink)
            .padding(.horizontal, 16)
            .frame(maxWidth: fullWidth ? .infinity : nil, minHeight: height)
            .background(Phosphor.amber)
            .background(Phosphor.amberDeep.offset(x: pressed ? 0 : 3, y: pressed ? 0 : 3))
            .offset(x: pressed ? 2 : 0, y: pressed ? 2 : 0)
            .contentShape(Rectangle())
    }
}

/// Cream text on a hairline border.
public struct GhostButtonStyle: ButtonStyle {
    var height: CGFloat

    public init(height: CGFloat = 36) { self.height = height }

    public func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(Fonts.grotesk(12.5, weight: 600))
            .foregroundColor(Phosphor.cream)
            .padding(.horizontal, 14)
            .frame(minHeight: height)
            .overlay(Rectangle().strokeBorder(Phosphor.dim, lineWidth: 1))
            .opacity(configuration.isPressed ? 0.7 : 1)
            .contentShape(Rectangle())
    }
}

/// A small solid cream button, for Keep.
public struct CreamButtonStyle: ButtonStyle {
    public init() {}

    public func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(Fonts.grotesk(12, weight: 600))
            .foregroundColor(Phosphor.ink)
            .padding(.horizontal, 12)
            .frame(minHeight: 26)
            .background(Phosphor.cream.opacity(configuration.isPressed ? 0.8 : 1))
            .contentShape(Rectangle())
    }
}

/// Dim text, no frame: the least important action in a row.
public struct QuietButtonStyle: ButtonStyle {
    public init() {}

    public func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(Fonts.grotesk(12, weight: 600))
            .foregroundColor(configuration.isPressed ? Phosphor.cream : Phosphor.dim)
            .padding(.horizontal, 8)
            .frame(minHeight: 26)
            .contentShape(Rectangle())
    }
}

/// Fine horizontal lines, like a CRT.
public struct Scanlines: View {
    var opacity: Double

    public init(opacity: Double = 0.42) { self.opacity = opacity }

    public var body: some View {
        Canvas { ctx, size in
            var y: CGFloat = 2
            while y < size.height {
                ctx.fill(Path(CGRect(x: 0, y: y, width: size.width, height: 1)), with: .color(.black.opacity(opacity)))
                y += 3
            }
        }
        .allowsHitTesting(false)
    }
}

/// Black glass with scanlines and a faint amber glow: the "screen" panels.
public struct ScreenPanel<Content: View>: View {
    var glow: UnitPoint
    var padding: CGFloat
    var content: Content

    public init(glow: UnitPoint = UnitPoint(x: 0.3, y: 0.3), padding: CGFloat = 14, @ViewBuilder content: () -> Content) {
        self.glow = glow
        self.padding = padding
        self.content = content()
    }

    public var body: some View {
        content
            .padding(padding)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(
                ZStack {
                    Phosphor.black
                    RadialGradient(colors: [Phosphor.amber.opacity(0.18), .clear], center: glow, startRadius: 0, endRadius: 260)
                    Scanlines()
                }
            )
            .overlay(Rectangle().strokeBorder(Phosphor.line, lineWidth: 1))
    }
}

/// Space Mono, uppercase, tracked out.
public struct SectionLabel: View {
    var text: String
    var color: Color

    public init(_ text: String, color: Color = Phosphor.dim) {
        self.text = text
        self.color = color
    }

    public var body: some View {
        Text(text.uppercased())
            .font(Fonts.mono(10.5))
            .tracking(1.05)
            .foregroundColor(color)
    }
}

/// A toggle chip: amber when on, hairline when off.
public struct Chip: View {
    var text: String
    var on: Bool
    var action: () -> Void

    public init(_ text: String, on: Bool, action: @escaping () -> Void) {
        self.text = text
        self.on = on
        self.action = action
    }

    public var body: some View {
        Button(action: action) {
            Text(text)
                .font(Fonts.grotesk(12, weight: 600))
                .foregroundColor(on ? Phosphor.ink : Phosphor.cream2)
                .padding(.horizontal, 12)
                .frame(height: 30)
                .background(on ? Phosphor.amber : .clear)
                .overlay(Rectangle().strokeBorder(on ? Phosphor.amber : Phosphor.line, lineWidth: 1))
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}

/// The setup steps down the left rail: done (amber square), current (cream arrow), to do (dim dot).
public struct StepRail: View {
    var steps: [String]
    var current: Int

    public init(steps: [String], current: Int) {
        self.steps = steps
        self.current = current
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 9) {
            ForEach(Array(steps.enumerated()), id: \.offset) { index, step in
                HStack(spacing: 6) {
                    if index < current {
                        Text("■").foregroundColor(Phosphor.amber)
                    } else if index == current {
                        Text("▸").foregroundColor(Phosphor.cream)
                    } else {
                        Text("·").foregroundColor(Phosphor.dim)
                    }
                    Text(step).foregroundColor(index == current ? Phosphor.cream : Phosphor.dim)
                }
                .font(Fonts.mono(11))
                .tracking(1.1)
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Step \(current + 1) of \(steps.count): \(steps[min(current, steps.count - 1)])")
    }
}

/// A stepped amber bar: amber blocks with amber-deep gaps, filling `fraction` of the width.
public struct SteppedBar: View {
    var fraction: Double
    var height: CGFloat

    public init(fraction: Double, height: CGFloat = 6) {
        self.fraction = fraction
        self.height = height
    }

    public var body: some View {
        GeometryReader { geo in
            ZStack(alignment: .leading) {
                Phosphor.line
                Canvas { ctx, size in
                    var x: CGFloat = 0
                    while x < size.width {
                        ctx.fill(Path(CGRect(x: x, y: 0, width: 6, height: size.height)), with: .color(Phosphor.amber))
                        ctx.fill(Path(CGRect(x: x + 6, y: 0, width: 2, height: size.height)), with: .color(Phosphor.amberDeep))
                        x += 8
                    }
                }
                .frame(width: geo.size.width * min(max(fraction, 0), 1))
                .clipped()
            }
        }
        .frame(height: height)
        .accessibilityElement()
        .accessibilityLabel("\(Int((fraction * 100).rounded())) percent")
    }
}

/// The strip across the top of a window: room for the traffic lights, a centred Space Mono title.
public struct TitleStrip: View {
    var title: String

    public init(_ title: String) { self.title = title }

    public var body: some View {
        ZStack {
            Phosphor.black
            SectionLabel(title)
        }
        .frame(height: 40)
        .overlay(alignment: .bottom) { Rectangle().fill(Phosphor.line).frame(height: 1) }
    }
}

/// View-local state without `@State`: with only the Command Line Tools, the macOS 27 SDK resolves `@State`
/// to a SwiftUI macro whose plugin isn't installed. `@StateObject private var x = Local(false)` works everywhere.
public final class Local<Value>: ObservableObject {
    @Published public var value: Value

    public init(_ value: Value) { self.value = value }
}

// MARK: Snapshot mode

private struct SnapshotModeKey: EnvironmentKey {
    static let defaultValue = false
}

public extension EnvironmentValues {
    /// True while rendering PNGs: AppKit-backed controls (text fields) can't be drawn by ImageRenderer,
    /// so views show a styled `Text` in their place.
    var isSnapshot: Bool {
        get { self[SnapshotModeKey.self] }
        set { self[SnapshotModeKey.self] = newValue }
    }
}

/// A labelled single-line field in Phosphor style.
public struct BinderField: View {
    var label: String
    @Binding var text: String
    var detected: Bool
    var placeholder: String
    @Environment(\.isSnapshot) private var isSnapshot

    public init(_ label: String, text: Binding<String>, detected: Bool = false, placeholder: String = "") {
        self.label = label
        self._text = text
        self.detected = detected
        self.placeholder = placeholder
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            HStack(spacing: 6) {
                Text(label).font(Fonts.grotesk(12)).foregroundColor(Phosphor.cream2)
                if detected { Text("DETECTED").font(Fonts.mono(10.5)).foregroundColor(Phosphor.amber) }
            }
            Group {
                if isSnapshot {
                    Text(text.isEmpty ? placeholder : text)
                        .lineLimit(1)
                        .foregroundColor(text.isEmpty ? Phosphor.dim : Phosphor.cream)
                        .frame(maxWidth: .infinity, alignment: .leading)
                } else {
                    TextField(placeholder, text: $text)
                        .textFieldStyle(.plain)
                        .foregroundColor(Phosphor.cream)
                }
            }
            .font(Fonts.grotesk(14))
            .padding(.horizontal, 12)
            .frame(height: 36)
            .background(Phosphor.black)
            .overlay(Rectangle().strokeBorder(Phosphor.line, lineWidth: 1))
            .accessibilityLabel(label)
        }
    }
}

/// The setup window's frame: title strip, the rail on the left (mascot, caption, steps), the step on the right.
public struct SetupShell<Content: View>: View {
    var step: Int
    var mood: Mascot.Mood
    var caption: String
    var content: Content

    public init(step: Int, mood: Mascot.Mood, caption: String, @ViewBuilder content: () -> Content) {
        self.step = step
        self.mood = mood
        self.caption = caption
        self.content = content()
    }

    public var body: some View {
        VStack(spacing: 0) {
            TitleStrip("BINDER · SETUP")
            HStack(spacing: 0) {
                VStack {
                    VStack(spacing: 14) {
                        Mascot(mood: mood, size: 170)
                        Text(caption)
                            .font(Fonts.vt(20))
                            .foregroundColor(Phosphor.amber)
                            .multilineTextAlignment(.center)
                    }
                    Spacer()
                    StepRail(steps: OnboardingStep.allCases.map(\.railLabel), current: step)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                .padding(.vertical, 28)
                .padding(.horizontal, 24)
                .frame(width: 260)
                .background(
                    ZStack {
                        Phosphor.black
                        RadialGradient(colors: [Phosphor.amber.opacity(0.2), .clear], center: UnitPoint(x: 0.5, y: 0.3), startRadius: 0, endRadius: 240)
                        Scanlines()
                    }
                )
                .overlay(alignment: .trailing) { Rectangle().fill(Phosphor.line).frame(width: 1) }
                content
                    .padding(.horizontal, 36)
                    .padding(.vertical, 28)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            }
        }
        .background(Phosphor.black2)
    }
}

/// Archivo heading plus a paragraph: the top of every setup step.
public struct StepHeading: View {
    var title: String
    var text: String

    public init(_ title: String, _ text: String) {
        self.title = title
        self.text = text
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title).font(Fonts.archivo(28, weight: 800)).foregroundColor(Phosphor.cream)
            Text(text).font(Fonts.grotesk(14)).foregroundColor(Phosphor.cream2).fixedSize(horizontal: false, vertical: true).lineSpacing(3)
        }
    }
}

/// A vertical ScrollView, or a plain stack while rendering snapshots (ImageRenderer can't draw NSScrollView).
public struct Scroll<Content: View>: View {
    var content: Content
    @Environment(\.isSnapshot) private var isSnapshot

    public init(@ViewBuilder content: () -> Content) { self.content = content() }

    public var body: some View {
        if isSnapshot {
            VStack(alignment: .leading, spacing: 0) { content }.frame(maxHeight: .infinity, alignment: .top).clipped()
        } else {
            ScrollView { content }
        }
    }
}

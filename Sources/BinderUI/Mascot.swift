import SwiftUI

/// Binder: a small ring binder with a glass dome for a head and its brain on show.
/// Drawn on the same 240 × 280 grid as the design's SVG, so every coordinate matches the artboards.
public struct Mascot: View {
    public enum Mood: Equatable, Sendable {
        case idle
        /// The brain flickers on as `progress` goes from 0 to 1.
        case booting(progress: Double)
        /// Eyes sweep left and right.
        case scanning
        /// Happy eyes, holding a pencil.
        case happy
        /// Pages flying in.
        case writing
        /// Holding a plug.
        case plugging
        /// Holding up a key, looking up.
        case holdingKey
    }

    var mood: Mood
    var size: CGFloat

    public init(mood: Mood = .idle, size: CGFloat = 170) {
        self.mood = mood
        self.size = size
    }

    public var body: some View {
        Group {
            if mood == .scanning {
                TimelineView(.animation) { timeline in
                    MascotCanvas(mood: mood, eyeShift: CGFloat(sin(timeline.date.timeIntervalSinceReferenceDate * .pi / 0.9)) * 7)
                }
            } else {
                MascotCanvas(mood: mood, eyeShift: 0)
            }
        }
        .frame(width: size, height: size * 280 / 240)
        .accessibilityElement()
        .accessibilityLabel(Self.label(mood))
    }

    static func label(_ mood: Mood) -> String {
        switch mood {
        case .idle: "Binder, a small ring binder with a glass dome head and a glowing brain"
        case .booting: "Binder booting up, its brain flickering on"
        case .scanning: "Binder looking left and right"
        case .happy: "Binder smiling, holding a pencil"
        case .writing: "Binder with pages flying in"
        case .plugging: "Binder holding a plug"
        case .holdingKey: "Binder holding up a small key"
        }
    }
}

/// The drawing itself. `eyeShift` moves the eyes sideways (in grid units).
struct MascotCanvas: View {
    var mood: Mascot.Mood
    var eyeShift: CGFloat

    var body: some View {
        Canvas { ctx, size in
            ctx.scaleBy(x: size.width / 240, y: size.height / 280)
            Self.draw(&ctx, mood: mood, eyeShift: eyeShift)
        }
    }

    static func rect(_ x: CGFloat, _ y: CGFloat, _ w: CGFloat, _ h: CGFloat) -> Path { Path(CGRect(x: x, y: y, width: w, height: h)) }

    static var dome: Path {
        var p = Path()
        p.move(to: CGPoint(x: 52, y: 118))
        p.addArc(center: CGPoint(x: 120, y: 118), radius: 68, startAngle: .degrees(180), endAngle: .degrees(360), clockwise: false)
        p.closeSubpath()
        return p
    }

    static var rim: Path {
        var p = Path()
        p.addArc(center: CGPoint(x: 120, y: 118), radius: 68, startAngle: .degrees(180), endAngle: .degrees(360), clockwise: false)
        return p
    }

    static let brainCircles: [(CGFloat, CGFloat, CGFloat)] = [(98, 96, 20), (120, 84, 24), (142, 96, 20), (108, 106, 16), (132, 106, 16)]

    static var gyri: Path {
        var p = Path()
        for (x, y, dx, dy) in [(94.0, 88.0, 8.0, -8.0), (130, 88, 8, -8), (98, 104, 6, 6), (130, 104, 6, 6)] as [(CGFloat, CGFloat, CGFloat, CGFloat)] {
            p.move(to: CGPoint(x: x, y: y))
            p.addQuadCurve(to: CGPoint(x: x + dx * 2, y: y), control: CGPoint(x: x + dx, y: y + dy))
        }
        return p
    }

    /// The dome, brain, rim and collar. Shared with the icon and the menu bar header.
    static func drawHead(_ ctx: inout GraphicsContext, brain: Double = 1, glow: Double = 0.2, rimWidth: CGFloat = 4) {
        ctx.fill(dome, with: .color(Phosphor.dome))
        ctx.drawLayer { layer in
            layer.clip(to: dome)
            layer.opacity = brain
            for (x, y, r) in brainCircles {
                layer.fill(Path(ellipseIn: CGRect(x: x - r, y: y - r, width: r * 2, height: r * 2)), with: .color(Phosphor.amber))
            }
            var fissure = Path()
            fissure.move(to: CGPoint(x: 120, y: 62))
            fissure.addLine(to: CGPoint(x: 120, y: 118))
            layer.stroke(fissure, with: .color(Phosphor.gyri), lineWidth: 3)
            layer.stroke(gyri, with: .color(Phosphor.gyri), lineWidth: 2.5)
            layer.fill(Path(ellipseIn: CGRect(x: 60, y: 52, width: 120, height: 80)), with: .color(Phosphor.amber.opacity(glow)))
        }
        ctx.drawLayer { layer in
            layer.clip(to: dome)
            var y: CGFloat = 60
            while y < 116 {
                layer.fill(rect(50, y, 140, 1.5), with: .color(Phosphor.black.opacity(0.4)))
                y += 6
            }
        }
        ctx.stroke(rim, with: .color(Phosphor.cream), lineWidth: rimWidth)
        var glare = Path()
        glare.move(to: CGPoint(x: 72, y: 96))
        glare.addQuadCurve(to: CGPoint(x: 98, y: 62), control: CGPoint(x: 76, y: 70))
        ctx.stroke(glare, with: .color(Phosphor.cream.opacity(0.55)), lineWidth: 4)
        ctx.fill(rect(40, 116, 160, 12), with: .color(Phosphor.amber))
    }

    static func draw(_ ctx: inout GraphicsContext, mood: Mascot.Mood, eyeShift: CGFloat) {
        let sand = GraphicsContext.Shading.color(Phosphor.sand)
        // Legs, feet, left arm.
        for r in [rect(82, 236, 14, 28), rect(144, 236, 14, 28), rect(74, 260, 26, 6), rect(140, 260, 26, 6), rect(26, 176, 24, 10)] {
            ctx.fill(r, with: sand)
        }
        drawProps(&ctx, mood: mood)
        // Paper edge, body, spine, rings.
        ctx.fill(rect(60, 228, 128, 8), with: .color(Phosphor.amber))
        ctx.fill(rect(48, 128, 144, 102), with: .color(Phosphor.cream))
        ctx.fill(rect(48, 128, 28, 102), with: sand)
        for y in [150.0, 180, 210] as [CGFloat] {
            ctx.stroke(Path(ellipseIn: CGRect(x: 54, y: y - 8, width: 16, height: 16)), with: .color(Phosphor.ink), lineWidth: 4)
        }
        // Face window, face, belly stripes.
        ctx.fill(rect(94, 146, 80, 50), with: .color(Phosphor.black))
        drawFace(&ctx, mood: mood, eyeShift: eyeShift)
        ctx.fill(rect(94, 206, 80, 3), with: .color(Phosphor.ink))
        ctx.fill(rect(94, 213, 80, 5), with: .color(Phosphor.ink))
        // Head.
        if case let .booting(progress) = mood {
            drawHead(&ctx, brain: flicker(progress), glow: 0.2 * progress)
        } else {
            drawHead(&ctx, brain: 1, glow: mood == .scanning ? 0.28 : 0.2)
        }
    }

    /// The brain's opacity while booting: a few hard flickers, then steady.
    static func flicker(_ p: Double) -> Double {
        switch p {
        case ..<0.08: return p / 0.08 * 0.9
        case ..<0.10: return 0.12
        case ..<0.18: return 1
        case ..<0.20: return 0.3
        default: return 1
        }
    }

    static func drawFace(_ ctx: inout GraphicsContext, mood: Mascot.Mood, eyeShift: CGFloat) {
        let amber = GraphicsContext.Shading.color(Phosphor.amber)
        var mouth = Path()
        switch mood {
        case .happy:
            var eyes = Path()
            eyes.move(to: CGPoint(x: 110, y: 170)); eyes.addLine(to: CGPoint(x: 117, y: 162)); eyes.addLine(to: CGPoint(x: 124, y: 170))
            eyes.move(to: CGPoint(x: 144, y: 170)); eyes.addLine(to: CGPoint(x: 151, y: 162)); eyes.addLine(to: CGPoint(x: 158, y: 170))
            ctx.stroke(eyes, with: amber, lineWidth: 3.5)
            mouth.move(to: CGPoint(x: 122, y: 182)); mouth.addQuadCurve(to: CGPoint(x: 146, y: 182), control: CGPoint(x: 134, y: 192))
        case .holdingKey:
            ctx.fill(rect(114, 154, 10, 12), with: amber)
            ctx.fill(rect(148, 154, 10, 12), with: amber)
            mouth.move(to: CGPoint(x: 126, y: 182)); mouth.addLine(to: CGPoint(x: 142, y: 182))
        case .writing:
            ctx.fill(rect(112, 162, 10, 14), with: amber)
            ctx.fill(rect(146, 162, 10, 14), with: amber)
            ctx.fill(rect(128, 182, 12, 7), with: amber)
        case .booting(let p) where p < 0.35:
            break // the face screen is still off
        default:
            ctx.fill(rect(112 + eyeShift, 160, 10, 16), with: amber)
            ctx.fill(rect(146 + eyeShift, 160, 10, 16), with: amber)
            if mood == .scanning {
                mouth.move(to: CGPoint(x: 126, y: 186)); mouth.addLine(to: CGPoint(x: 142, y: 186))
            } else {
                mouth.move(to: CGPoint(x: 124, y: 184)); mouth.addQuadCurve(to: CGPoint(x: 144, y: 184), control: CGPoint(x: 134, y: 191))
            }
        }
        ctx.stroke(mouth, with: amber, lineWidth: 3)
    }

    static func drawProps(_ ctx: inout GraphicsContext, mood: Mascot.Mood) {
        let sand = GraphicsContext.Shading.color(Phosphor.sand)
        let amber = GraphicsContext.Shading.color(Phosphor.amber)
        switch mood {
        case .holdingKey:
            ctx.fill(rect(190, 138, 10, 46), with: sand)
            ctx.stroke(Path(ellipseIn: CGRect(x: 186, y: 117, width: 18, height: 18)), with: amber, lineWidth: 4)
            ctx.fill(rect(193, 98, 4, 20), with: amber)
            ctx.fill(rect(197, 102, 7, 4), with: amber)
        case .happy:
            ctx.fill(rect(190, 170, 22, 10), with: sand)
            var pencil = ctx
            pencil.translateBy(x: 210, y: 154)
            pencil.rotate(by: .degrees(18))
            pencil.fill(rect(-4, -26, 8, 52), with: amber)
            pencil.fill(rect(-4, -26, 8, 8), with: .color(Phosphor.cream))
        case .writing:
            for (x, y, w, h, angle) in [(196.0, 150.0, 30.0, 38.0, 12.0), (14, 140, 26, 32, -14)] as [(CGFloat, CGFloat, CGFloat, CGFloat, Double)] {
                var page = ctx
                page.translateBy(x: x + w / 2, y: y + h / 2)
                page.rotate(by: .degrees(angle))
                let r = CGRect(x: -w / 2, y: -h / 2, width: w, height: h)
                page.fill(Path(r), with: .color(Phosphor.cream))
                page.stroke(Path(r), with: .color(Phosphor.ink), lineWidth: 2)
                var lines = Path()
                for i in 0..<3 {
                    let ly = -h / 2 + 10 + CGFloat(i) * 7
                    lines.move(to: CGPoint(x: -w / 2 + 5, y: ly)); lines.addLine(to: CGPoint(x: w / 2 - (i == 2 ? 10 : 5), y: ly))
                }
                page.stroke(lines, with: .color(Phosphor.ink), lineWidth: 2)
            }
        case .plugging:
            ctx.fill(rect(190, 170, 22, 10), with: sand)
            ctx.fill(rect(210, 160, 16, 30), with: amber)
            ctx.fill(rect(226, 164, 10, 4), with: .color(Phosphor.cream))
            ctx.fill(rect(226, 182, 10, 4), with: .color(Phosphor.cream))
            var cable = Path()
            cable.move(to: CGPoint(x: 218, y: 190))
            cable.addCurve(to: CGPoint(x: 120, y: 272), control1: CGPoint(x: 218, y: 240), control2: CGPoint(x: 160, y: 250))
            ctx.stroke(cable, with: amber, lineWidth: 4)
        default:
            ctx.fill(rect(190, 176, 24, 10), with: sand)
        }
    }
}

/// Just the dome, brain and collar: for the app icon, the menu bar header and small places.
public struct MascotHead: View {
    var size: CGFloat
    var rimWidth: CGFloat

    public init(size: CGFloat = 64, rimWidth: CGFloat = 6) {
        self.size = size
        self.rimWidth = rimWidth
    }

    public var body: some View {
        Canvas { ctx, s in
            // The head spans x 40…200, y 46…130 on the mascot's grid.
            ctx.scaleBy(x: s.width / 160, y: s.height / 84)
            ctx.translateBy(x: -40, y: -46)
            MascotCanvas.drawHead(&ctx, rimWidth: rimWidth)
        }
        .frame(width: size, height: size * 84 / 160)
        .accessibilityHidden(true)
    }
}

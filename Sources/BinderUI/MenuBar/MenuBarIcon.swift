import AppKit

public enum MenuState: Equatable, Sendable {
    case idle
    case review(Int)
    case handoffSoon
    case observing
}

/// The menu bar item: Binder himself in miniature (dome, brain, collar, binder body with its face window, legs),
/// drawn as a template so macOS tints it. While something waits for review the brain is left out of the template
/// and `BrainGlow` lays an amber one over it.
public enum MenuBarIcon {
    public static let size = NSSize(width: 18, height: 18)

    public static func image(state: MenuState, brain: Bool = true) -> NSImage {
        let image = NSImage(size: size, flipped: true) { _ in
            NSColor.black.set()
            let dome = domePath
            dome.lineWidth = 1.2
            dome.stroke()
            if brain {
                NSColor.black.withAlphaComponent(state == .observing ? 0.45 : 1).set()
                brainPath.fill()
                NSColor.black.set()
            }
            NSRect(x: 2.4, y: 6, width: 13.2, height: 1.3).fill()
            // Body, then the face window and ring holes cut out of it, then the eyes inside the window.
            NSRect(x: 3.5, y: 7.3, width: 11, height: 7.3).fill()
            let context = NSGraphicsContext.current
            context?.compositingOperation = .copy
            NSColor.clear.set()
            NSRect(x: 7, y: 8.5, width: 6, height: 3.8).fill()
            for y in [9.0, 11.2, 13.4] as [CGFloat] {
                NSBezierPath(ovalIn: NSRect(x: 4.1, y: y - 0.65, width: 1.3, height: 1.3)).fill()
            }
            context?.compositingOperation = .sourceOver
            NSColor.black.set()
            let look: CGFloat = state == .observing ? 0.7 : 0
            NSRect(x: 8.3 + look, y: 9.3, width: 1, height: 1.6).fill()
            NSRect(x: 10.7 + look, y: 9.3, width: 1, height: 1.6).fill()
            // Legs and feet.
            for x in [6.2, 10.6] as [CGFloat] {
                NSRect(x: x, y: 14.6, width: 1.2, height: 2.4).fill()
                NSRect(x: x - 0.6, y: 16.6, width: 2, height: 0.9).fill()
            }
            if state == .handoffSoon {
                NSRect(x: 14.2, y: 0.4, width: 3.6, height: 3.6).fill()
            }
            return true
        }
        image.isTemplate = true
        image.accessibilityDescription = accessibilityLabel(state: state)
        return image
    }

    /// The dome's outline, on the icon's 18 pt grid (y down).
    static var domePath: NSBezierPath {
        let p = NSBezierPath()
        p.appendArc(withCenter: NSPoint(x: 9, y: 6), radius: 5.2, startAngle: 180, endAngle: 0, clockwise: false)
        p.close()
        return p
    }

    /// The brain: the mascot's five lobes, scaled into the dome.
    static let lobes: [(x: CGFloat, y: CGFloat, r: CGFloat)] = [(7.3, 4.3, 1.55), (9, 3.4, 1.85), (10.7, 4.3, 1.55), (8.1, 5.1, 1.25), (9.9, 5.1, 1.25)]

    static var brainPath: NSBezierPath {
        let p = NSBezierPath()
        for l in lobes { p.appendOval(in: NSRect(x: l.x - l.r, y: l.y - l.r, width: l.r * 2, height: l.r * 2)) }
        p.windingRule = .nonZero
        return p
    }

    static var brainCGPath: CGPath {
        let p = CGMutablePath()
        for l in lobes { p.addEllipse(in: CGRect(x: l.x - l.r, y: l.y - l.r, width: l.r * 2, height: l.r * 2)) }
        return p
    }

    /// The count next to the icon when there's something to review.
    public static func title(state: MenuState) -> NSAttributedString? {
        guard case let .review(n) = state, n > 0 else { return nil }
        Fonts.register()
        let font = NSFont(name: "VT323-Regular", size: 17) ?? .monospacedDigitSystemFont(ofSize: 13, weight: .regular)
        return NSAttributedString(string: "\(n)", attributes: [.font: font, .baselineOffset: -1])
    }

    public static func accessibilityLabel(state: MenuState) -> String {
        switch state {
        case .idle: "Binder"
        case let .review(n): "Binder, \(n) to review"
        case .handoffSoon: "Binder, this session is getting long"
        case .observing: "Binder, looking around"
        }
    }
}

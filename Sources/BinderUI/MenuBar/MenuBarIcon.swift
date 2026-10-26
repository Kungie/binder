import AppKit

public enum MenuState: Equatable, Sendable {
    case idle
    case review(Int)
    case handoffSoon
    case observing
}

/// The menu bar item: a monochrome dome, brain and collar, drawn as a template so macOS tints it.
public enum MenuBarIcon {
    public static func image(state: MenuState) -> NSImage {
        let image = NSImage(size: NSSize(width: 18, height: 18), flipped: false) { _ in
            NSColor.black.set()
            let dome = NSBezierPath()
            dome.appendArc(withCenter: NSPoint(x: 9, y: 6), radius: 7, startAngle: 0, endAngle: 180)
            dome.close()
            dome.lineWidth = 1.6
            dome.stroke()
            let brain: CGFloat = state == .observing ? 2 : 3
            NSColor.black.withAlphaComponent(state == .observing ? 0.55 : 1).set()
            NSBezierPath(ovalIn: NSRect(x: 9 - brain, y: 8.5 - brain, width: brain * 2, height: brain * 2)).fill()
            NSColor.black.set()
            NSBezierPath(rect: NSRect(x: 1, y: 4, width: 16, height: 2)).fill()
            if state == .handoffSoon {
                NSBezierPath(rect: NSRect(x: 13.5, y: 12.5, width: 4.5, height: 4.5)).fill()
            }
            return true
        }
        image.isTemplate = true
        image.accessibilityDescription = accessibilityLabel(state: state)
        return image
    }

    /// The count next to the icon when there's something to review.
    public static func title(state: MenuState) -> NSAttributedString? {
        guard case let .review(n) = state, n > 0 else { return nil }
        Fonts.register()
        let font = NSFont(name: "VT323-Regular", size: 17) ?? .monospacedDigitSystemFont(ofSize: 13, weight: .regular)
        return NSAttributedString(string: " \(n)", attributes: [.font: font, .baselineOffset: -1])
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

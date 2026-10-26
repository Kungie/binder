import AppKit
import QuartzCore

/// The amber brain over the menu bar mascot. It lights up while something waits for review, and when new
/// catches arrive it flashes and three small stars twinkle around the dome. Layers, so the template icon
/// underneath keeps following the menu bar's own tint.
@MainActor
final class BrainGlow {
    /// What the glow should do when the inbox goes from `previous` to `current` items.
    /// `previous` is nil the first time, so launching with a full inbox lights the brain without the sparkle.
    static func decide(previous: Int?, current: Int) -> (lit: Bool, sparkle: Bool) {
        (current > 0, previous.map { current > $0 } ?? false)
    }

    /// Where the stars sit on the icon's 18 pt grid (y down), and how big they are.
    static let stars: [(x: CGFloat, y: CGFloat, size: CGFloat, delay: Double)] = [(1.9, 1.9, 3.4, 0), (16.2, 1.4, 2.8, 0.22), (16.4, 5.3, 2.2, 0.45)]

    private let container = CALayer()
    private let brain = CAShapeLayer()
    private var starLayers: [CAShapeLayer] = []
    private(set) var isLit = false

    init() {
        container.masksToBounds = false
        brain.path = MenuBarIcon.brainCGPath
        brain.fillColor = NSColor(srgbRed: 0.949, green: 0.651, blue: 0.047, alpha: 1).cgColor
        brain.shadowColor = brain.fillColor
        brain.shadowOffset = .zero
        brain.shadowRadius = 2
        brain.shadowOpacity = 0.8
        container.addSublayer(brain)
        for star in Self.stars {
            let layer = CAShapeLayer()
            layer.path = Self.starPath(size: star.size)
            layer.fillColor = brain.fillColor
            layer.bounds = CGRect(x: 0, y: 0, width: star.size, height: star.size)
            layer.position = CGPoint(x: star.x, y: star.y)
            layer.opacity = 0
            container.addSublayer(layer)
            starLayers.append(layer)
        }
        container.isHidden = true
    }

    /// Puts the glow over the icon inside `button`.
    func attach(to button: NSStatusBarButton) {
        button.wantsLayer = true
        guard let host = button.layer, container.superlayer !== host else { return }
        host.addSublayer(container)
        // Draw on the same y-down grid as the icon, whichever way the button's layer is flipped.
        container.isGeometryFlipped = !button.isFlipped
    }

    func place(in button: NSStatusBarButton) {
        guard let cell = button.cell as? NSButtonCell else { return }
        let r = cell.imageRect(forBounds: button.bounds)
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        container.frame = CGRect(x: r.midX - MenuBarIcon.size.width / 2, y: r.midY - MenuBarIcon.size.height / 2,
                                 width: MenuBarIcon.size.width, height: MenuBarIcon.size.height)
        container.contentsScale = button.window?.backingScaleFactor ?? 2
        CATransaction.commit()
    }

    func set(lit: Bool, reduceMotion: Bool) {
        guard lit != isLit else { return }
        isLit = lit
        container.isHidden = !lit
        brain.removeAllAnimations()
        guard lit, !reduceMotion else { return }
        let breathe = CABasicAnimation(keyPath: "shadowOpacity")
        breathe.fromValue = 0.35
        breathe.toValue = 1
        breathe.duration = 1.6
        breathe.autoreverses = true
        breathe.repeatCount = .infinity
        breathe.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
        brain.add(breathe, forKey: "breathe")
    }

    /// New context arrived: the brain flickers on brighter and the stars twinkle three times.
    func sparkle(reduceMotion: Bool) {
        guard isLit, !reduceMotion else { return }
        let flash = CAKeyframeAnimation(keyPath: "shadowRadius")
        flash.values = [2, 5, 2.5, 4.5, 2]
        flash.keyTimes = [0, 0.15, 0.35, 0.55, 1]
        flash.duration = 1.4
        brain.add(flash, forKey: "flash")
        let flicker = CAKeyframeAnimation(keyPath: "opacity")
        flicker.values = [0.2, 1, 0.45, 1, 1]
        flicker.keyTimes = [0, 0.1, 0.16, 0.26, 1]
        flicker.duration = 1.4
        brain.add(flicker, forKey: "flicker")
        let now = CACurrentMediaTime()
        for (layer, star) in zip(starLayers, Self.stars) {
            let twinkle = CAAnimationGroup()
            let scale = CAKeyframeAnimation(keyPath: "transform.scale")
            scale.values = [0.1, 1.15, 0.1]
            let fade = CAKeyframeAnimation(keyPath: "opacity")
            fade.values = [0, 1, 0]
            let turn = CABasicAnimation(keyPath: "transform.rotation.z")
            turn.fromValue = 0
            turn.toValue = CGFloat.pi / 2
            twinkle.animations = [scale, fade, turn]
            twinkle.duration = 0.7
            twinkle.repeatCount = 3
            twinkle.beginTime = now + star.delay
            twinkle.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
            layer.add(twinkle, forKey: "twinkle")
        }
    }

    /// A four-pointed star filling a `size` square.
    static func starPath(size s: CGFloat) -> CGPath {
        let p = CGMutablePath()
        let c = s / 2, w = s * 0.13
        p.move(to: CGPoint(x: c, y: 0))
        p.addLine(to: CGPoint(x: c + w, y: c - w))
        p.addLine(to: CGPoint(x: s, y: c))
        p.addLine(to: CGPoint(x: c + w, y: c + w))
        p.addLine(to: CGPoint(x: c, y: s))
        p.addLine(to: CGPoint(x: c - w, y: c + w))
        p.addLine(to: CGPoint(x: 0, y: c))
        p.addLine(to: CGPoint(x: c - w, y: c - w))
        p.closeSubpath()
        return p
    }
}

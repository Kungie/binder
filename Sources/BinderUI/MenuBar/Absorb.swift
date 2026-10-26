import SwiftUI

/// Keep: the card squashes, then is sucked toward the brain on a skewed, shrinking path.
/// 22 % of the time squashing to 0.97 × 0.9, the rest flying to the brain at 0.03 × 0.06, skewed −18°.
struct Absorb: ViewModifier {
    var active: Bool
    /// From the card's top-left corner to the brain's centre, in panel coordinates.
    var target: CGSize
    @StateObject private var stage = Local(0)
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    static let duration = CardPhase.absorbDuration
    static let squash = 0.22

    func body(content: Content) -> some View {
        let squashed = stage.value >= 1, flown = stage.value >= 2
        return content
            .scaleEffect(x: flown ? 0.03 : (squashed ? 0.97 : 1), y: flown ? 0.06 : (squashed ? 0.9 : 1), anchor: UnitPoint(x: 0.12, y: 0))
            .transformEffect(CGAffineTransform(a: 1, b: 0, c: flown ? tan(-18 * .pi / 180) : 0, d: 1, tx: 0, ty: 0))
            .offset(x: flown ? target.width : 0, y: flown ? target.height : 0)
            .opacity(reduceMotion ? (stage.value > 0 ? 0 : 1) : (flown ? 0.15 : 1))
            .onChange(of: active, initial: true) { _, on in
                guard on, stage.value == 0 else { return }
                if reduceMotion {
                    withAnimation(.easeOut(duration: 0.15)) { stage.value = 2 }
                    return
                }
                withAnimation(.easeOut(duration: Self.duration * Self.squash)) { stage.value = 1 }
                DispatchQueue.main.asyncAfter(deadline: .now() + Self.duration * Self.squash) {
                    withAnimation(.timingCurve(0.55, 0, 0.85, 0.35, duration: Self.duration * (1 - Self.squash))) { stage.value = 2 }
                }
            }
    }
}

import SwiftUI

/// Drop: an amber shredder edge bites twice, then the card falls apart in eight strips.
/// Timings follow the design's prototype exactly.
public enum Shred {
    public static let strips = 8
    public static let stripDuration = 0.85
    public static let delays: [Double] = (0..<8).map { (k: Int) -> Double in
        let slot = (k * 37) % 8
        return 0.12 + Double(slot) * 0.035
    }
    public static let rotations: [Double] = (0..<8).map { (k: Int) -> Double in
        let sign = k % 2 == 1 ? 1 : -1
        let degrees = 4 + (k * 3) % 9
        return Double(sign * degrees)
    }
}

/// Eight copies of the card, each masked to its own vertical slice, falling and turning one after another.
struct ShredView<Card: View>: View {
    var card: Card
    @StateObject private var falling = Local(false)
    @StateObject private var bite = Local(false)
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        GeometryReader { geo in
            let w = geo.size.width / CGFloat(Shred.strips)
            ZStack(alignment: .topLeading) {
                if reduceMotion {
                    card.opacity(falling.value ? 0 : 1)
                } else {
                    ForEach(0..<Shred.strips, id: \.self) { k in
                        card
                            .frame(width: geo.size.width, height: geo.size.height)
                            .mask(alignment: .topLeading) {
                                Rectangle().frame(width: w + 0.5, height: geo.size.height).offset(x: CGFloat(k) * w)
                            }
                            .rotationEffect(.degrees(falling.value ? Shred.rotations[k] : 0), anchor: UnitPoint(x: (CGFloat(k) + 0.5) / CGFloat(Shred.strips), y: 0.5))
                            .offset(y: falling.value ? 150 : 0)
                            .opacity(falling.value ? 0 : 1)
                            .animation(.timingCurve(0.5, 0, 0.9, 0.5, duration: Shred.stripDuration).delay(Shred.delays[k]), value: falling.value)
                    }
                    Rectangle()
                        .fill(Phosphor.amber)
                        .frame(height: 4)
                        .padding(.horizontal, -4)
                        .offset(y: -3)
                        .scaleEffect(x: bite.value ? 0.6 : 1, y: 1)
                        .opacity(falling.value ? 0 : 1)
                        .animation(.easeInOut(duration: 0.09).repeatCount(4, autoreverses: true), value: bite.value)
                }
            }
        }
        .onAppear {
            bite.value = true
            withAnimation(.easeOut(duration: reduceMotion ? 0.15 : 0.01)) { falling.value = true }
        }
        .accessibilityLabel("Dropped")
    }
}

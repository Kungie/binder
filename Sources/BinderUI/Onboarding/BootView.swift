import SwiftUI

/// The first screen: the window powers on like a CRT, the brain flickers on, a boot log types out.
/// Timings follow `design/flow/boot.dc.html`. A click or Return skips to the end.
struct BootView: View {
    var start: () -> Void
    @StateObject private var startedAt = Local(Date())
    @Environment(\.isSnapshot) private var isSnapshot
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    static let log: [(String, Double, Bool)] = [
        ("BINDER 1.0", 0.5, true),
        ("CHECKING THE MAC ......... OK", 0.9, false),
        ("WARMING UP THE BRAIN ..... OK", 1.3, false),
        ("CALIBRATING NOSINESS ..... 100%", 1.8, false),
        ("SECRETS .................. NOT INTERESTED", 2.3, false),
        ("READY", 2.8, true)
    ]

    var body: some View {
        VStack(spacing: 0) {
            TitleStrip("BINDER")
            TimelineView(.animation(paused: isSnapshot)) { timeline in
                screen(t: isSnapshot || reduceMotion ? 5 : timeline.date.timeIntervalSince(startedAt.value), now: timeline.date)
            }
        }
        .background(Phosphor.black2)
        .contentShape(Rectangle())
        .onTapGesture { startedAt.value = Date().addingTimeInterval(-5) }
    }

    func clamp(_ x: Double) -> Double { min(max(x, 0), 1) }

    @ViewBuilder
    func screen(t: Double, now: Date) -> some View {
        let crt = clamp((t - 0.15) / 0.35)
        ZStack {
            Phosphor.black
            RadialGradient(colors: [Phosphor.amber.opacity(0.2), .clear], center: UnitPoint(x: 0.3, y: 0.5), startRadius: 0, endRadius: 420)
            Scanlines()
            HStack(spacing: 0) {
                Mascot(mood: .booting(progress: clamp((t - 0.7) / 1.1)), size: 260)
                    .frame(width: 380)
                VStack(alignment: .leading, spacing: 22) {
                    VStack(alignment: .leading, spacing: 2) {
                        ForEach(Self.log, id: \.0) { line, at, bright in
                            HStack(spacing: 0) {
                                Text(line)
                                if line == "READY" {
                                    Text("_").opacity(Int(now.timeIntervalSince1970 * 2) % 2 == 0 || isSnapshot ? 1 : 0)
                                }
                            }
                            .foregroundColor(bright ? Phosphor.cream : Phosphor.amber)
                            .opacity(t >= at ? 1 : 0)
                        }
                    }
                    .font(Fonts.vt(22))
                    .shadow(color: Phosphor.amber.opacity(0.35), radius: 6)
                    SteppedBar(fraction: (clamp((t - 0.4) / 2.6) * 26).rounded(.down) / 26, height: 8)
                    VStack(alignment: .leading, spacing: 10) {
                        Text("Hello. I'm Binder.")
                            .font(Fonts.archivo(36, weight: 900))
                            .foregroundColor(Phosphor.cream)
                            .shadow(color: Phosphor.black, radius: 0, x: 0, y: 3)
                        Text("I remember the boring parts, so your agents stop asking where the site is hosted.")
                            .font(Fonts.grotesk(15))
                            .foregroundColor(Phosphor.cream2)
                    }
                    .opacity(clamp((t - 3.2) / 0.5))
                    .offset(y: 8 * (1 - clamp((t - 3.2) / 0.5)))
                    HStack(spacing: 14) {
                        Button("Let's start", action: start)
                            .buttonStyle(PrimaryButtonStyle(size: 14, height: 40))
                            .keyboardShortcut(.defaultAction)
                        Text("Takes about two minutes.").font(Fonts.grotesk(13)).foregroundColor(Phosphor.dim)
                    }
                    .opacity(clamp((t - 3.5) / 0.5))
                }
                .padding(.top, 40)
                .padding(.trailing, 56)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .scaleEffect(x: 1, y: max(crt, 0.008), anchor: .center)
        .opacity(t < 0.15 ? 0 : 1)
    }
}

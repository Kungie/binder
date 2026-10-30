import AppKit
import SwiftUI

/// The app icon: Binder's dome and brain on black CRT glass with an amber glow, on the macOS icon grid
/// (an 824 pt rounded square inside 1024, corner radius 185).
public struct AppIcon: View {
    public init() {}

    public var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 185, style: .continuous)
                .fill(Phosphor.black)
                .overlay(
                    RadialGradient(colors: [Phosphor.amber.opacity(0.42), .clear], center: UnitPoint(x: 0.5, y: 0.42), startRadius: 0, endRadius: 420)
                        .clipShape(RoundedRectangle(cornerRadius: 185, style: .continuous))
                )
                .overlay(Scanlines(opacity: 0.3).clipShape(RoundedRectangle(cornerRadius: 185, style: .continuous)))
                .overlay(RoundedRectangle(cornerRadius: 185, style: .continuous).strokeBorder(Phosphor.line, lineWidth: 6))
                .frame(width: 824, height: 824)
            MascotHead(size: 600, rimWidth: 7).offset(y: 20)
        }
        .frame(width: 1024, height: 1024)
    }

    @MainActor
    public static func image(pixels: Int) -> NSImage? {
        Snapshot.image(AppIcon(), size: CGSize(width: 1024, height: 1024), scale: CGFloat(pixels) / 1024)
    }
}

/// The installer window's background (640 × 420): Finder draws Binder.app at (170, 200) and Applications at (470, 200);
/// amber chevrons point from one to the other, and the mascot's dome peeks up from the bottom edge.
public struct DMGBackground: View {
    public init() {}

    public var body: some View {
        ZStack {
            Phosphor.black
            RadialGradient(colors: [Phosphor.amber.opacity(0.16), .clear], center: UnitPoint(x: 0.5, y: 0.48), startRadius: 0, endRadius: 330)
            Scanlines()
            VStack(spacing: 8) {
                Text("Drag Binder into Applications.")
                    .font(Fonts.archivo(22, weight: 900))
                    .foregroundColor(Phosphor.cream)
                    .shadow(color: Phosphor.black, radius: 0, x: 0, y: 2)
                    .shadow(color: Phosphor.amber, radius: 0, x: 0, y: 4)
                Text("Then open it from there. I'll take it from here.").font(Fonts.grotesk(13)).foregroundColor(Phosphor.cream2)
            }
            .position(x: 320, y: 62)
            HStack(spacing: 10) {
                ForEach([1.0, 0.65, 0.35], id: \.self) { opacity in
                    Chevron().stroke(Phosphor.amber, lineWidth: 5).frame(width: 22, height: 28).opacity(opacity)
                }
            }
            .position(x: 320, y: 200)
            MascotHead(size: 150).position(x: 320, y: 420 - 34)
            Text("v1.0.7").font(Fonts.vt(18)).foregroundColor(Phosphor.dim).position(x: 610, y: 398)
        }
        .frame(width: 640, height: 420)
        .clipped()
    }

    struct Chevron: Shape {
        func path(in r: CGRect) -> Path {
            var p = Path()
            p.move(to: CGPoint(x: r.minX + 2, y: r.minY + 2))
            p.addLine(to: CGPoint(x: r.maxX - 4, y: r.midY))
            p.addLine(to: CGPoint(x: r.minX + 2, y: r.maxY - 2))
            return p
        }
    }
}

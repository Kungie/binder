import AppKit
import SwiftUI

/// `BinderApp snapshot <dir>`: renders screens with demo data to PNGs, for checking them against the design.
@MainActor
public enum Snapshot {
    public static func image<V: View>(_ view: V, size: CGSize, scale: CGFloat = 2) -> NSImage? {
        Fonts.register()
        let renderer = ImageRenderer(content: view.frame(width: size.width, height: size.height).environment(\.isSnapshot, true))
        renderer.scale = scale
        return renderer.nsImage
    }

    public static func render<V: View>(_ view: V, size: CGSize, to url: URL, scale: CGFloat = 2) throws {
        guard let image = image(view, size: size, scale: scale), let tiff = image.tiffRepresentation,
              let png = NSBitmapImageRep(data: tiff)?.representation(using: .png, properties: [:])
        else { throw CocoaError(.fileWriteUnknown) }
        try png.write(to: url)
    }

    /// True when some pixel isn't fully transparent.
    public static func hasInk(_ rep: NSBitmapImageRep) -> Bool {
        for y in stride(from: 0, to: rep.pixelsHigh, by: 4) {
            for x in stride(from: 0, to: rep.pixelsWide, by: 4) where (rep.colorAt(x: x, y: y)?.alphaComponent ?? 0) > 0.05 {
                return true
            }
        }
        return false
    }

    public static func all(to dir: URL) {
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        for (name, view, size) in shots() {
            do {
                try render(view, size: size, to: dir.appendingPathComponent("\(name).png"))
                print("wrote \(name).png")
            } catch {
                print("could not render \(name)")
            }
        }
    }

    /// Every screen Binder can render without a running app. Later tasks add theirs here.
    static func shots() -> [(String, AnyView, CGSize)] {
        [
            ("mascot-moods", AnyView(MoodSheet()), CGSize(width: 1240, height: 260)),
            ("components", AnyView(ComponentSheet()), CGSize(width: 720, height: 300))
        ] + DemoVault.Kind.allCases.map { kind in
            ("panel-\(kind.rawValue)", AnyView(PanelContainer(model: DemoVault.model(kind)).frame(maxHeight: .infinity, alignment: .top).background(Phosphor.black)),
             CGSize(width: PanelView.width, height: 820))
        }
    }
}

/// Every mood side by side, for review.
struct MoodSheet: View {
    var body: some View {
        HStack(spacing: 20) {
            ForEach(Array([Mascot.Mood.idle, .booting(progress: 0.15), .scanning, .happy, .writing, .plugging, .holdingKey].enumerated()), id: \.offset) { _, mood in
                Mascot(mood: mood, size: 150)
            }
        }
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Phosphor.black)
    }
}

/// Buttons, labels, chips and a screen panel.
struct ComponentSheet: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack(spacing: 12) {
                Button("Let's start") {}.buttonStyle(PrimaryButtonStyle(size: 14, height: 40))
                Button("Not yet") {}.buttonStyle(GhostButtonStyle())
                Button("Keep") {}.buttonStyle(CreamButtonStyle())
                Button("Drop") {}.buttonStyle(QuietButtonStyle())
                MascotHead(size: 64)
            }
            HStack(spacing: 8) {
                Chip("Short answers", on: true) {}
                Chip("Explain trade-offs", on: false) {}
                SectionLabel("Caught this session")
            }
            ScreenPanel {
                VStack(alignment: .leading, spacing: 8) {
                    SectionLabel("This session · dawn · Claude Code")
                    Text("71%").font(Fonts.vt(56)).foregroundColor(Phosphor.amber)
                    SteppedBar(fraction: 0.71)
                }
            }
        }
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(Phosphor.black2)
    }
}

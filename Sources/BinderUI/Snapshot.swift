import AppKit
import BinderCore
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
        } + OnboardingStep.allCases.map { step in
            ("onboarding-\(step.rawValue)-\(step.railLabel.lowercased().replacingOccurrences(of: " ", with: "-"))",
             AnyView(OnboardingView(model: demoOnboarding(step))), CGSize(width: 880, height: 720))
        } + [
            ("readme", AnyView(ReadmeScene()), CGSize(width: 400, height: 860)),
            ("mascot", AnyView(Mascot(mood: .idle, size: 240)), CGSize(width: 240, height: 280)),
            ("library", AnyView(demoLibrary(.projects)), CGSize(width: 1240, height: 820)),
            ("library-settings", AnyView(demoLibrary(.settings)), CGSize(width: 1240, height: 820)),
            ("library-graph", AnyView(demoLibrary(.graph)), CGSize(width: 1240, height: 820))
        ]
    }
}

@MainActor
func demoOnboarding(_ step: OnboardingStep) -> OnboardingModel {
    let home = FileManager.default.temporaryDirectory.appendingPathComponent("binder-demo-home-\(UUID().uuidString)")
    let model = OnboardingModel(home: home, runner: SystemRunner(), permissions: SystemPermissions(),
                                settingsURL: home.appendingPathComponent(".claude/settings.json"), binderPath: "/Applications/Binder.app/Contents/Helpers/binder")
    model.loadDemo(step: step)
    return model
}

@MainActor
func demoLibrary(_ kind: SectionKind) -> some View {
    let app = DemoVault.model(.review)
    let vault = app.vault
    _ = try? Handoff.write(HandoffCard(done: ["Panel redesign merged"], left: ["Notarization"], dontTouch: ["Resources/Fonts"],
                                       next: "Release notes for 0.3."), project: "dawn", vault: vault, now: Date())
    try? NoteIO.update(vault.url("projects/dawn.md"), title: "dawn") {
        $0.replacingObserved(["- repo: github.com/kungie/dawn · main · clean", "- stack: Swift package, SwiftUI menu bar app",
                              "- release: GitHub Releases, zip and dmg", "- site: Cloudflare Pages, see [[site]]", "- builds on: [[this-mac]]"])
            .setting("verified", ISO8601DateFormatter().string(from: Date())).setting("source", "observed, you")
    }
    try? NoteIO.write(Note.new(title: "Me").replacingObserved([]), to: vault.url("me.md"))
    let library = LibraryModel(vault: vault, home: FileManager.default.temporaryDirectory)
    library.selectedKind = kind
    return LibraryView(model: library, app: app)
}

/// The README's picture: a dark menu bar with Binder's item highlighted and its panel hanging under it.
struct ReadmeScene: View {
    /// The lit brain `BrainGlow` lays over the menu bar icon while something waits for review.
    static var amberBrain: NSImage {
        NSImage(size: MenuBarIcon.size, flipped: true) { _ in
            NSColor(Phosphor.amber).setFill()
            MenuBarIcon.brainPath.fill()
            return true
        }
    }

    var body: some View {
        let model = DemoVault.model(.review)
        return VStack(alignment: .trailing, spacing: 0) {
            HStack(spacing: 16) {
                Spacer()
                Text("Mon 09:41").font(.system(size: 13, weight: .medium)).foregroundColor(.white.opacity(0.85))
                HStack(spacing: 1) {
                    ZStack {
                        Image(nsImage: MenuBarIcon.image(state: .review(4), brain: false)).renderingMode(.template).foregroundColor(.white)
                        Image(nsImage: Self.amberBrain)
                    }
                    Text("4").font(Fonts.vt(17)).foregroundColor(.white)
                }
                .padding(.horizontal, 7).frame(height: 22)
                .background(Color.white.opacity(0.18))
                .clipShape(RoundedRectangle(cornerRadius: 4))
            }
            .padding(.horizontal, 14)
            .frame(height: 30)
            .background(Color(white: 0.12))
            // The notch points at the icon, which sits about 324 pt into the panel; its tip 3 pt under the bar.
            PanelContainer(model: model, geometry: PanelGeometry(arrowX: 324))
                .padding(.trailing, 14)
                .padding(.top, 3)
                .shadow(color: .black.opacity(0.6), radius: 24, y: 12)
            Spacer(minLength: 0)
        }
        .frame(width: 400, height: 860, alignment: .top)
        .background(
            LinearGradient(colors: [Color(hex: 0x1B1210), Phosphor.black], startPoint: .top, endPoint: .bottom)
        )
        .clipShape(RoundedRectangle(cornerRadius: 10))
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

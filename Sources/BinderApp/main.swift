import AppKit
import BinderUI

// Binder is one app binary with a few ways in:
//   Binder                          the menu bar app
//   Binder snapshot <dir>           renders screens with demo data to PNGs
//   Binder icon <file>              renders the 1024 px app icon (used by scripts/build-app.sh)
//   Binder dmg-background <file>    renders the installer window's background at 2x
let args = CommandLine.arguments
switch args.dropFirst().first {
case "snapshot" where args.count > 2:
    _ = NSApplication.shared
    MainActor.assumeIsolated { Snapshot.all(to: URL(fileURLWithPath: args[2])) }
    exit(0)
case "icon" where args.count > 2:
    _ = NSApplication.shared
    MainActor.assumeIsolated {
        guard let image = AppIcon.image(pixels: 1024), let tiff = image.tiffRepresentation,
              let png = NSBitmapImageRep(data: tiff)?.representation(using: .png, properties: [:]) else { exit(1) }
        try? png.write(to: URL(fileURLWithPath: args[2]))
    }
    exit(0)
case "dmg-background" where args.count > 2:
    _ = NSApplication.shared
    MainActor.assumeIsolated {
        let base = URL(fileURLWithPath: args[2])
        try? Snapshot.render(DMGBackground(), size: CGSize(width: 640, height: 420), to: base, scale: 1)
        let retina = base.deletingPathExtension().path + "@2x.png"
        try? Snapshot.render(DMGBackground(), size: CGSize(width: 640, height: 420), to: URL(fileURLWithPath: retina), scale: 2)
    }
    exit(0)
default:
    let app = NSApplication.shared
    let delegate = MainActor.assumeIsolated { AppDelegate() }
    app.delegate = delegate
    app.run()
}

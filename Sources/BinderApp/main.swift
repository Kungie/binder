import AppKit
import BinderUI

// Binder is one app binary with a few ways in:
//   Binder                    the menu bar app
//   Binder snapshot <dir>     renders screens with demo data to PNGs
let args = CommandLine.arguments
if args.count > 2, args[1] == "snapshot" {
    _ = NSApplication.shared
    MainActor.assumeIsolated { Snapshot.all(to: URL(fileURLWithPath: args[2])) }
    exit(0)
}

let app = NSApplication.shared
app.setActivationPolicy(.accessory)
app.run()

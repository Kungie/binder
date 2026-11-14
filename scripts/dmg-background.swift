// Renders the DMG window's background: `swift scripts/dmg-background.swift app/dmg/background.svg app/dmg/background.tiff`.
// Draws the SVG at 1x and 2x with AppKit and joins them into one TIFF, so Finder picks the sharp one on Retina.
import AppKit

let args = CommandLine.arguments
guard args.count == 3, let svg = NSImage(contentsOfFile: args[1]) else {
    FileHandle.standardError.write("usage: dmg-background.swift <in.svg> <out.tiff>\n".data(using: .utf8)!)
    exit(1)
}

let size = NSSize(width: 660, height: 400)
let tmp = FileManager.default.temporaryDirectory.appendingPathComponent("binder-dmg-\(getpid())")
try FileManager.default.createDirectory(at: tmp, withIntermediateDirectories: true)
defer { try? FileManager.default.removeItem(at: tmp) }

func render(scale: Int) throws -> String {
    let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: Int(size.width) * scale, pixelsHigh: Int(size.height) * scale,
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
        colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    rep.size = size
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    svg.draw(in: NSRect(origin: .zero, size: size))
    NSGraphicsContext.restoreGraphicsState()
    let path = tmp.appendingPathComponent("\(scale)x.png").path
    try rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: path))
    return path
}

let one = try render(scale: 1)
let two = try render(scale: 2)
let tiffutil = Process()
tiffutil.executableURL = URL(fileURLWithPath: "/usr/bin/tiffutil")
tiffutil.arguments = ["-cathidpicheck", one, two, "-out", args[2]]
try tiffutil.run()
tiffutil.waitUntilExit()
exit(tiffutil.terminationStatus)

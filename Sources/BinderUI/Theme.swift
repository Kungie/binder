import AppKit
import CoreText
import SwiftUI

/// The Phosphor palette: black CRT glass, cream text, amber phosphor. No red in the app.
public enum Phosphor {
    public static let black = Color(hex: 0x050303)
    public static let black2 = Color(hex: 0x0E0908)
    public static let line = Color(hex: 0x2A1B19)
    public static let cream = Color(hex: 0xF3E9D6)
    public static let cream2 = Color(hex: 0xE6D9BE)
    public static let sand = Color(hex: 0xD2C09B)
    public static let dim = Color(hex: 0xA09189)
    public static let amber = Color(hex: 0xF2A60C)
    public static let amberDeep = Color(hex: 0x8F5A00)
    public static let gyri = Color(hex: 0x5A3A00)
    public static let ink = Color(hex: 0x160D0B)
    public static let dome = Color(hex: 0x160D0B)
}

public extension Color {
    init(hex: UInt32) {
        self.init(red: Double((hex >> 16) & 0xFF) / 255, green: Double((hex >> 8) & 0xFF) / 255, blue: Double(hex & 0xFF) / 255)
    }
}

/// The bundled fonts (SIL Open Font License), registered for this process only.
/// Every accessor falls back to a system font, so a build without them still reads fine.
public enum Fonts {
    nonisolated(unsafe) private static var registered = false

    public static func register() {
        guard !registered else { return }
        registered = true
        // Inside Binder.app the fonts sit in Contents/Resources/Fonts. Only outside an app bundle (tests, snapshots
        // from the build folder) do we touch Bundle.module, whose accessor stops the process when the bundle is missing.
        let inApp = Bundle.main.resourceURL?.appendingPathComponent("Fonts")
        let dirs = inApp.map { FileManager.default.fileExists(atPath: $0.path) } == true ? [inApp!] : [Bundle.module.resourceURL?.appendingPathComponent("Fonts")].compactMap { $0 }
        for dir in dirs {
            let files = (try? FileManager.default.contentsOfDirectory(at: dir, includingPropertiesForKeys: nil)) ?? []
            for file in files where ["woff2", "ttf", "otf"].contains(file.pathExtension) {
                CTFontManagerRegisterFontsForURL(file as CFURL, .process, nil)
            }
        }
    }

    private static let wght = 0x7767_6874 // 'wght'
    private static let wdth = 0x7764_7468 // 'wdth'

    private static func variable(_ name: String, family: String, size: CGFloat, axes: [Int: CGFloat]) -> CTFont? {
        let base = CTFontDescriptorCreateWithNameAndSize(name as CFString, size)
        let values = axes.reduce(into: [NSNumber: NSNumber]()) { $0[NSNumber(value: $1.key)] = NSNumber(value: Double($1.value)) }
        let font = CTFontCreateWithFontDescriptor(
            CTFontDescriptorCreateCopyWithAttributes(base, [kCTFontVariationAttribute: values] as CFDictionary), size, nil)
        return (CTFontCopyFamilyName(font) as String).hasPrefix(family) ? font : nil
    }

    /// Archivo at 125 % width: titles and buttons.
    public static func archivo(_ size: CGFloat, weight: CGFloat = 800) -> Font {
        register()
        if let f = variable("ArchivoRoman-Regular", family: "Archivo", size: size, axes: [wdth: 125, wght: weight]) { return Font(f) }
        return .system(size: size, weight: .black).width(.expanded)
    }

    /// VT323: anything on a screen.
    public static func vt(_ size: CGFloat) -> Font {
        register()
        return NSFont(name: "VT323-Regular", size: size) != nil ? .custom("VT323-Regular", size: size) : .system(size: size * 0.8, design: .monospaced)
    }

    /// Space Grotesk: body text.
    public static func grotesk(_ size: CGFloat, weight: CGFloat = 400) -> Font {
        register()
        if let f = variable("SpaceGrotesk-Light", family: "Space Grotesk", size: size, axes: [wght: weight]) { return Font(f) }
        return .system(size: size)
    }

    /// Space Mono Bold, for the small uppercase labels.
    public static func mono(_ size: CGFloat) -> Font {
        register()
        return NSFont(name: "SpaceMono-Bold", size: size) != nil ? .custom("SpaceMono-Bold", size: size) : .system(size: size, weight: .bold, design: .monospaced)
    }
}

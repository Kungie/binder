import AppKit
import Testing
@testable import BinderUI

@MainActor
@Suite struct IconTests {
    @Test func iconRenders1024() throws {
        let image = try #require(AppIcon.image(pixels: 1024))
        let rep = try #require(NSBitmapImageRep(data: image.tiffRepresentation!))
        #expect(rep.pixelsWide == 1024 && rep.pixelsHigh == 1024)
        #expect(Snapshot.hasInk(rep))
        // The corners are transparent: it's a rounded square, not a full bleed.
        #expect((rep.colorAt(x: 2, y: 2)?.alphaComponent ?? 1) < 0.05)
    }

    @Test func dmgBackgroundRenders() throws {
        let image = try #require(Snapshot.image(DMGBackground(), size: CGSize(width: 640, height: 420)))
        let rep = try #require(NSBitmapImageRep(data: image.tiffRepresentation!))
        #expect(rep.pixelsWide == 1280 && Snapshot.hasInk(rep))
    }
}

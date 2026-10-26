import AppKit
import SwiftUI
import Testing
@testable import BinderUI

@MainActor
@Suite struct MascotTests {
    @Test func mascotRendersEveryMood() throws {
        let moods: [Mascot.Mood] = [.idle, .booting(progress: 0.5), .scanning, .happy, .writing, .plugging, .holdingKey]
        for mood in moods {
            let image = try #require(Snapshot.image(Mascot(mood: mood, size: 120), size: CGSize(width: 120, height: 140)), "\(mood)")
            let rep = try #require(image.representations.first as? NSBitmapImageRep ?? NSBitmapImageRep(data: image.tiffRepresentation!))
            #expect(rep.pixelsWide == 240 && rep.pixelsHigh == 280, "\(mood)")
            #expect(Snapshot.hasInk(rep), "\(mood) rendered blank")
        }
    }

    @Test func fontsRegisterFromBundle() {
        Fonts.register()
        #expect(NSFont(name: "VT323-Regular", size: 12) != nil)
        #expect(NSFont(name: "SpaceMono-Bold", size: 12) != nil)
    }
}

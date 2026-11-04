import Foundation
import Testing
@testable import BinderCore
@testable import BinderUI

@MainActor
@Suite struct LibrarySearchTests {
    @Test func sidebarMatchesContent() throws {
        let vault = try fixtureVault()
        try Memory.add("Built with Astro.", to: "projects/dawn.md", vault: vault)
        try Memory.add("Behind a Cloudflare Tunnel.", to: "projects/dawn/hosting.md", vault: vault)
        let m = LibraryModel(vault: vault, home: makeTempDir("home"))
        #expect(m.matches("astro")["projects/dawn.md"] == "Built with Astro.")
        // A topic note's hit shows on its project's row.
        #expect(m.matches("tunnel")["projects/dawn.md"] == "Behind a Cloudflare Tunnel.")
        #expect(m.matches("").isEmpty)
    }
}

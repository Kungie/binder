import AppKit
import SwiftUI
import Testing
@testable import BinderCore
@testable import BinderUI

/// Scrolling content must stay under the title strip: a scroll view reaching into the title bar
/// draws its content over the strip as you scroll.
@MainActor
@Suite(.serialized) struct LibraryWindowTests {
    func window(showing kind: SectionKind) throws -> NSWindow {
        let vault = Vault(root: makeTempDir("window"))
        try vault.ensureLayout()
        for i in 0..<30 {
            try ProposalStore(vault: vault).add(Proposal.make(kind: .decision, source: .command, text: "Catch number \(i) from this session.",
                                                              evidence: "x", target: "projects/dawn.md", project: "dawn", session: "s"))
        }
        let app = AppModel(vault: vault, home: makeTempDir("home"))
        app.refresh()
        let library = LibraryModel(vault: vault, home: makeTempDir("home"))
        library.selectedKind = kind
        let window = try #require(LibraryWindowController(model: library, app: app, actions: LibraryActions()).window)
        window.setFrame(NSRect(x: 100, y: 100, width: 1240, height: 820), display: true)
        window.orderFront(nil)
        RunLoop.current.run(until: Date().addingTimeInterval(0.2))
        window.contentView?.layoutSubtreeIfNeeded()
        return window
    }

    func scrollViews(in view: NSView) -> [NSScrollView] {
        (view as? NSScrollView).map { [$0] } ?? view.subviews.flatMap(scrollViews)
    }

    @Test(arguments: [SectionKind.inbox, .settings])
    func scrollViewsStopBelowTheTitleBar(kind: SectionKind) throws {
        let window = try window(showing: kind)
        defer { window.orderOut(nil) }
        let content = try #require(window.contentView)
        let views = scrollViews(in: content)
        #expect(!views.isEmpty)
        let limit = window.frame.height - window.titlebarHeight
        for view in views {
            let top = view.convert(view.bounds, to: nil).maxY
            #expect(top <= limit, "scroll view top \(top), title bar starts at \(limit)")
            #expect(view.contentInsets.top == 0)
        }
    }
}

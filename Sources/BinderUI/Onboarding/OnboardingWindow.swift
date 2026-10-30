import AppKit
import SwiftUI

/// The setup window's content: one step at a time.
public struct OnboardingView: View {
    @ObservedObject var model: OnboardingModel
    var openSession: (String) -> Void
    var done: () -> Void

    public init(model: OnboardingModel, openSession: @escaping (String) -> Void = { _ in }, done: @escaping () -> Void = {}) {
        self.model = model
        self.openSession = openSession
        self.done = done
    }

    public var body: some View {
        Group {
            switch model.step {
            case .boot: BootView { model.next() }
            case .permissions: PermissionsView(model: model)
            case .lookAround: LookAroundView(model: model)
            case .aboutYou: AboutYouView(model: model)
            case .vault: VaultView(model: model)
            case .connect: ConnectView(model: model, openSession: openSession, done: done)
            }
        }
        .frame(minWidth: 880, minHeight: 720)
        .background(Phosphor.black2)
        .environment(\.colorScheme, .dark)
    }
}

/// A dark, full-size-content window for setup; the traffic lights sit on Binder's own title strip.
@MainActor
public final class OnboardingWindowController: NSWindowController {
    public init(model: OnboardingModel, openSession: @escaping (String) -> Void, done: @escaping () -> Void) {
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 880, height: 720),
                              styleMask: [.titled, .closable, .miniaturizable, .fullSizeContentView], backing: .buffered, defer: false)
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .hidden
        window.title = "Binder setup"
        window.appearance = NSAppearance(named: .darkAqua)
        window.backgroundColor = NSColor(Phosphor.black)
        window.isReleasedWhenClosed = false
        super.init(window: window)
        let hosting = NSHostingView(rootView: OnboardingView(model: model, openSession: openSession, done: { [weak window] in
            window?.close()
            done()
        }).environment(\.titlebarHeight, window.titlebarHeight))
        hosting.safeAreaRegions = [] // the title strip is the title bar; see LibraryWindowController
        window.contentView = hosting
        window.center()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError() }
}

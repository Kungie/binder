import AppKit
import SwiftUI

/// Says whether the view's window is really in front of the user: on screen, not covered or minimized, with
/// Binder the active app. SwiftUI's timelines don't stop by themselves: one keeps drawing in a closed window.
struct ScreenProbe: NSViewRepresentable {
    var changed: (Bool) -> Void

    static func live(appActive: Bool, occlusion: NSWindow.OcclusionState) -> Bool {
        appActive && occlusion.contains(.visible)
    }

    func makeNSView(context: Context) -> Probe { Probe(changed) }

    func updateNSView(_ view: Probe, context: Context) { view.changed = changed }

    final class Probe: NSView {
        var changed: (Bool) -> Void
        private var last: Bool?

        init(_ changed: @escaping (Bool) -> Void) {
            self.changed = changed
            super.init(frame: .zero)
            let center = NotificationCenter.default
            center.addObserver(self, selector: #selector(report), name: NSApplication.didBecomeActiveNotification, object: nil)
            center.addObserver(self, selector: #selector(report), name: NSApplication.didResignActiveNotification, object: nil)
        }

        @available(*, unavailable)
        required init?(coder: NSCoder) { fatalError() }

        override func viewWillMove(toWindow newWindow: NSWindow?) {
            let center = NotificationCenter.default
            center.removeObserver(self, name: NSWindow.didChangeOcclusionStateNotification, object: window)
            if let newWindow {
                center.addObserver(self, selector: #selector(report), name: NSWindow.didChangeOcclusionStateNotification, object: newWindow)
            }
        }

        override func viewDidMoveToWindow() {
            // After this update pass: the answer changes SwiftUI state.
            DispatchQueue.main.async { [weak self] in self?.report() }
        }

        @objc private func report() {
            let live = window.map { ScreenProbe.live(appActive: NSApp.isActive, occlusion: $0.occlusionState) } ?? false
            guard live != last else { return }
            last = live
            changed(live)
        }
    }
}

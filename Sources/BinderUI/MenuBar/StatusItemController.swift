import AppKit
import Combine
import SwiftUI

/// Binder's menu bar item and its drop-down panel: a plain borderless panel pinned under the item,
/// exactly as tall as its content (the same approach as Dawn, instead of MenuBarExtra's fixed glass window).
@MainActor
public final class StatusItemController: NSObject {
    private let model: AppModel
    private let statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
    private let hosting: NSHostingController<PanelContainer>
    private let geometry: PanelGeometry
    private let panel: BinderPanel
    private var sizeObservation: NSKeyValueObservation?
    private var changes: AnyCancellable?
    private var outsideClick: Any?
    private var escapeKey: Any?
    private let glow = BrainGlow()
    private var lastPending: Int?
    public var observing = false { didSet { updateItem() } }

    public init(model: AppModel, actions: PanelActions) {
        self.model = model
        let geometry = PanelGeometry()
        hosting = NSHostingController(rootView: PanelContainer(model: model, geometry: geometry, actions: actions))
        hosting.sizingOptions = [.preferredContentSize]
        panel = BinderPanel(contentRect: NSRect(x: 0, y: 0, width: PanelView.width, height: 400),
                            styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: true)
        self.geometry = geometry
        super.init()
        hosting.view.autoresizingMask = [.width, .height]
        panel.contentView = hosting.view
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.level = .statusBar
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .transient, .ignoresCycle]
        panel.isReleasedWhenClosed = false
        panel.hidesOnDeactivate = false
        panel.isMovable = false

        if let button = statusItem.button {
            button.target = self
            button.action = #selector(toggle)
            button.imagePosition = .imageLeading
            button.imageHugsTitle = true
            glow.attach(to: button)
        }
        updateItem()
        sizeObservation = hosting.observe(\.preferredContentSize, options: [.new]) { [weak self] _, _ in
            Task { @MainActor in self?.place() }
        }
        NotificationCenter.default.addObserver(forName: NSApplication.didChangeScreenParametersNotification, object: nil, queue: .main) { [weak self] _ in
            Task { @MainActor in self?.place() }
        }
        changes = model.objectWillChange
            .debounce(for: .milliseconds(30), scheduler: RunLoop.main)
            .sink { [weak self] _ in self?.updateItem() }
    }

    var menuState: MenuState {
        if observing { return .observing }
        if model.handoffSuggested { return .handoffSoon }
        return model.pendingCount > 0 ? .review(model.pendingCount) : .idle
    }

    private func updateItem() {
        guard let button = statusItem.button else { return }
        let state = menuState
        let pending = model.pendingCount
        let (lit, sparkle) = BrainGlow.decide(previous: lastPending, current: pending)
        lastPending = pending
        let reduceMotion = NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
        button.image = MenuBarIcon.image(state: state, brain: !lit)
        button.attributedTitle = MenuBarIcon.title(state: state) ?? NSAttributedString(string: "")
        button.setAccessibilityLabel(MenuBarIcon.accessibilityLabel(state: state))
        button.layoutSubtreeIfNeeded()
        glow.place(in: button)
        glow.set(lit: lit, reduceMotion: reduceMotion)
        if sparkle { glow.sparkle(reduceMotion: reduceMotion) }
        if panel.isVisible { place() }
    }

    @objc private func toggle() {
        panel.isVisible ? hide() : show()
    }

    public func show() {
        model.refresh()
        hosting.view.layoutSubtreeIfNeeded()
        place()
        panel.orderFrontRegardless()
        panel.makeKey()
        statusItem.button?.highlight(true)
        outsideClick = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak self] _ in
            Task { @MainActor in self?.hide() }
        }
        escapeKey = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            if event.keyCode == 53 { self?.hide(); return nil }
            return event
        }
    }

    public func hide() {
        panel.orderOut(nil)
        statusItem.button?.highlight(false)
        if let m = outsideClick { NSEvent.removeMonitor(m) }
        if let m = escapeKey { NSEvent.removeMonitor(m) }
        outsideClick = nil
        escapeKey = nil
    }

    /// Notch tip a hair under the menu bar, panel centred under the icon and kept inside the screen;
    /// if the panel has to shift to stay on screen, the notch moves so it still points at the icon.
    private func place() {
        guard let button = statusItem.button, let window = button.window else { return }
        let anchor = window.convertToScreen(button.convert(button.bounds, to: nil))
        var size = hosting.preferredContentSize
        if size.height < 1 { size = hosting.view.fittingSize }
        size.width = PanelView.width
        let screen = window.screen?.visibleFrame ?? NSScreen.main?.visibleFrame ?? .zero
        let x = min(max(anchor.midX - size.width / 2, screen.minX + 6), screen.maxX - size.width - 6)
        // The status item's window spans the full menu bar height, so its bottom is the menu bar's edge.
        let top = window.frame.minY - 3
        geometry.arrowX = min(max(anchor.midX - x, 16), size.width - 16)
        panel.setFrame(NSRect(x: x, y: top - size.height, width: size.width, height: size.height), display: true)
    }
}

/// A borderless panel that takes clicks and typing (Edit) without activating the app.
final class BinderPanel: NSPanel {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }
}

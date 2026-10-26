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
    private let panel: BinderPanel
    private var sizeObservation: NSKeyValueObservation?
    private var changes: AnyCancellable?
    private var outsideClick: Any?
    private var escapeKey: Any?
    public var observing = false { didSet { updateItem() } }

    public init(model: AppModel, actions: PanelActions) {
        self.model = model
        hosting = NSHostingController(rootView: PanelContainer(model: model, actions: actions))
        hosting.sizingOptions = [.preferredContentSize]
        panel = BinderPanel(contentRect: NSRect(x: 0, y: 0, width: PanelView.width, height: 400),
                            styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: true)
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
        button.image = MenuBarIcon.image(state: state)
        button.attributedTitle = MenuBarIcon.title(state: state) ?? NSAttributedString(string: "")
        button.setAccessibilityLabel(MenuBarIcon.accessibilityLabel(state: state))
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

    /// Centred under the item, kept inside the screen, top edge just under the menu bar.
    private func place() {
        guard let button = statusItem.button, let window = button.window else { return }
        let anchor = window.convertToScreen(button.convert(button.bounds, to: nil))
        var size = hosting.preferredContentSize
        if size.height < 1 { size = hosting.view.fittingSize }
        size.width = PanelView.width
        let screen = window.screen?.visibleFrame ?? NSScreen.main?.visibleFrame ?? .zero
        let x = min(max(anchor.midX - size.width / 2, screen.minX + 6), screen.maxX - size.width - 6)
        let top = window.frame.minY - 4
        panel.setFrame(NSRect(x: x, y: top - size.height, width: size.width, height: size.height), display: true)
    }
}

/// A borderless panel that takes clicks and typing (Edit) without activating the app.
final class BinderPanel: NSPanel {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }
}

import AppKit
import BinderCore
import Combine
import SwiftUI

/// Starts setup the first time, and the menu bar, observers and handoff nudges every time after that.
@MainActor
public final class AppDelegate: NSObject, NSApplicationDelegate {
    let home = FileManager.default.homeDirectoryForCurrentUser
    var onboarding: OnboardingWindowController?
    var library: LibraryWindowController?
    var libraryModel: LibraryModel?
    var status: StatusItemController?
    var app: AppModel?
    var scheduler: ObserveScheduler?
    let notifier = HandoffNotifier()
    var sessionWatch: AnyCancellable?

    public override init() { super.init() }

    /// The CLI inside the bundle, which the hooks call.
    var helperPath: String { Bundle.main.bundleURL.appendingPathComponent("Contents/Helpers/binder").path }
    var installer: HookInstaller { HookInstaller(settingsURL: HookInstaller.defaultSettingsURL, binderPath: helperPath) }
    var vault: Vault { Vault(root: VaultLocation.resolve(env: ProcessInfo.processInfo.environment, home: home)) }

    public func applicationDidFinishLaunching(_ notification: Notification) {
        Fonts.register()
        NSApp.setActivationPolicy(.regular)
        NSApp.mainMenu = MainMenu.make(delegate: self)
        if OnboardingModel.isOnboarded(home: home) { startApp() } else { showOnboarding() }
    }

    public func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        if app != nil { showLibrary() } else { onboarding?.showWindow(nil) }
        return true
    }

    func showOnboarding() {
        let model = OnboardingModel(home: home, runner: SystemRunner(), permissions: SystemPermissions(),
                                    settingsURL: HookInstaller.defaultSettingsURL, binderPath: helperPath)
        onboarding = OnboardingWindowController(model: model, openSession: { path in
            SessionLauncher.launch(in: URL(fileURLWithPath: (path as NSString).expandingTildeInPath))
        }, done: { [weak self] in
            self?.onboarding = nil
            self?.startApp()
        })
        NSApp.activate(ignoringOtherApps: true)
        onboarding?.showWindow(nil)
    }

    func startApp() {
        let model = AppModel(vault: vault, home: home)
        model.refresh()
        model.startWatching()
        app = model
        status = StatusItemController(model: model, actions: PanelActions(
            handOff: { [weak self] in self?.handOff() },
            openSession: { [weak self] in
                guard let cwd = self?.app?.session?.cwd else { return }
                SessionLauncher.launch(in: URL(fileURLWithPath: cwd))
            },
            openBinder: { [weak self] in self?.showLibrary() },
            addMemory: { [weak self] in self?.showLibrary() }
        ))
        scheduler = ObserveScheduler(vault: { [weak self] in self?.vault ?? Vault(root: Vault.defaultRoot) }, home: home,
                                     onRunning: { [weak self] in self?.status?.observing = $0 },
                                     onFinished: { [weak self] in self?.app?.refresh() })
        scheduler?.start()
        sessionWatch = model.$session.compactMap { $0 }.sink { [weak self] session in
            guard let self, let app = self.app else { return }
            if self.notifier.check(session, threshold: app.state.clampedThreshold) { self.notifier.post(for: session) }
        }
    }

    func handOff() {
        guard let app, let session = app.session else { return }
        let vault = app.vault, consent = app.state.summarizerConsent
        Task.detached {
            _ = HandoffAction.run(session: session, vault: vault, consent: consent, runner: SystemRunner(extraEnvironment: ["BINDER_NESTED": "1"]))
        }
    }

    func showLibrary() {
        guard let app else { return }
        status?.hide()
        if library == nil {
            let installer = installer, vault = vault
            let model = LibraryModel(vault: vault, home: home)
            libraryModel = model
            library = LibraryWindowController(model: model, app: app, actions: LibraryActions(
                openSession: { SessionLauncher.launch(in: URL(fileURLWithPath: $0)) },
                runDoctor: { [weak self] in Doctor.report(vault: self?.vault ?? vault, installer: installer, runner: SystemRunner()) },
                installHooks: { add in (try? add ? installer.install() : installer.uninstall()) != nil },
                hooksInstalled: { installer.isInstalled },
                vaultMoved: { [weak self] moved in self?.app?.relocate(to: moved) }
            ))
        }
        NSApp.activate(ignoringOtherApps: true)
        library?.showWindow(nil)
    }

    /// Settings… in the app menu: the main window, on its Settings page.
    @objc func showSettings() {
        guard app != nil else { onboarding?.showWindow(nil); return }
        showLibrary()
        libraryModel?.selectedKind = .settings
    }

    @objc func openBinder() {
        if app != nil { showLibrary() } else { onboarding?.showWindow(nil) }
    }
}

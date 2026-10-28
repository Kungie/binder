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
    var updater: Updater?

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
        let version = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? Binder.version
        let updater = Updater(current: version, vault: { [weak self] in self?.app?.vault ?? model.vault },
                              app: Bundle.main.bundleURL, relaunch: { Self.relaunch() })
        self.updater = updater
        updater.start()
        status = StatusItemController(model: model, actions: PanelActions(
            handOff: { [weak self] in self?.handOff() },
            openSession: { [weak self] in
                guard let cwd = self?.app?.session?.cwd else { return }
                SessionLauncher.launch(in: URL(fileURLWithPath: cwd))
                self?.app?.handoff = nil
            },
            openBinder: { [weak self] in self?.showLibrary() },
            addMemory: { [weak self] in self?.showLibrary() }
        ), updater: updater)
        scheduler = ObserveScheduler(vault: { [weak self] in self?.vault ?? Vault(root: Vault.defaultRoot) }, home: home,
                                     onRunning: { [weak self] in self?.status?.observing = $0 },
                                     onFinished: { [weak self] in self?.app?.refresh() })
        scheduler?.start()
        sessionWatch = model.$session.compactMap { $0 }.sink { [weak self] session in
            guard let self, let app = self.app else { return }
            if self.notifier.check(session, threshold: app.state.clampedThreshold) { self.notifier.post(for: session) }
        }
    }

    /// Writes the handoff card, then puts it on the clipboard as a prompt, for sessions Binder can't open itself
    /// (Claude Code in the desktop app). The next session in that folder also gets the card from the hook.
    func handOff() {
        guard let app, let session = app.session else { return }
        let vault = app.vault, consent = app.state.summarizerConsent
        app.handoff = .writing(session: session.session)
        Task.detached {
            let url = HandoffAction.run(session: session, vault: vault, consent: consent, runner: SystemRunner(extraEnvironment: ["BINDER_NESTED": "1"]))
            let project = HandoffAction.project(for: session)
            let prompt = url == nil ? nil : Handoff.latest(project: project, vault: vault).map { Handoff.prompt($0.card, project: project) }
            await MainActor.run {
                guard app.handoff == .writing(session: session.session) else { return }
                if let prompt {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(prompt, forType: .string)
                    app.handoff = .copied(session: session.session)
                } else {
                    app.handoff = .failed(session: session.session)
                }
            }
        }
    }

    func showLibrary() {
        guard let app else { return }
        status?.hide()
        if library == nil {
            let installer = installer, vault = vault
            let model = LibraryModel(vault: vault, home: home)
            libraryModel = model
            library = LibraryWindowController(model: model, app: app, updater: updater, actions: LibraryActions(
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

    /// Check for Updates… in the app menu: Settings, with a check running.
    @objc func checkForUpdates() {
        showSettings()
        guard let updater else { return }
        Task { await updater.check(manual: true) }
    }

    /// Opens the freshly installed app a second after this one quits.
    static func relaunch() {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/bin/sh")
        p.arguments = ["-c", "sleep 1; /usr/bin/open \"$0\"", Bundle.main.bundleURL.path]
        try? p.run()
        NSApp.terminate(nil)
    }

    @objc func openBinder() {
        if app != nil { showLibrary() } else { onboarding?.showWindow(nil) }
    }
}

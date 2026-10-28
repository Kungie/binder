import AppKit
import BinderCore
import SwiftUI

/// Every setting in one plain page.
struct SettingsView: View {
    @ObservedObject var model: LibraryModel
    var actions: LibraryActions
    var updater: Updater?
    @StateObject private var doctor = Local<[DoctorLine]>([])
    @StateObject private var message = Local<String?>(nil)
    @StateObject private var hooks = Local<Bool?>(nil)

    var body: some View {
        Scroll {
            VStack(alignment: .leading, spacing: 22) {
                Text("Settings").font(Fonts.archivo(30, weight: 800)).foregroundColor(Phosphor.cream)
                group("Binder folder") {
                    Text(model.vault.root.path).font(Fonts.vt(19)).foregroundColor(Phosphor.cream).textSelection(.enabled)
                    HStack(spacing: 8) {
                        Button("Change") { move("Choose where your binder folder goes.") }.buttonStyle(GhostButtonStyle(height: 30))
                        Button("Put it in my Obsidian vault") { move("Choose your Obsidian vault. Binder goes in a folder inside it.") }.buttonStyle(GhostButtonStyle(height: 30))
                        Button("Show in Finder") { model.revealInFinder(nil) }.buttonStyle(QuietButtonStyle())
                    }
                }
                group("Where your projects live") {
                    ForEach(model.state.projectRoots, id: \.self) { root in
                        HStack {
                            Text(root).font(Fonts.vt(19)).foregroundColor(Phosphor.cream2)
                            Spacer()
                            Button("Remove") { model.update { $0.projectRoots.removeAll { $0 == root } } }.buttonStyle(QuietButtonStyle())
                        }
                    }
                    Button("Add a folder") { addRoot() }.buttonStyle(GhostButtonStyle(height: 30))
                }
                group("Look around every") {
                    HStack(spacing: 8) {
                        ForEach([1, 6, 24], id: \.self) { hours in
                            Chip(hours == 1 ? "1 hour" : "\(hours) hours", on: model.state.observeIntervalHours == hours) {
                                model.update { $0.observeIntervalHours = hours }
                            }
                        }
                    }
                }
                group("Suggest a handoff at") {
                    HStack(spacing: 14) {
                        Slider(value: Binding(get: { model.state.handoffThreshold }, set: { v in model.update { $0.handoffThreshold = (v * 20).rounded() / 20 } }),
                               in: 0.5...0.9)
                            .tint(Phosphor.amber)
                            .frame(width: 260)
                            .accessibilityLabel("Handoff threshold")
                        Text("\(Int((model.state.clampedThreshold * 100).rounded()))% OF CONTEXT").font(Fonts.vt(20)).foregroundColor(Phosphor.amber)
                    }
                }
                group("Read sessions with your Claude plan") {
                    HStack(spacing: 8) {
                        Chip("On", on: model.state.summarizerConsent) { model.update { $0.summarizerConsent = true } }
                        Chip("Off", on: !model.state.summarizerConsent) { model.update { $0.summarizerConsent = false } }
                    }
                    Text("Catches decisions you only said out loud, and moves lines they make untrue to Superseded. It reads talk in batches with Haiku on your own plan, roughly the talk itself plus a thousand tokens per batch.")
                        .font(Fonts.grotesk(12.5)).foregroundColor(Phosphor.dim)
                        .fixedSize(horizontal: false, vertical: true)
                }
                group("Claude Code hooks") {
                    let installed = hooks.value ?? actions.hooksInstalled()
                    HStack(spacing: 10) {
                        Text(installed ? "CONNECTED" : "NOT CONNECTED").font(Fonts.vt(20)).foregroundColor(installed ? Phosphor.amber : Phosphor.dim)
                        Button(installed ? "Remove" : "Install") {
                            if actions.installHooks(!installed) { hooks.value = !installed }
                            else { message.value = "~/.claude/settings.json isn't valid JSON, so I left it alone." }
                        }
                        .buttonStyle(GhostButtonStyle(height: 30))
                    }
                }
                group("Updates") {
                    HStack(spacing: 8) {
                        Chip("Check every day", on: model.state.checkForUpdates) { model.update { $0.checkForUpdates = true } }
                        Chip("Off", on: !model.state.checkForUpdates) { model.update { $0.checkForUpdates = false } }
                    }
                    if let updater { UpdateSettings(updater: updater) }
                    Text("Binder asks GitHub for the latest release, nothing else. Updates replace the app only; your binder and settings stay as they are.")
                        .font(Fonts.grotesk(12.5)).foregroundColor(Phosphor.dim)
                        .fixedSize(horizontal: false, vertical: true)
                }
                group("Check everything") {
                    Button("Run binder doctor") { doctor.value = actions.runDoctor() }.buttonStyle(GhostButtonStyle(height: 30))
                    ForEach(Array(doctor.value.enumerated()), id: \.offset) { _, line in
                        HStack(alignment: .firstTextBaseline, spacing: 8) {
                            Text(line.ok ? "OK" : "!!").font(Fonts.vt(18)).foregroundColor(line.ok ? Phosphor.amber : Phosphor.cream)
                            Text(line.text).font(Fonts.grotesk(12.5)).foregroundColor(Phosphor.cream2)
                        }
                    }
                }
                if let text = message.value { Text(text).font(Fonts.grotesk(13)).foregroundColor(Phosphor.cream2) }
            }
            .padding(32)
            .frame(maxWidth: 720, alignment: .leading)
        }
    }

    func group<C: View>(_ title: String, @ViewBuilder content: () -> C) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            SectionLabel(title)
            content()
        }
        .padding(.bottom, 4)
        .frame(maxWidth: .infinity, alignment: .leading)
        .overlay(alignment: .bottom) { Rectangle().fill(Phosphor.line).frame(height: 1).offset(y: 12) }
    }

    func choose(_ message: String) -> URL? {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.canCreateDirectories = true
        panel.message = message
        panel.prompt = "Choose"
        return panel.runModal() == .OK ? panel.url : nil
    }

    func move(_ prompt: String) {
        guard let url = choose(prompt) else { return }
        do {
            try model.moveVault(into: url)
            actions.vaultMoved(model.vault)
        } catch { message.value = "Couldn't move the binder: \(error.localizedDescription)" }
    }

    func addRoot() {
        guard let url = choose("Choose a folder where you keep projects.") else { return }
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        let path = url.path.hasPrefix(home) ? "~" + url.path.dropFirst(home.count) : url.path
        model.update { if !$0.projectRoots.contains(path) { $0.projectRoots.append(path) } }
    }
}

/// This version, the last answer from GitHub, and the buttons to check and to update.
struct UpdateSettings: View {
    @ObservedObject var updater: Updater

    var body: some View {
        HStack(spacing: 10) {
            Button("Check for updates") { Task { await updater.check(manual: true) } }
                .buttonStyle(GhostButtonStyle(height: 30))
                .disabled(updater.status == .checking)
            if case .available = updater.status {
                Button("Update now") { Task { await updater.install() } }.buttonStyle(PrimaryButtonStyle(size: 12, height: 30))
            }
            Text(line).font(Fonts.vt(19)).foregroundColor(highlight ? Phosphor.amber : Phosphor.cream2)
        }
        if case let .failed(message) = updater.status {
            Text(message).font(Fonts.grotesk(12.5)).foregroundColor(Phosphor.cream2).fixedSize(horizontal: false, vertical: true)
        }
    }

    var highlight: Bool {
        if case .available = updater.status { return true }
        return false
    }

    var line: String {
        switch updater.status {
        case .idle: return "THIS IS BINDER \(updater.current)"
        case .checking: return "CHECKING…"
        case let .upToDate(date):
            let f = DateFormatter()
            f.dateFormat = "HH:mm"
            return "UP TO DATE · \(updater.current) · CHECKED \(f.string(from: date))"
        case let .available(version): return "\(version) IS OUT · YOU HAVE \(updater.current)"
        case let .installing(version): return "UPDATING TO \(version)…"
        case .failed: return "THIS IS BINDER \(updater.current)"
        }
    }
}

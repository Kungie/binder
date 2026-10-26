import AppKit
import SwiftUI

/// Binder writes the folder. Each file appears in the tree as it's written; a raw note shows what's inside.
struct VaultView: View {
    @ObservedObject var model: OnboardingModel
    @StateObject private var error = Local<String?>(nil)
    @StateObject private var shownAt = Local(Date())
    @Environment(\.isSnapshot) private var isSnapshot

    var body: some View {
        SetupShell(step: OnboardingStep.vault.rawValue, mood: .writing, caption: "WRITING IT\nALL DOWN.") {
            VStack(alignment: .leading, spacing: 16) {
                StepHeading("Here's your binder.",
                            "Plain Markdown files. Open them in Obsidian, in any editor, or never. Lines between the observed markers are mine to rewrite. Everything else is yours.")
                HStack(spacing: 10) {
                    Text(displayPath).font(Fonts.vt(20)).foregroundColor(Phosphor.cream).lineLimit(1).truncationMode(.middle).frame(maxWidth: .infinity, alignment: .leading)
                    Button("Change") { choose(message: "Choose where your binder folder goes.") }.buttonStyle(GhostButtonStyle(height: 28))
                    Button("Put it in my Obsidian vault") { choose(message: "Choose your Obsidian vault. Binder goes in a folder inside it.") }
                        .buttonStyle(GhostButtonStyle(height: 28))
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 10)
                .background(Phosphor.black)
                .overlay(Rectangle().strokeBorder(Phosphor.line, lineWidth: 1))
                HStack(alignment: .top, spacing: 14) {
                    TimelineView(.animation(paused: isSnapshot)) { timeline in
                        tree(elapsed: isSnapshot ? 99 : timeline.date.timeIntervalSince(shownAt.value))
                    }
                    .frame(width: 210)
                    preview
                }
                .frame(maxHeight: .infinity)
                if let message = error.value {
                    Text(message).font(Fonts.grotesk(13)).foregroundColor(Phosphor.cream2)
                }
                Text("\(model.writtenFiles.count) NOTES · 0 SECRETS · EVERY LINE CAME FROM THIS MAC OR FROM YOU")
                    .font(Fonts.vt(19)).foregroundColor(Phosphor.dim)
                HStack(spacing: 10) {
                    Button("Continue") { model.next() }.buttonStyle(PrimaryButtonStyle(size: 14, height: 40)).keyboardShortcut(.defaultAction)
                    Button("Open in Finder") { NSWorkspace.shared.activateFileViewerSelecting([model.vaultRoot]) }.buttonStyle(GhostButtonStyle(height: 40))
                }
            }
        }
        .onAppear {
            guard !isSnapshot, model.writtenFiles.isEmpty else { return }
            do {
                try model.createVault()
                shownAt.value = Date()
            } catch {
                self.error.value = "Couldn't write to \(model.vaultRoot.path): \(error.localizedDescription)"
            }
        }
    }

    var displayPath: String {
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        let path = model.vaultRoot.path
        return path.hasPrefix(home) ? "~" + path.dropFirst(home.count) : path
    }

    /// The written files as a tree, each line appearing 0.2 s after the one before.
    func tree(elapsed: Double) -> some View {
        var lines: [(String, Bool)] = [(displayPath, false)]
        var lastFolder = ""
        let files = model.writtenFiles
        for (i, file) in files.enumerated() {
            let parts = file.split(separator: "/").map(String.init)
            let lastTop = files.last.map { $0.split(separator: "/").first.map(String.init) ?? $0 } == parts.first
            if parts.count == 2 {
                if parts[0] != lastFolder {
                    lines.append(("\(lastTop ? "└" : "├") \(parts[0])/", false))
                    lastFolder = parts[0]
                }
                let lastInFolder = i == files.count - 1 || !files[i + 1].hasPrefix(parts[0] + "/")
                lines.append(("\(lastTop ? " " : "│") \(lastInFolder ? "└" : "├") \(parts[1])", true))
            } else {
                lines.append(("├ \(file)", true))
            }
        }
        // A long tree scrolls inside its panel, so Continue never drops below the window.
        return ScreenPanel(glow: UnitPoint(x: 0.2, y: 0.2), padding: 12) {
            Scroll {
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(Array(lines.enumerated()), id: \.offset) { index, line in
                        HStack(spacing: 4) {
                            Text(index == 0 ? "~/" + model.vaultRoot.lastPathComponent : line.0).foregroundColor(index == 0 ? Phosphor.cream : Phosphor.cream2).lineLimit(1)
                            if line.1 { Text("■").foregroundColor(Phosphor.amber) }
                        }
                        .opacity(elapsed >= Double(index) * 0.2 ? 1 : 0)
                    }
                }
                .font(Fonts.vt(19))
                .frame(maxWidth: .infinity, alignment: .topLeading)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        }
        .accessibilityLabel("Folder tree, \(model.writtenFiles.count) notes")
    }

    var preview: some View {
        VStack(spacing: 0) {
            HStack {
                SectionLabel(model.previewName)
                Spacer()
                SectionLabel("Obsidian-ready", color: Phosphor.amber)
            }
            .padding(.horizontal, 12)
            .frame(height: 30)
            .overlay(alignment: .bottom) { Rectangle().fill(Phosphor.line).frame(height: 1) }
            Scroll {
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(Array(model.previewText.components(separatedBy: "\n").prefix(18).enumerated()), id: \.offset) { _, line in
                        Text(line.isEmpty ? " " : line).foregroundColor(color(line)).lineLimit(1)
                    }
                }
                .font(Fonts.vt(18))
                .padding(.horizontal, 14)
                .padding(.vertical, 10)
                .frame(maxWidth: .infinity, alignment: .topLeading)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        }
        .background(Phosphor.black)
        .overlay(Rectangle().strokeBorder(Phosphor.line, lineWidth: 1))
    }

    func color(_ line: String) -> Color {
        if line.hasPrefix("---") || line.hasPrefix("<!--") || line.contains(": ") && !line.hasPrefix("- ") { return Phosphor.dim }
        if line.hasPrefix("#") { return Phosphor.cream }
        if line.contains("[[") { return Phosphor.amber }
        return Phosphor.cream2
    }

    func choose(message: String) {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.canCreateDirectories = true
        panel.message = message
        panel.prompt = "Choose"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        do {
            try model.moveVault(into: url)
        } catch {
            self.error.value = "Couldn't move the binder: \(error.localizedDescription)"
        }
    }
}

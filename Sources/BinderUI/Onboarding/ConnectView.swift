import SwiftUI

/// Hooks into Claude Code, the summarizer's permission, and a first real try.
struct ConnectView: View {
    @ObservedObject var model: OnboardingModel
    var openSession: (String) -> Void
    var done: () -> Void
    @StateObject private var error = Local<String?>(nil)

    var body: some View {
        SetupShell(step: OnboardingStep.connect.rawValue, mood: .plugging, caption: "PLUGGING IN.") {
            VStack(alignment: .leading, spacing: 14) {
                StepHeading("Last step. Plug me in.",
                            "I add two small hooks. Agents read your binder when a session starts and leave me notes when it ends. Your other hooks stay where they are.")
                claudeCard
                HStack(spacing: 12) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Codex").font(Fonts.grotesk(15, weight: 600)).foregroundColor(Phosphor.cream)
                        Text("Codex support comes in a later version.").font(Fonts.grotesk(13)).foregroundColor(Phosphor.cream2)
                    }
                    Spacer()
                    Text("LATER").font(Fonts.vt(22)).foregroundColor(Phosphor.dim)
                }
                .padding(.horizontal, 14).padding(.vertical, 10)
                .overlay(Rectangle().strokeBorder(Phosphor.line, lineWidth: 1))
                consent
                tryIt
                if let message = error.value { Text(message).font(Fonts.grotesk(13)).foregroundColor(Phosphor.cream2) }
                Spacer(minLength: 0)
                HStack(spacing: 10) {
                    Button("Done") {
                        do { try model.finish(); done() } catch { self.error.value = "Couldn't save: \(error.localizedDescription)" }
                    }
                    .buttonStyle(PrimaryButtonStyle(size: 14, height: 40))
                    .keyboardShortcut(.defaultAction)
                    Text("I'll be in the menu bar.").font(Fonts.grotesk(13)).foregroundColor(Phosphor.dim)
                }
            }
        }
    }

    var claudeCard: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Claude Code").font(Fonts.grotesk(15, weight: 600)).foregroundColor(Phosphor.cream)
                    SectionLabel(model.claudeFound ? "Found in ~/.claude" : "Not found yet")
                }
                Spacer()
                if model.connected {
                    Text("CONNECTED").font(Fonts.vt(22)).foregroundColor(Phosphor.amber)
                } else {
                    Button("Connect") {
                        do { try model.connect() } catch { self.error.value = "~/.claude/settings.json isn't valid JSON, so I left it alone." }
                    }
                    .buttonStyle(PrimaryButtonStyle(size: 12, height: 32))
                }
            }
            .padding(.horizontal, 14).padding(.vertical, 12)
            .overlay(alignment: .bottom) { Rectangle().fill(Phosphor.line).frame(height: 1) }
            VStack(alignment: .leading, spacing: 0) {
                Text("~/.claude/settings.json").foregroundColor(Phosphor.dim)
                (Text("+ ").foregroundColor(Phosphor.amber) + Text("SessionStart  →  binder card"))
                (Text("+ ").foregroundColor(Phosphor.amber) + Text("Stop          →  binder capture"))
            }
            .font(Fonts.vt(18))
            .foregroundColor(Phosphor.cream2)
            .padding(.horizontal, 14).padding(.vertical, 10)
        }
        .background(Phosphor.black)
        .overlay(Rectangle().strokeBorder(Phosphor.line, lineWidth: 1))
    }

    var consent: some View {
        HStack(alignment: .center, spacing: 12) {
            Text("Can I read your sessions with your Claude plan, to catch decisions you only said out loud and keep old ones from going stale? I read in batches with Haiku: roughly the talk itself plus a thousand tokens each time.")
                .font(Fonts.grotesk(13))
                .foregroundColor(Phosphor.cream2)
                .fixedSize(horizontal: false, vertical: true)
            switch model.summarizerConsent {
            case true?: Text("ON").font(Fonts.vt(22)).foregroundColor(Phosphor.amber)
            case false?: Text("OFF").font(Fonts.vt(22)).foregroundColor(Phosphor.dim)
            case nil:
                HStack(spacing: 6) {
                    Button("Yes") { model.setConsent(true) }.buttonStyle(PrimaryButtonStyle(size: 12, height: 30))
                    Button("Not now") { model.setConsent(false) }.buttonStyle(GhostButtonStyle(height: 30))
                }
                .fixedSize()
            }
        }
        .padding(.horizontal, 14).padding(.vertical, 10)
        .overlay(Rectangle().strokeBorder(Phosphor.line, lineWidth: 1))
    }

    @ViewBuilder
    var tryIt: some View {
        if let project = model.tryItProject {
            ScreenPanel(glow: UnitPoint(x: 0.15, y: 0.2), padding: 14) {
                VStack(alignment: .leading, spacing: 10) {
                    SectionLabel("Try it")
                    (Text("Open a session in ") + Text(project.name).font(Fonts.vt(18)).foregroundColor(Phosphor.amber)
                        + Text(" and ask where it's hosted. It should answer without asking you back."))
                        .font(Fonts.grotesk(14))
                        .foregroundColor(Phosphor.cream)
                    Button("Open a session in \(project.name)") { openSession(project.path) }
                        .buttonStyle(PrimaryButtonStyle(size: 12, height: 32))
                }
            }
        }
        Text("Changed your mind later? binder uninstall takes both hooks out. I'll also ask GitHub once a day for a newer Binder; that's off in Settings if you'd rather.")
            .font(Fonts.mono(11))
            .foregroundColor(Phosphor.dim)
    }
}

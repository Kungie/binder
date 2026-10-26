import SwiftUI

/// Each permission says exactly why Binder wants it, and asks macOS only when the user clicks.
struct PermissionsView: View {
    @ObservedObject var model: OnboardingModel

    static let rows: [(Permission, String, String, Bool)] = [
        (.folders, "Desktop and Documents", "Your projects live there. I read repo and deploy files, nothing else.", false),
        (.localNetwork, "Local network", "To spot your homelab. I knock on SSH and a few web ports, then leave.", false),
        (.notifications, "Notifications", "One nudge when a session gets long. Not a pen pal.", false),
        (.terminal, "Terminal", "So \"Open a new session\" can open one for you, in the right folder.", true)
    ]

    var body: some View {
        SetupShell(step: OnboardingStep.permissions.rawValue, mood: .holdingKey, caption: "I ASK ONCE. PROMISE.") {
            VStack(alignment: .leading, spacing: 16) {
                StepHeading("A few permissions first.", "macOS asks on my behalf. I use each one only for the thing written next to it.")
                VStack(spacing: 0) {
                    ForEach(Self.rows, id: \.0) { permission, title, why, optional in
                        row(title: title, why: why, optional: optional) { control(permission, optional: optional) }
                    }
                    row(title: "Open at login", why: "So I'm around when you are.", optional: false) {
                        let on = model.permissionStatus[.login] == .granted
                        Button { Task { await model.request(.login) } } label: {
                            HStack { if on { Spacer() }; Rectangle().fill(Phosphor.ink).frame(width: 18, height: 18); if !on { Spacer() } }
                                .padding(3)
                                .frame(width: 44, height: 24)
                                .background(on ? Phosphor.amber : Phosphor.line)
                        }
                        .buttonStyle(.plain)
                        .accessibilityLabel("Open at login")
                        .accessibilityValue(on ? "On" : "Off")
                    }
                }
                .overlay(alignment: .top) { Rectangle().fill(Phosphor.line).frame(height: 1) }
                Text("~/.ssh, ~/.claude and ~/.codex need no permission. I read them. I never write to ~/.ssh.")
                    .font(Fonts.mono(11))
                    .foregroundColor(Phosphor.dim)
                    .lineSpacing(4)
                Spacer(minLength: 0)
                HStack(spacing: 10) {
                    Button("Continue") { model.next() }.buttonStyle(PrimaryButtonStyle(size: 14, height: 40)).keyboardShortcut(.defaultAction)
                    Button("Skip the optional one") { model.next() }.buttonStyle(GhostButtonStyle(height: 40))
                }
            }
        }
    }

    @ViewBuilder
    func control(_ p: Permission, optional: Bool) -> some View {
        switch model.permissionStatus[p] ?? .unknown {
        case .granted: Text("GRANTED").font(Fonts.vt(20)).foregroundColor(Phosphor.amber)
        case .asked: Text("ASKED").font(Fonts.vt(20)).foregroundColor(Phosphor.amber)
        case .denied: Text("DENIED").font(Fonts.vt(20)).foregroundColor(Phosphor.dim)
        case .unknown:
            if optional {
                Button("Allow") { Task { await model.request(p) } }.buttonStyle(GhostButtonStyle(height: 32))
            } else {
                Button("Allow") { Task { await model.request(p) } }.buttonStyle(PrimaryButtonStyle(size: 12, height: 32))
            }
        }
    }

    func row<C: View>(title: String, why: String, optional: Bool, @ViewBuilder control: () -> C) -> some View {
        HStack(alignment: .center, spacing: 16) {
            VStack(alignment: .leading, spacing: 3) {
                HStack(spacing: 6) {
                    Text(title).font(Fonts.grotesk(15, weight: 600)).foregroundColor(Phosphor.cream)
                    if optional {
                        Text("OPTIONAL").font(Fonts.mono(10.5)).foregroundColor(Phosphor.dim)
                            .padding(.horizontal, 5).padding(.vertical, 1)
                            .overlay(Rectangle().strokeBorder(Phosphor.line, lineWidth: 1))
                    }
                }
                Text(why).font(Fonts.grotesk(13)).foregroundColor(Phosphor.cream2).fixedSize(horizontal: false, vertical: true)
            }
            Spacer(minLength: 8)
            control().frame(minWidth: 92, alignment: .trailing)
        }
        .padding(.vertical, 11)
        .overlay(alignment: .bottom) { Rectangle().fill(Phosphor.line).frame(height: 1) }
    }
}

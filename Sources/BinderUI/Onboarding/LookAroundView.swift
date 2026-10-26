import SwiftUI

/// Binder reads what's already on the Mac. Rows flip from LOOKING to their result as each observer finishes.
struct LookAroundView: View {
    @ObservedObject var model: OnboardingModel
    @StateObject private var showHint = Local(false)

    var body: some View {
        SetupShell(step: OnboardingStep.lookAround.rawValue, mood: model.lookingDone ? .idle : .scanning, caption: "JUST LOOKING.\nNOT TOUCHING.") {
            VStack(alignment: .leading, spacing: 14) {
                StepHeading("Let me look around first.",
                            "I read what's already on this Mac. Your keys stay where they are. I only write down which key opens which door.")
                ScreenPanel(glow: UnitPoint(x: 0.85, y: 0.15), padding: 0) {
                    VStack(spacing: 0) {
                        ForEach(Array(model.rows.enumerated()), id: \.element.id) { index, row in
                            rowView(row)
                                .overlay(alignment: .bottom) {
                                    if index < model.rows.count - 1 { Rectangle().fill(Phosphor.line).frame(height: 1) }
                                }
                        }
                    }
                    .padding(.horizontal, 16)
                    .padding(.vertical, 4)
                }
                HStack(spacing: 12) {
                    SteppedBar(fraction: progress)
                    Text("READ \(model.filesRead) FILES · WROTE 0").font(Fonts.vt(18)).foregroundColor(Phosphor.dim)
                }
                if showHint.value {
                    Text("Anything I got wrong can be fixed later in the main window. I keep what you correct.")
                        .font(Fonts.grotesk(13)).foregroundColor(Phosphor.cream2)
                }
                Spacer(minLength: 0)
                HStack(spacing: 10) {
                    Button("Looks right") { model.next() }
                        .buttonStyle(PrimaryButtonStyle(size: 14, height: 40))
                        .keyboardShortcut(.defaultAction)
                        .disabled(!model.lookingDone)
                        .opacity(model.lookingDone ? 1 : 0.5)
                    Button("Fix something") { showHint.value = true }.buttonStyle(GhostButtonStyle(height: 40))
                }
            }
        }
        .task { if model.rows.isEmpty { await model.runLookAround() } }
    }

    var progress: Double {
        guard !model.rows.isEmpty else { return 0 }
        return Double(model.rows.filter { $0.status != .looking }.count) / Double(model.rows.count)
    }

    @ViewBuilder
    func rowView(_ row: LookRow) -> some View {
        HStack(spacing: 12) {
            Text(row.label).font(Fonts.mono(11)).foregroundColor(Phosphor.dim).frame(width: 84, alignment: .leading)
            Text(row.status == .looking ? "…" : row.detail)
                .font(Fonts.grotesk(13))
                .foregroundColor(Phosphor.cream)
                .frame(maxWidth: .infinity, alignment: .leading)
            if row.status == .ask, row.label == "HOMELAB" {
                HStack(spacing: 6) {
                    Button("Mine") { model.answerHomelab(mine: true) }.buttonStyle(PrimaryButtonStyle(size: 11, height: 28))
                    Button("Not mine") { model.answerHomelab(mine: false) }.buttonStyle(GhostButtonStyle(height: 28))
                }
            } else {
                Text(status(row.status))
                    .font(Fonts.vt(20))
                    .foregroundColor(row.status == .found ? Phosphor.amber : (row.status == .looking ? Phosphor.dim : Phosphor.cream))
                    .frame(width: 80, alignment: .trailing)
            }
        }
        .frame(minHeight: 34)
        .padding(.vertical, 3)
    }

    func status(_ s: LookStatus) -> String {
        switch s {
        case .looking: "LOOKING"
        case .found: "FOUND"
        case .half: "HALF"
        case .ask: "ASK"
        }
    }
}

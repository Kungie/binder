import BinderCore
import SwiftUI

/// One thing Binder caught: where it came from, what it says, and Keep / Edit / Drop.
public struct ProposalCard: View {
    let proposal: Proposal
    @ObservedObject var model: AppModel
    @Environment(\.isSnapshot) private var isSnapshot

    public static func tag(for p: Proposal) -> String {
        var tag: String
        switch p.source {
        case .command: tag = "FROM A COMMAND · \(p.evidence)"
        case .observed: tag = "SEEN ON THIS MAC · \(p.evidence)"
        case .chat: tag = "FROM THE CHAT"
        }
        if p.confidence == .low { tag += " · PROBABLY A ONE-OFF" }
        return tag
    }

    var editing: Binding<String>? {
        guard case let .editing(text)? = model.phases[proposal.id] else { return nil }
        return Binding(get: {
            if case let .editing(t)? = model.phases[proposal.id] { return t }
            return text
        }, set: { model.phases[proposal.id] = .editing($0) })
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(Self.tag(for: proposal))
                .font(Fonts.mono(10.5))
                .foregroundColor(Phosphor.amber)
                .lineLimit(1)
                .truncationMode(.middle)
            if let editing, isSnapshot {
                Text(editing.wrappedValue)
                    .font(Fonts.grotesk(13.5))
                    .foregroundColor(Phosphor.cream)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(6)
                    .background(Phosphor.black)
                    .overlay(Rectangle().strokeBorder(Phosphor.amber.opacity(0.6), lineWidth: 1))
            } else if let editing {
                TextField("", text: editing, axis: .vertical)
                    .textFieldStyle(.plain)
                    .font(Fonts.grotesk(13.5))
                    .foregroundColor(Phosphor.cream)
                    .padding(6)
                    .background(Phosphor.black)
                    .overlay(Rectangle().strokeBorder(Phosphor.amber.opacity(0.6), lineWidth: 1))
                    .onSubmit { model.keep(proposal.id) }
                    .accessibilityLabel("Edit before keeping")
            } else {
                Text(proposal.text)
                    .font(Fonts.grotesk(13.5))
                    .foregroundColor(Phosphor.cream)
                    .fixedSize(horizontal: false, vertical: true)
            }
            HStack(spacing: 6) {
                Button("Keep") { model.keep(proposal.id) }.buttonStyle(CreamButtonStyle())
                if editing == nil {
                    Button("Edit") { model.edit(proposal.id) }.buttonStyle(GhostButtonStyle(height: 26))
                } else {
                    Button("Cancel") { model.cancelEdit(proposal.id) }.buttonStyle(GhostButtonStyle(height: 26))
                }
                Button("Drop") { model.drop(proposal.id) }.buttonStyle(QuietButtonStyle())
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Phosphor.black2)
        .overlay(Rectangle().strokeBorder(Phosphor.line, lineWidth: 1))
    }
}

import BinderCore
import SwiftUI

/// The right column: linked notes, the last handoff, and exactly what agents see for this project.
struct ContextColumn: View {
    var detail: NoteDetail
    var session: SessionStatus?
    var openSession: (String) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            if !detail.links.isEmpty {
                VStack(alignment: .leading, spacing: 8) {
                    SectionLabel("Linked")
                    FlowLayout(spacing: 6) {
                        ForEach(detail.links, id: \.self) { link in
                            Text("[[\(link)]]").font(Fonts.vt(18)).foregroundColor(Phosphor.amber)
                                .padding(.horizontal, 8).frame(height: 26)
                                .overlay(Rectangle().strokeBorder(Phosphor.line, lineWidth: 1))
                        }
                    }
                }
            }
            if let handoff = detail.lastHandoff { handoffCard(handoff) }
            if let card = detail.cardPreview {
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        SectionLabel("What agents see in \(detail.title)")
                        Spacer()
                        Text("≈ \(detail.cardTokens) TOKENS").font(Fonts.vt(17)).foregroundColor(Phosphor.dim)
                    }
                    Scroll {
                        Text(card).font(Fonts.vt(17)).foregroundColor(Phosphor.cream2).lineSpacing(1)
                            .frame(maxWidth: .infinity, alignment: .topLeading)
                            .textSelection(.enabled)
                    }
                    .padding(.horizontal, 12).padding(.vertical, 10)
                    .background(ZStack { Phosphor.black; Scanlines() })
                    .overlay(Rectangle().strokeBorder(Phosphor.line, lineWidth: 1))
                }
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 26)
        .frame(width: 300)
        .frame(maxHeight: .infinity, alignment: .top)
        .overlay(alignment: .leading) { Rectangle().fill(Phosphor.line).frame(width: 1) }
    }

    func handoffCard(_ h: HandoffCard) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            SectionLabel("Last handoff")
            Grid(alignment: .leading, horizontalSpacing: 10, verticalSpacing: 5) {
                row("DONE", h.done.joined(separator: "; "))
                row("LEFT", h.left.joined(separator: "; "))
                row("DON'T", h.dontTouch.joined(separator: "; "))
                row("NEXT", h.next)
            }
            if let session, session.project == detail.title {
                Button("Pick up from here") { openSession(session.cwd) }.buttonStyle(CreamButtonStyle())
            }
        }
        .padding(.horizontal, 14).padding(.vertical, 12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .overlay(Rectangle().strokeBorder(Phosphor.line, lineWidth: 1))
    }

    func row(_ label: String, _ value: String) -> some View {
        GridRow(alignment: .firstTextBaseline) {
            Text(label).font(Fonts.mono(10.5)).foregroundColor(Phosphor.amber)
            Text(value.isEmpty ? "nothing noted" : value).font(Fonts.grotesk(13)).foregroundColor(Phosphor.cream).fixedSize(horizontal: false, vertical: true)
        }
    }
}

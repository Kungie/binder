import BinderCore
import SwiftUI

/// One note: what Binder observed, what was decided (and where it came from), the user's own text, and a field to add more.
struct NoteView: View {
    @ObservedObject var model: LibraryModel
    var detail: NoteDetail
    @StateObject private var draft = Local("")
    @StateObject private var refused = Local(false)
    @Environment(\.isSnapshot) private var isSnapshot

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 8) {
                    SectionLabel(detail.ref.path)
                    Text(detail.title)
                        .font(Fonts.archivo(44, weight: 900))
                        .foregroundColor(Phosphor.cream)
                        .shadow(color: Phosphor.black, radius: 0, x: 0, y: 3)
                        .shadow(color: Phosphor.amber, radius: 0, x: 0, y: 6)
                        .padding(.bottom, 6)
                    HStack(spacing: 8) {
                        if let verified = detail.verified { chip("VERIFIED \(Self.short(verified))", amber: true) }
                        if let source = detail.source { chip(source.contains("you") ? "FROM THIS MAC + YOU" : (source == "observed" ? "FROM THIS MAC" : source.uppercased())) }
                        if !detail.decisions.isEmpty { chip("\(detail.decisions.count) DECISION\(detail.decisions.count == 1 ? "" : "S")") }
                    }
                }
                Spacer()
                HStack(spacing: 8) {
                    if LibraryModel.obsidianInstalled { Button("Open in Obsidian") { model.openInObsidian(detail.ref) }.buttonStyle(GhostButtonStyle(height: 32)) }
                    Button("Edit") { model.openInEditor(detail.ref) }.buttonStyle(CreamButtonStyle())
                }
            }
            Scroll {
                VStack(alignment: .leading, spacing: 18) {
                    if !detail.facts.isEmpty { observed }
                    if !detail.decisions.isEmpty { decisions }
                    if !detail.superseded.isEmpty { superseded }
                    if !detail.freeText.isEmpty {
                        VStack(alignment: .leading, spacing: 6) {
                            SectionLabel("Your notes")
                            ForEach(Array(detail.freeText.enumerated()), id: \.offset) { _, line in
                                Text(line).font(Fonts.grotesk(13.5)).foregroundColor(Phosphor.cream2).fixedSize(horizontal: false, vertical: true)
                            }
                        }
                    }
                }
            }
            addMemory
        }
        .padding(.horizontal, 32)
        .padding(.vertical, 26)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }

    static func short(_ iso: String) -> String {
        guard let date = ISO8601DateFormatter().date(from: iso) else { return iso.uppercased() }
        let f = DateFormatter()
        f.dateFormat = Calendar.current.isDateInToday(date) ? "HH:mm 'TODAY'" : "d MMM HH:mm"
        return f.string(from: date).uppercased()
    }

    func chip(_ text: String, amber: Bool = false) -> some View {
        Text(text).font(Fonts.vt(17)).foregroundColor(amber ? Phosphor.amber : Phosphor.cream2)
            .padding(.horizontal, 8).frame(height: 24)
            .overlay(Rectangle().strokeBorder(Phosphor.line, lineWidth: 1))
    }

    var observed: some View {
        ScreenPanel(glow: UnitPoint(x: 0.9, y: 0.1), padding: 0) {
            VStack(alignment: .leading, spacing: 0) {
                HStack {
                    SectionLabel("Observed · I rewrite this part")
                    Spacer()
                    if let verified = detail.verified { Text("LAST \(Self.short(verified))").font(Fonts.vt(17)).foregroundColor(Phosphor.dim) }
                }
                .padding(.bottom, 6)
                ForEach(Array(detail.facts.enumerated()), id: \.offset) { _, fact in
                    HStack(alignment: .firstTextBaseline, spacing: 0) {
                        Text(fact.key.uppercased()).font(Fonts.mono(10.5)).foregroundColor(Phosphor.dim).frame(width: 110, alignment: .leading)
                        LinkedText(fact.value).font(Fonts.grotesk(13.5))
                        Spacer(minLength: 0)
                    }
                    .padding(.vertical, 7)
                    .overlay(alignment: .top) { Rectangle().fill(Phosphor.line).frame(height: 1) }
                }
            }
            .padding(.horizontal, 16)
            .padding(.top, 12)
            .padding(.bottom, 6)
        }
    }

    var decisions: some View {
        VStack(alignment: .leading, spacing: 0) {
            SectionLabel("Decisions").padding(.bottom, 6)
            ForEach(Array(detail.decisions.enumerated()), id: \.offset) { _, d in
                HStack(alignment: .firstTextBaseline, spacing: 12) {
                    Text(Self.day(d.date)).font(Fonts.vt(18)).foregroundColor(Phosphor.amber).frame(width: 70, alignment: .leading)
                    Text(d.text).font(Fonts.grotesk(13.5)).foregroundColor(Phosphor.cream).frame(maxWidth: .infinity, alignment: .leading)
                    Text(d.provenance.uppercased()).font(Fonts.mono(10.5)).foregroundColor(Phosphor.dim).frame(width: 150, alignment: .trailing)
                }
                .padding(.vertical, 8)
                .overlay(alignment: .top) { Rectangle().fill(Phosphor.line).frame(height: 1) }
            }
        }
    }

    /// Lines later ones made untrue: kept out of the card, shown dim here, one click to bring back.
    var superseded: some View {
        VStack(alignment: .leading, spacing: 0) {
            SectionLabel("Superseded · not on the card").padding(.bottom, 6)
            ForEach(Array(detail.superseded.prefix(12).enumerated()), id: \.offset) { _, line in
                HStack(alignment: .firstTextBaseline, spacing: 12) {
                    Text(CardBuilder.decisionText(line.replacingOccurrences(of: #" · superseded \d{4}-\d{2}-\d{2}$"#, with: "", options: .regularExpression)))
                        .font(Fonts.grotesk(12.5)).foregroundColor(Phosphor.dim).strikethrough(color: Phosphor.line)
                        .frame(maxWidth: .infinity, alignment: .leading)
                    Button("Restore") { model.restore(line, in: detail.ref) }.buttonStyle(QuietButtonStyle())
                }
                .padding(.vertical, 4)
            }
        }
    }

    static func day(_ date: String) -> String {
        let p = DateFormatter()
        p.dateFormat = "yyyy-MM-dd"
        guard let d = p.date(from: date) else { return date }
        let f = DateFormatter()
        f.dateFormat = "d MMM"
        return f.string(from: d).uppercased()
    }

    var addMemory: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                Group {
                    if isSnapshot {
                        Text("Tell me something worth remembering about \(detail.title)").foregroundColor(Phosphor.dim)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    } else {
                        TextField("Tell me something worth remembering about \(detail.title)", text: Binding(get: { draft.value }, set: { draft.value = $0 }))
                            .textFieldStyle(.plain).foregroundColor(Phosphor.cream)
                            .onSubmit(file)
                    }
                }
                .font(Fonts.grotesk(14))
                .padding(.horizontal, 12)
                .frame(height: 40)
                .background(Phosphor.black)
                .overlay(Rectangle().strokeBorder(Phosphor.line, lineWidth: 1))
                .accessibilityLabel("Add a memory to \(detail.title)")
                Button("File it", action: file).buttonStyle(PrimaryButtonStyle(size: 13, height: 40))
            }
            if refused.value {
                Text("That looks like a secret, so I didn't keep it. Store a pointer instead, like where the key lives.")
                    .font(Fonts.grotesk(12)).foregroundColor(Phosphor.cream2)
            }
        }
    }

    func file() {
        let text = draft.value
        guard !text.trimmingCharacters(in: .whitespaces).isEmpty else { return }
        refused.value = !model.addMemory(text, to: detail.ref)
        if !refused.value { draft.value = "" }
    }
}

/// Text with `[[wikilinks]]` drawn in amber.
struct LinkedText: View {
    var text: String

    init(_ text: String) { self.text = text }

    var body: some View {
        var result = Text("")
        var rest = Substring(text)
        while let open = rest.range(of: "[["), let close = rest.range(of: "]]", range: open.upperBound..<rest.endIndex) {
            result = result + Text(String(rest[..<open.lowerBound])).foregroundColor(Phosphor.cream)
            result = result + Text(String(rest[open.lowerBound..<close.upperBound])).foregroundColor(Phosphor.amber)
            rest = rest[close.upperBound...]
        }
        return result + Text(String(rest)).foregroundColor(Phosphor.cream)
    }
}

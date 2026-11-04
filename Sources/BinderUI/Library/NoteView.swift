import BinderCore
import SwiftUI

/// One note: what Binder observed, what was decided (and where it came from), the user's own text, and a field to add more.
struct NoteView: View {
    @ObservedObject var model: LibraryModel
    var detail: NoteDetail
    @StateObject private var draft = Local("")
    @StateObject private var refused = Local(false)
    @StateObject private var splitting = Local(false)
    @StateObject private var splitMessage = Local<String?>(nil)
    @StateObject private var summarizing = Local(false)
    /// The text being edited, while the editor is open.
    @StateObject private var editing = Local<String?>(nil)
    @StateObject private var editOpened = Local(Date.distantPast)
    @StateObject private var conflict = Local(false)
    @StateObject private var hovered = Local<String?>(nil)
    @StateObject private var lineToDelete = Local<String?>(nil)
    @StateObject private var confirmNoteDelete = Local(false)
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
                    if let usage = Self.usageLine(detail.usage, now: Date()) {
                        Text(usage.uppercased()).font(Fonts.mono(10.5)).foregroundColor(Phosphor.dim)
                            .help("Session cards are counted exactly. Used is an estimate from the commands and replies in your sessions.")
                    }
                    if !detail.topics.isEmpty { topics }
                    if editing.value == nil { noteActions }
                    if let message = splitMessage.value { Text(message).font(Fonts.grotesk(12.5)).foregroundColor(Phosphor.cream2) }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                HStack(spacing: 8) {
                    if editing.value != nil {
                        Button("Cancel") { editing.value = nil }.buttonStyle(GhostButtonStyle(height: 32))
                        Button("Save") { save(force: false) }.buttonStyle(CreamButtonStyle())
                    } else {
                        if LibraryModel.obsidianInstalled { Button("Open in Obsidian") { model.openInObsidian(detail.ref) }.buttonStyle(GhostButtonStyle(height: 32)) }
                        Button("Edit") {
                            let (text, opened) = model.beginEdit(detail.ref)
                            editOpened.value = opened
                            editing.value = text
                        }
                        .buttonStyle(CreamButtonStyle())
                    }
                }
                .fixedSize()
            }
            if editing.value != nil { editor } else { content; addMemory }
        }
        .padding(.horizontal, 32)
        .padding(.vertical, 26)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .onChange(of: detail.ref) { _, _ in editing.value = nil }
        .alert("Changed outside Binder", isPresented: Binding(get: { conflict.value }, set: { conflict.value = $0 })) {
            Button("Overwrite") { save(force: true) }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("This note changed while you were editing. Overwrite it with your version?")
        }
        .confirmationDialog("Delete this line?", isPresented: Binding(get: { lineToDelete.value != nil }, set: { if !$0 { lineToDelete.value = nil } })) {
            Button("Delete", role: .destructive) {
                if let line = lineToDelete.value { model.delete(line: line, in: detail.ref) }
                lineToDelete.value = nil
            }
        }
        .confirmationDialog("Move \(detail.title) to the Trash?", isPresented: Binding(get: { confirmNoteDelete.value }, set: { confirmNoteDelete.value = $0 })) {
            Button("Move to Trash", role: .destructive) { try? model.deleteNote(detail.ref) }
        }
    }

    /// Everything outside the observed block, as Markdown. The observed part sits above it, read-only.
    var editor: some View {
        VStack(alignment: .leading, spacing: 12) {
            if !detail.facts.isEmpty { observed }
            SectionLabel("Your note · Markdown")
            TextEditor(text: Binding(get: { editing.value ?? "" }, set: { editing.value = $0 }))
                .font(Fonts.grotesk(13.5))
                .foregroundColor(Phosphor.cream)
                .scrollContentBackground(.hidden)
                .padding(8)
                .background(Phosphor.black)
                .overlay(Rectangle().strokeBorder(Phosphor.line, lineWidth: 1))
                .frame(maxHeight: .infinity)
                .accessibilityLabel("Edit \(detail.title)")
        }
    }

    func save(force: Bool) {
        guard let text = editing.value else { return }
        switch model.save(text, to: detail.ref, opened: editOpened.value, force: force) {
        case .saved: editing.value = nil
        case .changedOnDisk: conflict.value = true
        }
    }

    var content: some View {
            Scroll {
                VStack(alignment: .leading, spacing: 18) {
                    if !detail.facts.isEmpty { observed }
                    if !detail.decisions.isEmpty { decisions }
                    if !keptFiles.isEmpty { files }
                    if !detail.fading.isEmpty { fading }
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
    }

    static func short(_ iso: String) -> String {
        guard let date = ISO8601DateFormatter().date(from: iso) else { return iso.uppercased() }
        let f = DateFormatter()
        f.dateFormat = Calendar.current.isDateInToday(date) ? "HH:mm 'TODAY'" : "d MMM HH:mm"
        return f.string(from: date).uppercased()
    }

    /// Less frequent actions, kept out of the header.
    var noteActions: some View {
        HStack(spacing: 8) {
            if LibraryModel.overviewProject(detail.ref) != nil {
                Button(splitting.value ? "Sorting…" : "Split by topic", action: split)
                    .buttonStyle(GhostButtonStyle(height: 26)).disabled(splitting.value)
                Button(summarizing.value ? "Reading the code…" : "Summarize code", action: summarize)
                    .buttonStyle(GhostButtonStyle(height: 26)).disabled(summarizing.value)
            }
            if detail.ref.path != "me.md" {
                Button("Delete note") { confirmNoteDelete.value = true }.buttonStyle(QuietButtonStyle())
            }
        }
        .padding(.top, 4)
    }

    /// The project's topic notes; one click opens a topic.
    var topics: some View {
        HStack(alignment: .top, spacing: 8) {
            Text("TOPICS").font(Fonts.mono(10.5)).foregroundColor(Phosphor.dim).frame(height: 26)
            FlowLayout(spacing: 6) {
                ForEach(detail.topics) { topic in
                    Button(topic.title) { model.selected = topic }.buttonStyle(GhostButtonStyle(height: 26)).fixedSize()
                }
            }
        }
        .padding(.top, 4)
    }

    func split() {
        splitting.value = true
        splitMessage.value = nil
        let ref = detail.ref
        Task {
            let result = await model.splitByTopic(ref)
            splitting.value = false
            splitMessage.value = Self.splitMessage(result)
        }
    }

    /// `UNTIL 8 NOV`: when a temporary fact stops being true.
    static func untilLabel(_ date: Date) -> String {
        let f = DateFormatter()
        f.timeZone = TimeZone(identifier: "UTC")
        f.dateFormat = "d MMM"
        return "UNTIL \(f.string(from: date).uppercased())"
    }

    /// `On 40 session cards · used 12 times · last today`, or nil when nothing used it yet.
    static func usageLine(_ c: UseCounts, now: Date) -> String? {
        var parts: [String] = []
        if c.cards > 0 { parts.append("On \(c.cards) session card\(c.cards == 1 ? "" : "s")") }
        if c.used > 0 { parts.append(c.used == 1 ? "used once" : "used \(c.used) times") }
        guard !parts.isEmpty else { return nil }
        if let last = [c.lastCard, c.lastUsed].compactMap({ $0 }).max() {
            let calendar = Calendar.current
            if calendar.isDate(last, inSameDayAs: now) { parts.append("last today") }
            else if calendar.isDate(last, inSameDayAs: now.addingTimeInterval(-86_400)) { parts.append("last yesterday") }
            else {
                let f = DateFormatter()
                f.dateFormat = "d MMM"
                parts.append("last \(f.string(from: last))")
            }
        }
        let line = parts.joined(separator: " · ")
        return line.prefix(1).uppercased() + line.dropFirst()
    }

    /// `On 31 cards · used 4 times`, for one line.
    static func lineUsage(_ c: UseCounts) -> String? {
        var parts: [String] = []
        if c.cards > 0 { parts.append("On \(c.cards) card\(c.cards == 1 ? "" : "s")") }
        if c.used > 0 { parts.append(c.used == 1 ? "used once" : "used \(c.used) times") }
        guard !parts.isEmpty else { return nil }
        let line = parts.joined(separator: " · ")
        return line.prefix(1).uppercased() + line.dropFirst()
    }

    func summarize() {
        summarizing.value = true
        splitMessage.value = nil
        let ref = detail.ref
        Task {
            let added = await model.summarizeCode(ref)
            summarizing.value = false
            splitMessage.value = Self.summaryMessage(added, consent: model.state.summarizerConsent)
        }
    }

    static func summaryMessage(_ added: Int?, consent: Bool) -> String {
        guard consent else { return "Needs Read sessions with your Claude plan, in Settings." }
        guard let added else { return "Couldn't find this project's folder, or the model didn't answer. Try again in a bit." }
        return added == 0 ? "Nothing new about the code." : "\(added) \(added == 1 ? "catch" : "catches") about the code waiting in the menu bar."
    }

    static func splitMessage(_ result: TopicSplit.Result?) -> String {
        guard let result else { return "Needs Read sessions with your Claude plan, in Settings." }
        guard result.lines > 0 else { return "Nothing to sort. Everything here is about the whole project." }
        return "Moved \(result.lines) line\(result.lines == 1 ? "" : "s") into \(result.topics) topic\(result.topics == 1 ? "" : "s")."
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
                    VStack(alignment: .leading, spacing: 3) {
                        HStack(alignment: .firstTextBaseline, spacing: 8) {
                            Text(Self.withoutEmbed(d.text)).font(Fonts.grotesk(13.5)).foregroundColor(Phosphor.cream)
                            if let until = d.until { Text(Self.untilLabel(until)).font(Fonts.mono(10.5)).foregroundColor(Phosphor.amber) }
                        }
                        ForEach(Array(d.details.enumerated()), id: \.offset) { _, line in
                            Text(line).font(Fonts.grotesk(12.5)).foregroundColor(Phosphor.dim).fixedSize(horizontal: false, vertical: true)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    if hovered.value == d.line {
                        Button("Wrong") { model.markWrong(d.line, in: detail.ref) }.buttonStyle(QuietButtonStyle())
                            .help("Move it to Superseded and never suggest it again")
                        if let usage = Self.lineUsage(d.usage) {
                            Text(usage.uppercased()).font(Fonts.mono(10.5)).foregroundColor(Phosphor.dim)
                                .help("An estimate from the commands and replies in your sessions.")
                        }
                        Button("Delete") { lineToDelete.value = d.line }.buttonStyle(QuietButtonStyle())
                    }
                    Text(d.provenance.uppercased()).font(Fonts.mono(10.5)).foregroundColor(Phosphor.dim).frame(width: 150, alignment: .trailing)
                }
                .padding(.vertical, 8)
                .contentShape(Rectangle())
                .onHover { inside in
                    if inside { hovered.value = d.line } else if hovered.value == d.line { hovered.value = nil }
                }
                .overlay(alignment: .top) { Rectangle().fill(Phosphor.line).frame(height: 1) }
            }
        }
    }

    /// Old lines no agent used: off the card, still here, one click to keep them on it.
    var fading: some View {
        VStack(alignment: .leading, spacing: 0) {
            SectionLabel("Fading · not on the card").padding(.bottom, 6)
            ForEach(Array(detail.fading.enumerated()), id: \.offset) { _, d in
                HStack(alignment: .firstTextBaseline, spacing: 12) {
                    Text(Self.day(d.date)).font(Fonts.vt(18)).foregroundColor(Phosphor.dim).frame(width: 70, alignment: .leading)
                    Text(Self.withoutEmbed(d.text)).font(Fonts.grotesk(12.5)).foregroundColor(Phosphor.dim)
                        .frame(maxWidth: .infinity, alignment: .leading)
                    Button("Keep on card") { model.pin(d.line, in: detail.ref) }.buttonStyle(QuietButtonStyle())
                }
                .padding(.vertical, 4)
            }
        }
    }

    /// Lines later ones made untrue: kept out of the card, shown dim here, one click to bring back.
    var superseded: some View {
        VStack(alignment: .leading, spacing: 0) {
            SectionLabel("Superseded · not on the card").padding(.bottom, 6)
            ForEach(Array(detail.superseded.prefix(12).enumerated()), id: \.offset) { _, line in
                HStack(alignment: .firstTextBaseline, spacing: 12) {
                    Text(CardBuilder.decisionText(line.replacingOccurrences(of: Note.supersededSuffix, with: "", options: .regularExpression)))
                        .font(Fonts.grotesk(12.5)).foregroundColor(Phosphor.dim).strikethrough(color: Phosphor.line)
                        .frame(maxWidth: .infinity, alignment: .leading)
                    Button("Restore") { model.restore(line, in: detail.ref) }.buttonStyle(QuietButtonStyle())
                }
                .padding(.vertical, 4)
            }
        }
    }

    /// `![[files/dawn/x.png]] Caption` → `files/dawn/x.png`.
    static func embed(in text: String) -> String? {
        text.firstMatch(of: #/!\[\[(files/[^\]]+)\]\]/#).map { String($0.1) }
    }

    static func withoutEmbed(_ text: String) -> String {
        text.replacing(#/!\[\[files/[^\]]+\]\]\s*/#, with: "")
    }

    var keptFiles: [String] { detail.decisions.compactMap { Self.embed(in: $0.text) } }

    /// The note's kept images and PDFs; one click opens a file in Preview.
    var files: some View {
        VStack(alignment: .leading, spacing: 8) {
            SectionLabel("Files")
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 10) {
                    ForEach(keptFiles, id: \.self) { path in
                        let url = model.vault.url(path)
                        Button { NSWorkspace.shared.open(url) } label: {
                            Group {
                                if url.pathExtension != "pdf", let image = NSImage(contentsOf: url) {
                                    Image(nsImage: image).resizable().aspectRatio(contentMode: .fill)
                                } else {
                                    Text(url.pathExtension.uppercased()).font(Fonts.vt(22)).foregroundColor(Phosphor.amber)
                                }
                            }
                            .frame(width: 120, height: 80)
                            .clipped()
                            .overlay(Rectangle().strokeBorder(Phosphor.line, lineWidth: 1))
                        }
                        .buttonStyle(.plain)
                        .help(url.lastPathComponent)
                        .accessibilityLabel("Open \(url.lastPathComponent)")
                    }
                }
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

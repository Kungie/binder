import BinderCore
import SwiftUI

/// The part Binder can't guess: who you are, how agents should work with you, your taste, what you're on.
struct AboutYouView: View {
    @ObservedObject var model: OnboardingModel
    @StateObject private var newStyle = Local("")
    @StateObject private var newTaste = Local("")
    @Environment(\.isSnapshot) private var isSnapshot

    static let styles = ["Short answers", "Ask before pushing", "Ask before deleting files", "Recommend, don't list",
                         "Explain trade-offs", "Commit often", "Don't ask about hosting"]

    var body: some View {
        SetupShell(step: OnboardingStep.aboutYou.rawValue, mood: .happy, caption: "THE PART I\nCAN'T GUESS.") {
            VStack(alignment: .leading, spacing: 10) {
                StepHeading("Now, about you.", "Agents will read this before they say hello. Write it the way you'd brief a new colleague.")
                VStack(alignment: .leading, spacing: 10) {
                    SectionLabel("You")
                    HStack(spacing: 10) {
                        BinderField("Name", text: $model.profile.name, placeholder: "What agents should call you")
                        BinderField("Language", text: $model.profile.language, placeholder: "English")
                        BinderField("Time zone", text: $model.profile.timeZone, detected: true)
                    }
                    BinderField("What you do", text: $model.profile.role, placeholder: "One line is plenty")
                }
                VStack(alignment: .leading, spacing: 10) {
                    SectionLabel("How agents should work with you")
                    FlowLayout(spacing: 8) {
                        ForEach(Self.styles + model.profile.workingStyle.filter { !Self.styles.contains($0) }, id: \.self) { style in
                            Chip(style, on: model.profile.workingStyle.contains(style)) { toggle(style) }
                        }
                        addField(newStyle, placeholder: "Add your own") { model.profile.workingStyle.append($0) }
                    }
                }
                VStack(alignment: .leading, spacing: 10) {
                    HStack(spacing: 0) {
                        SectionLabel("Taste · ")
                        SectionLabel("Mostly detected from your repos", color: Phosphor.amber)
                    }
                    FlowLayout(spacing: 8) {
                        ForEach(model.profile.taste, id: \.self) { item in
                            tasteChip(item, avoided: false) {
                                model.profile.taste.removeAll { $0 == item }
                                model.profile.avoid.append(item)
                            }
                        }
                        ForEach(model.profile.avoid, id: \.self) { item in
                            tasteChip(item, avoided: true) { model.profile.avoid.removeAll { $0 == item } }
                        }
                        addField(newTaste, placeholder: "Add a tool") { model.profile.taste.append($0) }
                    }
                    Text("Tap one you'd rather not use to strike it. Tap again to remove it.")
                        .font(Fonts.grotesk(11.5)).foregroundColor(Phosphor.dim)
                }
                VStack(alignment: .leading, spacing: 6) {
                    SectionLabel("Now and next")
                    Group {
                        if isSnapshot {
                            Text(model.profile.nowAndNext).frame(maxWidth: .infinity, minHeight: 38, alignment: .topLeading)
                        } else {
                            TextEditor(text: $model.profile.nowAndNext).scrollContentBackground(.hidden).frame(height: 38)
                        }
                    }
                    .font(Fonts.grotesk(14))
                    .foregroundColor(Phosphor.cream)
                    .padding(8)
                    .background(Phosphor.black)
                    .overlay(Rectangle().strokeBorder(Phosphor.line, lineWidth: 1))
                    .accessibilityLabel("Now and next")
                }
                Spacer(minLength: 0)
                HStack(spacing: 10) {
                    Button("Write it down") { model.next() }.buttonStyle(PrimaryButtonStyle(size: 14, height: 40))
                    Button("Fill in later") { model.next() }.buttonStyle(GhostButtonStyle(height: 40))
                }
            }
        }
    }

    func toggle(_ style: String) {
        if let i = model.profile.workingStyle.firstIndex(of: style) { model.profile.workingStyle.remove(at: i) }
        else { model.profile.workingStyle.append(style) }
    }

    func tasteChip(_ text: String, avoided: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(text.uppercased())
                .font(Fonts.vt(18))
                .strikethrough(avoided, color: Phosphor.dim)
                .foregroundColor(avoided ? Phosphor.dim : Phosphor.cream)
                .padding(.horizontal, 10)
                .frame(height: 28)
                .background(avoided ? Color.clear : Phosphor.black)
                .overlay(Rectangle().strokeBorder(avoided ? Phosphor.dim : Phosphor.line, style: StrokeStyle(lineWidth: 1, dash: avoided ? [3, 2] : [])))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(avoided ? "\(text), rather not use" : text)
    }

    @ViewBuilder
    func addField(_ box: Local<String>, placeholder: String, add: @escaping (String) -> Void) -> some View {
        if !isSnapshot {
            TextField(placeholder, text: Binding(get: { box.value }, set: { box.value = $0 }))
                .textFieldStyle(.plain)
                .font(Fonts.grotesk(12, weight: 600))
                .foregroundColor(Phosphor.cream)
                .padding(.horizontal, 10)
                .frame(width: 130, height: 30)
                .overlay(Rectangle().strokeBorder(Phosphor.dim, style: StrokeStyle(lineWidth: 1, dash: [3, 2])))
                .onSubmit {
                    let value = box.value.trimmingCharacters(in: .whitespaces)
                    if !value.isEmpty { add(value) }
                    box.value = ""
                }
        }
    }
}

/// Lays children out left to right, wrapping to a new row when the width runs out.
struct FlowLayout: Layout {
    var spacing: CGFloat = 8

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let width = proposal.width ?? 600
        var x: CGFloat = 0, y: CGFloat = 0, row: CGFloat = 0
        for view in subviews {
            let size = view.sizeThatFits(.unspecified)
            if x > 0, x + size.width > width { x = 0; y += row + spacing; row = 0 }
            x += size.width + spacing
            row = max(row, size.height)
        }
        return CGSize(width: width, height: y + row)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var x = bounds.minX, y = bounds.minY, row: CGFloat = 0
        for view in subviews {
            let size = view.sizeThatFits(.unspecified)
            if x > bounds.minX, x + size.width > bounds.maxX { x = bounds.minX; y += row + spacing; row = 0 }
            view.place(at: CGPoint(x: x, y: y), proposal: ProposedViewSize(size))
            x += size.width + spacing
            row = max(row, size.height)
        }
    }
}

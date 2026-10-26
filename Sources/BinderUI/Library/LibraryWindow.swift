import AppKit
import BinderCore
import SwiftUI

/// What the main window can ask the app to do.
public struct LibraryActions {
    public var openSession: (String) -> Void = { _ in }
    public var runDoctor: () -> [DoctorLine] = { [] }
    public var installHooks: (Bool) -> Bool = { _ in false }
    public var hooksInstalled: () -> Bool = { false }
    public var vaultMoved: (Vault) -> Void = { _ in }

    public init(openSession: @escaping (String) -> Void = { _ in }, runDoctor: @escaping () -> [DoctorLine] = { [] },
                installHooks: @escaping (Bool) -> Bool = { _ in false }, hooksInstalled: @escaping () -> Bool = { false },
                vaultMoved: @escaping (Vault) -> Void = { _ in }) {
        self.openSession = openSession
        self.runDoctor = runDoctor
        self.installHooks = installHooks
        self.hooksInstalled = hooksInstalled
        self.vaultMoved = vaultMoved
    }
}

/// The main window: sidebar, the note (or inbox, or settings), and the context column.
public struct LibraryView: View {
    @ObservedObject var model: LibraryModel
    @ObservedObject var app: AppModel
    var actions: LibraryActions
    @StateObject private var search = Local("")

    public init(model: LibraryModel, app: AppModel, actions: LibraryActions = LibraryActions()) {
        self.model = model
        self.app = app
        self.actions = actions
    }

    public var body: some View {
        VStack(spacing: 0) {
            TitleStrip("BINDER")
            HStack(spacing: 0) {
                sidebar
                switch model.selectedKind {
                case .inbox:
                    InboxView(app: app).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                case .settings:
                    SettingsView(model: model, actions: actions).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                default:
                    if let ref = model.selected {
                        let detail = model.note(ref)
                        NoteView(model: model, detail: detail)
                        ContextColumn(detail: detail, session: app.session, openSession: actions.openSession)
                    } else {
                        Text("Nothing here yet.").font(Fonts.grotesk(14)).foregroundColor(Phosphor.dim)
                            .frame(maxWidth: .infinity, maxHeight: .infinity)
                    }
                }
            }
        }
        .frame(minWidth: 1040, minHeight: 700)
        .background(Phosphor.black2)
        .ignoresSafeArea(.container, edges: .top)
        .environment(\.colorScheme, .dark)
        .onAppear { model.reload() }
    }

    // MARK: Sidebar

    var sidebar: some View {
        VStack(alignment: .leading, spacing: 14) {
            BinderSearchField(text: Binding(get: { search.value }, set: { search.value = $0 }))
            Scroll {
                VStack(alignment: .leading, spacing: 2) {
                    ForEach(model.sections.filter { $0.kind != .settings }) { section in
                        sidebarRow(section)
                        if model.selectedKind == section.kind || !search.value.isEmpty {
                            ForEach(filtered(section.items)) { item in
                                Button { model.selected = item; model.selectedKind = section.kind } label: {
                                    Text((model.selected == item ? "▸ " : "") + item.title.uppercased())
                                        .lineLimit(1)
                                        .frame(maxWidth: .infinity, minHeight: 30, alignment: .leading)
                                        .padding(.leading, 20)
                                        .background(model.selected == item ? Phosphor.line : .clear)
                                        .foregroundColor(model.selected == item ? Phosphor.amber : Phosphor.dim)
                                }
                                .buttonStyle(.plain)
                                .accessibilityAddTraits(model.selected == item ? .isSelected : [])
                            }
                        }
                    }
                }
                .font(Fonts.mono(11))
                .tracking(0.9)
            }
            HStack(spacing: 10) {
                MascotHead(size: 40, rimWidth: 8)
                VStack(alignment: .leading, spacing: 0) {
                    Text("\(app.memories) MEMORIES").font(Fonts.vt(17)).foregroundColor(Phosphor.cream2)
                    Text("LAST LOOK \(lastLook)").font(Fonts.vt(17)).foregroundColor(Phosphor.dim)
                }
            }
            .padding(.horizontal, 8).padding(.vertical, 10)
            .overlay(alignment: .top) { Rectangle().fill(Phosphor.line).frame(height: 1) }
            Button { model.selectedKind = .settings } label: {
                Text("SETTINGS").font(Fonts.mono(11)).tracking(0.9)
                    .foregroundColor(model.selectedKind == .settings ? Phosphor.amber : Phosphor.dim)
                    .frame(maxWidth: .infinity, minHeight: 30, alignment: .leading)
                    .padding(.horizontal, 8)
                    .overlay(Rectangle().strokeBorder(Phosphor.line, lineWidth: 1))
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 16)
        .frame(width: 220)
        .background(ZStack { Phosphor.black; Scanlines() })
        .overlay(alignment: .trailing) { Rectangle().fill(Phosphor.line).frame(width: 1) }
    }

    func sidebarRow(_ section: LibrarySection) -> some View {
        Button {
            model.selectedKind = section.kind
            if section.kind != .inbox { model.selected = section.items.first }
        } label: {
            HStack {
                Text(section.kind.label).foregroundColor(model.selectedKind == section.kind ? Phosphor.cream : Phosphor.cream2)
                Spacer()
                if section.kind == .inbox {
                    if model.inboxCount > 0 {
                        Text("\(model.inboxCount)").font(Fonts.vt(18)).foregroundColor(Phosphor.ink).padding(.horizontal, 6).background(Phosphor.amber)
                    }
                } else if section.kind != .me, !section.items.isEmpty {
                    Text("\(section.items.count)").font(Fonts.vt(18)).foregroundColor(Phosphor.dim)
                }
            }
            .frame(minHeight: 30)
            .padding(.horizontal, 8)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    func filtered(_ items: [NoteRef]) -> [NoteRef] {
        let q = search.value.trimmingCharacters(in: .whitespaces).lowercased()
        return q.isEmpty ? items : items.filter { $0.title.lowercased().contains(q) || $0.path.lowercased().contains(q) }
    }

    var lastLook: String {
        guard let date = model.state.lastObserve else { return "NEVER" }
        let f = DateFormatter()
        f.dateFormat = Calendar.current.isDateInToday(date) ? "HH:mm" : "d MMM"
        return f.string(from: date).uppercased()
    }
}

struct BinderSearchField: View {
    @Binding var text: String
    @Environment(\.isSnapshot) private var isSnapshot

    var body: some View {
        Group {
            if isSnapshot {
                Text(text.isEmpty ? "Search your binder" : text).foregroundColor(Phosphor.dim).frame(maxWidth: .infinity, alignment: .leading)
            } else {
                TextField("Search your binder", text: $text).textFieldStyle(.plain).foregroundColor(Phosphor.cream)
            }
        }
        .font(Fonts.grotesk(13))
        .padding(.horizontal, 10)
        .frame(height: 32)
        .background(Phosphor.black2)
        .overlay(Rectangle().strokeBorder(Phosphor.line, lineWidth: 1))
        .accessibilityLabel("Search your binder")
    }
}

/// The inbox in the main window: the same cards as the menu bar, with more room.
struct InboxView: View {
    @ObservedObject var app: AppModel

    var body: some View {
        Scroll {
            VStack(alignment: .leading, spacing: 12) {
                SectionLabel("Inbox · \(app.pendingCount) to review")
                if app.proposals.isEmpty {
                    Text("ALL FILED.").font(Fonts.vt(26)).foregroundColor(Phosphor.amber)
                    Text("Nothing to review. I'll call you when there is.").font(Fonts.grotesk(13)).foregroundColor(Phosphor.cream2)
                } else {
                    ForEach(app.proposals) { p in
                        if app.phases[p.id] == .shredding {
                            ShredView(card: ProposalCard(proposal: p, model: app).allowsHitTesting(false)).frame(height: 92)
                        } else {
                            ProposalCard(proposal: p, model: app).opacity(app.phases[p.id] == .absorbing ? 0.3 : 1)
                        }
                    }
                    if app.pendingCount > 1 {
                        Button("Keep all \(app.pendingCount)") { app.keepAll() }.buttonStyle(PrimaryButtonStyle(size: 13, height: 36))
                    }
                }
            }
            .padding(32)
            .frame(maxWidth: 640, alignment: .leading)
        }
    }
}

/// A dark main window whose title bar is Binder's own title strip.
@MainActor
public final class LibraryWindowController: NSWindowController {
    public init(model: LibraryModel, app: AppModel, actions: LibraryActions) {
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1240, height: 820),
                              styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView], backing: .buffered, defer: false)
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .hidden
        window.title = "Binder"
        window.appearance = NSAppearance(named: .darkAqua)
        window.backgroundColor = NSColor(Phosphor.black)
        window.isReleasedWhenClosed = false
        window.setFrameAutosaveName("BinderLibrary")
        super.init(window: window)
        window.contentView = NSHostingView(rootView: LibraryView(model: model, app: app, actions: actions)
            .environment(\.titlebarHeight, window.titlebarHeight))
        window.center()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError() }
}

import BinderCore
import Foundation

public enum OnboardingStep: Int, CaseIterable, Codable, Sendable {
    case boot, permissions, lookAround, aboutYou, vault, connect

    public var railLabel: String {
        ["HELLO", "PERMISSIONS", "LOOK AROUND", "ABOUT YOU", "YOUR BINDER", "CONNECT"][rawValue]
    }
}

public enum LookStatus: Equatable, Sendable { case looking, found, half, ask }

public struct LookRow: Identifiable, Equatable, Sendable {
    public var label: String
    public var detail: String
    public var status: LookStatus
    public var id: String { label }
}

/// The six setup steps: what's been asked, found, typed in and written, saved as it goes so a quit resumes.
@MainActor
public final class OnboardingModel: ObservableObject {
    @Published public var step: OnboardingStep = .boot
    @Published public var permissionStatus: [Permission: PermissionState] = [:]
    @Published public private(set) var rows: [LookRow] = []
    @Published public private(set) var lookingDone = false
    @Published public private(set) var filesRead = 0
    @Published public private(set) var homelabQuestion: String?
    @Published public private(set) var homelabAnswer: Bool?
    @Published public var profile = Profile()
    @Published public private(set) var vaultRoot: URL
    @Published public private(set) var writtenFiles: [String] = []
    @Published public private(set) var connected = false
    @Published public private(set) var summarizerConsent: Bool?

    let home: URL
    let runner: CommandRunner
    let permissions: PermissionRequesting
    let settingsURL: URL
    let binderPath: String
    let shell: String?
    private var observations: [Observation] = []

    public init(home: URL, runner: CommandRunner, permissions: PermissionRequesting, settingsURL: URL, binderPath: String,
                shell: String? = ProcessInfo.processInfo.environment["SHELL"]) {
        self.home = home
        self.runner = runner
        self.permissions = permissions
        self.settingsURL = settingsURL
        self.binderPath = binderPath
        self.shell = shell
        self.vaultRoot = VaultLocation.load(home: home) ?? home.appendingPathComponent("Binder")
        for p in Permission.allCases { permissionStatus[p] = permissions.status(p) }
        restore()
    }

    // MARK: Steps

    public func next() {
        if let following = OnboardingStep(rawValue: step.rawValue + 1) { step = following }
        save()
    }

    public func back() {
        if let previous = OnboardingStep(rawValue: step.rawValue - 1) { step = previous }
        save()
    }

    public func request(_ p: Permission) async {
        permissionStatus[p] = await permissions.request(p)
    }

    // MARK: Look around

    var projectRoots: [URL] {
        BinderState().projectRoots.map { $0.hasPrefix("~/") ? home.appendingPathComponent(String($0.dropFirst(2))) : URL(fileURLWithPath: $0) }
    }

    public func runLookAround() async {
        lookingDone = false
        rows = ["THIS MAC", "SSH", "GITHUB", "HOSTING", "DOMAINS", "PROJECTS", "AGENTS", "HOMELAB"].map { LookRow(label: $0, detail: "", status: .looking) }
        let ctx = ObserverContext(home: home, projectRoots: projectRoots, runner: runner, allowLAN: permissionStatus[.localNetwork] == .asked || permissionStatus[.localNetwork] == .granted)
        let observers: [Observer] = [MacObserver(shell: shell), SSHObserver(), AccountsObserver(), ProjectsObserver(), HomelabObserver()]
        observations = []
        for observer in observers {
            let obs = await Task.detached { observer.observe(ctx) }.value
            observations.append(obs)
            filesRead += obs.filesRead
            apply(observer.name, obs)
        }
        if profile.timeZone.isEmpty { profile.timeZone = TimeZone.current.identifier }
        if profile.taste.isEmpty { profile.taste = detectedTaste() }
        lookingDone = true
    }

    private func set(_ label: String, _ detail: String, _ status: LookStatus) {
        if let i = rows.firstIndex(where: { $0.label == label }) { rows[i] = LookRow(label: label, detail: detail, status: status) }
    }

    private func lines(_ obs: Observation, _ path: String) -> [String] {
        obs.notes.first { $0.path == path }?.observed ?? []
    }

    private func value(_ lines: [String], _ key: String) -> String? {
        lines.first { $0.hasPrefix("- \(key): ") }.map { String($0.dropFirst("- \(key): ".count)) }
    }

    private func plural(_ n: Int, _ one: String, _ many: String) -> String { "\(n) \(n == 1 ? one : many)" }

    private func apply(_ name: String, _ obs: Observation) {
        switch name {
        case "mac":
            let mac = lines(obs, "machines/this-mac.md")
            var parts: [String] = []
            if let v = value(mac, "macOS") { parts.append("macOS \(v)") }
            if let v = value(mac, "shell") { parts.append(v) }
            if let v = value(mac, "homebrew") { parts.append("Homebrew, \(v.replacingOccurrences(of: " formulae", with: "")) packages") }
            set("THIS MAC", parts.joined(separator: " · "), .found)
            let agents = value(mac, "agents") ?? ""
            let claude = agents.contains("Claude Code found"), codex = agents.contains("Codex found")
            set("AGENTS", "Claude Code \(claude ? "found" : "not installed") · Codex \(codex ? "found" : "not installed")",
                claude && codex ? .found : (claude || codex ? .half : .ask))
        case "ssh":
            let ssh = lines(obs, "access/ssh.md")
            let keys = ssh.filter { $0.hasPrefix("- key ") }
            let hosts = ssh.count - keys.count
            guard hosts + keys.count > 0 else { set("SSH", "No SSH setup found", .half); return }
            var parts = [plural(hosts, "host", "hosts"), plural(keys.count, "key", "keys")]
            let idle = keys.filter { $0.hasSuffix("opens nothing") }.count
            if idle > 0 { parts.append(idle == 1 ? "1 key opens nothing" : "\(idle) keys open nothing") }
            set("SSH", parts.joined(separator: " · "), .found)
        case "accounts":
            let accounts = lines(obs, "access/accounts.md")
            if let login = value(accounts, "github"), login != "installed, not signed in" {
                set("GITHUB", "Signed in as \(login)", .found)
            } else {
                set("GITHUB", "gh isn't signed in", .half)
            }
            let names = [("vercel", "Vercel"), ("netlify", "Netlify"), ("fly.io", "Fly.io"), ("cloudflare", "Cloudflare"),
                         ("hetzner cloud", "Hetzner Cloud"), ("supabase", "Supabase")]
            let signedIn = names.filter { key, _ in (value(accounts, key).map { $0 != "installed, not signed in" }) ?? false }.map(\.1)
            switch signedIn.count {
            case 0: set("HOSTING", "No hosting tools signed in", .half)
            case 1: set("HOSTING", "\(signedIn[0]), signed in", .found)
            case 2: set("HOSTING", "\(signedIn[0]) and \(signedIn[1]), both signed in", .found)
            default: set("HOSTING", "\(signedIn.dropLast().joined(separator: ", ")) and \(signedIn.last!), all signed in", .found)
            }
        case "projects":
            let projects = obs.notes.filter { $0.path.hasPrefix("projects/") }
            set("PROJECTS", projects.isEmpty ? "No repos in your project folders" : (projects.count == 1 ? "1 repo found" : "\(projects.count) repos found"),
                projects.isEmpty ? .half : .found)
            let domains = Set(projects.flatMap { note in
                (value(note.observed, "domains") ?? "").split(separator: ",").map { $0.split(separator: " ").first.map(String.init) ?? "" }.filter { !$0.isEmpty }
            })
            set("DOMAINS", domains.isEmpty ? "None in project configs" : "\(plural(domains.count, "domain", "domains")) in project configs",
                domains.isEmpty ? .half : .found)
        case "homelab":
            guard let first = obs.proposals.first else { set("HOMELAB", "Nothing found on the local network", .half); return }
            let text = first.text
            homelabQuestion = text.hasSuffix(" is part of your homelab.")
                ? text.replacingOccurrences(of: " is part of your homelab.", with: " looks like your homelab. Yours?")
                : text + " Yours?"
            set("HOMELAB", homelabQuestion!, .ask)
        default:
            break
        }
    }

    public func answerHomelab(mine: Bool) {
        homelabAnswer = mine
        set("HOMELAB", mine ? "Yours. Noted." : "Not yours. Skipped.", mine ? .found : .half)
        save()
    }

    func detectedTaste() -> [String] {
        var counts: [String: Int] = [:]
        for obs in observations {
            for note in obs.notes where note.path.hasPrefix("projects/") {
                for language in (value(note.observed, "languages") ?? "").split(separator: ",") {
                    counts[language.trimmingCharacters(in: .whitespaces), default: 0] += 1
                }
            }
        }
        return counts.sorted { $0.value != $1.value ? $0.value > $1.value : $0.key < $1.key }.prefix(6).map(\.key)
    }

    // MARK: The vault

    /// Writes what was found and what the user typed, then points the CLI at the folder.
    public func createVault() throws {
        let vault = Vault(root: vaultRoot)
        try vault.ensureLayout()
        let replay = observations.map { Replay(observation: $0) }
        let ctx = ObserverContext(home: home, projectRoots: projectRoots, runner: runner)
        _ = try ObserveRun.run(vault: vault, ctx: ctx, observers: replay)
        let store = ProposalStore(vault: vault)
        for p in store.pending() where p.agent == "binder" && p.target.hasPrefix("servers/") {
            switch homelabAnswer {
            case true?: try store.keep(p.id)
            case false?: try store.drop(p.id)
            case nil: continue
            }
        }
        try NoteIO.update(vault.url("me.md"), title: "Me") { MeNote.render(profile, into: $0) }
        try VaultLocation.save(vaultRoot, home: home)
        writtenFiles = Self.notes(in: vault)
        save()
    }

    /// Moves the binder into another folder (an Obsidian vault, say) and repoints everything at it.
    public func moveVault(into folder: URL) throws {
        let destination = folder.appendingPathComponent("Binder")
        if FileManager.default.fileExists(atPath: vaultRoot.path) {
            try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
            try FileManager.default.moveItem(at: vaultRoot, to: destination)
        }
        vaultRoot = destination
        try VaultLocation.save(vaultRoot, home: home)
        writtenFiles = Self.notes(in: Vault(root: vaultRoot))
        save()
    }

    static func notes(in vault: Vault) -> [String] {
        let fm = FileManager.default
        var found: [String] = []
        let enumerator = fm.enumerator(at: vault.root, includingPropertiesForKeys: nil, options: [.skipsHiddenFiles])
        while let url = enumerator?.nextObject() as? URL {
            guard url.pathExtension == "md" else { continue }
            let relative = String(url.standardizedFileURL.path.dropFirst(vault.root.standardizedFileURL.path.count + 1))
            if !relative.hasPrefix("handoffs/") { found.append(relative) }
        }
        let order = ["me.md": 0, "machines": 1, "access": 2, "servers": 3, "projects": 4]
        return found.sorted {
            let a = order[$0.components(separatedBy: "/").first!] ?? (order[$0] ?? 9), b = order[$1.components(separatedBy: "/").first!] ?? (order[$1] ?? 9)
            return a != b ? a < b : $0 < $1
        }
    }

    // MARK: Connect and finish

    public func connect() throws {
        try HookInstaller(settingsURL: settingsURL, binderPath: binderPath).install()
        connected = true
    }

    public var claudeFound: Bool { FileManager.default.fileExists(atPath: home.appendingPathComponent(".claude").path) }

    public func setConsent(_ yes: Bool) {
        summarizerConsent = yes
        let vault = Vault(root: vaultRoot)
        var state = BinderState.load(vault)
        state.summarizerConsent = yes
        try? state.save(vault)
        save()
    }

    public func finish() throws {
        let vault = Vault(root: vaultRoot)
        var state = BinderState.load(vault)
        state.onboarded = true
        try state.save(vault)
        try? FileManager.default.removeItem(at: Self.progressURL(home: home))
    }

    public static func isOnboarded(home: URL) -> Bool {
        BinderState.load(Vault(root: VaultLocation.resolve(env: ProcessInfo.processInfo.environment, home: home))).onboarded
    }

    /// The first project with a domain or deploy target, for "Try it".
    public var tryItProject: (name: String, path: String)? {
        for obs in observations {
            for note in obs.notes where note.path.hasPrefix("projects/") {
                let repo = value(note.observed, "repo") ?? ""
                let path = repo.hasPrefix("~") || repo.hasPrefix("/") ? repo : nil
                return (note.title, path ?? note.title)
            }
        }
        return nil
    }

    /// Believable content for snapshots, without running any observer.
    func loadDemo(step: OnboardingStep) {
        self.step = step
        rows = [
            LookRow(label: "THIS MAC", detail: "macOS 27.0.1 · zsh · Homebrew, 214 packages", status: .found),
            LookRow(label: "SSH", detail: "3 hosts · 4 keys · 1 key opens nothing", status: .found),
            LookRow(label: "GITHUB", detail: "Signed in as Kungie", status: .found),
            LookRow(label: "HOSTING", detail: "Cloudflare and Vercel, both signed in", status: .found),
            LookRow(label: "DOMAINS", detail: "2 domains in project configs", status: .found),
            LookRow(label: "PROJECTS", detail: "6 repos found", status: .found),
            LookRow(label: "AGENTS", detail: "Claude Code found · Codex not installed", status: .half),
            LookRow(label: "HOMELAB", detail: "Something answers SSH on 192.168.1.20. Yours?", status: .ask)
        ]
        homelabQuestion = "Something answers SSH on 192.168.1.20. Yours?"
        filesRead = 41
        lookingDone = true
        profile = Profile(name: "Eren", role: "One-person studio. Small instruments and Mac apps.", timeZone: "Europe/Berlin",
                          language: "Turkish in chat, English in files", workingStyle: ["Short answers", "Ask before pushing", "Recommend, don't list"],
                          taste: ["Swift", "SwiftUI", "HTML", "Python"], avoid: ["Electron"], nowAndNext: "Shipping Binder v1. Keeping Dawn healthy.")
        writtenFiles = ["me.md", "machines/this-mac.md", "access/ssh.md", "access/accounts.md", "servers/hetzner-1.md",
                        "servers/homelab.md", "projects/dawn.md", "projects/site.md", "projects/binder.md"]
        permissionStatus[.folders] = .granted
        demoPreview = """
        ---
        verified: 2026-10-26 09:14
        source: observed, you
        ---
        # dawn
        <!-- binder:observed start -->
        - repo: github.com/kungie/dawn
        - stack: Swift, SwiftUI
        - release: GitHub Releases, zip + dmg
        - site: Cloudflare Pages, see [[site]]
        <!-- binder:observed end -->
        ## Decisions
        - 10-17 licence is AGPL-3.0
        """
    }

    var demoPreview: String?

    /// The raw text of the first project note, for the Vault step's preview.
    public var previewText: String {
        if let demoPreview { return demoPreview }
        guard let first = writtenFiles.first(where: { $0.hasPrefix("projects/") }) else { return "" }
        return (try? String(contentsOf: vaultRoot.appendingPathComponent(first), encoding: .utf8)) ?? ""
    }

    public var previewName: String { writtenFiles.first(where: { $0.hasPrefix("projects/") }) ?? "" }

    // MARK: Progress

    struct Progress: Codable {
        var step: OnboardingStep
        var profile: Profile
        var homelabAnswer: Bool?
        var consent: Bool?
    }

    static func progressURL(home: URL) -> URL {
        home.appendingPathComponent("Library/Application Support/Binder/onboarding.json")
    }

    func save() {
        let url = Self.progressURL(home: home)
        try? FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        let progress = Progress(step: step, profile: profile, homelabAnswer: homelabAnswer, consent: summarizerConsent)
        try? JSONEncoder().encode(progress).write(to: url, options: .atomic)
    }

    func restore() {
        guard let data = try? Data(contentsOf: Self.progressURL(home: home)),
              let progress = try? JSONDecoder().decode(Progress.self, from: data) else { return }
        step = progress.step
        profile = progress.profile
        homelabAnswer = progress.homelabAnswer
        summarizerConsent = progress.consent
    }
}

/// Hands back an observation that was already made, so the vault gets exactly what the user saw.
struct Replay: Observer {
    let name = "replay"
    let observation: Observation
    func observe(_ ctx: ObserverContext) -> Observation { observation }
}

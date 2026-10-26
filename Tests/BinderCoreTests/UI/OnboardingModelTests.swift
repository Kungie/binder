import Foundation
import Testing
@testable import BinderCore
@testable import BinderUI

struct FakePermissions: PermissionRequesting {
    var answer: PermissionState = .granted
    func status(_ p: Permission) -> PermissionState { .unknown }
    func request(_ p: Permission) async -> PermissionState { answer }
}

/// A home with an SSH config (one homelab host), Claude Code, and one project.
private func makeHome() throws -> URL {
    let home = makeTempDir("home")
    let ssh = home.appendingPathComponent(".ssh")
    try FileManager.default.createDirectory(at: ssh.appendingPathComponent("config.d"), withIntermediateDirectories: true)
    try FileManager.default.copyItem(at: fixture("ssh/config"), to: ssh.appendingPathComponent("config"))
    try FileManager.default.copyItem(at: fixture("ssh/config.d/homelab"), to: ssh.appendingPathComponent("config.d/homelab"))
    write("ssh-ed25519 AAAA test\n", to: ssh.appendingPathComponent("id_ed25519_hetzner.pub"))
    try FileManager.default.createDirectory(at: home.appendingPathComponent(".claude"), withIntermediateDirectories: true)
    let repo = home.appendingPathComponent("Desktop/dawn")
    write("[remote \"origin\"]\n\turl = git@github.com:Kungie/dawn.git\n", to: repo.appendingPathComponent(".git/config"))
    write("ref: refs/heads/main\n", to: repo.appendingPathComponent(".git/HEAD"))
    write("// app", to: repo.appendingPathComponent("Sources/App.swift"))
    write("name = \"dawn\"\n", to: repo.appendingPathComponent("wrangler.toml"))
    return home
}

private let runner = FakeRunner(results: [
    "sw_vers -productVersion": ok("27.0.1\n"),
    "uname -m": ok("arm64\n"),
    "brew list --formula -1": ok("gh\ngit\n"),
    "gh auth status": ok("✓ Logged in to github.com account Kungie (keyring)\n"),
    "wrangler whoami": ok("You are logged in with an OAuth Token\n")
])

@MainActor
@Suite(.serialized) struct OnboardingModelTests {
    func model(_ home: URL) -> OnboardingModel {
        OnboardingModel(home: home, runner: runner, permissions: FakePermissions(),
                        settingsURL: home.appendingPathComponent(".claude/settings.json"), binderPath: "/Applications/Binder.app/Contents/Helpers/binder",
                        shell: "/bin/zsh")
    }

    @Test func stepsAdvanceInOrder() throws {
        let m = model(try makeHome())
        #expect(m.step == .boot)
        var seen: [OnboardingStep] = [m.step]
        for _ in 1..<OnboardingStep.allCases.count { m.next(); seen.append(m.step) }
        #expect(seen == OnboardingStep.allCases)
        #expect(OnboardingStep.allCases.map(\.railLabel) == ["HELLO", "PERMISSIONS", "LOOK AROUND", "ABOUT YOU", "YOUR BINDER", "CONNECT"])
        m.next()
        #expect(m.step == .connect)
    }

    @Test func lookAroundBuildsRowsFromObservations() async throws {
        let m = model(try makeHome())
        await m.runLookAround()
        let rows = Dictionary(uniqueKeysWithValues: m.rows.map { ($0.label, $0) })
        #expect(rows["THIS MAC"]?.status == .found)
        #expect(rows["THIS MAC"]?.detail == "macOS 27.0.1 · zsh · Homebrew, 2 packages")
        #expect(rows["SSH"]?.detail == "5 hosts · 1 key")
        #expect(rows["GITHUB"]?.detail == "Signed in as Kungie")
        #expect(rows["HOSTING"]?.detail == "Cloudflare, signed in")
        #expect(rows["PROJECTS"]?.detail == "1 repo found")
        #expect(rows["AGENTS"]?.status == .half)
        #expect(rows["AGENTS"]?.detail == "Claude Code found · Codex not installed")
        #expect(rows["HOMELAB"]?.status == .ask)
        #expect(m.homelabQuestion == "proxmox at 192.168.1.20 looks like your homelab. Yours?")
        #expect(m.rows.map(\.label) == ["THIS MAC", "SSH", "GITHUB", "HOSTING", "DOMAINS", "PROJECTS", "AGENTS", "HOMELAB"])
        #expect(m.filesRead > 0)
        #expect(m.profile.taste == ["Swift"])
        #expect(!m.profile.timeZone.isEmpty)
        #expect(m.profile.name.isEmpty)
    }

    @Test func createVaultWritesNotesMeAndPointer() async throws {
        let home = try makeHome()
        let m = model(home)
        await m.runLookAround()
        m.answerHomelab(mine: true)
        m.profile.name = "Eren"
        m.profile.workingStyle = ["Short answers"]
        try m.createVault()
        let vault = Vault(root: m.vaultRoot)
        #expect(m.vaultRoot.path == home.appendingPathComponent("Binder").path)
        #expect(VaultLocation.load(home: home)?.path == m.vaultRoot.path)
        #expect(NoteIO.read(vault.url("me.md")).map(MeNote.profile)?.name == "Eren")
        #expect(NoteIO.read(vault.url("projects/dawn.md"))?.observedLines.contains("- repo: github.com/kungie/dawn") == true)
        #expect(NoteIO.read(vault.url("servers/proxmox.md"))?.decisions.first?.contains("part of your homelab") == true)
        #expect(m.writtenFiles.contains("projects/dawn.md") && m.writtenFiles.contains("me.md"))
        #expect(ProposalStore(vault: vault).pending().isEmpty)
    }

    @Test func aboutYouMergesIntoExistingMe() async throws {
        let home = try makeHome()
        let root = home.appendingPathComponent("Binder")
        try Memory.add("Answers in Turkish", to: "me.md", vault: Vault(root: root))
        let m = model(home)
        await m.runLookAround()
        m.profile.name = "Eren"
        try m.createVault()
        let me = try #require(NoteIO.read(Vault(root: root).url("me.md")))
        #expect(me.decisions.count == 1)
        #expect(MeNote.profile(from: me).name == "Eren")
    }

    @Test func moveVaultKeepsFiles() async throws {
        let home = try makeHome()
        let m = model(home)
        await m.runLookAround()
        try m.createVault()
        let obsidian = makeTempDir("obsidian")
        try m.moveVault(into: obsidian)
        #expect(m.vaultRoot.path == obsidian.appendingPathComponent("Binder").path)
        #expect(FileManager.default.fileExists(atPath: m.vaultRoot.appendingPathComponent("projects/dawn.md").path))
        #expect(!FileManager.default.fileExists(atPath: home.appendingPathComponent("Binder").path))
        #expect(VaultLocation.load(home: home)?.path == m.vaultRoot.path)
    }

    @Test func connectInstallsHooksOnce() throws {
        let home = try makeHome()
        let m = model(home)
        try m.connect()
        try m.connect()
        #expect(m.connected)
        let data = try Data(contentsOf: home.appendingPathComponent(".claude/settings.json"))
        let json = try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any])
        let stop = (json["hooks"] as? [String: Any])?["Stop"] as? [Any] ?? []
        #expect(stop.count == 1)
    }

    @Test func consentSaved() async throws {
        let home = try makeHome()
        let m = model(home)
        await m.runLookAround()
        try m.createVault()
        m.setConsent(true)
        #expect(BinderState.load(Vault(root: m.vaultRoot)).summarizerConsent)
        m.setConsent(false)
        #expect(!BinderState.load(Vault(root: m.vaultRoot)).summarizerConsent)
    }

    @Test func resumeAfterQuit() throws {
        let home = try makeHome()
        let first = model(home)
        first.next()
        first.next()
        first.profile.nowAndNext = "Shipping Binder."
        first.next()
        let second = model(home)
        #expect(second.step == .aboutYou)
        #expect(second.profile.nowAndNext == "Shipping Binder.")
    }

    @Test func finishMarksOnboarded() async throws {
        let home = try makeHome()
        let m = model(home)
        await m.runLookAround()
        try m.createVault()
        try m.finish()
        #expect(BinderState.load(Vault(root: m.vaultRoot)).onboarded)
        #expect(OnboardingModel.isOnboarded(home: home))
    }

    @Test func permissionRequestRecordsState() async throws {
        let m = model(try makeHome())
        await m.request(.notifications)
        #expect(m.permissionStatus[.notifications] == .granted)
    }
}

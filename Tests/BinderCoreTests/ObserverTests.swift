import Foundation
import Testing
@testable import BinderCore

/// A temp home with ~/.ssh holding the fixture config, two key pairs and a key no host uses.
private func makeHome() throws -> URL {
    let home = makeTempDir("home")
    let ssh = home.appendingPathComponent(".ssh")
    try FileManager.default.createDirectory(at: ssh.appendingPathComponent("config.d"), withIntermediateDirectories: true)
    try FileManager.default.copyItem(at: fixture("ssh/config"), to: ssh.appendingPathComponent("config"))
    try FileManager.default.copyItem(at: fixture("ssh/config.d/homelab"), to: ssh.appendingPathComponent("config.d/homelab"))
    for key in ["id_ed25519_hetzner", "id_ed25519_github", "id_ed25519_old"] {
        write("-----BEGIN OPENSSH " + "PRIVATE KEY-----\nnot a real key\n", to: ssh.appendingPathComponent(key))
        write("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIExample \(key)@mac\n", to: ssh.appendingPathComponent("\(key).pub"))
    }
    return home
}

private func sshRunner(_ home: URL) -> FakeRunner {
    var results: [String: CommandResult] = [:]
    for (key, fp) in [("id_ed25519_hetzner", "SHA256:hetznerFingerprint"), ("id_ed25519_github", "SHA256:githubFingerprint"),
                      ("id_ed25519_old", "SHA256:oldFingerprint")] {
        let path = home.appendingPathComponent(".ssh/\(key).pub").path
        results["ssh-keygen -lf \(path)"] = ok("256 \(fp) \(key)@mac (ED25519)\n")
    }
    return FakeRunner(results: results)
}

@Suite struct ObserverTests {
    @Test func sshObserverNeverOpensPrivateKeys() throws {
        let home = try makeHome()
        let obs = SSHObserver().observe(ObserverContext(home: home, projectRoots: [], runner: sshRunner(home)))
        let allText = obs.notes.flatMap(\.observed).joined(separator: "\n")
        #expect(!allText.contains("PRIVATE"))
        #expect(!allText.contains("not a real key"))
        #expect(obs.filesRead == 2 + 3) // config, config.d/homelab, three .pub files
        let ssh = try #require(obs.notes.first { $0.path == "access/ssh.md" })
        #expect(ssh.title == "SSH")
        #expect(ssh.observed.contains("- hetzner-1: eren@116.203.10.20:2222 · key id_ed25519_hetzner"))
        #expect(ssh.observed.contains("- key id_ed25519_hetzner: ED25519 SHA256:hetznerFingerprint · opens hetzner-1"))
        let server = try #require(obs.notes.first { $0.path == "servers/hetzner-1.md" })
        #expect(server.observed == ["- ssh: hetzner-1", "- address: 116.203.10.20", "- user: eren", "- port: 2222", "- key: id_ed25519_hetzner"])
        #expect(!obs.notes.contains { $0.path.hasPrefix("servers/github") || $0.path == "servers/gh-work.md" })
    }

    @Test func keyWithNoHostSaysOpensNothing() throws {
        let home = try makeHome()
        let obs = SSHObserver().observe(ObserverContext(home: home, projectRoots: [], runner: sshRunner(home)))
        let ssh = try #require(obs.notes.first { $0.path == "access/ssh.md" })
        #expect(ssh.observed.contains("- key id_ed25519_old: ED25519 SHA256:oldFingerprint · opens nothing"))
        #expect(ssh.observed.contains("- key id_ed25519_github: ED25519 SHA256:githubFingerprint · opens github.com, gh-work, github-alt"))
    }

    @Test func sshObserverWithoutSSHFolderObservesNothing() {
        let obs = SSHObserver().observe(ObserverContext(home: makeTempDir("empty-home"), projectRoots: [], runner: FakeRunner()))
        #expect(obs.notes.isEmpty && obs.filesRead == 0)
    }

    @Test func accountsObserverSkipsMissingTools() throws {
        let runner = FakeRunner(results: [
            "git config --global user.name": ok("Eren\n"),
            "git config --global user.email": ok("eren@example.com\n"),
            "wrangler whoami": ok("You are logged in with an OAuth Token, associated with the email eren@example.com.\n")
        ])
        let obs = AccountsObserver().observe(ObserverContext(home: makeTempDir(), projectRoots: [], runner: runner))
        let note = try #require(obs.notes.first)
        #expect(note.path == "access/accounts.md" && note.title == "Accounts")
        #expect(note.observed == ["- git: Eren <eren@example.com>", "- cloudflare: signed in"])
    }

    @Test func accountsObserverParsesGhLogin() throws {
        let status = """
        github.com
          ✓ Logged in to github.com account Kungie (keyring)
          - Active account: true
          - Git operations protocol: https
        """
        let runner = FakeRunner(results: [
            "gh auth status": ok(status),
            "vercel whoami": CommandResult(status: 1, stdout: "", stderr: "Error: not logged in", timedOut: false)
        ])
        let obs = AccountsObserver().observe(ObserverContext(home: makeTempDir(), projectRoots: [], runner: runner))
        #expect(obs.notes.first?.observed == ["- github: Kungie", "- vercel: installed, not signed in"])
    }

    @Test func macObserverListsAgents() throws {
        let home = makeTempDir("home")
        try FileManager.default.createDirectory(at: home.appendingPathComponent(".claude"), withIntermediateDirectories: true)
        let runner = FakeRunner(results: [
            "sw_vers -productVersion": ok("27.0.1\n"),
            "uname -m": ok("arm64\n"),
            "brew list --formula -1": ok("gh\ngit\nwget\n")
        ])
        let obs = MacObserver(shell: "/bin/zsh").observe(ObserverContext(home: home, projectRoots: [], runner: runner))
        let note = try #require(obs.notes.first)
        #expect(note.path == "machines/this-mac.md" && note.title == "This Mac")
        #expect(note.observed == ["- macOS: 27.0.1", "- chip: arm64", "- shell: zsh", "- homebrew: 3 formulae",
                                  "- agents: Claude Code found, Codex not found"])
    }
}

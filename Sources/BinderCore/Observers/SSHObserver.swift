import Foundation

/// Which host opens with which key. Reads `~/.ssh/config` and the *names* of `*.pub` files,
/// and asks `ssh-keygen -lf` for public-key fingerprints. Never opens a private key.
public struct SSHObserver: Observer {
    public let name = "ssh"

    public init() {}

    static let codeHosts = ["github.com", "gitlab.com", "bitbucket.org", "ssh.dev.azure.com"]

    public func observe(_ ctx: ObserverContext) -> Observation {
        let sshDir = ctx.home.appendingPathComponent(".ssh")
        let loaded = SSHConfig.load(sshDir.appendingPathComponent("config"))
        let pubs = ((try? FileManager.default.contentsOfDirectory(atPath: sshDir.path)) ?? [])
            .filter { $0.hasSuffix(".pub") }.sorted()
        guard !loaded.hosts.isEmpty || !pubs.isEmpty else { return Observation() }

        var lines = loaded.hosts.map(hostLine)
        for pub in pubs {
            let keyName = String(pub.dropLast(4))
            let opens = loaded.hosts.filter { $0.keyNames.contains(keyName) }.map(\.alias)
            lines.append("- key \(keyName): \(fingerprint(sshDir.appendingPathComponent(pub), ctx)) · opens \(opens.isEmpty ? "nothing" : opens.joined(separator: ", "))")
        }

        var notes = [NoteUpdate(path: "access/ssh.md", title: "SSH", observed: lines)]
        for host in loaded.hosts where !isCodeHost(host) {
            var observed = ["- ssh: \(host.alias)"]
            if let hostName = host.hostName { observed.append("- address: \(hostName)") }
            if let user = host.user { observed.append("- user: \(user)") }
            if let port = host.port, port != 22 { observed.append("- port: \(port)") }
            if let key = host.keyNames.first { observed.append("- key: \(key)") }
            notes.append(NoteUpdate(path: "servers/\(host.alias).md", title: host.alias, observed: observed))
        }
        return Observation(notes: notes, filesRead: loaded.filesRead + pubs.count)
    }

    func hostLine(_ host: SSHHost) -> String {
        let target = (host.user.map { "\($0)@" } ?? "") + (host.hostName ?? host.alias) + (host.port.map { ":\($0)" } ?? "")
        let key = host.keyNames.first.map { "key \($0)" } ?? "default key"
        return "- \(host.alias): \(target) · \(key)"
    }

    func isCodeHost(_ host: SSHHost) -> Bool {
        Self.codeHosts.contains { host.alias == $0 || host.hostName == $0 }
    }

    /// `256 SHA256:abc… comment (ED25519)` → `ED25519 SHA256:abc…`.
    func fingerprint(_ pub: URL, _ ctx: ObserverContext) -> String {
        guard let result = ctx.runner.run("ssh-keygen", ["-lf", pub.path], input: nil, timeout: 3), result.status == 0
        else { return "fingerprint unknown" }
        let parts = result.stdout.trimmingCharacters(in: .whitespacesAndNewlines).split(separator: " ")
        guard parts.count >= 2 else { return "fingerprint unknown" }
        let type = parts.last.map { $0.trimmingCharacters(in: CharacterSet(charactersIn: "()")) } ?? ""
        return "\(type) \(parts[1])"
    }
}

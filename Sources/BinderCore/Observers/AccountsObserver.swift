import Foundation

/// Which developer accounts this Mac is signed in to. Each CLI runs only if installed, with a 3 s limit.
public struct AccountsObserver: Observer {
    public let name = "accounts"

    public init() {}

    public func observe(_ ctx: ObserverContext) -> Observation {
        let run = { (tool: String, args: [String]) in ctx.runner.run(tool, args, input: nil, timeout: 3) }
        var lines: [String] = []

        let gitName = run("git", ["config", "--global", "user.name"]).flatMap(firstLine)
        let gitEmail = run("git", ["config", "--global", "user.email"]).flatMap(firstLine)
        if let gitName { lines.append("- git: \(gitName)" + (gitEmail.map { " <\($0)>" } ?? "")) }

        if let gh = run("gh", ["auth", "status"]) {
            let text = gh.stdout + "\n" + gh.stderr
            let login = text.firstMatch(of: #/Logged in to github\.com (?:account|as) (\S+)/#).map { String($0.1) }
            lines.append("- github: \(login ?? "installed, not signed in")")
        }
        if let vercel = run("vercel", ["whoami"]) {
            lines.append("- vercel: \(vercel.status == 0 ? (lastLine(vercel) ?? "signed in") : "installed, not signed in")")
        }
        if let netlify = run("netlify", ["status"]) {
            lines.append("- netlify: \(signedIn(netlify.status == 0 && !netlify.stdout.contains("Not logged in")))")
        }
        if let fly = run("flyctl", ["auth", "whoami"]) ?? run("fly", ["auth", "whoami"]) {
            lines.append("- fly.io: \(signedIn(fly.status == 0))")
        }
        if let wrangler = run("wrangler", ["whoami"]) {
            lines.append("- cloudflare: \(signedIn(wrangler.stdout.contains("You are logged in")))")
        }
        if let hcloud = run("hcloud", ["context", "list"]) {
            let contexts = hcloud.stdout.split(separator: "\n").dropFirst().count
            lines.append("- hetzner cloud: \(hcloud.status == 0 && contexts > 0 ? "\(contexts) context\(contexts == 1 ? "" : "s")" : "installed, not signed in")")
        }
        if let supabase = run("supabase", ["projects", "list"]) {
            lines.append("- supabase: \(signedIn(supabase.status == 0))")
        }

        guard !lines.isEmpty else { return Observation() }
        return Observation(notes: [NoteUpdate(path: "access/accounts.md", title: "Accounts", observed: lines)])
    }

    private func signedIn(_ yes: Bool) -> String { yes ? "signed in" : "installed, not signed in" }

    private func firstLine(_ r: CommandResult) -> String? {
        guard r.status == 0 else { return nil }
        let line = r.stdout.split(separator: "\n").first.map { $0.trimmingCharacters(in: .whitespaces) }
        return line?.isEmpty == false ? line : nil
    }

    private func lastLine(_ r: CommandResult) -> String? {
        r.stdout.split(separator: "\n").last.map { $0.trimmingCharacters(in: .whitespaces) }.flatMap { $0.isEmpty ? nil : $0 }
    }
}

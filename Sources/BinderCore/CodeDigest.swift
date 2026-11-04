import Foundation

/// What the model reads to summarize a repo: its file list, README and build and deploy files, at most 20,000
/// characters. Private files (`.env`, keys) are never read, and nothing the secret filter flags goes in.
public enum CodeDigest {
    public static let maxCharacters = 20_000
    static let fileCap = 300
    static let readmeCap = 6_000
    static let manifestCap = 3_000
    static let manifests = ["Package.swift", "package.json", "Cargo.toml", "pyproject.toml", "go.mod", "Gemfile", "Makefile",
                            "Dockerfile", "docker-compose.yml", "compose.yaml", "fly.toml", "wrangler.toml", "vercel.json", "netlify.toml"]

    /// Files that may hold secrets: they're left out of the list and never opened.
    static func isPrivate(_ path: String) -> Bool {
        let name = (path as NSString).lastPathComponent.lowercased()
        let ext = (name as NSString).pathExtension
        return name.hasPrefix(".env") || name.hasPrefix("id_") || ["pem", "key", "p12", "pfx", "keystore"].contains(ext)
            || name.contains("credential") || name.contains("secret")
    }

    public static func build(repo: URL, runner: CommandRunner) -> String {
        let files = (runner.run("git", ["-C", repo.path, "ls-files"], input: nil, timeout: 10)?.stdout ?? "")
            .split(separator: "\n").map(String.init).filter { !isPrivate($0) }
        var blocks = ["FILES:\n" + listing(files)]
        func read(_ relative: String, cap: Int) -> String? {
            guard !isPrivate(relative), let text = try? String(contentsOf: repo.appendingPathComponent(relative), encoding: .utf8),
                  !SecretFilter.containsSecret(text) else { return nil }
            return String(text.prefix(cap))
        }
        if let readme = ["README.md", "README", "readme.md", "Readme.md"].lazy.compactMap({ read($0, cap: readmeCap) }).first {
            blocks.append("README:\n" + readme)
        }
        let workflows = files.filter { $0.hasPrefix(".github/workflows/") && ($0.hasSuffix(".yml") || $0.hasSuffix(".yaml")) }
        for path in manifests + workflows {
            if let text = read(path, cap: manifestCap) { blocks.append("\(path):\n" + text) }
        }
        return String(SecretFilter.redact(blocks.joined(separator: "\n\n")).prefix(maxCharacters))
    }

    /// The paths, or past 300 of them, each folder with its file count and the top-level files.
    static func listing(_ files: [String]) -> String {
        guard files.count > fileCap else { return files.joined(separator: "\n") }
        var folders: [String: Int] = [:], top: [String] = []
        for file in files {
            let parent = (file as NSString).deletingLastPathComponent
            if parent.isEmpty { top.append(file) } else { folders[parent, default: 0] += 1 }
        }
        let lines = top.sorted() + folders.sorted { $0.key < $1.key }.map { "\($0.key)/ (\($0.value) files)" }
        return lines.prefix(fileCap).joined(separator: "\n")
    }

    /// The project's repo under the project folders, the same way the projects observer finds it.
    public static func folder(for project: String, roots: [URL]) -> URL? {
        roots.flatMap { ProjectsObserver.repos(under: $0) }.first { $0.lastPathComponent == project }
    }
}

/// Summarize code, from the main window or `binder index`: the digest, one model call, the lines into the inbox.
public enum CodeSummary {
    /// How many catches were added, or nil when the model didn't answer.
    public static func run(project: String, repo: URL, vault: Vault, runner: CommandRunner, summarizer: ClaudeCLISummarizer) -> Int? {
        let digest = CodeDigest.build(repo: repo, runner: runner)
        let known = ClaudeCLISummarizer.Known.load(vault: vault, project: project)
        guard let lines = summarizer.codeLines(digest: digest, project: project, known: known) else { return nil }
        let store = ProposalStore(vault: vault)
        return lines.filter { (try? store.add($0)) == true }.count
    }
}

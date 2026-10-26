import Foundation

/// Git repos under the project roots: where they live, what they're written in, how they ship, which domains they serve.
public struct ProjectsObserver: Observer {
    public let name = "projects"

    public init() {}

    static let skipped: Set<String> = ["node_modules", ".build", "Pods", "DerivedData", "build", "dist", "vendor"]

    static let languages: [String: String] = [
        "swift": "Swift", "py": "Python", "js": "JavaScript", "jsx": "JavaScript", "mjs": "JavaScript", "ts": "TypeScript",
        "tsx": "TypeScript", "html": "HTML", "htm": "HTML", "css": "CSS", "scss": "SCSS", "go": "Go", "rs": "Rust",
        "rb": "Ruby", "php": "PHP", "java": "Java", "kt": "Kotlin", "c": "C", "h": "C", "cpp": "C++", "cc": "C++",
        "hpp": "C++", "m": "Objective-C", "mm": "Objective-C", "sh": "Shell", "lua": "Lua", "ex": "Elixir", "exs": "Elixir",
        "dart": "Dart", "vue": "Vue", "svelte": "Svelte", "astro": "Astro", "zig": "Zig", "cs": "C#"
    ]

    /// File → deploy target, in the order they're listed in the note.
    static let deployFiles: [(file: String, label: String)] = [
        ("fly.toml", "Fly.io"), ("vercel.json", "Vercel"), ("wrangler.toml", "Cloudflare (wrangler)"),
        ("wrangler.jsonc", "Cloudflare (wrangler)"), ("wrangler.json", "Cloudflare (wrangler)"), ("netlify.toml", "Netlify"),
        ("Dockerfile", "Docker"), ("docker-compose.yml", "Docker Compose"), ("docker-compose.yaml", "Docker Compose"),
        ("compose.yaml", "Docker Compose"), ("compose.yml", "Docker Compose"), (".github/workflows", "GitHub Actions")
    ]

    static let providers: [(needle: String, name: String)] = [
        ("pages.dev", "Cloudflare Pages"), ("workers.dev", "Cloudflare Workers"), ("vercel-dns", "Vercel"),
        ("vercel.app", "Vercel"), ("netlify", "Netlify"), ("github.io", "GitHub Pages"), ("fly.dev", "Fly.io")
    ]

    public func observe(_ ctx: ObserverContext) -> Observation {
        var obs = Observation()
        var usedNames = Set<String>()
        for repo in ctx.projectRoots.flatMap({ Self.repos(under: $0) }) {
            var name = repo.lastPathComponent
            if usedNames.contains(name) { name = "\(repo.deletingLastPathComponent().lastPathComponent)-\(name)" }
            usedNames.insert(name)
            let (lines, files) = describe(repo, ctx)
            obs.notes.append(NoteUpdate(path: "projects/\(name).md", title: name, observed: lines))
            obs.filesRead += files
        }
        return obs
    }

    /// Folders holding `.git`, up to three levels below `root`. A repo's own subfolders aren't searched.
    static func repos(under root: URL, depth: Int = 0) -> [URL] {
        let fm = FileManager.default
        if fm.fileExists(atPath: root.appendingPathComponent(".git").path) { return [root] }
        guard depth < 3, let children = try? fm.contentsOfDirectory(at: root, includingPropertiesForKeys: [.isDirectoryKey]) else { return [] }
        return children.sorted { $0.lastPathComponent < $1.lastPathComponent }.flatMap { child -> [URL] in
            let name = child.lastPathComponent
            guard !name.hasPrefix("."), !skipped.contains(name),
                  (try? child.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true else { return [] }
            return repos(under: child, depth: depth + 1)
        }
    }

    func describe(_ root: URL, _ ctx: ObserverContext) -> (lines: [String], filesRead: Int) {
        let repo = GitRepo(root: root, remote: GitRepo.originURL(gitDir: GitRepo.gitDirectory(root.appendingPathComponent(".git"))))
        var files = 2 // .git/config and .git/HEAD
        var lines = ["- repo: \(repo.remote.map(GitRepo.normalize(remote:)) ?? abbreviate(root, home: ctx.home))"]
        if let branch = Self.branch(root) { lines.append("- branch: \(branch)") }
        let langs = Self.topLanguages(root)
        if !langs.isEmpty { lines.append("- languages: \(langs.joined(separator: ", "))") }

        let fm = FileManager.default
        var deploys: [String] = []
        for (file, label) in Self.deployFiles where fm.fileExists(atPath: root.appendingPathComponent(file).path) && !deploys.contains(label) {
            deploys.append(label)
        }
        if !deploys.isEmpty { lines.append("- deploy: \(deploys.joined(separator: ", "))") }

        let (domains, read) = Self.domains(root)
        files += read
        if !domains.isEmpty {
            let described = domains.map { domain in provider(domain, ctx).map { "\(domain) (\($0))" } ?? domain }
            lines.append("- domains: \(described.joined(separator: ", "))")
        }
        return (lines, files)
    }

    static func branch(_ root: URL) -> String? {
        let gitDir = GitRepo.gitDirectory(root.appendingPathComponent(".git"))
        guard let head = try? String(contentsOf: gitDir.appendingPathComponent("HEAD"), encoding: .utf8)
            .trimmingCharacters(in: .whitespacesAndNewlines) else { return nil }
        if head.hasPrefix("ref: refs/heads/") { return String(head.dropFirst("ref: refs/heads/".count)) }
        return head.isEmpty ? nil : "detached at \(head.prefix(7))"
    }

    /// The three languages with the most files; ties go alphabetically. Stops counting after 5 000 files.
    static func topLanguages(_ root: URL) -> [String] {
        var counts: [String: Int] = [:]
        var seen = 0
        let enumerator = FileManager.default.enumerator(at: root, includingPropertiesForKeys: [.isDirectoryKey],
                                                        options: [.skipsHiddenFiles, .skipsPackageDescendants])
        while let url = enumerator?.nextObject() as? URL, seen < 5000 {
            if skipped.contains(url.lastPathComponent) { enumerator?.skipDescendants(); continue }
            seen += 1
            if let language = languages[url.pathExtension.lowercased()] { counts[language, default: 0] += 1 }
        }
        return counts.sorted { $0.value != $1.value ? $0.value > $1.value : $0.key < $1.key }.prefix(3).map(\.key)
    }

    /// Domains named in `CNAME`, `wrangler.toml` routes and `vercel.json` aliases.
    static func domains(_ root: URL) -> (domains: [String], filesRead: Int) {
        var found: [String] = []
        var read = 0
        func add(_ raw: String) {
            var d = raw.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            if let slash = d.firstIndex(of: "/") { d = String(d[..<slash]) }
            if d.hasPrefix("*.") { d.removeFirst(2) }
            if d.contains("."), !d.contains(" "), !found.contains(d) { found.append(d) }
        }
        if let cname = try? String(contentsOf: root.appendingPathComponent("CNAME"), encoding: .utf8) {
            read += 1
            cname.split(separator: "\n").first.map { add(String($0)) }
        }
        if let toml = try? String(contentsOf: root.appendingPathComponent("wrangler.toml"), encoding: .utf8) {
            read += 1
            for match in toml.matches(of: #/(?:route|pattern)\s*=\s*"([^"]+)"/#) { add(String(match.1)) }
        }
        if let data = try? Data(contentsOf: root.appendingPathComponent("vercel.json")),
           let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            read += 1
            if let one = json["alias"] as? String { add(one) }
            (json["alias"] as? [String])?.forEach(add)
        }
        return (found, read)
    }

    func provider(_ domain: String, _ ctx: ObserverContext) -> String? {
        guard let dig = ctx.runner.run("dig", ["+short", "CNAME", domain], input: nil, timeout: 2), dig.status == 0 else { return nil }
        let answer = dig.stdout.lowercased()
        return Self.providers.first { answer.contains($0.needle) }?.name
    }

    func abbreviate(_ url: URL, home: URL) -> String {
        let path = url.standardizedFileURL.path, homePath = home.standardizedFileURL.path
        return path.hasPrefix(homePath + "/") ? "~" + path.dropFirst(homePath.count) : path
    }
}

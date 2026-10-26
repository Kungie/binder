import Foundation
import Testing
@testable import BinderCore

/// A fake repo: `.git/config` with an origin, `.git/HEAD`, and whatever files the test lists.
@discardableResult
private func makeRepo(_ root: URL, _ name: String, remote: String?, branch: String = "main", files: [String: String] = [:]) -> URL {
    let repo = root.appendingPathComponent(name)
    var config = "[core]\n\tbare = false\n"
    if let remote { config += "[remote \"origin\"]\n\turl = \(remote)\n" }
    write(config, to: repo.appendingPathComponent(".git/config"))
    write("ref: refs/heads/\(branch)\n", to: repo.appendingPathComponent(".git/HEAD"))
    for (path, text) in files { write(text, to: repo.appendingPathComponent(path)) }
    return repo
}

@Suite struct ProjectsObserverTests {
    @Test func findsReposAndSkipsNodeModules() throws {
        let root = makeTempDir("projects")
        makeRepo(root, "dawn", remote: "git@github.com:Kungie/dawn.git", files: ["Sources/Dawn/App.swift": "//"])
        makeRepo(root.appendingPathComponent("clients"), "site", remote: nil, files: ["index.html": "<p>"])
        makeRepo(root.appendingPathComponent("site-tools/node_modules"), "leftpad", remote: "https://github.com/x/leftpad")
        makeRepo(root.appendingPathComponent(".hidden"), "secret-repo", remote: nil)
        let obs = ProjectsObserver().observe(ObserverContext(home: root, projectRoots: [root], runner: FakeRunner()))
        #expect(Set(obs.notes.map(\.path)) == ["projects/dawn.md", "projects/site.md"])
        let dawn = try #require(obs.notes.first { $0.title == "dawn" })
        #expect(dawn.observed.contains("- repo: github.com/kungie/dawn"))
        #expect(dawn.observed.contains("- branch: main"))
        #expect(obs.filesRead > 0)
    }

    @Test func detectsDeployFilesAndLanguages() throws {
        let root = makeTempDir("projects")
        makeRepo(root, "binder", remote: "https://github.com/Kungie/binder", branch: "core-cli", files: [
            "Package.swift": "", "Sources/A.swift": "", "Sources/B.swift": "", "Sources/C.swift": "",
            "scripts/build.py": "", "site/index.html": "", "site/style.css": "", "site/more.css": "",
            "fly.toml": "app = 'binder'", "Dockerfile": "FROM swift", ".github/workflows/ci.yml": "on: push"
        ])
        let obs = ProjectsObserver().observe(ObserverContext(home: root, projectRoots: [root], runner: FakeRunner()))
        let note = try #require(obs.notes.first)
        #expect(note.observed.contains("- branch: core-cli"))
        #expect(note.observed.contains("- languages: Swift, CSS, HTML"))
        #expect(note.observed.contains("- deploy: Fly.io, Docker, GitHub Actions"))
    }

    @Test func readsDomainsFromCNAMEAndWrangler() throws {
        let root = makeTempDir("projects")
        makeRepo(root, "site", remote: nil, files: [
            "CNAME": "dawnapp.example\n",
            "wrangler.toml": "name = \"site\"\nroutes = [\n  { pattern = \"shop.example.org/*\", zone_name = \"example.org\" }\n]\n"
        ])
        let runner = FakeRunner(results: ["dig +short CNAME dawnapp.example": ok("site-9x2.pages.dev.\n")])
        let obs = ProjectsObserver().observe(ObserverContext(home: root, projectRoots: [root], runner: runner))
        let note = try #require(obs.notes.first)
        #expect(note.observed.contains("- deploy: Cloudflare (wrangler)"))
        #expect(note.observed.contains("- domains: dawnapp.example (Cloudflare Pages), shop.example.org"))
    }
}

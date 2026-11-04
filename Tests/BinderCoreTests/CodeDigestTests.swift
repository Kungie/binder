import Foundation
import Testing
@testable import BinderCore

@Suite struct CodeDigestTests {
    func repo(files: [String]) -> (URL, FakeRunner) {
        let root = makeTempDir("digest").appendingPathComponent("dawn")
        try? FileManager.default.createDirectory(at: root.appendingPathComponent(".git"), withIntermediateDirectories: true)
        let runner = FakeRunner(results: ["git -C \(root.path) ls-files": ok(files.joined(separator: "\n") + "\n")])
        return (root, runner)
    }

    @Test func fileListFoldsPastTheCap() {
        let files = (0..<50_000).map { "Sources/Module\($0 % 7)/File\($0).swift" } + ["README.md"]
        let (root, runner) = repo(files: files)
        let digest = CodeDigest.build(repo: root, runner: runner)
        #expect(digest.count <= CodeDigest.maxCharacters)
        #expect(digest.contains("Sources/Module3/ (7143 files)"))
        #expect(!digest.contains("File12345.swift"))
    }

    @Test func manifestsCut() {
        let (root, runner) = repo(files: ["Package.swift", "README.md"])
        write(String(repeating: "// line\n", count: 2_000), to: root.appendingPathComponent("Package.swift"))
        write("# dawn\n\nA menu bar app.\n", to: root.appendingPathComponent("README.md"))
        let digest = CodeDigest.build(repo: root, runner: runner)
        let block = digest.components(separatedBy: "Package.swift:\n").last ?? ""
        #expect(block.count <= 3_100)
        #expect(digest.contains("README:\n# dawn"))
    }

    @Test func secretsNeverRead() {
        let (root, runner) = repo(files: [".env", "deploy/id_ed25519", "certs/site.pem", "fly.toml", "README.md"])
        write("TOKEN=abc123", to: root.appendingPathComponent(".env"))
        write("app = \"dawn\"\napi_token = \"ghp_abcdefghijklmnopqrstuvwxyz0123456789\"\n", to: root.appendingPathComponent("fly.toml"))
        let digest = CodeDigest.build(repo: root, runner: runner)
        #expect(!digest.contains("TOKEN=abc123"))
        #expect(!digest.contains("ghp_abcdefghijklmnopqrstuvwxyz0123456789"))
        #expect(!digest.contains("FILES:\n.env"))
        #expect(!digest.contains("id_ed25519") && !digest.contains("site.pem"))
    }

    @Test func linesDefaultToArchitecture() throws {
        let reply = #"{"new":[{"kind":"fact","scope":"project","text":"dawn is a SwiftUI menu bar app with a Claude Code hook."},{"kind":"fact","scope":"project","text":"Releases are built with make release.","topic":"release"}]}"#
        let lines = ClaudeCLISummarizer(runner: SpyRunner(reply: reply), session: nil).codeLines(digest: "FILES:\nx", project: "dawn")
        #expect(lines?.map(\.target) == ["projects/dawn/architecture.md", "projects/dawn/release.md"])
        #expect(lines?.first?.evidence == "summarized from the code")
    }

    @Test func codePromptAsksForEnglish() {
        #expect(ClaudeCLISummarizer.codeSystem.contains("English"))
    }

    @Test func noFolderNoRun() {
        #expect(CodeDigest.folder(for: "nowhere", roots: [makeTempDir("empty-roots")]) == nil)
        let roots = makeTempDir("roots")
        try? FileManager.default.createDirectory(at: roots.appendingPathComponent("dawn/.git"), withIntermediateDirectories: true)
        #expect(CodeDigest.folder(for: "dawn", roots: [roots])?.lastPathComponent == "dawn")
    }
}

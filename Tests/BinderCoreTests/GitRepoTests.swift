import Foundation
import Testing
@testable import BinderCore

@Suite struct GitRepoTests {
    @Test func normalizeRemoteForms() {
        let expected = "github.com/kungie/dawn"
        #expect(GitRepo.normalize(remote: "git@github.com:Kungie/dawn.git") == expected)
        #expect(GitRepo.normalize(remote: "https://github.com/Kungie/Dawn") == expected)
        #expect(GitRepo.normalize(remote: "https://github.com/Kungie/dawn.git/") == expected)
        #expect(GitRepo.normalize(remote: "ssh://git@github.com/Kungie/dawn.git") == expected)
        #expect(GitRepo.normalize(remote: "https://user:pass@github.com/Kungie/dawn") == expected)
    }

    @Test func locateFindsRepoFromSubfolder() throws {
        let root = makeTempDir().appendingPathComponent("dawn")
        write("""
        [core]
        \trepositoryformatversion = 0
        [remote "origin"]
        \turl = git@github.com:Kungie/dawn.git
        \tfetch = +refs/heads/*:refs/remotes/origin/*
        """, to: root.appendingPathComponent(".git/config"))
        let sub = root.appendingPathComponent("Sources/Dawn")
        try FileManager.default.createDirectory(at: sub, withIntermediateDirectories: true)
        let repo = try #require(GitRepo.locate(from: sub))
        #expect(repo.root.standardizedFileURL.path == root.standardizedFileURL.path)
        #expect(repo.name == "dawn")
        #expect(repo.remote == "git@github.com:Kungie/dawn.git")
        #expect(GitRepo.locate(from: makeTempDir()) == nil)
    }

    @Test func projectIDIsStable12Hex() {
        let a = GitRepo(root: URL(fileURLWithPath: "/a/dawn"), remote: "git@github.com:Kungie/dawn.git")
        let b = GitRepo(root: URL(fileURLWithPath: "/elsewhere/dawn-clone"), remote: "https://github.com/Kungie/dawn")
        #expect(a.projectID == b.projectID)
        #expect(a.projectID.count == 12)
        #expect(a.projectID.allSatisfy { "0123456789abcdef".contains($0) })
        let local = GitRepo(root: URL(fileURLWithPath: "/a/scratch"), remote: nil)
        #expect(local.projectID != a.projectID && local.projectID.count == 12)
    }
}

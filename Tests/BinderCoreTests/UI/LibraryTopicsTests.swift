import Foundation
import Testing
@testable import BinderCore
@testable import BinderUI

@MainActor
@Suite struct LibraryTopicsTests {
    func model() throws -> (LibraryModel, Vault) {
        let vault = try fixtureVault()
        try Memory.add("Runs on Fly.io", to: Topic.path(project: "dawn", topic: "hosting"), vault: vault)
        try Memory.add("REST, versioned", to: Topic.path(project: "dawn", topic: "api"), vault: vault)
        try Memory.add("On Cloudflare", to: "hosting/cloudflare.md", vault: vault)
        return (LibraryModel(vault: vault, home: makeTempDir("home")), vault)
    }

    @Test func projectDetailListsTopics() throws {
        let (m, _) = try model()
        let detail = m.note(NoteRef(title: "dawn", path: "projects/dawn.md"))
        #expect(detail.topics.map(\.path) == ["projects/dawn/api.md", "projects/dawn/hosting.md"])
    }

    @Test func sidebarProjectsIgnoreTopicFolders() throws {
        let (m, _) = try model()
        let projects = m.sections.first { $0.kind == .projects }?.items.map(\.path) ?? []
        #expect(!projects.contains { $0.hasPrefix("projects/dawn/") })
        #expect(m.sections.first { $0.kind == .hosting }?.items.map(\.path) == ["hosting/cloudflare.md"])
    }
}

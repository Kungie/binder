import Foundation
import Testing
@testable import BinderCore

@Suite struct CardTopicsTests {
    func day(_ offset: Int) -> Date { ISO8601DateFormatter().date(from: "2026-06-01T10:00:00Z")!.addingTimeInterval(Double(offset) * 86_400) }

    @Test func cardGroupsTopics() throws {
        let vault = try fixtureVault()
        try Memory.add("Runs on Fly.io", to: Topic.path(project: "dawn", topic: "hosting"), vault: vault,
                       now: ISO8601DateFormatter().date(from: "2026-11-04T10:00:00Z")!)
        let card = CardBuilder.build(vault: vault, repo: dawnRepo)
        #expect(card.contains("HOSTING:\n- 2026-11-04 · Runs on Fly.io"))
        let hosting = try #require(card.range(of: "HOSTING:")), project = try #require(card.range(of: "PROJECT dawn:"))
        #expect(project.lowerBound < hosting.lowerBound)
    }

    @Test func cardTrimsOldestTopicLineFirst() throws {
        let vault = try fixtureVault()
        let newestOverview = CardBuilder.decisionText(try #require(NoteIO.read(vault.url("projects/dawn.md"))?.decisions.last))
        for i in 0..<200 {
            try Memory.add("Topic fact number \(i) with a few more words to take up room", to: Topic.path(project: "dawn", topic: i % 2 == 0 ? "hosting" : "api"),
                           vault: vault, now: day(i))
        }
        let card = CardBuilder.build(vault: vault, repo: dawnRepo)
        #expect(card.count <= CardBuilder.budget)
        #expect(card.contains("Topic fact number 199 "))
        #expect(!card.contains("Topic fact number 0 "))
        #expect(card.contains(newestOverview))
    }

    @Test func cardNeverShowsDetails() throws {
        let vault = try fixtureVault()
        try NoteIO.update(vault.url("projects/dawn/hosting.md"), title: "hosting") {
            $0.appendingDecision("2026-11-04 · Runs on Fly.io · kept from the chat", detail: "A secret-free but long explanation")
        }
        #expect(!CardBuilder.build(vault: vault, repo: dawnRepo).contains("long explanation"))
    }

    @Test func topicChangeInvalidatesCache() throws {
        let vault = try fixtureVault()
        try Memory.add("Runs on Fly.io", to: Topic.path(project: "dawn", topic: "hosting"), vault: vault)
        let cache = CardCache(vault: vault)
        try cache.store("CARD", for: dawnRepo)
        try FileManager.default.setAttributes([.modificationDate: Date().addingTimeInterval(5)], ofItemAtPath: vault.url("projects/dawn/hosting.md").path)
        #expect(cache.cached(for: dawnRepo) == nil)
    }
}

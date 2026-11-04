import Foundation
import Testing
@testable import BinderCore

@Suite struct KeepTargetsTests {
    let vault = Vault(root: makeTempDir("keep"))
    var store: ProposalStore { ProposalStore(vault: vault) }

    func chat(_ text: String, target: String = "projects/dawn.md", detail: String? = nil, also: [String] = [], project: String = "dawn") -> Proposal {
        Proposal.make(kind: .fact, source: .chat, text: text, evidence: "said in session s", target: target,
                      project: project, session: "s", detail: detail, also: also)
    }

    func keep(_ p: Proposal, edited: String? = nil) throws {
        #expect(try store.add(p))
        try store.keep(p.id, editedText: edited, now: ISO8601DateFormatter().date(from: "2026-11-04T10:00:00Z")!)
    }

    func note(_ path: String) -> Note? { NoteIO.read(vault.url(path)) }

    @Test func keepWritesDetailUnderLine() throws {
        try keep(chat("The API runs on hetzner-1.", detail: "Moved off Fly.io."))
        #expect(note("projects/dawn.md")?.decisionItems == [DecisionItem(line: "2026-11-04 · The API runs on hetzner-1. · kept from the chat", details: ["Moved off Fly.io."])])
    }

    @Test func keepIntoTopicLinksOverviewOnce() throws {
        try keep(chat("Runs on Fly.io.", target: "projects/dawn/hosting.md"))
        try keep(chat("Uses a CDN.", target: "projects/dawn/hosting.md"))
        #expect(note("projects/dawn/hosting.md")?.decisions.count == 2)
        let links = note("projects/dawn.md")?.decisions.filter { $0.contains("[[dawn/hosting]]") } ?? []
        #expect(links.count == 1)
    }

    @Test func keepWritesEveryExtraNote() throws {
        try keep(chat("The API runs in Docker on hetzner-1.", also: ["servers/hetzner-1.md", "hosting/cloudflare.md"]))
        for path in ["servers/hetzner-1.md", "hosting/cloudflare.md"] {
            #expect(note(path)?.decisions == ["2026-11-04 · The API runs in Docker on hetzner-1. · for [[dawn]] · kept from the chat"], "\(path)")
        }
        #expect(note("projects/dawn.md")?.decisions == ["2026-11-04 · The API runs in Docker on hetzner-1. · kept from the chat"])
    }

    @Test func editedTextGoesEverywhere() throws {
        try keep(chat("Old.", also: ["servers/hetzner-1.md"]), edited: "New.")
        #expect(note("projects/dawn.md")?.decisions.first?.contains("New.") == true)
        #expect(note("servers/hetzner-1.md")?.decisions.first?.contains("New.") == true)
    }

    @Test func addRefusesOnlyWhenAllTargetsHaveIt() throws {
        try Memory.add("Runs in Docker.", to: "projects/dawn.md", vault: vault)
        #expect(try store.add(chat("Runs in Docker.", also: ["servers/hetzner-1.md"])))
        #expect(try !store.add(chat("Runs in Docker.")))
    }

    @Test func deployCommandAlsoFilesUnderProvider() {
        let p = CommandRules.proposals(for: "npx wrangler pages deploy dist", project: "dawn", session: "s")
        #expect(p.first?.also == ["hosting/cloudflare.md"])
        #expect(CommandRules.proposals(for: "flyctl deploy", project: "dawn", session: "s").first?.also == ["hosting/fly.md"])
        #expect(CommandRules.proposals(for: "ssh-copy-id -i ~/.ssh/id_ed25519 root@hetzner-1", project: nil, session: "s").first?.also == ["servers/hetzner-1.md"])
    }

    @Test func topicPathsForOddProjectNames() throws {
        try keep(chat("Hosted on Netlify.", target: Topic.path(project: "Personal Site", topic: "hosting"), project: "Personal Site"))
        #expect(FileManager.default.fileExists(atPath: vault.url("projects/Personal Site/hosting.md").path))
        #expect(note("projects/Personal Site.md")?.decisions.first?.contains("[[Personal Site/hosting]]") == true)
    }

    @Test func lanMachineNoteIsTitledWithItsAddress() throws {
        let p = Proposal.make(kind: .fact, source: .observed, text: "Something answers SSH on 10.0.7.50.", evidence: "local network",
                              target: "machines/10-0-7-50.md", project: nil, session: nil, agent: "binder")
        try keep(p)
        #expect(note("machines/10-0-7-50.md")?.body.hasPrefix("# 10.0.7.50") == true)
    }
}

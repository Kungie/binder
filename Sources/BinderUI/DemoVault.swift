import BinderCore
import Foundation

/// A throwaway vault filled with believable demo content, for snapshots and tests.
@MainActor
public enum DemoVault {
    public enum Kind: String, CaseIterable, Sendable { case review, empty, handoff, edit }

    public static func vault(_ kind: Kind) -> Vault {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("binder-demo-\(kind.rawValue)-\(UUID().uuidString)")
        let vault = Vault(root: root)
        try? vault.ensureLayout()
        let store = ProposalStore(vault: vault)
        let start = Date().addingTimeInterval(-600)
        let items: [(Proposal.Source, String, String, String, Proposal.Confidence)] = [
            (.command, "wrangler pages deploy", "The dawn site deploys to Cloudflare Pages.", "projects/dawn.md", .high),
            (.observed, "~/.ssh/config", "hetzner-1 opens with id_ed25519_hetzner. No new key needed.", "access/ssh.md", .high),
            (.chat, "said in session", "Release notes live in docs/releases, one file per version.", "projects/dawn.md", .medium),
            (.chat, "said in session", "\u{201C}Let\u{2019}s try Tailwind for a minute.\u{201D}", "projects/dawn.md", .low)
        ]
        let count = kind == .empty ? 0 : (kind == .handoff ? 1 : items.count)
        for (i, item) in items.prefix(count).enumerated() {
            try? store.add(Proposal.make(kind: .decision, source: item.0, text: item.2, evidence: item.1, target: item.3,
                                         project: "dawn", session: "demo", confidence: item.4, now: start.addingTimeInterval(Double(i))))
        }
        for decision in ["Licence is AGPL-3.0, not MIT.", "Ships as a free app.", "The panel stays 340 pt wide."] {
            try? Memory.add(decision, to: "projects/dawn.md", vault: vault)
        }
        let used = kind == .empty ? 64_000 : (kind == .handoff ? 164_000 : 142_000)
        try? SessionStatus(session: "demo", project: "dawn", cwd: "/Users/demo/Desktop/dawn",
                           reading: ContextReading(used: used, window: 200_000), updatedAt: Date()).save(vault)
        return vault
    }

    public static func model(_ kind: Kind) -> AppModel {
        let model = AppModel(vault: vault(kind), home: FileManager.default.temporaryDirectory)
        model.refresh()
        if kind == .edit, let first = model.proposals.first {
            model.phases[first.id] = .editing("Release notes live in docs/releases.")
        }
        return model
    }
}

import AppKit
import BinderCore
import Foundation
import SwiftUI

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
            // The first one shows a topic note and an extra note, as the summarizer files them.
            let first = i == 0
            try? store.add(Proposal.make(kind: .decision, source: item.0, text: item.2, evidence: item.1,
                                         target: first ? "projects/dawn/hosting.md" : item.3,
                                         project: "dawn", session: "demo", confidence: item.4, now: start.addingTimeInterval(Double(i)),
                                         also: first ? ["hosting/cloudflare.md"] : []))
        }
        if kind == .handoff, let png = demoDiagram() {
            // A file card: an image someone attached, already captioned.
            for var p in (try? FileCatch.stage([.attached(mediaType: "image/png", base64: png.base64EncodedString(), near: "")],
                                              vault: vault, project: "dawn", session: "demo", offeredSoFar: 0)) ?? [] {
                p.text = "Architecture of the dawn site and its API"
                try? store.add(p)
            }
        }
        for decision in ["Licence is AGPL-3.0, not MIT.", "Ships as a free app.", "The panel stays 340 pt wide."] {
            try? Memory.add(decision, to: "projects/dawn.md", vault: vault)
        }
        let used = kind == .empty ? 64_000 : (kind == .handoff ? 164_000 : 142_000)
        try? SessionStatus(session: "demo", project: "dawn", cwd: "/Users/demo/Desktop/dawn",
                           reading: ContextReading(used: used, window: 200_000), updatedAt: Date()).save(vault)
        return vault
    }

    /// A made-up developer's binder, full enough to show what the graph looks like after a few weeks:
    /// projects, servers and keys tied together by wikilinks and shared stacks.
    public static func graph() -> Vault {
        let vault = Vault(root: FileManager.default.temporaryDirectory.appendingPathComponent("binder-demo-graph-\(UUID().uuidString)"))
        try? vault.ensureLayout()
        let notes: [(String, String, [String])] = [
            ("me.md", "Me", ["- git: Sam <sam@example.com>", "- editor: Zed"]),
            ("machines/this-mac.md", "this-mac", ["- macOS 15.4 · arm64", "- shell: zsh", "- Docker Desktop, Homebrew"]),
            ("machines/homelab-pi.md", "homelab-pi", ["- Raspberry Pi 5 · 10.0.7.20", "- runs: [[pi-hole]], [[homelab-dash]]"]),
            ("access/ssh.md", "ssh", ["- hetzner-1 opens with id_ed25519_hetzner, see [[hetzner-1]]", "- homelab-pi opens with id_ed25519, see [[homelab-pi]]",
                                      "- nas opens with id_ed25519, see [[nas]]"]),
            ("access/accounts.md", "accounts", ["- Fly.io token: 1Password, item Fly", "- Cloudflare token: 1Password, item Cloudflare"]),
            ("servers/hetzner-1.md", "hetzner-1", ["- Ubuntu 24.04 · Docker", "- key: [[ssh]]", "- hosts: [[recipes-api]]"]),
            ("servers/nas.md", "nas", ["- Synology DS923+ · 10.0.7.30", "- backups for [[homelab-pi]]", "- key: [[ssh]]"]),
            ("servers/pi-hole.md", "pi-hole", ["- DNS for the home network", "- on [[homelab-pi]]"]),
            ("projects/recipes-api.md", "recipes-api", ["- repo: github.com/sam/recipes-api · main", "- languages: Go", "- deploy: Fly.io",
                                                        "- database: Postgres on [[hetzner-1]]", "- staging: [[staging]]"]),
            ("projects/portfolio.md", "portfolio", ["- repo: github.com/sam/portfolio · main", "- languages: TypeScript", "- deploy: Cloudflare Pages",
                                                    "- domains: sam.example"]),
            ("projects/weather-bot.md", "weather-bot", ["- repo: github.com/sam/weather-bot · main", "- languages: Python", "- deploy: Fly.io"]),
            ("projects/homelab-dash.md", "homelab-dash", ["- repo: github.com/sam/homelab-dash · main", "- languages: TypeScript, Python",
                                                          "- runs on [[homelab-pi]]", "- domains: sam.example"])
        ]
        for (path, title, observed) in notes {
            try? NoteIO.update(vault.url(path), title: title) {
                $0.replacingObserved(observed).setting("verified", ISO8601DateFormatter().string(from: Date())).setting("source", "observed, you")
            }
        }
        try? Memory.add("Commit often, ask before pushing.", to: "me.md", vault: vault)
        try? Memory.add("The API deploys with fly deploy from main.", to: "projects/recipes-api.md", vault: vault)
        try? Memory.add("The site deploys to Cloudflare Pages.", to: "projects/portfolio.md", vault: vault)
        var state = BinderState.load(vault)
        state.lastObserve = Date().addingTimeInterval(-40 * 60)
        try? state.save(vault)
        return vault
    }

    /// A small made-up architecture sketch, for the file card in the snapshots.
    static func demoDiagram() -> Data? {
        let view = ZStack {
            Phosphor.black2
            Rectangle().fill(Phosphor.amber.opacity(0.6)).frame(width: 260, height: 2)
            HStack(spacing: 28) {
                ForEach(["SITE", "API", "DB"], id: \.self) { label in
                    Text(label).font(Fonts.mono(14)).foregroundColor(Phosphor.amber)
                        .frame(width: 90, height: 56).background(Phosphor.black2)
                        .overlay(Rectangle().strokeBorder(Phosphor.amber, lineWidth: 2))
                }
            }
        }
        guard let image = Snapshot.image(view, size: CGSize(width: 420, height: 180)), let tiff = image.tiffRepresentation else { return nil }
        return NSBitmapImageRep(data: tiff)?.representation(using: .png, properties: [:])
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

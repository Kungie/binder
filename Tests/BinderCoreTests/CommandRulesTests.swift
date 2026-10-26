import Testing
@testable import BinderCore

@Suite struct CommandRulesTests {
    func texts(_ command: String, project: String? = "dawn") -> [String] {
        CommandRules.proposals(for: command, project: project, session: "s1").map(\.text)
    }

    @Test func eachRuleProducesItsText() {
        #expect(texts("npx wrangler pages deploy ./site --project-name dawn") == ["dawn deploys to Cloudflare Pages."])
        #expect(texts("wrangler deploy") == ["dawn deploys to Cloudflare Workers."])
        #expect(texts("vercel --prod") == ["dawn deploys to Vercel."])
        #expect(texts("vercel deploy") == ["dawn deploys to Vercel."])
        #expect(texts("netlify deploy --prod") == ["dawn deploys to Netlify."])
        #expect(texts("fly launch --name dawn") == ["dawn deploys to Fly.io."])
        #expect(texts("flyctl deploy") == ["dawn deploys to Fly.io."])
        #expect(texts("gh repo create Kungie/binder --public") == ["dawn has a GitHub repo, Kungie/binder."])
        #expect(texts("git remote add origin git@github.com:Kungie/dawn.git") == ["dawn's remote is github.com/kungie/dawn."])
        #expect(texts("ssh-keygen -t ed25519 -f ~/.ssh/id_ed25519_hetzner -C eren") == ["Key id_ed25519_hetzner was created."])
        #expect(texts("ssh-copy-id -i ~/.ssh/id_ed25519_hetzner.pub eren@116.203.10.20") == ["Your key was copied to 116.203.10.20."])
        #expect(texts("cd site && wrangler pages deploy . && git push") == ["dawn deploys to Cloudflare Pages."])
        #expect(texts("ls -la") == [])
    }

    @Test func keyRulesTargetSSHNoteAndWorkWithoutProject() {
        let p = CommandRules.proposals(for: "ssh-keygen -f ~/.ssh/id_new", project: nil, session: nil)
        #expect(p.map(\.text) == ["Key id_new was created."])
        #expect(p.first?.target == "access/ssh.md" && p.first?.kind == .access)
        #expect(texts("wrangler pages deploy", project: nil) == [])
    }

    @Test func commandWithTokenMakesNoProposal() {
        let token = "gh" + "p_" + String(repeating: "a1B2c3D4e5", count: 4)
        #expect(texts("export GITHUB_TOKEN=\(token) && vercel deploy") == [])
    }

    @Test func proposalsCarryEvidenceAndConfidence() throws {
        let p = try #require(CommandRules.proposals(for: "cd site && wrangler pages deploy .", project: "dawn", session: "s1").first)
        #expect(p.evidence == "wrangler pages deploy .")
        #expect(p.source == .command && p.confidence == .high && p.target == "projects/dawn.md" && p.session == "s1")
    }
}

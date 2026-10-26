# Binder

Binder is a macOS app and CLI that gives AI coding agents a lasting memory of you: who you are, your machines, your hosting, your SSH keys and your projects. It also hands a long session over to a fresh one without losing the thread. The marketing line is **AG~~I~~M: Artificial General ~~Intelligence~~ Memory**.

**Status:** the v1 spec is written and waiting for the user's review: `docs/specs/2026-10-26-binder-v1-design.md`. The screens are in `design/flow/`. Next comes the implementation plan. There is no code yet, and nothing gets built before the spec is approved. Where the spec and this file disagree, the spec wins.

**Purpose, in the user's words:** a virtual memory for individual developers and hobbyists. It covers how each project is going, personal preferences, preferred apps and tools, hosting and machine details, and where to find the keys for access. Every project and session can reach it, and it fills itself as automatically as possible. Everything the user has described is in v1.

## Working with the user

- Talk to the user in **Turkish**. Everything in the repo is **English**: file names, code, comments, UI copy, CLI, docs and the future site.
- When something is unclear, ask a short multiple-choice question rather than guessing.
- Workflow: brainstorm, then a written spec, then a plan, then code. The user approves each step.
- Don't mention the company or studio behind Binder anywhere in this repo.

## The problem

AI sessions get worse the longer they run, because old context crowds out the task. A fresh session doesn't know the topic, so the agent asks again: "where are we hosting this?", "should I create an SSH key?". Binder removes that trade-off. A new session starts out knowing your setup and where the work stands.

## Audience

Individual developers and hobbyists first, on macOS.

## What Binder knows

1. **You**: identity and role, working style (when to ask and when not to, tone, language, commit and PR habits), technical taste, and current projects and goals. Personal details are typed in by the user at onboarding. Technical taste is partly detected from repos and then corrected by the user.
2. **Machines**: this Mac, servers and the homelab.
3. **Hosting and app preferences**: which hosting, domains, services and tools the user uses.
4. **SSH**: which key opens which host, which keys exist, and which key opens nothing. **Never the private key material.**
5. **Projects**: repo, deploy method, domains, and decisions made along the way.

## How it learns

- **Observation** is the differentiator. Binder reads what is already on the machine:
  - `~/.ssh/config` and the list of keys
  - `gh auth status` and the git identity
  - CLIs the user is signed in to (Vercel, Netlify, Fly, Cloudflare `wrangler`, Hetzner `hcloud`, Supabase and similar)
  - deploy files in projects (`fly.toml`, `vercel.json`, `wrangler.toml`, `.github/workflows`, `Dockerfile`)
  - DNS records for the domains it finds
- **Conversation** is captured in two layers:
  1. Commands the agent actually ran in a session (`ssh-keygen`, `wrangler deploy`, `fly launch`). These are parsed deterministically from the transcript, with no model call.
  2. Decisions that were only said. These are optional, and get summarised with the user's own `claude -p` or `codex exec` on their own plan, with no API key.
- Anything captured from a conversation lands in a **proposal inbox**. Nothing from a conversation enters memory until the user approves it.

## How agents get it

- **Claude Code and Codex** come first, through global, user-level hooks that `binder install` adds and can remove. It never touches the user's other hooks.
- **Session start:** a short **context card** for the current folder. It tells the agent: this is verified, use it without asking; if something is missing, ask and propose.
- **Session end:** capture, which produces proposals in the inbox.
- MCP is a later, extra channel.

## Handoff

- Binder notices when a session is getting long, using the share of context used from the transcript, and offers "Hand off now".
- The user can also hand off at any time from the menu bar or the CLI.
- A handoff card holds what's done, what's left, what not to touch and the next action.
- The card waits in Binder. The next session opened in that folder picks it up automatically, and so does `/clear` in Claude Code, which fires SessionStart again. There is also an "Open a new session" button that starts `claude` or `codex` in the same folder in the user's terminal (Terminal, iTerm or Ghostty).

## Surfaces

- **Menu bar:** its main job is approving the potential context captured in the current session (Keep, Edit, Drop, Keep all). It also shows the handoff warning.
- **Main window:** onboarding, detailed settings, and browsing and editing memory. This is where everyday use beyond approvals happens.
- **CLI `binder`:** `card` (session start), `capture` (session end), `observe` (run the observers without the app), `install` and `uninstall` (hooks), and `handoff`.

## First launch

Binder looks around first and fills in everything it can detect: this Mac, SSH, GitHub, hosting, projects and the homelab ("something answers SSH on 192.168.1.20, yours?"). It only asks for what it can't guess: identity and role, working style, technical taste, and projects and goals.

## Architecture (agreed: approach A)

One Swift package with three targets:
- **BinderCore**: vault read and write, observers, the card builder, and transcript parsers for Claude Code (`~/.claude/projects/*.jsonl`) and Codex (`~/.codex/sessions`).
- **Binder.app**: the SwiftUI menu bar app with a main window. It schedules the observers, using FSEvents on watched files plus a periodic run.
- **binder**: the CLI that the hooks call.

The author's Dawn app (github.com/Kungie/dawn) is a Swift menu bar app that already uses Claude Code hooks. Its patterns are a useful reference. Binder is a separate repo and release.

## Storage

A plain Markdown folder. The default is `~/Binder`, and first launch also offers "put it inside my Obsidian vault". Binder never needs Obsidian, but its notes are written to feel native there: Properties in the frontmatter, and `[[wikilinks]]` between notes so the graph shows how they connect (project → server → SSH key). An Obsidian plugin can come later as an extra surface.

```
~/Binder/
  me.md
  machines/this-mac.md
  access/ssh.md
  access/accounts.md
  servers/<host>.md
  projects/<name>.md
  handoffs/<project>/<yyyy-mm-dd-hhmm>.md
  .binder/            # inbox, cached cards, transcript cursors, state, log (managed)
```

Each note has frontmatter (`verified`, `source`) and two kinds of content:
- An **observed block** between `<!-- binder:observed start -->` and `<!-- binder:observed end -->`. The observers rewrite it from scratch every time, so it always shows the real state of the machine.
- **`## Decisions`** and everything else. Only approved proposals and the user's own edits go here.

## Privacy rules

- Local only. No telemetry, no account, no cloud.
- Never store secrets (private keys, tokens, passwords). Store pointers instead, for example "the token is in 1Password, item Hetzner API".
- Personal details only ever come from the user.

## Mascot and voice

- **Binder is both the name and the mascot:** a small ring binder with a glass dome on its head that shows its brain, and its face in the binder's label window.
- **Tone:** lightly humorous and dry. For example: "I know where your SSH keys live. Not what's in them. I'm nosy, not reckless."
- **Copy rules:** short sentences, plain English, no exclamation marks, no marketing filler ("seamless", "supercharge", "unlock"), no em dashes. Be honest about what Binder can and can't do.

## Design (chosen: direction C, Phosphor)

The source is in `design/`, and the canvas is at https://claude.ai/artifact/TA4eWJEfD6Fi6EJ4oeaxjW. The look is a retro broadcast ident on a CRT: dark only, brutalist and warm, flat, with a corner radius of 0–2 pt (except the app icon), scanlines and a faint glow on "screen" panels.

| Token | Hex | Use |
|---|---|---|
| black | `#050303` | background |
| black-2 | `#0E0908` | raised surfaces |
| line | `#2A1B19` | hairlines, borders |
| cream | `#F3E9D6` | primary text |
| cream-2 | `#E6D9BE` | secondary text |
| sand | `#D2C09B` | mascot spine, legs |
| dim | `#A09189` | labels, captions |
| amber | `#F2A60C` | accent, brain, meters, primary buttons |
| amber-deep | `#8F5A00` | button shadow, meter stripes |
| gyri | `#5A3A00` | lines on the brain |
| ink | `#160D0B` | text on amber and cream |

- **Type:**
  - Archivo at 125 % width, weight 800–900, for the wordmark, titles and buttons. The wordmark has stacked offset shadows alternating black and amber.
  - VT323 for anything "on a screen": percentages, statuses, counts.
  - Space Grotesk for body text.
  - Space Mono Bold, uppercase, for small labels.
- **Primary buttons:** amber fill, ink text and a hard amber-deep offset shadow. Body text must meet WCAG AA, with a minimum text size of 10.5 pt.

## Competitors (to stay honest about)

- **claude-mem** (very large, session continuity across agents): the strongest on handoff. Ours must be clearly better or work alongside it.
- **Supermemory**, **Mem0 / OpenMemory**: memory layers aimed at conversations and apps.
- **Basic Memory**: Markdown and Obsidian plus MCP, the closest to our storage idea.
- **hippo-memory**: a memory CLI and MCP.
- **avenoxbeyin**: an Obsidian second brain driven by hooks.
- **Mori**: a general macOS memory app in the menu bar.
- The agents' own `CLAUDE.md`, `AGENTS.md` and built-in memory.
- Handoff skills.

**Our angle:** Binder observes the machine instead of only remembering what was said, it's a native Mac app with an approval inbox, and it builds the handoff in.

## Decided (26 Oct 2026)

- **License:** AGPL-3.0, the same as Dawn.
- **Price:** free and open source.
- **Vault:** `~/Binder` by default, with an option to put it inside an Obsidian vault.
- **Handoff:** the card waits and is picked up automatically, plus an "Open a new session" button.
- **Design:** direction C, Phosphor.

## Open questions

- When the repo goes public. It is private for now.
- A brand check for the name "Binder" (mybinder.org and Android's Binder IPC also exist).
- How to measure "context used" for Codex sessions.
- Which observers ship in v1, and how often they run.

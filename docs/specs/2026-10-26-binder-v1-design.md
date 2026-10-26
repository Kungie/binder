# Binder v1 · design spec

26 October 2026. Status: **written, waiting for review.**

The screens for this spec are on the canvas page "v1 flow" (https://claude.ai/artifact/TA4eWJEfD6Fi6EJ4oeaxjW), with a copy in `design/flow/`. The visual direction is C · Phosphor. Tokens and type are in `CLAUDE.md`.

## 1. Purpose

Binder gives individual developers and hobbyists a virtual memory that every project and every AI session can read. That memory covers:

- how each project is going
- the user's personal preferences
- the apps and tools they prefer
- hosting and machine details
- where to find the keys needed for access

Binder keeps that memory fresh by itself, as automatically as it safely can.

The problem it solves is a dilemma. Long sessions lose sharpness, but fresh sessions don't know the topic. With Binder a fresh session starts informed, so starting fresh becomes cheap.

## 2. Scope

**Everything below ships in v1.**

**Not in v1:**
- sync between Macs
- cloud features or accounts
- MCP
- an Obsidian plugin
- Windows or Linux
- teams
- agents other than Claude Code and Codex

## 3. The journey

### 3.1 Install

The DMG opens a window with the same "drag to Applications" layout Dawn uses. It is built with `dmgbuild` from `scripts/dmg-settings.py`, and the background shows the Phosphor scanlines, the Binder icon, amber chevrons, the Applications folder and the mascot's dome peeking up from the bottom edge. The copy reads: "Drag Binder into Applications. Then open it from there. I'll take it from here."

The app is ad-hoc signed like Dawn. Notarization is an open question (§12).

### 3.2 First run: boot (artboard *Boot*)

The window powers on like a CRT: it stretches vertically from a line. The mascot's brain flickers on, its face screen switches on, and it blinks. A boot log types out in VT323:

```
BINDER 1.0
CHECKING THE MAC ......... OK
WARMING UP THE BRAIN ..... OK
CALIBRATING NOSINESS ..... 100%
SECRETS .................. NOT INTERESTED
READY_
```

A stepped amber progress bar fills. Then "Hello. I'm Binder." rises in, with the "Let's start" button and "Takes about two minutes." The whole sequence takes about 3.5 seconds and can be skipped with a click or Return.

The setup window is fixed at 880 × 600 pt or larger, with a left rail that shows the mascot (its mood changes per step) and the step list: HELLO · PERMISSIONS · LOOK AROUND · ABOUT YOU · YOUR BINDER · CONNECT.

### 3.3 Permissions (artboard *Permissions*)

Each row says exactly why Binder needs the permission. The row's button triggers the real macOS prompt, and its status reads GRANTED, Allow or OPTIONAL.

| Permission | Why (copy) | How on macOS |
|---|---|---|
| Desktop and Documents | Your projects live there. I read repo and deploy files, nothing else. | TCC prompt on first access to `~/Desktop` and `~/Documents` (the app is not sandboxed) |
| Local network | To spot your homelab. I knock on SSH and a few web ports, then leave. | `NSLocalNetworkUsageDescription`; the prompt appears on the first LAN connection. This is known to be flaky on macOS 15+. The app must run from /Applications, and we show a "Check again" path if it doesn't appear. |
| Notifications | One nudge when a session gets long. Not a pen pal. | `UNUserNotificationCenter` authorization |
| Terminal (optional) | So "Open a new session" can open one for you, in the right folder. | `NSAppleEventsUsageDescription`; Apple Events to Terminal or iTerm. Ghostty is opened with `open -na`. |
| Open at login | So I'm around when you are. | `SMAppService.mainApp` |

Footnote: "~/.ssh, ~/.claude and ~/.codex need no permission. I read them. I never write to ~/.ssh."

### 3.4 Let me look around first (artboard *LookAround*)

The observers (§6) run once. Rows appear one by one with "LOOKING" in dim text, then flip to their result: FOUND in amber, HALF in cream (for example "Claude Code found · Codex not installed"), or a question. The homelab row asks "Something answers SSH on 192.168.1.20. Yours?" with Mine / Not mine. While this runs, the mascot's eyes scan left and right. A counter reads `READ 41 FILES · WROTE 0`, and nothing is written to disk until step 3.6. Buttons: "Looks right" and "Fix something", which opens that row for editing.

### 3.5 About you (artboard *AboutYou*)

This step asks only what Binder can't detect:
- **You:** name, what you do. The time zone is detected.
- **How agents should work with you:** toggle chips (language in chat and in files, short answers, ask before pushing, recommend rather than list, explain trade-offs, commit often), plus your own chips.
- **Taste:** detected from repos (languages, frameworks, tools), editable. Anything struck through means "rather not use".
- **Now and next:** free text.

Buttons: "Write it down" and "Fill in later".

### 3.6 Your binder (artboard *Vault*)

Binder creates the folder. Each file appears in the tree as it is written. A preview shows the raw Markdown of one project note, with the observed markers and `[[wikilinks]]`. The location defaults to `~/Binder`, with "Change" and "Put it in my Obsidian vault" options. The footer counts what was written: `9 NOTES · 0 SECRETS · EVERY LINE CAME FROM THIS MAC OR FROM YOU`.

### 3.7 Connect (artboard *Connect*)

Each supported agent gets a card. A found agent shows the exact hook lines Binder adds, then CONNECTED. A missing one shows WAITING: "The day ~/.codex shows up, I'll offer to connect it." A "Try it" box suggests opening a session in a real project and asking a question the agent used to ask back. "Open a session in <project>" does this. The footer reads: "Changed your mind later? `binder uninstall` takes both hooks out."

### 3.8 Every day: the menu bar (artboard *MenuBar*)

The menu bar icon is a monochrome template image of the dome, the brain and the collar. It has four states:
- idle
- `n` items to review (with the count)
- handoff soon (a small square)
- observing (the brain dims and pulses)

The panel is 350 pt wide and its main job is approvals:
1. **Header:** the brain icon (the target of the "keep" animation) and `215 MEMORIES · 3 TO REVIEW`.
2. **This session:** project, agent, and context used as a big VT323 percentage with a stepped bar. At the threshold, "Getting long." appears with **Hand off now** and **Not yet**.
3. **Caught this session:** one card per proposal, each with its source tag (FROM A COMMAND · `<cmd>`, SEEN ON THIS MAC · `<file>`, FROM THE CHAT, or FROM THE CHAT · PROBABLY A ONE-OFF) and Keep / Edit / Drop.
   - **Keep:** the card squashes slightly, then is sucked toward the brain along a skewed, shrinking path (0.7 s, ease-in). The brain gulps (scales to 1.22× and back in 0.5 s), and the memory count goes up by one.
   - **Drop:** a 4 px amber shredder edge bites twice across the top. The card splits into 8 vertical strips that fall and rotate in a staggered order (about 0.85 s each, about 1.1 s in total), and then the slot collapses.
   - **Edit:** the text becomes an inline field, and Return keeps the edited version.
   - **Keep all n:** keeps the cards one after another, 220 ms apart.
   - When the list is empty: "ALL FILED. Nothing to review. I'll call you when there is."
4. **Footer:** Open Binder · Add a memory.

All animations respect Reduce Motion. With it on, items fade toward or away from the brain in 150 ms.

### 3.9 Every day: the main window (artboard *Library*)

- **Sidebar:** search, Inbox (count), Me, Machines, Access, Servers, Projects (expandable), Handoffs, Settings. A small brain footer shows `215 MEMORIES · LAST LOOK 09:14`.
- **Note view:** breadcrumb, title, chips (VERIFIED, source, decision count), the **observed** block (key/value pairs, "I rewrite this part"), **Decisions** (date, text, provenance: kept from a command, kept from the chat, written by you), and an "Add a memory" field ("File it"). Buttons: Open in Obsidian · Edit.
- **Right column:** Linked notes, Last handoff (done, left, don't, next, plus "Pick up from here"), and **What agents see in this project**, a live preview of the exact context card with its token estimate.
- **Settings:** vault location, project roots to scan, observers on and off, observer schedule, handoff threshold, conversation summarizer on and off (and which CLI it uses), connected agents, the secret filter test, and reset.

## 4. Architecture

One Swift package, macOS 14+:

| Target | Kind | Job |
|---|---|---|
| `BinderCore` | library | Vault model and I/O, note parser and writer, observers, proposal store, card builder, transcript parsers (Claude Code, Codex), context meter, secret filter, hook installer |
| `Binder` | SwiftUI app (menu bar + windows) | Onboarding, menu bar panel, main window, scheduling, FSEvents, notifications |
| `binder` | CLI (swift-argument-parser) | `card`, `capture`, `observe`, `handoff`, `install`, `uninstall`, `doctor` |

- **The CLI works without the app.** Hooks call the CLI, and the CLI reads and writes the vault directly. The app watches the vault and the inbox with FSEvents and reacts. The two share no other channel.
- **Hooks never hurt a session.** Every hook call has a 2 s timeout, always exits 0, and logs errors to `~/Binder/.binder/log`. `binder card` serves a precomputed card from `.binder/cards/<project-id>.md` when one is fresh. The target is under 150 ms.
- **Project identity:** the git remote URL, normalized (or the folder path when there is no remote), hashed to a `project-id`.

## 5. Data

### 5.1 Vault

```
~/Binder/
  me.md
  machines/this-mac.md
  access/ssh.md
  access/accounts.md
  servers/<host>.md
  projects/<name>.md
  handoffs/<project>/<yyyy-mm-dd-hhmm>.md
  .binder/                 # not meant for people
    inbox/<id>.json        # pending proposals
    cards/<project-id>.md  # precomputed context cards
    cursors.json           # transcript read positions per session
    state.json             # observer runs, settings
    log
```

### 5.2 Note format

```markdown
---
verified: 2026-10-26T09:14:00+02:00
source: [observed, you]
links: ["[[site]]", "[[this-mac]]"]
---
# dawn

<!-- binder:observed start -->
- repo: github.com/Kungie/dawn
- site: Cloudflare Pages, see [[site]]
<!-- binder:observed end -->

## Decisions
- 2026-10-17 · Licence is AGPL-3.0 · kept from the chat

(anything else the user writes)
```

Rules:
- Observers replace only the text between the markers.
- Writes are atomic: write a temp file, then rename.
- If the user has edited the file since the last read (the mtime and hash differ), Binder re-reads it before writing.
- If the markers are missing, Binder re-adds them at the top and never touches other text.

### 5.3 Proposal

```json
{ "id": "p_8f2c", "createdAt": "…", "session": "…", "agent": "claude-code",
  "project": "dawn", "kind": "decision|fact|preference|access",
  "source": "command|observed|chat", "evidence": "wrangler pages deploy",
  "text": "The dawn site deploys to Cloudflare Pages.",
  "target": "projects/dawn.md#Decisions", "confidence": "high|medium|low" }
```

Keep appends the text to the target. Drop deletes the file and remembers its hash, so the same proposal is never offered twice.

## 6. Observers (v1)

All observers are read-only, run with timeouts and only when the tool exists, and none of them reads secret material.

| Observer | Reads | Writes to |
|---|---|---|
| Mac | `sw_vers`, `$SHELL`, `brew list` count, chip, memory | `machines/this-mac.md` |
| SSH | `~/.ssh/config` (Host, HostName, User, Port, IdentityFile), the names of `~/.ssh/*.pub` files, `ssh-keygen -lf` on **public** keys only | `access/ssh.md`, `servers/*.md` |
| Git and GitHub | `git config --global user.*`, `gh auth status` | `access/accounts.md` |
| Hosting CLIs | `vercel whoami`, `netlify status`, `flyctl auth whoami`, `wrangler whoami`, `hcloud context list`, `supabase projects list` (each one only if it is installed, 3 s timeout) | `access/accounts.md` |
| Projects | Git repos up to 3 levels deep under the chosen roots: remote, branch, dirty state, languages by file type, deploy files (`fly.toml`, `vercel.json`, `wrangler.toml`, `netlify.toml`, `.github/workflows/*`, `Dockerfile`, `docker-compose.yml`) | `projects/*.md` |
| Domains | Domains found in those configs and `CNAME` files; `dig` A/CNAME records to name the provider | the project notes |
| Homelab | Private-IP hosts from the SSH config, and, with the local network permission, a light probe of the local /24 on ports 22, 80, 443, 8006 (Proxmox) and 8123 (Home Assistant). It identifies hosts by SSH banner and HTTP title. Found hosts are proposed, never written directly. | `servers/*.md` via proposals |
| Agents | Whether `~/.claude` and `~/.codex` exist, and their versions | `machines/this-mac.md` |

**When observers run:**
- on app launch
- every 6 h
- through FSEvents on `~/.ssh/config`, `~/.gitconfig` and the project roots (debounced to 30 s)
- with `binder observe`

## 7. Capture from sessions

1. The Stop hook (Claude Code) and its Codex equivalent call `binder capture`. Binder reads the transcript from the stored cursor onward: `~/.claude/projects/*/<session>.jsonl` or `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`.
2. **Layer 1, commands (deterministic, always on):** rules match the tool calls the agent ran and turn them into proposals. For example, `wrangler pages deploy` becomes "<project> deploys to Cloudflare Pages", `ssh-keygen -f X` becomes "key X was created", `gh repo create` becomes "repo created", and `fly launch` becomes "deploys to Fly".
3. **Layer 2, conversation (on by default when a supported CLI exists, shown in Settings with its cost):** Binder sends the condensed new part of the transcript to the user's own `claude -p` or `codex exec` with a fixed prompt that returns proposal JSON. There is no API key, and it uses the user's plan.
4. **Every proposal passes the secret filter** before it is stored:
   - Binder looks for patterns like `-----BEGIN`, `AKIA…`, `ghp_…`, `sk-…`, `xox…`, `password=`, and long high-entropy strings.
   - A match drops the proposal and logs it with the match redacted.
5. **Dedup:** a proposal that matches an existing memory exactly or by normalized text is skipped.

## 8. The context card

Binder builds a card per project and delivers it on SessionStart. For Claude Code this is `additionalContext` in the hook output; Codex uses its SessionStart hook. The card is plain text, about 600 tokens at most, assembled in this order:

1. The header: `BINDER CARD · VERIFIED, USE IT. If something you need is missing, ask once and say what you'd write down.`
2. Me: name, language, working style.
3. The project: observed facts and recent decisions.
4. Linked servers and access: host, user and key path, never key contents.
5. The pending handoff, if there is one.

When the budget is tight, older decisions go first. Main window → "What agents see" shows the exact card.

## 9. Handoff

- **The context meter:**
  - **Claude Code:** the last assistant message's `input_tokens + cache_read_input_tokens + cache_creation_input_tokens`, divided by the window. The window is 200k by default and 1M when the model id carries `[1m]` or the usage has already passed 200k.
  - **Codex:** the latest `token_count` event, divided by `model_context_window` (from the event when it's present, otherwise from config or a known default). The totals are cumulative, so Binder uses the last turn's input.
  - Both show "estimate" in small type.
- **The warning** comes at 70 % by default (adjustable from 50 to 90 %): a menu bar state, plus one notification per session.
- **Hand off now** writes `handoffs/<project>/<ts>.md` with four headings: Done, Left, Don't touch, Next. The summarizer (Layer 2) fills them from the session. Without it, a deterministic fallback fills them: the last user request, the files edited by tool calls, and any open todos found in the transcript.
- **Pickup:** the next SessionStart in that project gets the card. This includes Claude Code's `/clear`, which fires SessionStart with source `clear`. The handoff is marked consumed after it's delivered once, and it stays visible in the Handoffs section.
- **Open a new session** opens the user's terminal in the project folder and runs `claude` or `codex`.

## 10. Privacy and security

- Everything stays local. There is no telemetry and no account.
- The only network use is the hosting CLIs the user already signed in to (run read-only), `dig`, and the optional LAN probe.
- Never stored: private keys, tokens, passwords, `.env` values. Pointers are fine ("in 1Password, item Hetzner API").
- Binder never writes to `~/.ssh`. It writes to `~/.claude/settings.json` and the Codex config only through install and uninstall, which are idempotent and back up the original first.
- Personal details only ever come from the user.

## 11. Errors

- **A tool is missing:** its row shows a dim "not installed" and the observer is skipped. It is never an error dialog.
- **A permission is denied:** the row stays "Allow". Features degrade, for example no homelab probe. Settings shows what's missing.
- **The vault is moved or deleted:** the app asks for the vault location again, and the CLI prints a one-line hint and exits 0.
- **A transcript format changes:** the parser fails closed (no proposals), and `binder doctor` reports which agent's format failed.
- **Hook conflicts:** Binder adds its own entries and never reorders or removes others. `doctor` verifies them.

## 12. Testing

- **Unit tests in `BinderCore`:**
  - parsers: the SSH config, the note format, and Claude Code and Codex transcript fixtures
  - the context meter
  - the secret filter, with a corpus of real-looking secrets
  - dedup
  - golden files for the card builder
- **Integration:**
  - run the hook installer and uninstaller against a temporary `HOME`
  - `binder card` and `binder capture` against fixture vaults and transcripts
  - check the card's time budget
- **UI:** a snapshot command like Dawn's (`Binder snapshot docs`) renders every onboarding step and panel state to PNGs for review.
- **Manual first-run checklist** on a clean user account, with permissions reset (`tccutil reset`).

## 13. Open questions

1. **Notarization:** paying for an Apple Developer ID ($99 a year) removes Gatekeeper warnings and makes the local network permission more reliable. Dawn currently ships unnotarized.
2. Check the Codex hook event names and the rollout token fields against the current Codex source before building the Codex parser.
3. Whether the summarizer should ask once at first run ("Use your Claude plan to read sessions? About N tokens per session") or be silently on with a toggle.
4. The App Store already has an unrelated "Binder - Journal & Notes" app. Binder ships outside the App Store, but a quick trademark search is still worth doing before launch.

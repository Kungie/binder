# Binder

AG~~I~~M. Artificial General ~~Intelligence~~ Memory.

Binder is a small macOS app and CLI that remembers you, your machines, your hosting and your SSH keys. A fresh AI session starts out knowing your setup and doesn't ask where the site is hosted. When a session gets long, Binder hands it over to a new one without losing the thread.

It works with Claude Code today. Codex support comes later.

## Install

Download `Binder.dmg` (or `Binder.zip`) from Releases and drag Binder into Applications. It isn't notarized, so the first time you open it, go to System Settings › Privacy & Security and click **Open Anyway**.

Setup takes about two minutes:

1. Binder asks for a few permissions, each with a one-line reason.
2. It looks around this Mac.
3. It asks only what it can't guess about you.
4. It writes your binder.
5. It plugs into Claude Code.

## What it reads, and what it never touches

Binder reads:
- `~/.ssh/config` and the names of your `*.pub` keys
- the git and `gh` identity
- the hosting CLIs you're signed in to: Vercel, Netlify, Fly, Cloudflare, Hetzner and Supabase
- the deploy files and domains in your projects

If you allow it, it also checks your local network for a homelab.

It writes Markdown into `~/Binder` (or a folder inside your Obsidian vault). It never writes to `~/.ssh` and never opens a private key. It stores pointers to secrets, never the secrets themselves. Nothing leaves your Mac: there's no account and no telemetry.

Reading your sessions with your own Claude plan, to catch decisions you only said out loud, is off until you say yes.

## The pieces

- **Menu bar.** Approve what a session caught. Keep sends it into Binder's brain, Drop shreds it. You'll also see a nudge when a session is getting long, with "Hand off now".
- **Main window.** Every note, what agents see for each project, the last handoff, and the settings.
- **`binder` CLI.** Lives in `Binder.app/Contents/Helpers/binder`. Claude Code's hooks call it:

  | Command | Called by | What it does |
  |---|---|---|
  | `binder card` | the SessionStart hook | gives the agent its context card |
  | `binder capture` | the Stop hook | catches what the session did |

  You can also run these yourself:
  - `binder observe` looks around and updates your notes
  - `binder handoff` writes a handoff card for a fresh session
  - `binder install` / `binder uninstall` add or remove the hooks
  - `binder doctor` checks that everything is wired up

## Build from source

macOS 14+, Swift 6 (the Command Line Tools are enough).

```sh
make test       # all tests
make app        # build/Binder.app and build/Binder.zip (and build/Binder.dmg with: pip install dmgbuild)
make snapshots  # renders every screen to docs/snapshots
```

## Licence

AGPL-3.0.

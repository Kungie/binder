# Binder 2

A personal desktop dashboard for one developer: a task table for the owner and for a Claude Code agent on the home server, projects with Obsidian notes, devices, the home server and a Bambu 3D printer. The spec is `docs/specs/2026-11-13-binder-2-design.md` and wins over this file. Plans live in `docs/plans/`.

## Working with the owner

- Talk to the owner in **Turkish**. Everything in the repo is **English**: code, comments, UI copy and docs.
- Workflow: spec, then plan, then code. The owner approves each step. When something is unclear, ask a short multiple-choice question.
- Never push without asking. No attribution lines in commits.
- The repo is public: never commit personal addresses, host names, serials or keys. Demo data uses a made-up developer and `10.0.7.x`.

## Layout

```
crates/core/      Rust library: SQLite, tasks, projects, config, Obsidian notes (no Tauri)
app/src-tauri/    Tauri app: thin commands over the core
app/src/          React + TypeScript + Tailwind interface
```

The interface talks to an `Api` interface: Tauri `invoke` in the app, an in-memory fake (`app/src/lib/memory.ts`) in tests and in the browser preview.

## Build and test

- `make test`, `make dev`, `make build`, `make preview`. Always build through `make`.
- `~/Desktop` syncs with iCloud, which breaks codesign on build output, so the Makefile sets `CARGO_TARGET_DIR` to `~/Library/Caches/Binder/target`.
- Data lives in `~/Library/Application Support/io.github.kungie.binder/` (`binder.db`, `config.toml`).
- The Obsidian vault lives on the Raspberry Pi and is mounted on the Mac at `/Volumes/vault` over SMB. Binder only appends under `## Done` in a project note and writes nothing when the vault isn't mounted.

## Look

Dark only, no colour, minimal. Panels like panelui.dev, chrome like Orca and Linear.

| Token | Hex |
|---|---|
| bg | `#000000` |
| panel | `#0A0A0A` |
| raised | `#141414` |
| line | `#1C1C1C` |
| line-strong | `#2A2A2A` |
| text | `#EDEDED` |
| text-2 | `#A1A1A1` |
| text-3 | `#6B6B6B` |

Inter 13 px (12 px in tables), weights 400 and 500 only; Geist Mono for numbers, ids and paths. Radius 6 px on controls, 8 px on panels. State shows through shape, never colour. Copy: plain English, no exclamation marks, no em dashes, no filler.

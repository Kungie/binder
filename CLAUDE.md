# Binder 2

A personal desktop dashboard for one developer: a task board for the owner and for a Claude Code agent on the home server, projects from GitHub with Obsidian notes, devices, the home server, Cloud (a file manager over an SMB share on the server) and a Bambu 3D printer. The specs are `docs/specs/2026-11-13-binder-2-design.md`, `docs/specs/2026-11-13-binder-2.1-design.md` and `docs/specs/2026-11-13-binder-2.2-design.md` (the later one wins where they differ), and they win over this file. Plans live in `docs/plans/`.

## Working with the owner

- Talk to the owner in **Turkish**. Everything in the repo is **English**: code, comments, UI copy and docs.
- Workflow: spec, then plan, then code. The owner approves each step. When something is unclear, ask a short multiple-choice question.
- Never push without asking. No attribution lines in commits.
- The repo is public: never commit personal addresses, host names, serials or keys. Demo data uses a made-up developer and `10.0.7.x`.

## Layout

```
crates/core/      Rust library, no Tauri: SQLite, tasks, projects, devices, notes, SSH runner (server status,
                  agent runs), git status, Bambu reports, Claude Code session summaries, hook install,
                  GitHub sync through `gh` (github.rs), Cloud file operations (cloud.rs), the Cloud fast path over
                  SSH (cloud_fast.rs, on shell.rs)
crates/cli/       binder-cli: the Claude Code hook (`hook <event>`) and the detached `summarize <session>`
app/src-tauri/    Tauri app: thin commands over the core, the printer MQTT workers (printer.rs), Cloud copies
                  in flight (transfers.rs), "Open with" through Launch Services (apps.rs)
app/src/          React + TypeScript + Tailwind interface
site/             the website at binder.erenkahraman.dev: static HTML and assets, no build step
```

The interface talks to an `Api` interface: Tauri `invoke` in the app, an in-memory fake (`app/src/lib/memory.ts`) in tests and in the browser preview.

## Build and test

- `make test`, `make dev`, `make build`, `make preview`. Always build through `make`.
- `~/Desktop` syncs with iCloud, which breaks codesign on build output, so the Makefile sets `CARGO_TARGET_DIR` to `~/Library/Caches/Binder/target`.
- Data lives in `~/Library/Application Support/io.github.kungie.binder/` (`binder.db`, `config.toml`). The app and `binder-cli` share `binder.db` (WAL, 5 s busy timeout).
- `binder-cli` ships as a Tauri sidecar: the Makefile copies it to `app/src-tauri/binaries/binder-cli-<triple>` before any build, and Tauri puts it in `Contents/MacOS/`.
- Everything on the home server goes through the `Remote` trait (`SshRemote` in the app, `FakeRemote` in tests): `ssh -o BatchMode=yes -o ConnectTimeout=5`, never touching `~/.ssh`. Agent runs keep their files in `~/.binder-agent/` on the server.
- Secrets: the printer access code lives in the Keychain (service `io.github.kungie.binder.printer`, account the serial); the printer certificate is pinned by SHA-256 in `config.toml`. Session text is redacted (`redact.rs`) before any model sees it.
- `make install` replaces `/Applications/Binder.app`. The owner runs it; don't run it unasked.
- The Obsidian vault lives on the Raspberry Pi and is mounted on the Mac at `/Volumes/vault` over SMB. Each project has a folder, `Projects/<name>/`, holding its note `<name>.md` (written once from a template) and `Daily/YYYY-MM-DD.md` files that Binder only appends `- HH:MM · text` lines to. Notes from before 2.2 at `Projects/<name>.md` are moved into the folder unchanged, and their `## Done` lines are still read. Nothing is written when the vault isn't mounted; lines wait in `pending_done` with their date and time. `meta.last_done` holds when a line was last written.
- Claude Code hooks find a session's project by its Local folder, then the repo's GitHub `origin`, then the repo folder's name. The app in `/Applications` points the hooks at its own `binder-cli` at start when they run another one; a dev build never does.
- Cloud is `/srv/cloud` on the server, shared as `[cloud]` (`scripts/cloud-share.sh`) and mounted at `/Volumes/cloud`. Every Cloud command refuses unless that path is a mount point, so nothing lands on the Mac's own disk. Paths never leave the share's root, and deletes go to `.trash/<id>/` beside an `.origin` file.
- **Fast path.** With a server set and a check passing (a token written over SMB and read back over SSH at `cloud_remote`, default `/srv/cloud`), folder uploads and downloads stream one tar, copies run `cp -a`, zips run `zip`/`unzip`, and sizes and free space are counted on the server. Every script is built with `remote::quote`. Without it everything runs over SMB (the `zip` crate for zips). Work writes to a `.binder-partial-<pid>-…` name and lands through the mount; partials of a dead process go to the trash as unfinished uploads.
- **More than one disk.** The Pi's SD card is mounted inside Cloud at `SD/`. Through SMB the Mac sees one device for the whole share, so a disk shows only as EXDEV on a rename: an item goes to the `.trash` of its own disk (`SD/.trash`, listed in `.trash/.disks`), and a move across disks is a copy then a remove, in the transfer tray. `lost+found` is never listed.
- GitHub sync runs `gh` with its own login on a second database connection, at start and on **Sync**. It only updates GitHub's facts on a project (URL, description, privacy, last push, language, archived), never the owner's edits, and never deletes. A GitHub project the owner deletes is remembered in `hidden_repos` and sync skips it until it's brought back.

## Look

Dark only and mostly black and white. Colour is small (an icon, a dot, a chip's icon) and always means something. Panels like panelui.dev, chrome like Orca and Linear, the task board like Roma.

| Token | Hex | Use |
|---|---|---|
| bg | `#0B0B0C` | window |
| panel | `#141416` | panels, board columns |
| card | `#1C1C1F` | cards, inputs, chips |
| raised | `#242428` | hover, selected, menus |
| line | `#26262A` | hairlines |
| text | `#EDEDED` | |
| text-2 | `#A1A1A1` | |
| text-3 | `#6B6B6B` | |

Meaning colours: backlog and canceled `#6B6B6B`, todo `#D4D4D4`, in progress `#F2C94C`, done `#8B7CF6`, urgent and needs attention `#F2994A`, error and offline `#EB5757`, online `#4CB782`. Each project has one of 8 colours (`#5E6AD2 #26B5CE #4CB782 #F2C94C #F2994A #EB5757 #B76EDB #8D8D8D`), from a hash of its name or chosen by the owner.

Inter 13 px (12 px in tables), weights 400 and 500 only; Geist Mono for numbers, ids and paths. Radius 8 px on controls, 10 px on cards and panels; chips are 24 px pills. Sidebar 208 px, detail screens at most 960 px wide. Copy: plain English, no exclamation marks, no em dashes, no filler.
- The website in `site/` runs on the home server in `~/sites/binder-site` (Caddy plus its own Cloudflare Tunnel, like the other sites there). To publish a change, `rsync -a --delete site/ <server>:sites/binder-site/public/`; Caddy serves it at once.

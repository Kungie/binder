# Binder 2

A personal desktop dashboard for one developer: a task board for the owner and for a Claude Code agent on the home server, projects from GitHub with Obsidian notes, a GitHub screen for pull requests and issues, devices, the home server and the websites on it (Sites), two agent terminals on it (Agents), Cloud (a file manager over an SMB share on the server) and a Bambu 3D printer. The specs are `docs/specs/2026-11-13-binder-2-design.md`, `docs/specs/2026-11-13-binder-2.1-design.md` and `docs/specs/2026-11-13-binder-2.2-design.md` and `docs/specs/2026-11-14-binder-2.3-design.md` and `docs/specs/2026-11-14-binder-2.4-design.md` and `docs/specs/2026-11-14-binder-2.5-design.md` and `docs/specs/2026-11-15-binder-2.7-design.md` and `docs/specs/2026-11-15-binder-3.1-design.md` (the later one wins where they differ), and they win over this file. Plans live in `docs/plans/`.

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
                  SSH (cloud_fast.rs, on shell.rs), the Agents' ssh and tmux command lines (agents.rs), the
                  agent's pick-up (pickup.rs), the night queue (night.rs), the container watch (watch.rs), the event log
                  (events.rs), the morning brief (brief.rs), the weekly note (weekly.rs), the sites (sites.rs), the
                  GitHub screen's reads and writes and the agent's GitHub prompts (gh_work.rs)
crates/cli/       binder-cli: the Claude Code hook (`hook <event>`) and the detached `summarize <session>`
app/src-tauri/    Tauri app: thin commands over the core, the printer MQTT workers (printer.rs), Cloud copies
                  in flight (transfers.rs), "Open with" through Launch Services (apps.rs), the Agents'
                  pseudo-terminals (terminal.rs), updates from GitHub releases (updates.rs), the pick-up and
                  watch loops and their notifications (background.rs)
app/src/          React + TypeScript + Tailwind interface
app/dmg/          the DMG window's background (background.svg, rendered by scripts/dmg-background.swift)
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
- The Obsidian vault lives on the Raspberry Pi and is mounted on the Mac at `/Volumes/vault` over SMB. Each project has a folder, `Projects/<name>/`, holding its note `<name>.md` (written once from a template) and `Daily/YYYY-MM-DD.md` files that Binder only appends `- HH:MM · text` lines to. Notes from before 2.2 at `Projects/<name>.md` are moved into the folder unchanged, and their `## Done` lines are still read. Binder mounts the vault share itself when it drops (`shares.rs`: macOS's `mount volume` with the Keychain's password, checked every minute; the address is `vault_share` or the `vault` share beside `cloud_share`), and Done today has Connect. A folder under `/Volumes` counts only when a share is mounted on it. Nothing is written when the vault isn't mounted; lines wait in `pending_done` with their date and time. `meta.last_done` holds when a line was last written.
- Claude Code hooks find a session's project by its Local folder, then the repo's GitHub `origin`, then the repo folder's name. The app in `/Applications` points the hooks at its own `binder-cli` at start when they run another one; a dev build never does.
- Cloud is `/srv/cloud` on the server, shared as `[cloud]` (`scripts/cloud-share.sh`) and mounted at `/Volumes/cloud`. Every Cloud command refuses unless that path is a mount point, so nothing lands on the Mac's own disk. Paths never leave the share's root, and deletes go to `.trash/<id>/` beside an `.origin` file.
- **Fast path.** With a server set and a check passing (a token written over SMB and read back over SSH at `cloud_remote`, default `/srv/cloud`), folder uploads and downloads stream one tar, copies run `cp -a`, zips run `zip`/`unzip`, and sizes and free space are counted on the server. Every script is built with `remote::quote`. Without it everything runs over SMB (the `zip` crate for zips). Work writes to a `.binder-partial-<pid>-…` name and lands through the mount; partials of a dead process go to the trash as unfinished uploads.
- **More than one disk.** The Pi's SD card is mounted inside Cloud at `SD/`. Through SMB the Mac sees one device for the whole share, so a disk shows only as EXDEV on a rename: an item goes to the `.trash` of its own disk (`SD/.trash`, listed in `.trash/.disks`), and a move across disks is a copy then a remove, in the transfer tray. `lost+found` is never listed.
- GitHub sync runs `gh` with its own login on a second database connection, at start and on **Sync**. It only updates GitHub's facts on a project (URL, description, privacy, last push, language, archived), never the owner's edits, and never deletes. A GitHub project the owner deletes is remembered in `hidden_repos` and sync skips it until it's brought back.
- **Agents.** Each terminal is a pseudo-terminal running the system `ssh -tt` into a fixed tmux session on the server, `binder-claude` (Claude Code) or `binder-qwen` (`docker exec -it ollama ollama run qwen3:4b`), with a login shell after the program so the session survives. Output bytes go to xterm.js untouched; `app/src/components/XtermView.tsx` is the only file that imports xterm. Quitting ends the local `ssh` only. On the server, Ollama runs from `~/apps/ollama`, and Claude Code's `~/.claude/settings.json` skips permission prompts behind a deny list; change that list only with the owner.
- **Pick-up.** While Binder runs, 30 s after start and then every 10 minutes, `pickup::round` polls every unended run and, when none is live and Settings › Claude Code › Pick up agent tasks is on, starts the next agent task in Todo (not a night task, in a project with an agent folder, never run before), highest priority then oldest, with edits-only access. A failed run is never picked up again.
- **Night tasks.** `tasks.night` marks them (turning it on assigns the agent). Each round makes the server's queue match them: one `~/.binder-agent/night/<run>.job` per task (quoted `dir=` and `flag=` lines) beside its `<run>.prompt`, and an `agent_runs` row with `queued = 1`. The owner's crontab gets one line, `0 3 * * * "$HOME"/.binder-agent/night.sh # binder-night`, only through Settings › Claude Code › Night tasks › Set up, and Remove takes out that line alone. `night.sh` runs the jobs one at a time from 03:00 and starts none from 07:00; once a job has a `.pid` it is read like any day run.
- **In the background.** Closing the window hides it; the Dock icon brings it back and ⌘Q quits. Every 60 s, with a server set and Settings › Home server › Notify me about containers on, `watch::Watch` reads the containers and sends a Mac notification when one stops, keeps restarting or turns unhealthy and when it recovers, and after three missed checks for the server itself.
- **Interface size** is `zoom` in `config.toml` (100, 110, 125 or 150), applied to the webview at start; ⌘+ ⌘- ⌘0 step it.
- **Updates.** Tauri's updater plugin checks `latest.json` on the latest GitHub release, 5 s after start and from Settings › Binder, and installs a signed `Binder.app.tar.gz`; development builds never update. The private key is in the Keychain (service `io.github.kungie.binder.updater`, account `release`): never print or copy it. `make release` needs `build/notes.md` (the release notes) and makes `build/Binder.dmg`, `build/Binder.app.tar.gz` and `build/latest.json`. The app is signed with the self-signed identity `Binder Self-Signed` in the login Keychain (a code-signing certificate made for Binder, key never exported): it keeps the app's code identity the same across builds, so macOS keeps its permissions after an update. Never print, export or copy that key. `make install` uses it when it's there and falls back to ad hoc. The DMG window is laid out through Finder by `scripts/dmg.sh`; the first time, macOS asks whether the terminal may control Finder. Publish them with `gh release create v<version>` only after the owner says yes. The download badge leaves the update files out.
- **Event log.** `events` holds container alerts (live, and after a gap of more than 5 minutes from Docker's own start and stop times), server restarts (from the uptime), print outcomes, the owner's restarts and issues brought in. Events are logged whenever a server is set, whatever the notification switches say; they are kept 35 days. `meta.watch_last` and `meta.server_boot` remember the last good check and the boot time.
- **Morning brief.** Overview shows `brief::items` since the last Dismiss (`meta.brief_dismissed`) or a day, from 05:00 to noon until dismissed, only when there is something.
- **GitHub issues.** Each sync brings open issues assigned to the owner, in repos that are projects, in as tasks once (`tasks.issue_repo/number/url`); a task whose issue left the list moves to Done once (`tasks.issue_listed`), and only if its issue was on the list; a full page of 200 results closes nothing, a renamed repo carries its issue tasks, and syncs run one at a time; a deleted one is remembered in `hidden_issues`. A failed issue read changes nothing. Projects get `open_prs` and `ci` (the default branch's status rollup) from one GraphQL call.
- **GitHub screen.** Lists, details and the owner's actions go through the Mac's `gh`, every value as its own argument, only for repos that are projects. The agent's review runs read-only (no Bash, Edit or Write) on the server from a diff the Mac fetched and waits as a draft; Fix with the agent is offered only on the owner's own pull requests; Work on it and Fix push from the server, so they need `gh` signed in there and the project's agent folder. A start that fails gives the task back (a task made only for the run is canceled), and a review that ended badly says so and stops waiting.
- **Containers.** The Server screen shows a container's last 200 log lines and restarts it after a confirmation; the restart is logged first so the watch stays quiet about it.
- **Overview layout.** `overview` in `config.toml`: two columns of widget ids, `split` (the left column's width in percent, 30 to 70) and `heights` (pixels, for widgets the owner sized). Edit mode drags widgets, sizes them from their bottom edge and the columns from the gutter; arrow keys do the same.
- **Sites.** Folders in `~/sites` with a compose file, and every compose project with a `cloudflared` container wherever it lives, so a newly hosted site shows up by itself; containers join a site by folder or compose project. The address comes from `site_addresses` in `config.toml`, else only from `hostname:` lines of `cloudflared/config.yml`, else (a tunnel set up on Cloudflare's side) the first hostname in the tunnel log's last line that names one; all filtered on the server. The check runs `/usr/bin/curl` on the Mac, https only. Every preview is a still snapshot: fetched from the Mac (https, at most 3 MB), scripts and refreshes removed, animations and transitions off, in a frame that runs no scripts; live pages made the screen stutter. That is why the CSP lets images, styles and fonts load over https. Refresh reads the snapshots again.
- **Repeating tasks.** `tasks.repeats` (v10): daily, weekdays, weekly, monthly or yearly. Moving one to Done (not Canceled) makes the next copy in `update_task`: same fields, Todo, due on the rule's next day after the old due date (or today), never on or before today; months count from the old due day, so a 31st falls on a shorter month's last day. The finished one gives up its rule, so reopening and finishing it again makes no second copy. `app/src/lib/repeat.ts` mirrors the rule for the fake.
- **Weekly note.** After an ISO week ends, `Weekly/<year>-W<week>.md` in the vault, up to four missed weeks; an existing file is never written over, and a write cut short removes its file. Tasks keep a `closed` time.
- The website in `site/` runs on the home server in `~/sites/binder-site` (Caddy plus its own Cloudflare Tunnel, like the other sites there). To publish a change, `rsync -a --delete site/ <server>:sites/binder-site/public/`; Caddy serves it at once.

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

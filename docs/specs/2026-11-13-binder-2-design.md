# Binder 2: a personal dashboard

Status: draft, waiting for review. Replaces the Swift app (1.x) in this repo. Ships as 2.0.0.

Binder 2 is a desktop app for one person: its owner. It shows what runs on the home server, lists the owner's projects, devices and 3D printer, and keeps one task table for the owner and for a Claude Code agent on the home server. Each project has a note in Obsidian that collects short "done" lines: the key decisions, as long-term memory. It has nothing in common with 1.x except the name and the repo.

**Language:** everything in the repo and the app is English.

## 1. Shell and stack

- **Tauri 2.** A Rust core and a React + TypeScript UI with Tailwind, built with Vite. A normal macOS window with a sidebar; no menu bar item, no onboarding, no mascot.
- **Panels:** PanelUI's web components (the shadcn-based registry at panelui.dev), copied into the project as source and themed with our tokens.
- **Layout:**
  ```
  Cargo.toml              workspace
  crates/core/            library: database, models, ssh, bambu, obsidian, done log
  crates/cli/             binary binder-cli, called by Claude Code hooks
  app/src-tauri/          Tauri app: commands that call crates/core
  app/src/                React UI
  ```
- **Data:**
  - Tasks, projects, devices and agent runs live in SQLite (`rusqlite`) at `~/Library/Application Support/io.github.kungie.binder/binder.db`.
  - Settings live in `config.toml` beside it: the Obsidian vault path, SSH host aliases, the printer's address and serial.
  - The printer's access code lives in the login Keychain, never in a file.
  - Nothing personal (addresses, host names, serials) is ever committed; the repo stays public.
- **Install:** `make install` builds the app and copies `Binder.app` to `/Applications`. No GitHub releases at first.
- **Tools:** Rust 1.99, Node 26 with npm, the Command Line Tools. No Xcode needed.

## 2. Look

Extremely minimal, black and white, dark only. Panels follow PanelUI's dashboard blocks; the chrome follows Orca (tight sidebar, quiet status bar) and Linear (dense tables, keyboard first, command palette).

| Token | Hex | Use |
|---|---|---|
| bg | `#000000` | window background |
| panel | `#0A0A0A` | panels, table |
| raised | `#141414` | selected row, active nav item, menus |
| line | `#1C1C1C` | hairlines, panel borders |
| line-strong | `#2A2A2A` | hover borders, inputs |
| text | `#EDEDED` | primary text |
| text-2 | `#A1A1A1` | secondary text |
| text-3 | `#6B6B6B` | labels, ids, meta |
| primary | `#EDEDED` fill, `#000000` text | the one primary button per view |

- **No colour at all.** State shows through shape and weight: Linear-style status circles (empty, dashed, half, full, check), filled or hollow dots for online and offline, strike-through for done.
- **Type:** Inter for UI at 13 px (12 px in tables), weights 400 and 500 only. Geist Mono for numbers, ids, addresses and logs.
- **Shape:** 6 px radius on controls, 8 px on panels, 1 px hairlines, no shadows except on menus and the command palette.
- **Motion:** 120 ms fades and slides only; none with reduced motion.
- **Keyboard:** ⌘K palette (go to, create, run). In tables: `j`/`k` move, `Enter` opens, `c` creates, `s` sets status, `a` assigns, `x` selects.

## 3. Screens

The sidebar holds Overview, Tasks, Projects, Devices, Server and Printer, then the active projects.

- **Overview:** a panel grid.
  - Panels: home server, printer, done today, and the task table filtered to open tasks.
  - Each panel links to its screen.
- **Tasks:** the main table.
  - Columns: status, id, title, project, assignee, priority, due date.
  - Views: All, Mine, Agent, and one per project.
  - A task opens in a side sheet with its notes (Markdown) and, for agent tasks, its run.
- **Projects:**
  - Each project shows its tasks, its done log (read from its Obsidian note) and an **Open note** button.
  - Its repo: branch, uncommitted changes, commits ahead of and behind its upstream, and the last commit's subject and date, read with `git` from the repo folder.
- **Devices:**
  - The owner's machines: this Mac, the Raspberry Pi, the printer and any added by hand.
  - Each shows a name, kind, address and online state, plus a free-text note.
- **Server:**
  - What runs on the home server: Docker containers (name, image, state, uptime) and running systemd services.
  - Load, memory, disk and temperature.
- **Printer:**
  - The Bambu printer's state: job name, progress, layer, time left, nozzle and bed temperatures.
  - Pause, resume and stop.

## 4. Tasks

- **Fields:**
  - id (project prefix and number, for example `DAW-21`; `TSK-n` without a project), title, notes.
  - status: backlog, todo, in progress, done or canceled.
  - priority: none, low, medium, high or urgent.
  - project, assignee (`me` or `agent`), due date, created and updated times.
- **Marking a task done** offers one line for the project's done log, prefilled with the title. Empty skips it.

## 5. Projects and Obsidian

- A project has a name, a short prefix, a local repo folder, its note path and a state (active, paused or done). Projects are added by hand, by picking a repo folder.
- **Start project** creates `<vault>/Projects/<name>.md` if it doesn't exist and opens it in Obsidian with `obsidian://open?path=…`.
  - The vault lives on the Raspberry Pi and reaches the Mac as an SMB share (section 5a). Its path defaults to `/Volumes/vault`, changeable in Settings.
  - When the share isn't mounted, Binder shows "Vault not connected" and **Start project** waits. Nothing is ever written to a local copy.
  - The note starts with frontmatter (`project`, `repo`, `started`), a title and an empty `## Done` section.
- **The done log** is the `## Done` section, one line per decision, newest last: `- 2026-11-13 · Chose Tauri over a pure Rust UI.`
  - Lines are short: one sentence, no detail, no code.
  - Binder only ever appends to `## Done`. Everything else in the note belongs to the owner.

## 5a. The vault on the home server

The owner's notes live on the Pi, not on the Mac.
- **On the Pi:**
  - The folder `/srv/vault` belongs to the owner's Pi user and is shared by Samba as `vault`.
  - Only that user may connect. No guests, SMB3 only, on the home network's interface only.
- **The Samba password** is set by the owner on the Pi (`sudo smbpasswd -a <user>`). Binder never sees it.
- **On the Mac:**
  - Finder connects to `smb://<pi>/vault` once and saves the password in the Keychain.
  - The share goes into Login Items, so it mounts at every login.
  - Obsidian opens `/Volumes/vault` as its vault, and Binder writes there.
- Setting this up is a one-time step on the Pi, done with the owner present. Binder itself installs nothing there.

## 6. Done lines from Claude Code sessions

- **Settings › Claude Code › Connect** adds three hooks to `~/.claude/settings.json`: `Stop`, `PreCompact` and `SessionEnd`. Each runs `binder-cli hook <event>`. Disconnect removes only those. Other hooks are never touched.
- `binder-cli` reads the hook's JSON from stdin, matches the session's `cwd` to a project's repo folder, and does nothing for folders that aren't a project.
- **Batching:**
  - `Stop` only marks the session.
  - Once 4,000 characters of new talk wait, or on `PreCompact` and `SessionEnd`, the new talk goes to the model once.
  - A cursor per session in the database makes sure each part of a transcript is read once.
- **The model call:**
  - `claude -p --model haiku`, bare, on the owner's own plan, with tools, MCP, settings and slash commands off and no session saved. It runs from an empty scratch folder.
  - Its input is the new talk, with secrets redacted, and the last 20 lines of the project's done log.
  - It returns at most 3 new key decisions as JSON, or none. Lines that repeat the log are dropped.
- New lines are written straight to the note, without approval. If the vault isn't mounted, a line waits in the database and is written the next time it is. The app's Projects screen and Overview show them, and each can be deleted there.

## 7. Home server

- Binder reaches the Pi with the system's `ssh`, using the host set in Settings (an alias from `~/.ssh/config` or `user@address`), with `BatchMode=yes` and a 5 second connect timeout. It never reads or writes keys and installs nothing on the Pi.
- **What it runs, all read only:**
  - `docker ps -a --format '{{json .}}'`
  - `systemctl list-units --type=service --state=running --no-legend --plain`
  - `/proc/loadavg`, `/proc/meminfo`, `df -B1 /`
  - `/sys/class/thermal/thermal_zone0/temp`
- **How often:** every 30 seconds while the window is in front, and not at all while it's hidden.
- **Online state** of every device is a TCP connect to its port 22 (or a port set per device) with a 2 second timeout.

## 8. Agent tasks

- A task assigned to `agent` gets a **Run** button. A run needs a folder on the Pi (default: the project's `agent_dir` setting, else `~`) and an access level:
  - **Edits only** (default): `--permission-mode acceptEdits`.
  - **Full access**: `--dangerously-skip-permissions`, chosen per task, with a one-line warning.
- **How a run works:**
  - Binder copies the prompt (title and notes) to `~/.binder-agent/<run>.prompt` with `scp`.
  - It starts `claude -p` in that folder with `setsid nohup`, so the run outlives the SSH connection, with the prompt on stdin. Output goes to `<run>.log`, the process id to `<run>.pid`, and the exit code to `<run>.exit`. (No tmux: the Pi doesn't have it, and nothing here needs a terminal to attach to.)
  - Claude Code on the Pi must already be signed in. Binder checks this once with `claude --version` and says so if it fails.
- **Status:** while a run is live, Binder polls it every 10 seconds (whether the process is alive, the exit file, and the last 40 lines of the log). The task's status follows: in progress while running, then done on exit code 0, else back to todo with the error shown.
- **Stop** ends the run's process group. Logs stay on the Pi; the run record (start, end, exit code, last lines) stays in the database.

## 9. Printer

- Bambu Lab in LAN mode, over MQTT with TLS on port 8883. User `bblp`, password the access code.
- On the first connection Binder pins the printer's self-signed certificate and refuses a different one after that.
- **Reading:** Binder subscribes to `device/<serial>/report` and sends `pushall` on connect.
- **Fields shown:** `gcode_state`, `mc_percent`, `mc_remaining_time`, `layer_num`, `total_layer_num`, `nozzle_temper`, `bed_temper`, `subtask_name`.
- **Commands:** pause, resume and stop go to `device/<serial>/request`. Stop asks for confirmation.
- One or more printers can be added; Overview shows the first one that is printing.

## 10. Repo

- Before anything is removed, a branch `legacy-swift` is created at `v1.2.0`. The 1.x releases stay on GitHub.
- On `main`, one commit removes the Swift package, its tests, scripts, design files and docs. The README and CLAUDE.md are rewritten for Binder 2.
- Old 1.x installs don't break: their updater ignores releases without a signed `Binder.zip`, and Binder 2 publishes none.
- Nothing is pushed without the owner's go-ahead.

## 11. Build order

Each part gets its own plan and ships working on its own:

1. Shell, theme, tasks and projects with Obsidian notes, Overview with the tasks panel.
2. Server and Devices.
3. Agent tasks on the Pi.
4. Bambu printer.
5. Done lines from Claude Code sessions.

## Testing

- `crates/core` is tested with `cargo test`. Each of these is tested with fixtures, never live hardware:
  - the database layer;
  - the parsers for `docker ps`, `systemctl`, `/proc` and Bambu reports;
  - the `## Done` writer, which leaves the rest of a note byte for byte;
  - hook install and remove, which leave other hooks untouched;
  - the batching cursor;
  - the agent run state machine.
- SSH, MQTT and `claude` are behind traits, so tests use fakes.
- The UI gets component tests (Vitest) for the task table's keyboard handling and status display.

## Not in 2.0

- The printer camera, notifications, light theme, mobile access, sync, multiple users.
- Writing anything on the home server other than the agent's run files.
- Memory cards for agents (the 1.x feature).

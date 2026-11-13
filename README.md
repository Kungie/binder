# Binder

Binder is a small desktop app for running one developer's workshop. It keeps a task board for you and for a Claude Code agent on your home server, brings in your GitHub repos as projects, lists your devices and 3D printer, shows what runs on the server and gives you a small file manager for a folder on it. Every project has a note in Obsidian that collects the key decisions as short dated lines.

It's mostly black and white, with a few colours that each mean something, keyboard first, and built for one person. The core is Rust, the window is Tauri with a React interface, and the data stays on your own machines.

## What it does

- **Tasks:** a board with Backlog, Todo, In progress and Done columns, or the same tasks as a keyboard-first table. Cards show two lines of notes and chips for status, due date, project and the agent. Drag a card to change its status or its place. A task assigned to the agent gets a **Run** button that starts `claude -p` on the home server over SSH, in the background, and follows it until it ends; an orange dot marks it when the run ends.
- **Projects:** your GitHub repos come in on their own through the `gh` CLI and its login, at start and with **Sync**. Your own edits stay, and nothing is deleted: a repo that's archived or gone moves to Archived. Projects that aren't on GitHub can be added by hand. A project can have a local folder (branch, changes, ahead and behind, last commit) and an Obsidian note whose `## Done` section is its long-term memory.
- **Cloud:** a file manager over a folder on the home server, shared over SMB. Browse, upload large files with progress, save to the Mac, rename, move, and delete into a trash you can restore from. Your phone reaches the same folder from the Files app.
- **Server:** containers, running services, load, memory, disk and temperature, read over SSH every 30 seconds. Read only.
- **Devices:** your machines and whether each one answers.
- **Printer:** a Bambu Lab printer in LAN mode over MQTT: job, progress, temperatures, pause, resume and stop.
- **Claude Code:** Settings › Claude Code adds three hooks. When a session in a project's repo has enough new talk, Haiku picks out up to three key decisions for the project's done log, after secrets are masked.

## Setup

- **Server:** Settings › Home server takes an SSH target, `user@host` or a `Host` from `~/.ssh/config`. Binder uses your keys through the system `ssh` and never stores a password.
- **Vault:** the Obsidian vault can live anywhere that mounts as a folder, such as an SMB share. Binder writes only when it's mounted; done lines wait otherwise.
- **GitHub:** install the GitHub CLI and run `gh auth login` once. Binder never reads or stores a token.
- **Cloud:** on the server, run `sudo scripts/cloud-share.sh <user>` once. It creates `/srv/cloud` and shares it as `[cloud]`. On the Mac, connect to `smb://<server>/cloud` in Finder, save the password in the Keychain, add the share to Login Items and put the same address in Settings › Cloud. On a phone, use Files › Connect to Server with the server's Tailscale address.
- **Printer:** the access code goes to the login Keychain, never to a file.
- macOS asks once for Local Network access. Allow it, or Binder can't reach addresses on your LAN.

Settings live in `~/Library/Application Support/io.github.kungie.binder/config.toml`, data in `binder.db` beside it.

## Build

You need Rust, Node with npm, and the Xcode Command Line Tools.

```sh
make dev      # run the app with hot reload
make test     # Rust and interface tests
make build    # build Binder.app, with binder-cli inside
make install  # build, then replace /Applications/Binder.app
make preview  # the interface alone in a browser, with demo data
```

The 1.x app, a Swift menu bar app that gave AI agents a memory, lives on the `legacy-swift` branch and in the 1.x releases.

## License

AGPL-3.0.

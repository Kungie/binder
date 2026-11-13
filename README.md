# Binder

Binder is a small desktop app for running one developer's workshop. It keeps a task table for you and for a Claude Code agent on your home server, lists your projects, devices and 3D printer, and shows what runs on the server. Every project has a note in Obsidian that collects the key decisions as short dated lines.

It's black and white, keyboard first, and built for one person. The core is Rust, the window is Tauri with a React interface, and the data stays on your own machines.

## What it does

- **Tasks:** one table for you and the agent. A task assigned to the agent gets a **Run** button that starts `claude -p` on the home server over SSH, in the background, and follows it until it ends.
- **Projects:** each has a repo folder (branch, changes, ahead and behind, last commit) and an Obsidian note whose `## Done` section is the project's long-term memory.
- **Server:** containers, running services, load, memory, disk and temperature, read over SSH every 30 seconds. Read only.
- **Devices:** your machines and whether each one answers.
- **Printer:** a Bambu Lab printer in LAN mode over MQTT: job, progress, temperatures, pause, resume and stop.
- **Claude Code:** Settings › Claude Code adds three hooks. When a session in a project's repo has enough new talk, Haiku picks out up to three key decisions for the project's done log, after secrets are masked.

## Setup

- **Server:** Settings › Home server takes an SSH target, `user@host` or a `Host` from `~/.ssh/config`. Binder uses your keys through the system `ssh` and never stores a password.
- **Vault:** the Obsidian vault can live anywhere that mounts as a folder, such as an SMB share. Binder writes only when it's mounted; done lines wait otherwise.
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

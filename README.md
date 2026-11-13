# Binder

Binder is a small desktop app for running one developer's workshop. It keeps a task table for you and for a Claude Code agent on your home server, lists your projects, devices and 3D printer, and shows what runs on the server. Every project has a note in Obsidian that collects the key decisions as short dated lines.

It's black and white, keyboard first, and built for one person. The core is Rust, the window is Tauri with a React interface, and the data stays on your own machines.

## Build

You need Rust, Node with npm, and the Xcode Command Line Tools.

```sh
make dev      # run the app with hot reload
make test     # Rust and interface tests
make build    # build Binder.app
make preview  # the interface alone in a browser, with demo data
```

The 1.x app, a Swift menu bar app that gave AI agents a memory, lives on the `legacy-swift` branch and in the 1.x releases.

## License

AGPL-3.0.

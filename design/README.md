# Design

**Chosen direction: C · Phosphor** (`phosphor.dc.html`). It has a black CRT ground, an amber phosphor accent and cream text, and it is dark only. The colour tokens and type are in `../CLAUDE.md`.

The canvas with all three directions: https://claude.ai/artifact/TA4eWJEfD6Fi6EJ4oeaxjW

What each direction shows:
- **The mascot.** Binder is a ring binder with a glass dome on its head and a brain inside. Its face is in the label window.
- **The app icon** and three menu bar states: idle, items to review, and handoff soon.
- **The menu bar panel.** It shows how much of the session's context is used, offers "Hand off now", and lists what was caught this session with Keep, Edit and Drop.
- **The main window at first launch.** Binder shows what it found on this Mac and asks only for what it can't guess.

`flow/` holds the v1 screens, in the order a new user sees them:
- `install`: the DMG window
- `boot`: animated, with a replay button
- `permissions`
- `look-around`: animated, and the homelab question can be answered
- `about-you`
- `vault`
- `connect`
- `menu-bar`: interactive. Keep sucks a card into the brain, Drop shreds it.
- `library`: the main window with a project note

The spec that goes with them is `../docs/specs/2026-10-26-binder-v1-design.md`.

`explorations/` keeps the two directions that weren't picked: A · Signal (signal red) and B · Oxblood (deep red with cream paper).

The `.dc.html` files are Claude Design canvas sources. Opened directly in a browser they render as plain HTML. They load their fonts from Google Fonts and need no other runtime to be readable.

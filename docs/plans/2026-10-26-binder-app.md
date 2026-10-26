# Binder app · implementation plan (plan 2 of 2)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the macOS app on top of `BinderCore`: onboarding with all six steps, the menu bar panel with keep and drop animations, the main window, background observing and handoffs, and a DMG.

**Architecture:**
- A new library target, `BinderUI`, holds every view and view model, so the logic is testable.
- The `BinderApp` executable target is only `main.swift`. It is bundled as `Binder.app/Contents/MacOS/Binder`.
- The `binder` CLI ships inside the bundle at `Contents/Helpers/binder`. It can't sit next to the app binary, because `Binder` and `binder` collide on a case-insensitive disk. The hooks point at this path.
- The menu bar follows Dawn's approach: an `NSStatusItem` and a borderless `NSPanel` that hosts SwiftUI. This avoids `MenuBarExtra` and its glass frame and fixed size.
- `Binder snapshot <dir>` renders screens with demo data to PNG files for visual checks.

**Tech stack:** SwiftUI and AppKit on macOS 14+, plus UserNotifications, ServiceManagement, CoreText and BinderCore. Tests use Swift Testing through `make test`.

**Spec:** `docs/specs/2026-10-26-binder-v1-design.md` §3 (journey), §3.8–3.9 (menu bar, main window), §9 (handoff), §10–11. The design is in `design/flow/*.dc.html` (Phosphor). Core context is in `CLAUDE.md`.

## Global constraints

- All of plan 1's constraints still hold: no third-party dependencies, English everywhere, `make test`, and secrets never stored.
- **Phosphor tokens only:**

  | Token | Hex | Token | Hex |
  |---|---|---|---|
  | black | `#050303` | black2 | `#0E0908` |
  | line | `#2A1B19` | cream | `#F3E9D6` |
  | cream2 | `#E6D9BE` | sand | `#D2C09B` |
  | dim | `#A09189` | amber | `#F2A60C` |
  | amberDeep | `#8F5A00` | gyri | `#5A3A00` |
  | ink | `#160D0B` | | |

  Red `#E2231A` is never used in the app UI.
- **Type:** Archivo at 125 % width, 800–900 (titles, buttons). VT323 for screen readouts. Space Grotesk for body text. Space Mono Bold, uppercase, for labels. The fonts are copied from Dawn's `Resources/Fonts` (all under the SIL OFL).
- **Shape:** dark only, flat, corner radius 0–2 pt (the app icon is the exception). Screen panels get scanlines and an amber glow.
- **Primary buttons:** amber fill, ink text and a hard 3 pt `amberDeep` offset shadow.
- **Text:** at least 10.5 pt and WCAG AA contrast.
- **Motion:** Reduce Motion replaces each animation with a 150 ms fade.
- **Copy:** use the exact strings from the design artboards. They are short, have no exclamation marks and no em dashes.
- **Vault location:** a shared pointer at `~/Library/Application Support/Binder/config.json` (`{"vault": "<path>"}`). The app writes it. The CLI reads it after `BINDER_VAULT` and before the `~/Binder` default.
- **Menu bar panel:** 350 pt wide. Setup window: 880 × 600 pt or larger. Main window: 1240 × 820 pt by default.

## Review focus

1. **The vault is moved into an Obsidian folder from the app.** The hooks must find it afterwards. Tested in Task 1: `cliPathsFollowVaultPointer`.
2. **Proposals change on disk while the panel is open** (capture adds one mid-animation, or the CLI keeps one). The list updates without crashing and never acts on a stale id. Tested in Task 3: `keepOfVanishedProposalIsIgnored`, `inboxChangeRefreshes`.
3. **No session file, a stale one, or one with no reading.** The panel shows no session card, and doesn't show 0 % as if it were real. Tested in Task 3: `staleSessionIsHidden`, `sessionWithoutReadingShowsNoMeter`.
4. **The handoff warning fires again and again for the same session.** One notification per session crossing. Tested in Task 7: `notifiesOncePerSession`.
5. **Onboarding quits halfway.** The next launch resumes setup and doesn't double-install hooks or overwrite `me.md`. Tested in Task 5: `resumeAfterQuit`, `aboutYouMergesIntoExistingMe`.

---

### Task 1: Core additions the app needs

**Files:**
- Create: `Sources/BinderCore/VaultLocation.swift`, `Sources/BinderCore/Profile.swift`, `Sources/BinderCore/Memory.swift`
- Modify: `Sources/BinderCore/BinderState.swift` (adds `onboarded: Bool = false`), `Sources/BinderCore/Capture.swift` (`SessionStatus.transcriptPath: String?`, filled from the input), `Sources/BinderCLI/Commands.swift` (`Paths.current()` uses `VaultLocation`)
- Test: `Tests/BinderCoreTests/VaultLocationTests.swift`, `ProfileTests.swift`, `MemoryTests.swift`

**Interfaces:**
- **`VaultLocation`:**
  - `static func configURL(home: URL) -> URL` (`<home>/Library/Application Support/Binder/config.json`)
  - `static func load(home: URL) -> URL?`
  - `static func save(_ root: URL, home: URL) throws`
  - `static func resolve(env: [String: String], home: URL) -> URL`, in the order `BINDER_VAULT`, then the pointer, then `<home>/Binder`
- **`Profile: Codable, Equatable`** has `name`, `role`, `timeZone`, `language`, `workingStyle: [String]`, `taste: [String]`, `avoid: [String]` and `nowAndNext`.
- **`MeNote.render(_ p: Profile, into existing: Note?) -> Note`.** It writes the user's part: `- name:`, `- does:`, `- time zone:`, `- language:`, then `## How to work with me` (list), `## Taste` (`- likes: a, b`, `- rather not: c`) and `## Now and next` (paragraph). It replaces only those lines and sections, and keeps the observed block, decisions and any other text.
- **`MeNote.profile(from note: Note) -> Profile`** reads them back.
- **`TasteDetector.languages(vault: Vault, limit: Int = 6) -> [String]`** ranks the `- languages:` values across `projects/*.md` by how many projects use each one.
- **`Memory.add(_ text: String, to target: String, vault: Vault, now: Date) throws -> Bool`** appends `"<date> · <text> · written by you"`. It refuses secrets and empty text, and returns false when it refuses.

- [ ] **Step 1:** Write these tests:
  - `resolveOrder`: env wins, then the pointer, then the default.
  - `pointerRoundTrips`
  - `cliPathsFollowVaultPointer`: run the built `binder doctor` with `BINDER_HOME` (no `BINDER_VAULT`) after saving a pointer; the output names the pointed folder.
  - `meNoteRendersSections`
  - `meNoteKeepsObservedAndDecisions`
  - `profileRoundTrips`
  - `tasteRanksLanguagesByProjects`
  - `memoryAddsWrittenByYou`
  - `memoryRefusesSecret`
  - `onboardedDefaultsFalse`
  - `sessionStatusKeepsTranscriptPath`
- [ ] **Step 2:** Run `make test FILTER="VaultLocation|Profile|Memory|Capture"`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the same command. Expected: PASS. Then run `make test` and see everything pass.
- [ ] **Step 5:** Commit with `feat(core): vault pointer, profile, manual memories, onboarding flag`.

### Task 2: App targets, the Phosphor theme, the mascot, and snapshots

**Files:**
- Modify: `Package.swift`. Add the targets `BinderUI` (depends on `BinderCore`, with `resources: [.copy("Resources/Fonts")]`) and `.executableTarget(name: "BinderApp", dependencies: ["BinderUI"])`, and make the tests depend on `BinderUI`.
- Create:
  - `Sources/BinderUI/Theme.swift`: `Phosphor` colours, `Fonts.register()`, `Fonts.archivo/vt/grotesk/mono`
  - `Sources/BinderUI/Components.swift`: `PrimaryButtonStyle`, `GhostButtonStyle`, `ScreenPanel`, `Scanlines`, `SectionLabel`, `Chip`, `StepRail`, `WindowChrome`
  - `Sources/BinderUI/Mascot.swift`
  - `Sources/BinderUI/Snapshot.swift`
  - `Sources/BinderUI/Resources/Fonts/*` (copied from Dawn)
  - `Sources/BinderApp/main.swift`
- Test: `Tests/BinderCoreTests/UI/MascotTests.swift`

**Interfaces:**
- **`Mascot(mood: Mood, size: CGFloat)`**, where `enum Mood { idle, booting(progress: Double), scanning, happy, writing, plugging, holdingKey }`. It draws the artboard SVG in SwiftUI `Canvas`, on the same 240 × 280 grid:
  - legs, arms and the paper edge
  - the cream body with a sand spine and three ink rings
  - the black face window with amber eyes
  - the dark dome with five amber circles forming the brain, `gyri` lines and scanlines
  - the cream rim and the amber collar

  `booting` fades the brain in by `progress`. `scanning` offsets the eyes by a `TimelineView` sine. The other moods change the eyes, mouth and props as on the artboards.
- **`MascotHead(size:)`** is the dome, brain and collar only, for icons.
- **`Snapshot.render<V: View>(_ view: V, size: CGSize, to url: URL) throws`** renders at 2x through `ImageRenderer`. `BinderApp snapshot <dir>` writes `mascot-moods.png` and `components.png`.
- **`BinderApp` `main.swift`:** `snapshot <dir>` calls `Snapshot.all(to:)`, and anything else starts `NSApplication` with an `AppDelegate`, a placeholder status item at this stage.

- [ ] **Step 1:** Write these tests:
  - `mascotRendersEveryMood`: each mood renders to a non-empty `NSImage` of the expected pixel size.
  - `fontsRegisterFromBundle`: after `Fonts.register()`, `NSFont(name: "VT323-Regular", size: 12) != nil`.
- [ ] **Step 2:** Run `make test FILTER=MascotTests`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `make test FILTER=MascotTests`. Expected: PASS. Then run `swift build && .build/debug/BinderApp snapshot /tmp/bsnap`, open `mascot-moods.png` and compare it with `design/flow/boot.dc.html`'s mascot.
- [ ] **Step 5:** Commit with `feat(app): phosphor theme, mascot, snapshot command`.

### Task 3: The app model and the inbox watcher

**Files:**
- Create: `Sources/BinderUI/AppModel.swift`, `Sources/BinderUI/DirectoryWatcher.swift`
- Test: `Tests/BinderCoreTests/UI/AppModelTests.swift`

**Interfaces:**
- **`@MainActor final class AppModel: ObservableObject`**, `init(vault: Vault, home: URL, now: @escaping () -> Date = Date.init)`:
  - **Published:** `proposals: [Proposal]`, `session: SessionStatus?`, `memories: Int` (the number of decision lines across the vault), `state: BinderState`, `phases: [String: CardPhase]`, where `enum CardPhase { idle, absorbing, shredding, editing(String) }`.
  - **Actions:**
    - `func refresh()`
    - `func keep(_ id: String, editedText: String? = nil)`, which sets `.absorbing`, then after `CardPhase.absorbDuration` (0.7 s) calls `ProposalStore.keep` and refreshes
    - `func drop(_ id: String)`, which sets `.shredding`, then after 1.1 s calls `ProposalStore.drop`
    - `func keepAll()`, staggered 0.22 s apart
    - `func edit(_ id: String)`
  - **Computed:** `var handoffSuggested: Bool` (fraction ≥ `state.clampedThreshold`) and `var pendingCount: Int`.
  - **The session shown** is the newest `SessionStatus` updated within the last 2 hours. An older one is stale.
  - **`DirectoryWatcher(url:onChange:)`** wraps `DispatchSource.makeFileSystemObjectSource` on the folder, debounced to 200 ms. `AppModel.startWatching()` watches `.binder/inbox` and `.binder/sessions`.
- Tests can shrink the delays through `static var timeScale: Double = 1`.

- [ ] **Step 1:** Write these tests:
  - `refreshLoadsPendingAndSession`
  - `keepAppendsDecisionAfterAbsorb`
  - `dropRemembersAfterShred`
  - `keepAllStaggers`
  - `keepOfVanishedProposalIsIgnored` (the file is deleted before the delay ends)
  - `inboxChangeRefreshes` (the watcher fires and the list updates)
  - `staleSessionIsHidden`
  - `sessionWithoutReadingShowsNoMeter`
  - `handoffSuggestedAtThreshold`
- [ ] **Step 2:** Run `make test FILTER=AppModelTests`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `make test FILTER=AppModelTests`. Expected: PASS.
- [ ] **Step 5:** Commit with `feat(app): app model with inbox and session watching`.

### Task 4: The menu bar (icon, panel, keep and drop animations)

**Files:**
- Create: `Sources/BinderUI/MenuBar/MenuBarIcon.swift`, `StatusItemController.swift`, `PanelView.swift`, `ProposalCard.swift`, `Absorb.swift`, `Shred.swift`
- Modify: `Sources/BinderUI/Snapshot.swift` (panel states), `Sources/BinderApp/main.swift`
- Test: `Tests/BinderCoreTests/UI/PanelTests.swift`

**Interfaces:**
- **`MenuBarIcon.image(state: MenuState) -> NSImage`**: an 18 pt template image (dome outline, brain dot, collar bar). `enum MenuState { idle, review(Int), handoffSoon, observing }`. With `review`, the count is drawn as the title in VT323.
- **`StatusItemController(model:)`** is Dawn's `StatusPanelController` pattern: a borderless panel pinned under the item that closes on an outside click or Escape.
- **`PanelView(model:)`**, 350 pt wide, laid out as on `design/flow/menu-bar.dc.html`:
  1. A header with `BrainTarget` (the gulp animation scales it 1→1.22→1 over 0.5 s when `model.memories` goes up) and `"<n> MEMORIES · <k> TO REVIEW"`.
  2. The session card (project · agent label, a big VT323 percentage, a stepped amber bar, "Getting long." with Hand off now / Not yet when `handoffSuggested`).
  3. "CAUGHT THIS SESSION" with the `ProposalCard`s.
  4. "Keep all n".
  5. The empty state: "ALL FILED. / Nothing to review. I'll call you when there is."
  6. The footer: Open Binder · Add a memory.
- **`ProposalCard`** shows the tag (`FROM A COMMAND · <evidence>`, `SEEN ON THIS MAC · <evidence>` or `FROM THE CHAT`, with `· PROBABLY A ONE-OFF` added for `confidence == .low`), the text, and Keep / Edit / Drop. In `.editing` it shows a `TextField`, and Return keeps the edited text.
- **`Absorb`:** a modifier that animates the card toward the brain's anchor (`anchorPreference` of the `BrainTarget` bounds). The card scales to 0.97 × 0.9 at 22 %, then moves to the target at scale 0.03, skewed −18°, with opacity 0.15, over 0.7 s with `.timingCurve(0.55, 0, 0.85, 0.35)`.
- **`Shred`:** a 4 pt amber bite bar flickers twice. Eight vertical strips of the same card render (each the card masked to 1/8 of its width) fall 150 pt and rotate ±4…12°, with delays 0.12 + ((k × 37) % 8) × 0.035 s and 0.85 s each.
- **Reduce Motion** (`accessibilityReduceMotion`): both become a 0.15 s opacity fade.

- [ ] **Step 1:** Write these tests:
  - `iconStatesAreTemplates18pt`
  - `tagTextForEachSource`
  - `shredDelaysMatchDesign` (the 8 delays equal the artboard's formula)
  - `panelSnapshotsRender` (the empty, review, handoff and edit states each render non-empty)
- [ ] **Step 2:** Run `make test FILTER=PanelTests`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `make test FILTER=PanelTests`. Expected: PASS. Then run `.build/debug/BinderApp snapshot /tmp/bsnap` and compare `panel-review.png` and `panel-empty.png` with `menu-bar.dc.html`.
- [ ] **Step 5:** Commit with `feat(app): menu bar panel with keep and drop animations`.

### Task 5: Onboarding

**Files:**
- Create: `Sources/BinderUI/Onboarding/OnboardingModel.swift`, `OnboardingWindow.swift`, `BootView.swift`, `PermissionsView.swift`, `LookAroundView.swift`, `AboutYouView.swift`, `VaultView.swift`, `ConnectView.swift`, `Sources/BinderUI/System/Permissions.swift`
- Test: `Tests/BinderCoreTests/UI/OnboardingModelTests.swift`

**Interfaces:**
- **`enum OnboardingStep: Int, CaseIterable { boot, permissions, lookAround, aboutYou, vault, connect }`**. The rail labels are HELLO, PERMISSIONS, LOOK AROUND, ABOUT YOU, YOUR BINDER and CONNECT.
- **`@MainActor final class OnboardingModel: ObservableObject`**, `init(vault:home:runner:permissions:installer:)`:
  - **Published:** `step`, `permissionStatus: [Permission: PermissionState]`, `rows: [LookRow]` with `LookRow { label; detail; status: .looking | .found | .half | .ask; }`, `profile: Profile`, `vaultRoot: URL`, `connected: Bool`, `summarizerConsent: Bool?`.
  - **Actions:**
    - `next()`, which saves the step to `.binder/onboarding.json` (not under the vault until the vault step; until then under `~/Library/Application Support/Binder/onboarding.json`)
    - `runLookAround() async`, which runs each default observer in turn and turns each `Observation` into a row: THIS MAC, SSH, GITHUB, HOSTING, DOMAINS, PROJECTS, AGENTS and HOMELAB, with the copy from `look-around.dc.html`
    - `answerHomelab(mine: Bool)`
    - `createVault() throws`, which runs `ensureLayout`, writes `ObserveRun` notes into the chosen root, writes `me.md` via `MeNote.render`, and saves the pointer
    - `connect() throws`, which runs `HookInstaller.install` with `Bundle.main` `Contents/Helpers/binder`
    - `setConsent(_:)`
    - `finish()`, which sets `state.onboarded`
- **`Permissions`** wraps the real requests:
  - Desktop and Documents: read the folder listing to trigger TCC
  - Local network: a TCP connect to the gateway, port 9
  - Notifications: `UNUserNotificationCenter.requestAuthorization`
  - Terminal: `NSAppleScript` `tell application "Terminal" to get name`
  - Login: `SMAppService.mainApp.register()`
  - **`protocol PermissionRequesting`** lets tests inject a fake.
- **Views:**
  - `BootView`: a `TimelineView` sequence matching `boot.dc.html` timings. The CRT turns on in 0.5 s, the brain flickers at 0.7 s, the log lines appear at 0.5/0.9/1.3/1.8/2.3/2.8 s, the bar fills from 0.4 s over 2.6 s, the title rises at 3.2 s and the button at 3.5 s. A click or Return skips ahead.
  - The other five follow their artboards, inside `WindowChrome` + `StepRail` + `Mascot(mood:)`.

- [ ] **Step 1:** Write these tests:
  - `stepsAdvanceInOrder`
  - `lookAroundBuildsRowsFromObservations` (with a fake runner: macOS row FOUND, AGENTS HALF, HOMELAB ASK)
  - `createVaultWritesNotesMeAndPointer`
  - `aboutYouMergesIntoExistingMe`
  - `connectInstallsHooksOnce`
  - `consentSaved`
  - `resumeAfterQuit` (a new model with the same home starts at the saved step)
  - `finishMarksOnboarded`
- [ ] **Step 2:** Run `make test FILTER=OnboardingModelTests`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `make test FILTER=OnboardingModelTests`. Expected: PASS. Then render the snapshots (`onboarding-<step>.png` for all six) and compare them with the artboards.
- [ ] **Step 5:** Commit with `feat(app): six-step onboarding`.

### Task 6: The main window (library, notes, settings)

**Files:**
- Create: `Sources/BinderUI/Library/LibraryModel.swift`, `LibraryWindow.swift`, `Sidebar.swift`, `NoteView.swift`, `ContextColumn.swift`, `SettingsView.swift`
- Test: `Tests/BinderCoreTests/UI/LibraryModelTests.swift`

**Interfaces:**
- **`@MainActor final class LibraryModel: ObservableObject`**, `init(vault:home:)`:
  - `sections: [LibrarySection]`, with `LibrarySection { kind: .inbox | .me | .machines | .access | .servers | .projects | .handoffs | .settings; items: [NoteRef] }` and `NoteRef { title; path }`
  - `selected: NoteRef?`
  - `func note(_ ref: NoteRef) -> NoteDetail`, where `NoteDetail` holds the title, `verified`, `source`, `observed: [(key, value)]`, `decisions: [Decision(date, text, provenance)]`, `freeText`, `links: [String]`, `lastHandoff: HandoffCard?` and `cardPreview: String` (from `CardBuilder` when it is a project) with `cardTokens` (≈ characters/4)
  - `func addMemory(_ text: String) -> Bool`
  - `func openInObsidian(_ ref: NoteRef)` (only when `md.obsidian` is installed), `func revealInFinder(_:)`, `func openInEditor(_:)`
- **`SettingsView`** covers:
  - the vault location ("Change", "Put it in my Obsidian vault"; it moves the folder and saves the pointer)
  - the project roots
  - "Look around every" (1 h, 6 h or 24 h)
  - the handoff threshold slider, from 50 to 90 %
  - the summarizer toggle (it shows its cost line)
  - Claude Code hooks (Install / Remove, with status)
  - "Run binder doctor" (it shows the lines)
- The layout matches `library.dc.html`: a 220 pt sidebar, the note, and a 300 pt right column.

- [ ] **Step 1:** Write these tests:
  - `sectionsListNotesByFolder`
  - `noteDetailParsesObservedAndDecisions`
  - `projectNoteHasCardPreview`
  - `addMemoryWritesWrittenByYou`
  - `inboxCountMatchesPending`
- [ ] **Step 2:** Run `make test FILTER=LibraryModelTests`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `make test FILTER=LibraryModelTests`. Expected: PASS. Then render the `library.png` and `settings.png` snapshots.
- [ ] **Step 5:** Commit with `feat(app): main window with notes, context column and settings`.

### Task 7: Background work (observing, handoff warning, handing off, opening a session)

**Files:**
- Create: `Sources/BinderUI/Background/ObserveScheduler.swift`, `HandoffNotifier.swift`, `SessionLauncher.swift`, `HandoffAction.swift`
- Modify: `AppModel.swift` (it owns these) and `PanelView.swift` (Hand off now, and Open a new session after a handoff)
- Test: `Tests/BinderCoreTests/UI/BackgroundTests.swift`

**Interfaces:**
- **`ObserveScheduler(model:, interval:)`** runs `ObserveRun` off the main thread:
  - on launch
  - every `interval` (6 h by default)
  - after FSEvents on `~/.ssh/config`, `~/.gitconfig` and the project roots, debounced to 30 s

  The menu bar shows `.observing` while it runs. `func shouldRun(now:lastRun:pendingEvent:) -> Bool` holds the decision logic.
- **`HandoffNotifier`**: `func check(_ status: SessionStatus, threshold: Double) -> Bool` returns true once per session id, when the fraction crosses the threshold. It posts a `UNNotification` "<project> is getting long. A fresh session will be sharper." The ids it has notified are remembered in `UserDefaults`.
- **`HandoffAction.run(session: SessionStatus, vault:, consent:) async -> URL?`** reads the transcript from `session.transcriptPath` (the last 2 MB), uses `ClaudeCLISummarizer.handoff` with consent and `Handoff.fallback` without, and writes the card.
- **`SessionLauncher`:**
  - `func preferredTerminal() -> Terminal` (Ghostty, iTerm, then Terminal, by what is installed)
  - `func launch(in dir: URL, terminal: Terminal) -> Bool`
  - the builders `static func appleScript(for: Terminal, dir: URL) -> String?` and `static func ghosttyArguments(dir: URL) -> [String]`
  - The command is `cd '<dir>' && claude`, with single-quote escaping.

- [ ] **Step 1:** Write these tests:
  - `schedulerRunsOnIntervalAndDebouncesEvents`
  - `notifiesOncePerSession`
  - `noNotificationBelowThreshold`
  - `handoffActionWritesFallbackWithoutConsent`
  - `launcherEscapesQuotesInPath`
  - `terminalScriptsAreWellFormed`
- [ ] **Step 2:** Run `make test FILTER=BackgroundTests`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `make test FILTER=BackgroundTests`. Expected: PASS. Then run `make test` and see everything pass.
- [ ] **Step 5:** Commit with `feat(app): scheduled observing, handoff warning, hand off and open a session`.

### Task 8: Packaging and wiring the app together

**Files:**
- Create: `Resources/Info.plist`, `scripts/build-app.sh`, `scripts/dmg-settings.py`, `Sources/BinderUI/AppDelegate.swift`, `Sources/BinderUI/Icon.swift`
- Modify: `Sources/BinderApp/main.swift`, `Makefile` (`app`, `install`, `run`, `snapshots`), `README.md` (install and first launch)

**Interfaces:**
- **`AppDelegate`:**
  1. Registers the fonts and sets `.accessory`.
  2. Shows onboarding when `!state.onboarded`.
  3. Otherwise starts `AppModel`, `StatusItemController`, `ObserveScheduler` and `HandoffNotifier`.
  4. Opens the main window from "Open Binder".
- **`main.swift` subcommands:**
  - `snapshot <dir>`
  - `icon <file>`: the 1024 px icon, `MascotHead` on black CRT glass with an amber glow, radius 22.4 %
  - `dmg-background <file>`: 640 × 420 as on `install.dc.html`
  - anything else runs the app
- **`Info.plist`:**
  - `CFBundleIdentifier io.github.kungie.binder`, `LSUIElement`, `LSMinimumSystemVersion 14.0`, `CFBundleExecutable Binder`
  - The usage strings, taken verbatim from the Permissions artboard:
    - `NSDesktopFolderUsageDescription` and `NSDocumentsFolderUsageDescription`: "Your projects live there. Binder reads repo and deploy files, nothing else."
    - `NSLocalNetworkUsageDescription`: "To spot your homelab. Binder knocks on SSH and a few web ports, then leaves."
    - `NSAppleEventsUsageDescription`: "So Open a new session can open one for you, in the right folder."
- **`build-app.sh`:**
  - Builds universal (arm64 + x86_64) `BinderApp` and `binder` in release.
  - Copies `BinderApp` → `Contents/MacOS/Binder` and `binder` → `Contents/Helpers/binder`, plus the fonts bundle and `Info.plist`.
  - Makes the icon (iconset, then `iconutil`).
  - Runs `xattr -cr` (the iCloud Desktop issue) and an ad-hoc `codesign`.
  - Builds the DMG with `dmgbuild` if it's present, and always a zip.
- **README:** download the DMG, drag it to Applications, first open via Privacy & Security › Open Anyway, and what Binder reads and never writes.

- [ ] **Step 1:** Write the test `iconRenders1024` (a non-empty 1024 × 1024 image).
- [ ] **Step 2:** Run `make test FILTER=Icon`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `make test` (all pass), then `make app`. Expected: `build/Binder.app` exists, `build/Binder.app/Contents/Helpers/binder help` prints the usage, and `codesign -v build/Binder.app` exits 0. `make snapshots` writes every PNG.
- [ ] **Step 5:** Commit with `feat(app): app delegate, icon, packaging and dmg`.

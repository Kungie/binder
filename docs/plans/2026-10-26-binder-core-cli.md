# Binder core and CLI · implementation plan (plan 1 of 2)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `BinderCore` and the `binder` CLI, so that Claude Code sessions already read and feed the vault through hooks before any app exists.

**Architecture:** One Swift package. `BinderCore` holds every rule: the vault, notes, observers, proposals, the transcript reader, the card builder, handoffs and the hook installer. `binder` is a thin CLI over it, called by Claude Code's SessionStart and Stop hooks. Plan 2 adds the SwiftUI app on top of the same core.

**Tech stack:** Swift 6 toolchain, `swift-tools-version:6.0` with the Swift 5 language mode, Foundation and CryptoKit, Swift Testing. No third-party dependencies.

**Spec:** `docs/specs/2026-10-26-binder-v1-design.md` (§4–§12). The project context is in `CLAUDE.md`.

## Global constraints

- **Platform:** `platforms: [.macOS(.v14)]`. Build and test with the Command Line Tools only: `swift build`, `swift test`. There is no Xcode.
- **Tests:** `import Testing`, `@Test`, `#expect`. Fixtures live in `Tests/BinderCoreTests/Fixtures/`, loaded through `Bundle.module`.
- **No third-party packages.**
- **English everywhere:** file names, code, output text. Copy is short, has no exclamation marks and no em dashes.
- **Vault:**
  - The default root is `~/Binder`. Every path is computed from a `Vault` value, so tests use a temp directory.
  - Never write inside `~/.ssh`. Never open a private key: only `*.pub` files and file names.
- **Hooks:**
  - The `card` and `capture` commands finish within **2 s** or abort silently.
  - They **always exit 0** and never print anything except the hook JSON.
  - Errors go to `<vault>/.binder/log`.
- **Card:** at most **2400 characters** (about 600 tokens). The header is exactly: `BINDER CARD · VERIFIED, USE IT. If something you need is missing, ask once and say what you'd write down.`
- **Handoff:** the warning threshold defaults to **0.70**, clamped to **0.50…0.90**.
- **Context window:** 200 000 tokens by default. It is 1 000 000 when the model id contains `[1m]` or the usage already exceeds 200 000.
- **Summarizer:** it runs only when `BinderState.summarizerConsent == true`. It uses `claude -p`.
- **Claude Code only.** Codex is deferred.

## Review focus

These are the failures most likely to bite a real user. Each one is pinned by a test in the task named.

1. **Hook stdin is empty, malformed, or the vault folder is missing.** The CLI prints nothing harmful and exits 0. Tested in Task 13: `cardWithGarbageStdinExitsCleanly`, `captureWithoutVaultExitsCleanly`.
2. **The user edits a note while an observer rewrites it, or deletes or duplicates the markers.** Text outside the markers survives byte for byte. Tested in Task 2: `userTextOutsideMarkersIsPreserved`, `duplicateMarkersCollapseToFirstPair`.
3. **A secret shows up in a Bash command or the chat** (for example `export GITHUB_TOKEN=ghp_…`). It is never stored in a proposal, a card or the log. Tested in Task 4 (filter corpus) and Task 10: `commandWithTokenMakesNoProposal`.
4. **`~/.claude/settings.json` already has other hooks, or is malformed.** The installer keeps the others, and refuses to write over a malformed file. Tested in Task 12: `installKeepsForeignHooks`, `installRefusesMalformedSettings`.
5. **Capture runs again on the same, growing transcript.** No duplicate proposals, and it only reads the new tail. Tested in Task 9 (`readsFromOffsetOnly`, `ignoresPartialLastLine`) and Task 10 (`secondCaptureAddsNothing`).

---

### Task 1: Package skeleton

**Files:**
- Create: `Package.swift`
- Create: `Sources/BinderCore/BinderCore.swift` (version constant only)
- Create: `Sources/BinderCLI/main.swift` (prints usage)
- Create: `Tests/BinderCoreTests/SmokeTests.swift`
- Create: `Tests/BinderCoreTests/Fixtures/.keep`
- Create: `Makefile` (`test`, `build`, `cli`)

**Interfaces:**
- Produces: `public enum Binder { public static let version = "1.0.0-dev" }`. The CLI executable is named `binder`.

`Package.swift` declares:
- the product `.executable(name: "binder", targets: ["BinderCLI"])`
- the targets `BinderCore`, `BinderCLI` (which depends on `BinderCore`) and `BinderCoreTests` (with `resources: [.copy("Fixtures")]`)
- `swiftLanguageModes: [.v5]`

- [ ] **Step 1:** Write `SmokeTests.swift`: `@Test func versionIsSet() { #expect(Binder.version.hasPrefix("1.")) }`.
- [ ] **Step 2:** Run `swift test`. Expected: it fails, because `Binder` is undefined until the core file exists. Then add the package files.
- [ ] **Step 3:** Run `swift test`. Expected: `1 test … passed`. Then run `swift build && .build/debug/binder help`. Expected: it prints the usage and exits 0.
- [ ] **Step 4:** Commit with `chore: package skeleton with core, cli and tests`.

### Task 2: The note format

**Files:**
- Create: `Sources/BinderCore/Note.swift`
- Test: `Tests/BinderCoreTests/NoteTests.swift`

**Interfaces:**
- Produces: `public struct Note: Equatable` with
  - `frontmatter: [(key: String, value: String)]`, kept in order
  - `body: String`
  - `static func parse(_ text: String) -> Note`
  - `func render() -> String`
  - `var observedLines: [String]`
  - `func replacingObserved(_ lines: [String]) -> Note`
  - `var decisions: [String]`
  - `func appendingDecision(_ line: String) -> Note`
  - `func setting(_ key: String, _ value: String) -> Note`
  - `static func new(title: String) -> Note`
- Constants: `Note.observedStart = "<!-- binder:observed start -->"` and `Note.observedEnd = "<!-- binder:observed end -->"`.

When the markers are missing, `replacingObserved` inserts the block right after the first `# ` heading, or at the top if there is none. `appendingDecision` appends `- <line>` under `## Decisions` and creates that section at the end when it is missing.

- [ ] **Step 1:** Write these tests:
  - `parseAndRenderRoundTrips`: a note with frontmatter, observed block, decisions and free text renders to the identical string.
  - `replacingObservedOnlyTouchesTheBlock`
  - `userTextOutsideMarkersIsPreserved`: free text above and below, including `\n\n` and trailing spaces, is unchanged after a replace.
  - `missingMarkersAreInsertedAfterTitle`
  - `duplicateMarkersCollapseToFirstPair`: the first start…end pair is replaced. Later stray markers are removed, and the text between them is kept.
  - `appendingDecisionCreatesSection`
  - `noteWithoutFrontmatterParses`
- [ ] **Step 2:** Run `swift test --filter NoteTests`. Expected: FAIL.
- [ ] **Step 3:** Implement `Note.swift`.
- [ ] **Step 4:** Run `swift test --filter NoteTests`. Expected: PASS.
- [ ] **Step 5:** Commit with `feat(core): markdown note with observed block and decisions`.

### Task 3: Vault layout, note I/O, project identity

**Files:**
- Create: `Sources/BinderCore/Vault.swift`, `Sources/BinderCore/NoteIO.swift`, `Sources/BinderCore/GitRepo.swift`, `Sources/BinderCore/BinderState.swift`
- Test: `Tests/BinderCoreTests/VaultTests.swift`, `Tests/BinderCoreTests/GitRepoTests.swift`

**Interfaces:**
- Produces `public struct Vault`:
  - `init(root: URL)`, `static var defaultRoot: URL` (`~/Binder`)
  - `func url(_ relative: String) -> URL`
  - `var internalDir`, `inbox`, `cards`, `sessions`, `logFile`, `stateFile`, `cursorsFile`, `droppedFile`: `URL`
  - `func ensureLayout() throws`, which creates `machines/`, `access/`, `servers/`, `projects/`, `handoffs/` and `.binder/{inbox,cards,sessions}`
  - `func log(_ message: String)`, which appends a timestamped line and never throws
- Produces `public enum NoteIO`:
  - `static func read(_ url: URL) -> Note?`
  - `static func write(_ note: Note, to url: URL) throws`, atomic, and creates parent folders
  - `static func update(_ url: URL, title: String, _ change: (Note) -> Note) throws`, which reads (or uses `Note.new(title:)`), changes and writes
- Produces `public struct GitRepo`:
  - `root: URL`, `remote: String?`, `name: String` (the root's folder name)
  - `static func locate(from dir: URL) -> GitRepo?`. It walks up to the folder containing `.git` and reads `[remote "origin"] url` from `.git/config` directly, without starting a process.
  - `var projectID: String`: SHA-256 of the normalized remote, or of the root path when there is no remote. It is the first 12 hex characters.
  - `static func normalize(remote: String) -> String`: `git@github.com:Kungie/dawn.git` and `https://github.com/Kungie/Dawn` both become `github.com/kungie/dawn`.
- Produces `public struct BinderState: Codable`:
  - `summarizerConsent = false`, `handoffThreshold = 0.70`, `projectRoots: [String] = ["~/Desktop", "~/Documents", "~/Developer", "~/Projects", "~/code"]`, `lastObserve: Date?`
  - `static func load(_ vault: Vault) -> BinderState` (defaults when the file is missing or broken)
  - `func save(_ vault: Vault) throws`
  - `var clampedThreshold: Double` (0.50…0.90)

- [ ] **Step 1:** Write these tests:
  - `ensureLayoutCreatesFolders`
  - `noteIOWritesAtomicallyAndCreatesParents`
  - `updateCreatesNoteWithTitle`
  - `logNeverThrowsWhenFolderMissing`
  - `normalizeRemoteForms`: the ssh, https and `.git`/no-`.git` forms are equal.
  - `locateFindsRepoFromSubfolder` (build a temp repo with a hand-written `.git/config`)
  - `projectIDIsStable12Hex`
  - `stateDefaultsWhenFileBroken`
  - `thresholdIsClamped`: 0.95 becomes 0.90 and 0.2 becomes 0.50.
- [ ] **Step 2:** Run them and see them fail.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run them and see them pass.
- [ ] **Step 5:** Commit with `feat(core): vault layout, note io, git repo identity, state`.

### Task 4: The secret filter

**Files:**
- Create: `Sources/BinderCore/SecretFilter.swift`
- Test: `Tests/BinderCoreTests/SecretFilterTests.swift`, `Tests/BinderCoreTests/Fixtures/secrets.txt` (one sample per line), `Tests/BinderCoreTests/Fixtures/not-secrets.txt`

**Interfaces:**
- Produces `public enum SecretFilter`:
  - `static func findings(in text: String) -> [String]`, which returns rule names
  - `static func containsSecret(_ text: String) -> Bool`
  - `static func redact(_ text: String) -> String`, which replaces each match with `[REDACTED:<rule>]`

The rules:

| Rule | Pattern |
|---|---|
| `private-key` | `-----BEGIN [A-Z ]*PRIVATE KEY-----` |
| `aws` | `AKIA[0-9A-Z]{16}` |
| `github` | `gh[pousr]_[A-Za-z0-9]{36,}` and `github_pat_[A-Za-z0-9_]{22,}` |
| `api-key` | `sk-(ant-)?[A-Za-z0-9_-]{20,}` |
| `slack` | `xox[abprs]-[A-Za-z0-9-]{10,}` |
| `assignment` | `(?i)\b(password|passwd|secret|token|api[_-]?key)\s*[:=]\s*['"]?[^\s'"]{6,}` |
| `url-credentials` | `://[^/\s:@]+:[^@\s/]+@` |
| `bearer` | `(?i)bearer\s+[A-Za-z0-9._~+/-]{20,}=*` |
| `high-entropy` | a run of 32+ characters from `[A-Za-z0-9+/_=-]` containing upper case, lower case and digits, with Shannon entropy above 4.2 bits per character |

Pure hex strings, such as commit SHAs, never match `high-entropy`.

- [ ] **Step 1:** Write these tests:
  - `everySampleSecretIsCaught`: each line of `secrets.txt` gives a non-empty `findings`.
  - `nothingInCleanCorpusIsCaught`: commit SHAs, UUIDs, `id_ed25519_hetzner`, `ssh-ed25519 AAAAC3Nza…` public keys, and paths like `~/.ssh/config` are not caught.
  - `redactKeepsSurroundingText`
- [ ] **Step 2:** Run them and see them fail.
- [ ] **Step 3:** Implement the filter with `NSRegularExpression`.
- [ ] **Step 4:** Run them and see them pass.
- [ ] **Step 5:** Commit with `feat(core): secret filter with corpus tests`.

### Task 5: Proposals and the inbox

**Files:**
- Create: `Sources/BinderCore/Proposal.swift`, `Sources/BinderCore/ProposalStore.swift`
- Test: `Tests/BinderCoreTests/ProposalStoreTests.swift`

**Interfaces:**
- Produces `public struct Proposal: Codable, Equatable, Identifiable`:
  - `id`, `createdAt: Date`, `session: String?`, `agent: String` (`"claude-code"` or `"binder"`), `project: String?`
  - `kind: Kind`, with cases `decision`, `fact`, `preference` and `access`
  - `source: Source`, with cases `command`, `observed` and `chat`
  - `evidence: String`, `text: String`, `target: String` (a vault-relative path, for example `projects/dawn.md`)
  - `confidence: Confidence`, with cases `high`, `medium` and `low`
  - `static func make(kind: Kind, source: Source, text: String, evidence: String, target: String, project: String?, session: String?, confidence: Confidence = .medium, agent: String = "claude-code", now: Date = .now) -> Proposal` assigns `id = "p_" + 8 random hex` and `createdAt = now`.
- Produces `public struct ProposalStore`:
  - `init(vault: Vault)`
  - `func add(_ p: Proposal) throws -> Bool`. It returns false when the proposal contains a secret (text or evidence), duplicates a pending proposal, duplicates a decision already in the target note, or was dropped before.
  - `func pending() -> [Proposal]`, sorted by `createdAt`
  - `func keep(_ id: String, editedText: String? = nil, now: Date = .now) throws`. It appends `"<yyyy-MM-dd> · <text> · <provenance>"` to the target's Decisions and deletes the inbox file. The provenance is `kept from a command`, `kept from the chat` or `seen on this Mac`.
  - `func drop(_ id: String) throws`. It deletes the file and appends the normalized-text hash to `dropped`.
  - `static func normalize(_ s: String) -> String`: lower case, punctuation removed, whitespace collapsed.

- [ ] **Step 1:** Write these tests:
  - `addWritesInboxFile`
  - `addRejectsSecret`
  - `addRejectsDuplicateOfPending`
  - `addRejectsDuplicateOfKeptDecision`
  - `droppedIsNeverOfferedAgain`
  - `keepAppendsDecisionWithProvenance`: the note contains `2026-10-26 · The dawn site deploys to Cloudflare Pages. · kept from a command`.
  - `keepUsesEditedText`
  - `pendingIsSortedOldestFirst`
- [ ] **Step 2:** Run them and see them fail.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run them and see them pass.
- [ ] **Step 5:** Commit with `feat(core): proposals, inbox, keep and drop`.

### Task 6: The command runner and the observer contract

**Files:**
- Create: `Sources/BinderCore/CommandRunner.swift`, `Sources/BinderCore/Observer.swift`
- Test: `Tests/BinderCoreTests/CommandRunnerTests.swift`, `Tests/BinderCoreTests/Support/FakeRunner.swift`

**Interfaces:**
- Produces `public struct CommandResult { status: Int32; stdout: String; stderr: String; timedOut: Bool }`.
- Produces `public protocol CommandRunner: Sendable { func run(_ tool: String, _ args: [String], input: String?, timeout: TimeInterval) -> CommandResult? }`. It returns `nil` when the tool isn't installed.
- Produces `public struct SystemRunner: CommandRunner`. It resolves the tool through `PATH` plus `/opt/homebrew/bin`, `/usr/local/bin` and `~/.local/bin`. It runs `Process` with pipes, and kills the process when the timeout passes.
- Produces `public struct ObserverContext { home: URL; projectRoots: [URL]; runner: CommandRunner; now: Date; allowLAN: Bool }`.
- Produces `public struct NoteUpdate { path: String; title: String; observed: [String] }`.
- Produces `public struct Observation { var notes: [NoteUpdate] = []; var proposals: [Proposal] = []; var filesRead = 0 }`.
- Produces `public protocol Observer { var name: String { get }; func observe(_ ctx: ObserverContext) -> Observation }`.
- Test support: `FakeRunner(results: ["tool args…": CommandResult])` returns `nil` for anything missing.

- [ ] **Step 1:** Write these tests:
  - `systemRunnerRunsEcho`
  - `systemRunnerReturnsNilForMissingTool`
  - `systemRunnerKillsOnTimeout`: `sleep 5` with a 0.3 s timeout gives `timedOut == true` within 1 s.
  - `systemRunnerPassesStdin`: `cat` echoes the input.
- [ ] **Step 2:** Run them and see them fail.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run them and see them pass.
- [ ] **Step 5:** Commit with `feat(core): command runner with timeouts, observer contract`.

### Task 7: The machine and access observers (Mac, SSH, accounts, agents)

**Files:**
- Create: `Sources/BinderCore/Observers/MacObserver.swift`, `SSHConfig.swift`, `SSHObserver.swift`, `AccountsObserver.swift`
- Test: `Tests/BinderCoreTests/SSHConfigTests.swift`, `ObserverTests.swift`, `Fixtures/ssh/config`, `Fixtures/ssh/config.d/homelab`

**Interfaces:**
- **`MacObserver`** writes `machines/this-mac.md` with `title: "This Mac"`. Its observed lines:
  - `- macOS: <ProductVersion>`
  - `- chip: <arm64|x86_64>`
  - `- shell: <basename $SHELL>`
  - `- homebrew: <n> formulae`
  - `- agents: Claude Code <found|not found>, Codex <found|not found>`
- **`SSHConfig.parse(_ text: String, includeBase: URL) -> [SSHHost]`**, with `SSHHost { alias, hostName?, user?, port?, identityFiles: [String] }`.
  - Wildcard hosts (`*`, `?`) are skipped.
  - `Include` is followed relative to `includeBase`, globs included.
  - Keywords are case-insensitive. `Host a b` gives two hosts.
- **`SSHObserver`** reads `~/.ssh/config` and the names of the `~/.ssh/*.pub` files, and runs `ssh-keygen -lf <pub>` for fingerprints. It writes:
  - `access/ssh.md` (`title: "SSH"`), with one line per host, `- <alias>: <user>@<hostName>:<port> · key <identity file name>`, and one line per key, `- key <name>: <type> <fingerprint> · opens <aliases or "nothing">`.
  - `servers/<alias>.md` for each host, with `- ssh: <alias>`, `- address: <hostName>`, `- user: <user>` and `- key: <name>`.
  - **It never reads a file without the `.pub` suffix** except `config`. A test checks this.
- **`AccountsObserver`** runs each of these with a 3 s timeout, only when the tool exists:
  - `git config --global user.name` and `user.email`
  - `gh auth status`, parsing `Logged in to github.com account <login>`
  - `vercel whoami`, `netlify status`, `flyctl auth whoami`, `wrangler whoami`, `hcloud context list`, `supabase projects list`

  It writes `access/accounts.md` (`title: "Accounts"`) with one line each, for example `- github: Kungie` and `- cloudflare: signed in`, and leaves out tools that aren't installed.

- [ ] **Step 1:** Write these tests:
  - `parsesHostsUsersPortsAndKeys`
  - `followsIncludeGlob`
  - `skipsWildcardHosts`
  - `sshObserverNeverOpensPrivateKeys`: a temp `~/.ssh` with `id_ed25519` (holding the text `PRIVATE`) and `id_ed25519.pub`; the observation's text never contains `PRIVATE`, and the file-read count matches config + `.pub` only.
  - `keyWithNoHostSaysOpensNothing`
  - `accountsObserverSkipsMissingTools`
  - `accountsObserverParsesGhLogin` (a `FakeRunner` with real `gh auth status` output)
  - `macObserverListsAgents`
- [ ] **Step 2:** Run them and see them fail.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run them and see them pass.
- [ ] **Step 5:** Commit with `feat(core): mac, ssh and accounts observers`.

### Task 8: The project, domain and homelab observers, and the observe run

**Files:**
- Create: `Sources/BinderCore/Observers/ProjectsObserver.swift`, `HomelabObserver.swift`, `Sources/BinderCore/ObserveRun.swift`
- Test: `Tests/BinderCoreTests/ProjectsObserverTests.swift`, `HomelabObserverTests.swift`, `ObserveRunTests.swift`

**Interfaces:**
- **`ProjectsObserver`** finds git repos up to depth 3 under the context's roots. It skips `node_modules`, `.build`, `Pods` and hidden folders. For each repo it writes `projects/<name>.md` (title `<name>`) with:
  - `- repo: <normalized remote or local path>`
  - `- branch: <HEAD ref>`, read from `.git/HEAD`
  - `- languages: <top 3 by file count from extension map>`
  - `- deploy: <list>`, matching the deploy files of spec §6 to Fly.io, Vercel, Cloudflare (wrangler), Netlify, GitHub Actions, Docker and Docker Compose
  - `- domains: <list>`, from `CNAME`, `wrangler.toml` `route`/`routes` and `vercel.json` `alias`, plus, when `dig` exists, `dig +short CNAME <domain>` mapped to a provider name (`pages.dev` is Cloudflare Pages, `vercel-dns`/`vercel.app` is Vercel, `netlify` is Netlify, `github.io` is GitHub Pages)
- **`HomelabObserver`** turns SSH hosts with private addresses (10/8, 172.16/12, 192.168/16, `.local`) into **proposals**, not notes:
  - `kind: .fact`, `source: .observed`, `target: "servers/<alias>.md"`, `text: "<alias> at <address> is part of your homelab."`
  - With `allowLAN`, it TCP-connects to ports 22, 80, 443, 8006 and 8123 on the current /24 (200 ms per port, at most 64 concurrent connections). It proposes `"Something answers SSH on <ip>."` for hosts that aren't already known.
  - The probe sits behind a `LANProbe` protocol, so tests inject fake answers.
- **`ObserveRun.run(vault: Vault, ctx: ObserverContext, observers: [Observer]) throws -> ObserveSummary`**:
  - It writes every `NoteUpdate` with `NoteIO.update` + `replacingObserved`, sets the frontmatter `verified` to an ISO-8601 time and `source` to `observed`, and adds the proposals.
  - It saves `lastObserve`.
  - `ObserveSummary { notesWritten: Int; proposalsAdded: Int; filesRead: Int }`
  - `static func defaultObservers() -> [Observer]` returns Mac, SSH, Accounts, Projects and Homelab.

- [ ] **Step 1:** Write these tests:
  - `findsReposAndSkipsNodeModules`
  - `detectsDeployFilesAndLanguages`
  - `readsDomainsFromCNAMEAndWrangler`
  - `homelabHostsBecomeProposalsNotNotes`
  - `lanProbeOnlyWhenAllowed`
  - `observeRunWritesNotesAndKeepsUserText`: a user line added below the observed block survives a second run.
  - `observeRunIsIdempotent`: two runs give the same note text and no new proposals.
- [ ] **Step 2:** Run them and see them fail.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run them and see them pass.
- [ ] **Step 5:** Commit with `feat(core): project, domain, homelab observers and observe run`.

### Task 9: Reading the Claude Code transcript, and the context meter

**Files:**
- Create: `Sources/BinderCore/Transcript.swift`, `Sources/BinderCore/ContextMeter.swift`
- Test: `Tests/BinderCoreTests/TranscriptTests.swift`, `Fixtures/transcript.jsonl` (built from a real session's shapes: user text, assistant text with `usage`, `tool_use` Bash/Edit/Write/TodoWrite, `tool_result`)

**Interfaces:**
- Produces `public enum TranscriptEntry: Equatable`, with these cases:
  - `.user(String)`
  - `.assistant(String)`
  - `.command(String)` (Bash `input.command`)
  - `.fileEdit(String)` (Edit/Write/MultiEdit `input.file_path`)
  - `.todos([String])` (TodoWrite contents that aren't completed)
  - `.usage(Usage, model: String?)`
- Produces `public struct Usage: Equatable { input: Int; cacheRead: Int; cacheCreation: Int; var total: Int }`.
- Produces `public struct TranscriptChunk { entries: [TranscriptEntry]; endOffset: UInt64; sessionID: String?; cwd: String? }`.
- Produces `public enum Transcript { static func read(_ url: URL, from offset: UInt64) throws -> TranscriptChunk }`. It reads only bytes after `offset`, stops at the last `\n` (a partial line waits for next time) and skips lines that aren't JSON.
- Produces `public struct Cursors { init(vault:); func offset(for session: String) -> UInt64; func set(_ offset: UInt64, for session: String) throws }`.
- Produces `public struct ContextReading: Codable, Equatable { used: Int; window: Int; var fraction: Double }` and `public enum ContextMeter { static func reading(from entries: [TranscriptEntry]) -> ContextReading? }`, which takes the last `.usage` and applies the window rule from the global constraints.

- [ ] **Step 1:** Write these tests:
  - `parsesAllEntryKinds`
  - `readsFromOffsetOnly`
  - `ignoresPartialLastLine`: the fixture plus a half-written line gives an `endOffset` before it, and appending the rest later yields that entry.
  - `skipsGarbageLines`
  - `meterUses200kWindowByDefault`: 341 470 tokens of usage switches to 1M, and 120 000 gives 0.60.
  - `meterUses1MForBracketModel`
  - `cursorsPersist`
- [ ] **Step 2:** Run them and see them fail.
- [ ] **Step 3:** Implement with `FileHandle.seek(toOffset:)` and `JSONSerialization`.
- [ ] **Step 4:** Run them and see them pass.
- [ ] **Step 5:** Commit with `feat(core): incremental claude code transcript reader and context meter`.

### Task 10: Capture (command rules, summarizer, session file)

**Files:**
- Create: `Sources/BinderCore/CommandRules.swift`, `Sources/BinderCore/Summarizer.swift`, `Sources/BinderCore/Capture.swift`
- Test: `Tests/BinderCoreTests/CommandRulesTests.swift`, `CaptureTests.swift`

**Interfaces:**
- **`CommandRules.proposals(for command: String, project: String?, session: String?) -> [Proposal]`.** The rules, all `source: .command`, `confidence: .high` and `target: "projects/<project>.md"` (or `access/ssh.md` for keys):

  | The command matches | Proposal text |
  |---|---|
  | `wrangler pages deploy` | `"<project> deploys to Cloudflare Pages."` |
  | `wrangler deploy` | `"<project> deploys to Cloudflare Workers."` |
  | `vercel` + `deploy` or `--prod` | `"<project> deploys to Vercel."` |
  | `netlify deploy` | `"<project> deploys to Netlify."` |
  | `fly deploy`, `fly launch`, `flyctl deploy` | `"<project> deploys to Fly.io."` |
  | `gh repo create <name>` | `"<project> has a GitHub repo, <name>."` |
  | `git remote add origin <url>` | `"<project>'s remote is <normalized url>."` |
  | `ssh-keygen … -f <path>` | `"Key <file name> was created."` (`kind: .access`) |
  | `ssh-copy-id … <host>` | `"Your key was copied to <host>."` (`kind: .access`) |

  A command with a secret in it gives `[]`.
- **`Summarizer`:**
  - `public protocol Summarizer { func proposals(from entries: [TranscriptEntry], project: String?) -> [Proposal] }`
  - `public struct ClaudeCLISummarizer: Summarizer`. It condenses the entries (user and assistant text only, redacted, at most 12 000 characters), runs `claude -p` with a fixed prompt through `CommandRunner` with a 60 s timeout, and parses a JSON array of `{kind, text, confidence}` into `source: .chat` proposals. Any failure gives `[]`.
- **`Capture.run(input: HookInput, vault: Vault, runner: CommandRunner, summarizer: Summarizer?) throws -> CaptureResult`:**
  1. Read from the cursor, then apply the rules to `.command` entries.
  2. Call the summarizer **only when `BinderState.summarizerConsent` is true**.
  3. Add the proposals through `ProposalStore`.
  4. Save the cursor.
  5. Write `.binder/sessions/<session>.json`: `{project, cwd, reading, updatedAt}`.
- `CaptureResult { proposalsAdded: Int; reading: ContextReading? }`.
- **`HookInput: Decodable`:** `session_id`, `transcript_path`, `cwd`, `hook_event_name`, `source` (the last two optional).

- [ ] **Step 1:** Write these tests:
  - `eachRuleProducesItsText`
  - `commandWithTokenMakesNoProposal` (`export GITHUB_TOKEN=ghp_… && vercel deploy`)
  - `captureAddsProposalsFromFixture`
  - `secondCaptureAddsNothing`
  - `summarizerNotCalledWithoutConsent` (a spy summarizer)
  - `summarizerCalledWithConsent`
  - `captureWritesSessionReading`
- [ ] **Step 2:** Run them and see them fail.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run them and see them pass.
- [ ] **Step 5:** Commit with `feat(core): capture with command rules and consented summarizer`.

### Task 11: The card builder and handoffs

**Files:**
- Create: `Sources/BinderCore/CardBuilder.swift`, `Sources/BinderCore/Handoff.swift`
- Test: `Tests/BinderCoreTests/CardBuilderTests.swift`, `HandoffTests.swift`, `Fixtures/vault/` (`me.md`, `projects/dawn.md`, `servers/hetzner-1.md`, `access/ssh.md`), `Fixtures/card-dawn.txt` (golden)

**Interfaces:**
- **`CardBuilder.build(vault: Vault, repo: GitRepo?) -> String`.** The card is assembled in this order:
  1. The header from the global constraints.
  2. `ME:` with the observed lines and the first 6 lines of free text from `me.md`.
  3. `PROJECT <name>:` with its observed lines, then its decisions, newest first.
  4. `SERVERS AND ACCESS:` with the lines from `servers/*.md` and `access/ssh.md` that the project note links to, or that mention it.
  5. `PICK UP FROM LAST TIME:` with the pending handoff's four sections.

  It drops the oldest decisions, then the access lines, until the card is ≤ 2400 characters. When there is no repo, the card has only the header and ME. The result passes `SecretFilter.redact` as a last guard.
- **`CardCache`:** `func cached(for repo: GitRepo) -> String?` returns the card only when `.binder/cards/<id>.md` is newer than every file it read. `func store(_ card: String, for repo: GitRepo) throws`.
- **`Handoff`:**
  - `public struct HandoffCard { done: [String]; left: [String]; dontTouch: [String]; next: String }`
  - `static func fallback(from entries: [TranscriptEntry]) -> HandoffCard`. `done` is the files edited. `left` is the open todos. `dontTouch` is empty. `next` is the last user message, trimmed to 200 characters.
  - `static func write(_ card: HandoffCard, project: String, vault: Vault, now: Date) throws -> URL` writes `handoffs/<project>/<yyyy-MM-dd-HHmm>.md` with headings `## Done`, `## Left`, `## Don't touch` and `## Next`, plus the frontmatter `consumed: false`.
  - `static func pending(project: String, vault: Vault) -> HandoffCard?` returns the newest card where `consumed: false`.
  - `static func markConsumed(project: String, vault: Vault) throws`.

- [ ] **Step 1:** Write these tests:
  - `cardMatchesGolden`
  - `cardRespectsBudget` (40 decisions: length ≤ 2400, and the newest decision is still present)
  - `cardWithoutRepoHasHeaderAndMe`
  - `cardNeverContainsSecrets` (a decision holding `ghp_…` shows up redacted)
  - `cacheInvalidatesWhenNoteChanges`
  - `fallbackHandoffFromFixture`
  - `pendingThenConsumed`
- [ ] **Step 2:** Run them and see them fail.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run them and see them pass.
- [ ] **Step 5:** Commit with `feat(core): context card with budget, cache and handoffs`.

### Task 12: The hook installer and doctor

**Files:**
- Create: `Sources/BinderCore/HookInstaller.swift`, `Sources/BinderCore/Doctor.swift`
- Test: `Tests/BinderCoreTests/HookInstallerTests.swift`, `Fixtures/settings-with-hooks.json`

**Interfaces:**
- **`HookInstaller(settingsURL: URL, binderPath: String)`:**
  - `isInstalled: Bool`, `install() throws`, `uninstall() throws`
  - Events: `SessionStart` runs `"<binderPath>" card`, and `Stop` runs `"<binderPath>" capture`. Each entry is `{"matcher": "", "hooks": [{"type": "command", "command": …, "timeout": 5}]}`.
  - "Ours" means the command contains `binder" card` or `binder" capture`.
  - Before the first change it writes a backup at `settings.json.binder-backup`, once.
  - A malformed file throws `HookInstallerError.malformedSettings` without writing anything.
  - A missing file is created.
  - It preserves the order of the other keys and entries as far as JSONSerialization allows, and writes with `.prettyPrinted` and `.withoutEscapingSlashes`.
- **`Doctor.report(vault: Vault, installer: HookInstaller, runner: CommandRunner) -> [DoctorLine]`**, with `DoctorLine { ok: Bool; text: String }`. It checks that the vault exists, the hooks are installed, `claude` is found, and the last capture and observe times. It also tries the newest transcript, and reports `Claude Code transcript format: unreadable` if no entry parses.

- [ ] **Step 1:** Write these tests:
  - `installAddsBothEvents`
  - `installIsIdempotent`
  - `installKeepsForeignHooks` (the fixture holds a Dawn hook; it is still there after install and after uninstall)
  - `installRefusesMalformedSettings` (the file bytes are unchanged)
  - `uninstallRemovesOnlyOurs`
  - `backupWrittenOnce`
  - `doctorFlagsMissingVault`
- [ ] **Step 2:** Run them and see them fail.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run them and see them pass.
- [ ] **Step 5:** Commit with `feat(core): claude code hook installer and doctor`.

### Task 13: The CLI commands

**Files:**
- Modify: `Sources/BinderCLI/main.swift`
- Create: `Sources/BinderCLI/Commands.swift`, `Sources/BinderCore/HookIO.swift`
- Test: `Tests/BinderCoreTests/HookIOTests.swift`, `Tests/BinderCoreTests/CLITests.swift` (runs the built `binder` with a temp `HOME` and `BINDER_VAULT`)

**Interfaces:**
- **`HookIO`:**
  - `static func readInput(_ data: Data) -> HookInput?` returns `nil` on any error.
  - `static func sessionStartOutput(_ card: String) -> Data` produces `{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":<card>}}`.
- **Environment:** `BINDER_VAULT` overrides the vault root, and `BINDER_HOME` overrides home, both for tests.
- **Commands:**

  | Command | What it does |
  |---|---|
  | `binder card` | Hook mode. It reads stdin, then `GitRepo.locate(cwd)`, `CardCache` or `CardBuilder`, then marks the pending handoff consumed and prints the hook JSON. **On any failure it prints nothing.** |
  | `binder capture` | Hook mode. It reads stdin and runs `Capture.run` with the summarizer only when there is consent. It prints nothing. |
  | `binder observe` | Runs `ObserveRun` with the default observers, `allowLAN` false unless `--lan`, then prints `Read <n> files. Wrote <m> notes. <k> things to review.` |
  | `binder handoff [--cwd <dir>]` | Reads the newest transcript for that project, writes the fallback card (or the summarizer's, with consent), then prints the path. |
  | `binder install` / `binder uninstall` | Runs the installer with the path of the current executable, then prints `Hooks added to ~/.claude/settings.json.` or `Hooks removed.` |
  | `binder doctor` | Prints one line per check, `ok` or `!!` and then the text. |
  | `binder help` | Prints the usage. |

- **Hook mode** has a 2 s watchdog (a `DispatchSourceTimer` calling `exit(0)`), a `do/catch` around everything that sends errors to `vault.log`, and an exit code that is always 0.

- [ ] **Step 1:** Write these tests:
  - `hookInputParsesSessionStart`
  - `hookInputNilOnGarbage`
  - `sessionStartOutputShape`
  - CLI: `cardPrintsHookJSONForFixtureRepo`
  - `cardWithGarbageStdinExitsCleanly` (exit 0, empty stdout)
  - `captureWithoutVaultExitsCleanly`
  - `observePrintsSummary`
  - `installThenUninstallRoundTrip`
- [ ] **Step 2:** Run them and see them fail.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `swift test` (the full suite). Expected: all tests pass.
- [ ] **Step 5:** Run a manual end-to-end check:
  - `swift build -c release`
  - `BINDER_VAULT=/tmp/bv .build/release/binder observe`
  - `echo '{"session_id":"x","cwd":"'"$PWD"'","hook_event_name":"SessionStart"}' | BINDER_VAULT=/tmp/bv .build/release/binder card`

  Expected: the observe summary line, then hook JSON whose `additionalContext` starts with `BINDER CARD`.
- [ ] **Step 6:** Commit with `feat(cli): card, capture, observe, handoff, install, doctor`.

---

**Plan 2 (written after this one ships):** the SwiftUI app (`BinderApp`). It covers onboarding with all six steps and their animations, the menu bar panel with keep and drop, the main window, the Phosphor theme and bundled fonts, notifications, "Open a new session", packaging (universal binary, Info.plist usage strings, icon, dmgbuild) and the snapshot command.

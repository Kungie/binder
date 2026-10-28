import BinderCore
import Foundation

/// Where everything lives. `BINDER_VAULT` and `BINDER_HOME` override the defaults, for tests.
struct Paths {
    let home: URL
    let vault: Vault
    let settings: URL
    let binder: String

    static func current() -> Paths {
        let env = ProcessInfo.processInfo.environment
        let home = env["BINDER_HOME"].map { URL(fileURLWithPath: $0) } ?? FileManager.default.homeDirectoryForCurrentUser
        let vault = Vault(root: VaultLocation.resolve(env: env, home: home))
        let binder = Bundle.main.executableURL?.resolvingSymlinksInPath().path ?? CommandLine.arguments[0]
        return Paths(home: home, vault: vault, settings: home.appendingPathComponent(".claude/settings.json"), binder: binder)
    }

    var vaultExists: Bool {
        var isDir: ObjCBool = false
        return FileManager.default.fileExists(atPath: vault.root.path, isDirectory: &isDir) && isDir.boolValue
    }

    func expand(_ path: String) -> URL {
        path.hasPrefix("~/") ? home.appendingPathComponent(String(path.dropFirst(2))) : URL(fileURLWithPath: path)
    }
}

enum Commands {
    // MARK: Hook mode: quiet, fast, always exit 0

    /// SessionStart: print the context card for the folder the session started in.
    static func card(_ paths: Paths, stdin: Data) {
        guard paths.vaultExists, let input = HookIO.readInput(stdin) else { return }
        let repo = CardBuilder.project(for: URL(fileURLWithPath: input.cwd), vault: paths.vault)
        let card: String
        if let repo {
            let cache = CardCache(vault: paths.vault)
            if let cached = cache.cached(for: repo) {
                card = cached
            } else {
                card = CardBuilder.build(vault: paths.vault, repo: repo)
                try? cache.store(card, for: repo)
            }
            if Handoff.pending(project: repo.name, vault: paths.vault) != nil {
                try? Handoff.markConsumed(project: repo.name, vault: paths.vault)
            }
        } else {
            card = CardBuilder.build(vault: paths.vault, repo: nil)
        }
        FileHandle.standardOutput.write(HookIO.sessionStartOutput(card))
    }

    /// Stop: catch commands right away. If the user agreed, and enough talk has piled up (or long enough has
    /// passed), hand that stretch to the summarizer in a detached process.
    static func capture(_ paths: Paths, stdin: Data) {
        guard paths.vaultExists, let input = HookIO.readInput(stdin) else { return }
        do {
            let result = try Capture.run(input: input, vault: paths.vault, runner: SystemRunner(), summarizer: nil)
            if BinderState.load(paths.vault).summarizerConsent, let transcript = input.transcriptPath,
               let range = SummarizeQueue(vault: paths.vault).claim(session: input.sessionID, transcript: URL(fileURLWithPath: transcript),
                                                                    startingAt: result.startOffset, now: Date()) {
                HookIO.spawnDetached(paths.binder, ["summarize", input.sessionID, transcript,
                                                    String(range.lowerBound), String(range.upperBound), input.cwd])
            }
        } catch {
            paths.vault.log("capture failed: \(error)")
        }
    }

    /// The detached half of capture: read the claimed stretch, window by window, against what the binder
    /// already says. New things go to the inbox; lines the talk made untrue move to Superseded.
    /// Arguments: session, transcript, start, end, cwd.
    static func summarize(_ paths: Paths, _ args: [String]) {
        guard args.count == 5, let start = UInt64(args[2]), let end = UInt64(args[3]),
              BinderState.load(paths.vault).summarizerConsent else { return }
        let (session, transcript, cwd) = (args[0], args[1], args[4])
        // One summarizer at a time, so two never rewrite the same note at once.
        let lock = open(paths.vault.internalDir.appendingPathComponent("summarize.lock").path, O_CREAT | O_RDWR, 0o644)
        if lock >= 0 { flock(lock, LOCK_EX) }
        defer { if lock >= 0 { flock(lock, LOCK_UN); close(lock) } }
        do {
            let chunk = try Transcript.read(URL(fileURLWithPath: transcript), from: start, upTo: end)
            let project = projectName(cwd: cwd, paths: paths)
            let summarizer = ClaudeCLISummarizer(runner: ClaudeCLISummarizer.runner(), session: session)
            let store = ProposalStore(vault: paths.vault)
            for window in ClaudeCLISummarizer.windows(chunk.entries) {
                let review = summarizer.review(entries: window, project: project,
                                               known: .load(vault: paths.vault, project: project))
                for p in review.proposals { _ = try store.add(p) }
                for (target, group) in Dictionary(grouping: review.outdated, by: \.target) {
                    try Memory.supersede(group.map(\.line), in: target, vault: paths.vault)
                }
            }
        } catch {
            paths.vault.log("summarize failed: \(error)")
        }
    }

    /// The project a session's things belong to: its git repo, a folder with its own note, or a folder that sits
    /// inside one of your project folders (`~/Desktop/Vektor Labs`). Your home or a project folder itself is none.
    static func projectName(cwd: String, paths: Paths) -> String? {
        let folder = URL(fileURLWithPath: cwd).standardizedFileURL
        if let repo = CardBuilder.project(for: folder, vault: paths.vault) { return repo.name }
        let roots = BinderState.load(paths.vault).projectRoots.map { paths.expand($0).standardizedFileURL.path }
        let parent = folder.deletingLastPathComponent().path
        return roots.contains(parent) ? folder.lastPathComponent : nil
    }

    // MARK: Commands people run

    /// Moves stale and repeated lines to Superseded: notes changed since the last tidy, or all with --all.
    static func tidy(_ paths: Paths, everything: Bool) -> Int32 {
        guard paths.vaultExists else { print("No binder at \(paths.vault.root.path) yet."); return 1 }
        let consent = BinderState.load(paths.vault).summarizerConsent
        let moved = Tidy.run(vault: paths.vault, summarizer: consent ? ClaudeCLISummarizer(runner: ClaudeCLISummarizer.runner(), session: nil) : nil,
                             everything: everything)
        if moved.isEmpty {
            print("Nothing stale.")
        } else {
            for (path, n) in moved.sorted(by: { $0.key < $1.key }) { print("\(path): \(n) \(plural(n, "line")) to Superseded") }
        }
        if !consent { print("Only exact repeats were checked. Turn on reading with your Claude plan to catch outdated lines too.") }
        return 0
    }

    static func observe(_ paths: Paths, lan: Bool) -> Int32 {
        let state = BinderState.load(paths.vault)
        let ctx = ObserverContext(home: paths.home, projectRoots: state.projectRoots.map(paths.expand), runner: SystemRunner(), allowLAN: lan)
        do {
            let s = try ObserveRun.run(vault: paths.vault, ctx: ctx, observers: ObserveRun.defaultObservers())
            print("Read \(s.filesRead) \(plural(s.filesRead, "file")). Wrote \(s.notesWritten) \(plural(s.notesWritten, "note")). \(s.proposalsAdded) \(plural(s.proposalsAdded, "thing")) to review.")
            return 0
        } catch {
            print("Couldn't write to \(paths.vault.root.path): \(error.localizedDescription)")
            return 1
        }
    }

    static func handoff(_ paths: Paths, cwd: String) -> Int32 {
        let dir = URL(fileURLWithPath: cwd)
        let repo = GitRepo.locate(from: dir)
        let project = repo?.name ?? dir.lastPathComponent
        let folder = paths.home.appendingPathComponent(".claude/projects")
            .appendingPathComponent(HookIO.claudeProjectFolder(for: (repo?.root ?? dir).standardizedFileURL.path))
        guard let transcript = newestTranscript(in: folder) else {
            print("No Claude Code session found for \(project).")
            return 1
        }
        let entries = (try? Transcript.read(transcript, from: 0).entries) ?? []
        var card: HandoffCard?
        if BinderState.load(paths.vault).summarizerConsent {
            card = ClaudeCLISummarizer(runner: ClaudeCLISummarizer.runner(), session: nil).handoff(from: entries)
        }
        do {
            let url = try Handoff.write(card ?? Handoff.fallback(from: entries), project: project, vault: paths.vault, now: Date())
            print(url.path)
            return 0
        } catch {
            print("Couldn't write the handoff: \(error.localizedDescription)")
            return 1
        }
    }

    static func install(_ paths: Paths, remove: Bool) -> Int32 {
        let installer = HookInstaller(settingsURL: paths.settings, binderPath: paths.binder)
        do {
            if remove {
                try installer.uninstall()
                print("Hooks removed.")
            } else {
                try installer.install()
                print("Hooks added to \(paths.settings.path).")
            }
            return 0
        } catch HookInstallerError.malformedSettings {
            print("\(paths.settings.path) isn't valid JSON, so I left it alone. Fix it and try again.")
            return 1
        } catch {
            print("Couldn't update \(paths.settings.path): \(error.localizedDescription)")
            return 1
        }
    }

    static func doctor(_ paths: Paths) -> Int32 {
        let installer = HookInstaller(settingsURL: paths.settings, binderPath: paths.binder)
        let lines = Doctor.report(vault: paths.vault, installer: installer, runner: SystemRunner())
        for line in lines { print("\(line.ok ? "ok" : "!!")  \(line.text)") }
        return lines.allSatisfy(\.ok) ? 0 : 1
    }

    // MARK: Helpers

    static func newestTranscript(in folder: URL) -> URL? {
        let files = (try? FileManager.default.contentsOfDirectory(at: folder, includingPropertiesForKeys: [.contentModificationDateKey])) ?? []
        return files.filter { $0.pathExtension == "jsonl" }.max { modified($0) < modified($1) }
    }

    static func modified(_ url: URL) -> Date {
        (try? url.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate) ?? .distantPast
    }

    static func plural(_ n: Int, _ word: String) -> String { n == 1 ? word : word + "s" }
}

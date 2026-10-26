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
        let vault = Vault(root: env["BINDER_VAULT"].map { URL(fileURLWithPath: $0) } ?? Vault.defaultRoot)
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
        let repo = GitRepo.locate(from: URL(fileURLWithPath: input.cwd))
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

    /// Stop: catch commands right away; if the user agreed, hand the slow summarizer to a detached process.
    static func capture(_ paths: Paths, stdin: Data) {
        guard paths.vaultExists, let input = HookIO.readInput(stdin) else { return }
        do {
            let result = try Capture.run(input: input, vault: paths.vault, runner: SystemRunner(), summarizer: nil)
            if BinderState.load(paths.vault).summarizerConsent, let transcript = input.transcriptPath, result.endOffset > result.startOffset {
                HookIO.spawnDetached(paths.binder, ["summarize", input.sessionID, transcript, String(result.startOffset), input.cwd])
            }
        } catch {
            paths.vault.log("capture failed: \(error)")
        }
    }

    /// The detached half of capture: read the same part of the transcript and ask the summarizer.
    static func summarize(_ paths: Paths, _ args: [String]) {
        guard args.count == 4, let offset = UInt64(args[2]), BinderState.load(paths.vault).summarizerConsent else { return }
        let (session, transcript, cwd) = (args[0], args[1], args[3])
        do {
            let chunk = try Transcript.read(URL(fileURLWithPath: transcript), from: offset)
            let project = GitRepo.locate(from: URL(fileURLWithPath: cwd))?.name
            let summarizer = ClaudeCLISummarizer(runner: SystemRunner(extraEnvironment: ["BINDER_NESTED": "1"]), session: session)
            let store = ProposalStore(vault: paths.vault)
            for p in summarizer.proposals(from: chunk.entries, project: project) { _ = try store.add(p) }
        } catch {
            paths.vault.log("summarize failed: \(error)")
        }
    }

    // MARK: Commands people run

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
            card = ClaudeCLISummarizer(runner: SystemRunner(extraEnvironment: ["BINDER_NESTED": "1"]), session: nil).handoff(from: entries)
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

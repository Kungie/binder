import Foundation

public struct DoctorLine: Equatable, Sendable {
    public var ok: Bool
    public var text: String
}

/// `binder doctor`: is everything wired up, and can Binder still read Claude Code's transcripts?
public enum Doctor {
    public static func report(vault: Vault, installer: HookInstaller, runner: CommandRunner) -> [DoctorLine] {
        let fm = FileManager.default
        var lines: [DoctorLine] = []
        var isDir: ObjCBool = false
        if fm.fileExists(atPath: vault.root.path, isDirectory: &isDir), isDir.boolValue {
            lines.append(DoctorLine(ok: true, text: "Binder folder: \(vault.root.path)"))
        } else {
            lines.append(DoctorLine(ok: false, text: "No binder at \(vault.root.path). Run binder observe to make one."))
        }

        lines.append(installer.isInstalled
            ? DoctorLine(ok: true, text: "Claude Code hooks are in \(installer.settingsURL.path)")
            : DoctorLine(ok: false, text: "Claude Code hooks are missing. Run binder install."))

        if let version = runner.run("claude", ["--version"], input: nil, timeout: 5), version.status == 0 {
            lines.append(DoctorLine(ok: true, text: "Claude Code \(version.stdout.trimmingCharacters(in: .whitespacesAndNewlines))"))
        } else {
            lines.append(DoctorLine(ok: false, text: "claude isn't on PATH, so the summarizer can't run."))
        }

        let state = BinderState.load(vault)
        lines.append(state.lastObserve.map { DoctorLine(ok: true, text: "Last look around: \(stamp($0))") }
            ?? DoctorLine(ok: false, text: "Binder hasn't looked around yet. Run binder observe."))

        let sessions = ((try? fm.contentsOfDirectory(at: vault.sessions, includingPropertiesForKeys: nil)) ?? [])
            .compactMap { SessionStatus.load(session: $0.deletingPathExtension().lastPathComponent, vault: vault)?.updatedAt }
        lines.append(DoctorLine(ok: true, text: sessions.max().map { "Last capture: \(stamp($0))" } ?? "No session captured yet."))

        let projects = installer.settingsURL.deletingLastPathComponent().appendingPathComponent("projects")
        if let newest = newestTranscript(in: projects) {
            let size = (try? fm.attributesOfItem(atPath: newest.path)[.size] as? UInt64) ?? 0
            let readable = (try? Transcript.read(newest, from: size > 400_000 ? size - 400_000 : 0).entries.isEmpty) == false
            lines.append(readable
                ? DoctorLine(ok: true, text: "Claude Code transcript format: readable")
                : DoctorLine(ok: false, text: "Claude Code transcript format: unreadable. Binder can't catch anything until it's updated."))
        }
        lines.append(DoctorLine(ok: true, text: "Summarizer: \(state.summarizerConsent ? "on" : "off (asks first)")"))
        return lines
    }

    static func newestTranscript(in projects: URL) -> URL? {
        let fm = FileManager.default
        let dirs = (try? fm.contentsOfDirectory(at: projects, includingPropertiesForKeys: nil)) ?? []
        let files = dirs.flatMap { (try? fm.contentsOfDirectory(at: $0, includingPropertiesForKeys: [.contentModificationDateKey])) ?? [] }
            .filter { $0.pathExtension == "jsonl" }
        return files.max { modified($0) < modified($1) }
    }

    static func modified(_ url: URL) -> Date {
        (try? url.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate) ?? .distantPast
    }

    static func stamp(_ date: Date) -> String {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.dateFormat = "yyyy-MM-dd HH:mm"
        return f.string(from: date)
    }
}

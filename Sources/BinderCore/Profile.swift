import Foundation

/// What the user tells Binder about themselves at onboarding. Lives in `me.md`.
public struct Profile: Codable, Equatable, Sendable {
    public var name = ""
    public var role = ""
    public var timeZone = ""
    public var language = ""
    public var workingStyle: [String] = []
    public var taste: [String] = []
    public var avoid: [String] = []
    public var nowAndNext = ""

    public init(name: String = "", role: String = "", timeZone: String = "", language: String = "", workingStyle: [String] = [],
                taste: [String] = [], avoid: [String] = [], nowAndNext: String = "") {
        self.name = name
        self.role = role
        self.timeZone = timeZone
        self.language = language
        self.workingStyle = workingStyle
        self.taste = taste
        self.avoid = avoid
        self.nowAndNext = nowAndNext
    }
}

/// Writes a `Profile` into `me.md` and reads it back. Only Binder's own lines and three sections are replaced;
/// the observed block, decisions and anything else the user wrote stay.
public enum MeNote {
    static let keys = [("name", \Profile.name), ("does", \Profile.role), ("time zone", \Profile.timeZone), ("language", \Profile.language)]
    static let workSection = "## How to work with me"
    static let tasteSection = "## Taste"
    static let nowSection = "## Now and next"
    static let managedSections: Set<String> = [workSection, tasteSection, nowSection]

    public static func render(_ p: Profile, into existing: Note?) -> Note {
        let base = existing ?? Note.new(title: "Me")
        let (preamble, sections) = split(base.body)

        var title = "# Me"
        var observed: [String] = []
        var others: [String] = []
        var inObserved = false
        for line in preamble {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            if trimmed.hasPrefix("# "), observed.isEmpty, others.isEmpty, title == "# Me" { title = line; continue }
            if trimmed == Note.observedStart { inObserved = true }
            if inObserved { observed.append(line); if trimmed == Note.observedEnd { inObserved = false }; continue }
            if keys.contains(where: { trimmed.hasPrefix("- \($0.0):") }) { continue }
            others.append(line)
        }
        while others.first?.trimmingCharacters(in: .whitespaces).isEmpty == true { others.removeFirst() }
        while others.last?.trimmingCharacters(in: .whitespaces).isEmpty == true { others.removeLast() }

        var out = [title, ""]
        if !observed.isEmpty { out += observed }
        let keyLines = keys.compactMap { key, path -> String? in p[keyPath: path].isEmpty ? nil : "- \(key): \(p[keyPath: path])" }
        out += keyLines
        if !others.isEmpty { out += [""] + others }
        if !p.workingStyle.isEmpty { out += ["", workSection] + p.workingStyle.map { "- \($0)" } }
        if !p.taste.isEmpty || !p.avoid.isEmpty {
            out += ["", tasteSection]
            if !p.taste.isEmpty { out.append("- likes: \(p.taste.joined(separator: ", "))") }
            if !p.avoid.isEmpty { out.append("- rather not: \(p.avoid.joined(separator: ", "))") }
        }
        if !p.nowAndNext.isEmpty { out += ["", nowSection, p.nowAndNext] }
        for section in sections where !managedSections.contains(section.first?.trimmingCharacters(in: .whitespaces) ?? "") {
            var lines = section
            while lines.last?.trimmingCharacters(in: .whitespaces).isEmpty == true { lines.removeLast() }
            out += [""] + lines
        }
        var note = base
        note.body = out.joined(separator: "\n") + "\n"
        return note
    }

    public static func profile(from note: Note) -> Profile {
        var p = Profile()
        let (preamble, sections) = split(note.body)
        for line in preamble {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            for (key, path) in keys where trimmed.hasPrefix("- \(key):") {
                p[keyPath: path] = String(trimmed.dropFirst("- \(key):".count)).trimmingCharacters(in: .whitespaces)
            }
        }
        for section in sections {
            let heading = section.first?.trimmingCharacters(in: .whitespaces) ?? ""
            let body = section.dropFirst().map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
            switch heading {
            case workSection:
                p.workingStyle = body.filter { $0.hasPrefix("- ") }.map { String($0.dropFirst(2)) }
            case tasteSection:
                for line in body {
                    if line.hasPrefix("- likes:") { p.taste = list(line.dropFirst("- likes:".count)) }
                    if line.hasPrefix("- rather not:") { p.avoid = list(line.dropFirst("- rather not:".count)) }
                }
            case nowSection:
                p.nowAndNext = body.joined(separator: "\n")
            default:
                continue
            }
        }
        return p
    }

    private static func list(_ s: Substring) -> [String] {
        s.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
    }

    /// The lines before the first `## ` heading, and each `## ` section with its heading.
    static func split(_ body: String) -> (preamble: [String], sections: [[String]]) {
        var preamble: [String] = []
        var sections: [[String]] = []
        for line in body.components(separatedBy: "\n") {
            if line.hasPrefix("## ") { sections.append([line]) }
            else if sections.isEmpty { preamble.append(line) }
            else { sections[sections.count - 1].append(line) }
        }
        return (preamble, sections)
    }
}

/// The languages the user actually writes, ranked by how many projects use them.
public enum TasteDetector {
    public static func languages(vault: Vault, limit: Int = 6) -> [String] {
        let dir = vault.url("projects")
        let names = (try? FileManager.default.contentsOfDirectory(atPath: dir.path)) ?? []
        var counts: [String: Int] = [:]
        for name in names where name.hasSuffix(".md") {
            guard let line = NoteIO.read(dir.appendingPathComponent(name))?.observedLines.first(where: { $0.hasPrefix("- languages:") }) else { continue }
            for language in line.dropFirst("- languages:".count).split(separator: ",") {
                counts[language.trimmingCharacters(in: .whitespaces), default: 0] += 1
            }
        }
        return counts.sorted { $0.value != $1.value ? $0.value > $1.value : $0.key < $1.key }.prefix(limit).map(\.key)
    }
}

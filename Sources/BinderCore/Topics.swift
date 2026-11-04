import Foundation

/// A project's topic notes: `projects/<project>/<topic>.md`, beside the overview `projects/<project>.md`.
public enum Topic {
    static let maxLength = 24

    /// Lowercase letters, digits and single dashes, at most 24 characters; nil when nothing is left.
    /// Anything else, slashes and dots included, becomes a dash, so a slug can never leave its folder.
    public static func slug(_ raw: String) -> String? {
        let mapped = raw.lowercased().unicodeScalars.map { scalar -> Character in
            scalar.isASCII && CharacterSet.alphanumerics.contains(scalar) ? Character(scalar) : "-"
        }
        let joined = String(mapped).split(separator: "-").joined(separator: "-")
        let cut = String(joined.prefix(maxLength)).trimmingCharacters(in: CharacterSet(charactersIn: "-"))
        return cut.isEmpty ? nil : cut
    }

    public static func path(project: String, topic: String) -> String { "projects/\(project)/\(topic).md" }

    /// Links a topic from the project's overview the first time, so Obsidian and the graph see it.
    public static func link(project: String, topic: String, vault: Vault, now: Date = .now) throws {
        let link = "[[\(project)/\(topic)]]"
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.dateFormat = "yyyy-MM-dd"
        try NoteIO.update(vault.url("projects/\(project).md"), title: project) {
            $0.render().contains(link) ? $0 : $0.appendingDecision("\(f.string(from: now)) · Topic: \(link) · written by Binder")
        }
    }

    /// The project's topics, A to Z, by file name.
    public static func topics(of project: String, vault: Vault) -> [String] {
        let names = (try? FileManager.default.contentsOfDirectory(atPath: vault.url("projects/\(project)").path)) ?? []
        return names.filter { $0.hasSuffix(".md") }.map { String($0.dropLast(3)) }.sorted()
    }
}

/// Extra notes a catch also goes into, named by the model as `server:<name>` or `hosting:<provider>`.
public enum Destination {
    public static func path(for tag: String) -> String? {
        guard let colon = tag.firstIndex(of: ":") else { return nil }
        let folder: String
        switch tag[..<colon].lowercased() {
        case "server": folder = "servers"
        case "hosting": folder = "hosting"
        default: return nil
        }
        return Topic.slug(String(tag[tag.index(after: colon)...])).map { "\(folder)/\($0).md" }
    }
}

/// Sorts a project overview's decisions into topic notes, once, when the user asks.
public enum TopicSplit {
    public struct Result: Equatable, Sendable {
        public var lines: Int
        public var topics: Int

        public init(lines: Int, topics: Int) {
            self.lines = lines
            self.topics = topics
        }
    }

    /// Moves only the lines the model names, each with its detail. Topic links stay in the overview.
    public static func run(project: String, vault: Vault, summarizer: ClaudeCLISummarizer, now: Date = .now) throws -> Result {
        let url = vault.url("projects/\(project).md")
        guard let before = NoteIO.read(url) else { return Result(lines: 0, topics: 0) }
        let candidates = before.decisions.reversed().filter { !CardBuilder.decisionText($0).hasPrefix("Topic: [[") }
        let sorted = summarizer.topicsFor(lines: Array(candidates), existing: Topic.topics(of: project, vault: vault))
        guard !sorted.isEmpty else { return Result(lines: 0, topics: 0) }
        // The model can take a minute: take the lines from the note as it is now, so a Keep made meanwhile stays.
        var moving: [(topic: String, items: [[String]])] = []
        try NoteIO.update(url, title: project) { current in
            var note = current
            moving = []
            for (topic, lines) in sorted.sorted(by: { $0.key < $1.key }) {
                let (rest, taken) = note.takingDecisions(Set(lines))
                guard !taken.isEmpty else { continue }
                note = rest
                moving.append((topic, taken))
            }
            return note
        }
        var result = Result(lines: 0, topics: 0)
        for (topic, items) in moving {
            try NoteIO.update(vault.url(Topic.path(project: project, topic: topic)), title: topic) { topicNote in
                items.reduce(topicNote) { $0.appendingDecisionItem($1) }
            }
            try Topic.link(project: project, topic: topic, vault: vault, now: now)
            result.lines += items.count
            result.topics += 1
        }
        return result
    }
}

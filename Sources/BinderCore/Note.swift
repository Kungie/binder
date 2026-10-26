import Foundation

/// A Markdown note in the vault: optional frontmatter, then a body that may hold one observed block
/// (rewritten by observers) and a `## Decisions` section (written only from approved proposals or by the user).
/// Everything else in the body belongs to the user and survives every rewrite byte for byte.
public struct Note: Equatable {
    public struct Field: Equatable {
        public var key: String
        public var value: String
        /// A frontmatter line without `key: value` shape, kept verbatim.
        var bare = false

        public init(key: String, value: String) {
            self.key = key
            self.value = value
        }

        var line: String { bare ? key : "\(key): \(value)" }
    }

    public static let observedStart = "<!-- binder:observed start -->"
    public static let observedEnd = "<!-- binder:observed end -->"
    static let decisionsHeading = "## Decisions"

    public var frontmatter: [Field]
    public var body: String

    public init(frontmatter: [Field] = [], body: String) {
        self.frontmatter = frontmatter
        self.body = body
    }

    public static func new(title: String) -> Note {
        Note(body: "# \(title)\n")
    }

    // MARK: Parsing and rendering

    public static func parse(_ text: String) -> Note {
        guard text.hasPrefix("---\n") else { return Note(body: text) }
        let rest = text.dropFirst(4)
        guard let close = rest.range(of: "\n---\n") ?? (rest.hasPrefix("---\n") ? rest.startIndex..<rest.index(rest.startIndex, offsetBy: 4) : nil)
        else { return Note(body: text) }
        let header = String(rest[rest.startIndex..<close.lowerBound])
        let fields = header.isEmpty ? [] : header.split(separator: "\n", omittingEmptySubsequences: false).map(field)
        return Note(frontmatter: fields, body: String(rest[close.upperBound...]))
    }

    private static func field(_ line: Substring) -> Field {
        guard let colon = line.firstIndex(of: ":") else {
            var bare = Field(key: String(line), value: "")
            bare.bare = true
            return bare
        }
        let key = line[..<colon].trimmingCharacters(in: .whitespaces)
        let value = line[line.index(after: colon)...].trimmingCharacters(in: .whitespaces)
        return Field(key: key, value: value)
    }

    public func render() -> String {
        guard !frontmatter.isEmpty else { return body }
        return "---\n" + frontmatter.map(\.line).joined(separator: "\n") + "\n---\n" + body
    }

    public func setting(_ key: String, _ value: String) -> Note {
        var note = self
        if let i = note.frontmatter.firstIndex(where: { $0.key == key && !$0.bare }) {
            note.frontmatter[i].value = value
        } else {
            note.frontmatter.append(Field(key: key, value: value))
        }
        return note
    }

    public func value(_ key: String) -> String? {
        frontmatter.first { $0.key == key && !$0.bare }?.value
    }

    // MARK: Observed block

    private var lines: [String] { body.components(separatedBy: "\n") }

    private static func isMarker(_ line: String, _ marker: String) -> Bool {
        line.trimmingCharacters(in: .whitespaces) == marker
    }

    private static func observedRange(in lines: [String]) -> (start: Int, end: Int)? {
        guard let start = lines.firstIndex(where: { isMarker($0, observedStart) }),
              let end = lines[(start + 1)...].firstIndex(where: { isMarker($0, observedEnd) })
        else { return nil }
        return (start, end)
    }

    public var observedLines: [String] {
        let all = lines
        guard let range = Self.observedRange(in: all) else { return [] }
        return Array(all[(range.start + 1)..<range.end])
    }

    public func replacingObserved(_ observed: [String]) -> Note {
        var all = lines
        let block = [Self.observedStart] + observed + [Self.observedEnd]
        if let range = Self.observedRange(in: all) {
            let tail = all[(range.end + 1)...].filter { !Self.isMarker($0, Self.observedStart) && !Self.isMarker($0, Self.observedEnd) }
            all = Array(all[..<range.start]) + block + tail
        } else {
            all.removeAll { Self.isMarker($0, Self.observedStart) || Self.isMarker($0, Self.observedEnd) }
            if let title = all.firstIndex(where: { $0.hasPrefix("# ") }) {
                all.insert(contentsOf: [""] + block, at: title + 1)
            } else {
                all.insert(contentsOf: block, at: 0)
            }
        }
        var note = self
        note.body = all.joined(separator: "\n")
        return note
    }

    // MARK: Decisions

    /// The section's line indexes: the heading, and the end (exclusive): the next heading or the end of the body.
    private static func decisionsSection(in lines: [String]) -> (heading: Int, end: Int)? {
        guard let heading = lines.firstIndex(where: { $0.trimmingCharacters(in: .whitespaces) == decisionsHeading }) else { return nil }
        let end = lines[(heading + 1)...].firstIndex(where: { $0.hasPrefix("#") }) ?? lines.count
        return (heading, end)
    }

    public var decisions: [String] {
        let all = lines
        guard let section = Self.decisionsSection(in: all) else { return [] }
        return all[(section.heading + 1)..<section.end].filter { $0.hasPrefix("- ") }.map { String($0.dropFirst(2)) }
    }

    public func appendingDecision(_ line: String) -> Note {
        var note = self
        var all = lines
        if let section = Self.decisionsSection(in: all) {
            let lastItem = all[(section.heading + 1)..<section.end].lastIndex(where: { $0.hasPrefix("- ") }) ?? section.heading
            all.insert("- \(line)", at: lastItem + 1)
            note.body = all.joined(separator: "\n")
        } else {
            var text = body
            if !text.isEmpty && !text.hasSuffix("\n") { text += "\n" }
            note.body = text + "\n\(Self.decisionsHeading)\n- \(line)\n"
        }
        return note
    }
}

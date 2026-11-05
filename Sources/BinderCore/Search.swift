import Foundation

/// One line that matches: the note it's in, the line as the note has it, what to show, and how well it matched.
public struct SearchHit: Equatable, Sendable {
    public var path: String
    public var line: String
    public var text: String
    public var score: Double
}

/// `binder search`: keyword ranking (BM25) over every line Binder keeps. No model, no network.
public enum Search {
    static let k1 = 1.2, b = 0.75

    struct Doc {
        var path: String
        var line: String
        var text: String
        var tokens: [String]
    }

    static func tokens(_ s: String) -> [String] {
        s.lowercased().unicodeScalars.split { !CharacterSet.alphanumerics.contains($0) }.map { String(String.UnicodeScalarView($0)) }
    }

    /// Every decision (with what's written under it), observed line and line of the user's own text.
    static func docs(_ vault: Vault, includeSuperseded: Bool) -> [Doc] {
        var docs: [Doc] = []
        for path in UsageHandles.notes(vault: vault) {
            guard let note = NoteIO.read(vault.url(path)) else { continue }
            func add(_ line: String, _ text: String) { docs.append(Doc(path: path, line: line, text: text, tokens: tokens(text))) }
            for item in note.decisionItems {
                let text = ([CardBuilder.decisionText(item.line)] + item.details).joined(separator: " · ")
                add(item.line, text)
            }
            for line in note.observedLines where line.hasPrefix("- ") { add(String(line.dropFirst(2)), String(line.dropFirst(2))) }
            for line in CardBuilder.freeLines(note) { add(line, line.hasPrefix("- ") ? String(line.dropFirst(2)) : line) }
            if includeSuperseded { for line in note.superseded { add(line, CardBuilder.decisionText(line)) } }
        }
        return docs
    }

    public static func run(_ query: String, vault: Vault, project: String? = nil, includeSuperseded: Bool = false, limit: Int = 12) -> [SearchHit] {
        let terms = Array(Set(tokens(query)))
        guard !terms.isEmpty, limit > 0 else { return [] }
        let docs = docs(vault, includeSuperseded: includeSuperseded)
        guard !docs.isEmpty else { return [] }
        let n = Double(docs.count)
        let average = docs.reduce(0) { $0 + Double($1.tokens.count) } / n
        var df: [String: Double] = [:]
        for doc in docs { for t in Set(doc.tokens) where terms.contains(t) { df[t, default: 0] += 1 } }
        let phrase = tokens(query).joined(separator: " ")
        var hits: [SearchHit] = []
        for doc in docs {
            var score = 0.0
            for term in terms {
                let tf = Double(doc.tokens.filter { $0 == term }.count)
                guard tf > 0, let d = df[term] else { continue }
                let idf = log(1 + (n - d + 0.5) / (d + 0.5))
                score += idf * tf * (k1 + 1) / (tf + k1 * (1 - b + b * Double(doc.tokens.count) / max(average, 1)))
            }
            guard score > 0 else { continue }
            if terms.count > 1, doc.tokens.joined(separator: " ").contains(phrase) { score += 2 }
            if let project, doc.path == "projects/\(project).md" || doc.path.hasPrefix("projects/\(project)/") { score *= 1.5 }
            hits.append(SearchHit(path: doc.path, line: doc.line, text: doc.text, score: score))
        }
        return Array(hits.sorted { $0.score != $1.score ? $0.score > $1.score : $0.path < $1.path }.prefix(limit))
    }

    /// `path: line`, best first, redacted, kept under `budget` characters.
    public static func render(_ hits: [SearchHit], budget: Int = 1_500) -> String {
        guard !hits.isEmpty else { return "Nothing in the binder matches." }
        var out: [String] = [], size = 0
        for hit in hits {
            let line = SecretFilter.redact("\(hit.path): \(hit.text)")
            guard size + line.count + 1 <= budget else { break }
            out.append(line)
            size += line.count + 1
        }
        return out.joined(separator: "\n")
    }
}

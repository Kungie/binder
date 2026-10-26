import Foundation

/// Turns commands an agent actually ran into proposals. Deterministic: no model involved.
public enum CommandRules {
    struct Rule {
        let regex: NSRegularExpression
        let kind: Proposal.Kind
        let projectBound: Bool
        let text: (_ project: String, _ capture: String?) -> String

        init(_ pattern: String, kind: Proposal.Kind = .decision, projectBound: Bool = true, text: @escaping (String, String?) -> String) {
            self.regex = try! NSRegularExpression(pattern: pattern)
            self.kind = kind
            self.projectBound = projectBound
            self.text = text
        }
    }

    /// Checked per command segment (split on `&&`, `||`, `;` and `|`); the first rule that matches a segment wins.
    static let rules: [Rule] = [
        Rule(#"\bwrangler\s+pages\s+deploy\b"#) { p, _ in "\(p) deploys to Cloudflare Pages." },
        Rule(#"\bwrangler\s+deploy\b"#) { p, _ in "\(p) deploys to Cloudflare Workers." },
        Rule(#"\bvercel\b.*(?:\bdeploy\b|--prod\b)"#) { p, _ in "\(p) deploys to Vercel." },
        Rule(#"\bnetlify\s+deploy\b"#) { p, _ in "\(p) deploys to Netlify." },
        Rule(#"\b(?:fly|flyctl)\s+(?:deploy|launch)\b"#) { p, _ in "\(p) deploys to Fly.io." },
        Rule(#"\bgh\s+repo\s+create\s+([^\s-][^\s]*)"#) { p, name in "\(p) has a GitHub repo, \(name ?? "")." },
        Rule(#"\bgit\s+remote\s+add\s+origin\s+(\S+)"#) { p, url in "\(p)'s remote is \(GitRepo.normalize(remote: url ?? ""))." },
        Rule(#"\bssh-keygen\b.*\s-f\s+(\S+)"#, kind: .access, projectBound: false) { _, path in
            "Key \(((path ?? "") as NSString).lastPathComponent) was created."
        },
        Rule(#"\bssh-copy-id\b.*\s(\S+)\s*$"#, kind: .access, projectBound: false) { _, target in
            "Your key was copied to \((target ?? "").split(separator: "@").last.map(String.init) ?? "")."
        }
    ]

    public static func proposals(for command: String, project: String?, session: String?) -> [Proposal] {
        guard !SecretFilter.containsSecret(command) else { return [] }
        var result: [Proposal] = []
        for segment in segments(command) {
            let range = NSRange(segment.startIndex..., in: segment)
            for rule in rules {
                guard let match = rule.regex.firstMatch(in: segment, range: range) else { continue }
                if rule.projectBound && project == nil { break }
                let capture = match.numberOfRanges > 1 ? Range(match.range(at: 1), in: segment).map { String(segment[$0]) } : nil
                let text = rule.text(project ?? "", capture)
                if !result.contains(where: { $0.text == text }) {
                    result.append(Proposal.make(kind: rule.kind, source: .command, text: text, evidence: segment,
                                                target: rule.projectBound ? "projects/\(project!).md" : "access/ssh.md",
                                                project: project, session: session, confidence: .high))
                }
                break
            }
        }
        return result
    }

    static func segments(_ command: String) -> [String] {
        command.components(separatedBy: CharacterSet(charactersIn: "\n"))
            .flatMap { $0.replacingOccurrences(of: "&&", with: "\u{1}").replacingOccurrences(of: "||", with: "\u{1}")
                .components(separatedBy: CharacterSet(charactersIn: "\u{1};|")) }
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }
    }
}

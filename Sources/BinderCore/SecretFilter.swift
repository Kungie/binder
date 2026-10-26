import Foundation

/// Finds things that look like secrets so they never reach a proposal, a card or the log.
/// It errs on the side of catching too much: a dropped proposal costs little, a stored token costs a lot.
public enum SecretFilter {
    struct Rule {
        let name: String
        let regex: NSRegularExpression

        init(_ name: String, _ pattern: String) {
            self.name = name
            self.regex = try! NSRegularExpression(pattern: pattern)
        }
    }

    /// Specific shapes first, so redaction names the most precise rule.
    static let rules: [Rule] = [
        Rule("private-key", #"-----BEGIN [A-Z ]*PRIVATE KEY-----"#),
        Rule("aws", #"AKIA[0-9A-Z]{16}"#),
        Rule("github", #"gh[pousr]_[A-Za-z0-9]{36,}|github_pat_[A-Za-z0-9_]{22,}"#),
        Rule("api-key", #"sk-(?:ant-)?[A-Za-z0-9_-]{20,}"#),
        Rule("slack", #"xox[abprs]-[A-Za-z0-9-]{10,}"#),
        Rule("url-credentials", #"://[^/\s:@]+:[^@\s/]+@"#),
        Rule("bearer", #"(?i)bearer\s+[A-Za-z0-9._~+/-]{20,}=*"#),
        // `TOKEN=…`, `password: …`, `secret_key = …`; the value must not be a plain number or already redacted.
        Rule("assignment", #"(?i)(?:password|passwd|secret|token|api[_-]?key)[A-Za-z_]*\s*[:=]\s*['"]?(?!\[REDACTED)(?![0-9]+\b)[^\s'"]{6,}"#)
    ]

    /// Not secrets, even though they look random: public keys and key fingerprints.
    static let harmless = try! NSRegularExpression(
        pattern: #"(?:ssh-(?:ed25519|rsa|dss)|ecdsa-sha2-nistp\d+) [A-Za-z0-9+/=]+|SHA256:[A-Za-z0-9+/=]+"#)

    static let candidate = try! NSRegularExpression(pattern: #"[A-Za-z0-9_+=-]{32,}"#)

    public static func findings(in text: String) -> [String] {
        var names = rules.filter { matches($0.regex, in: text) }.map(\.name)
        if names.isEmpty, !highEntropyRanges(in: text).isEmpty { names.append("high-entropy") }
        return names
    }

    public static func containsSecret(_ text: String) -> Bool {
        !findings(in: text).isEmpty
    }

    /// Replaces each match with `[REDACTED:<rule>]` and keeps everything around it.
    public static func redact(_ text: String) -> String {
        var result = text
        for rule in rules {
            result = rule.regex.stringByReplacingMatches(
                in: result, range: NSRange(result.startIndex..., in: result), withTemplate: "[REDACTED:\(rule.name)]")
        }
        for range in highEntropyRanges(in: result).reversed() {
            result.replaceSubrange(range, with: "[REDACTED:high-entropy]")
        }
        return result
    }

    // MARK: High entropy

    static func highEntropyRanges(in text: String) -> [Range<String.Index>] {
        let masked = mask(harmless, in: text)
        return candidate.matches(in: masked, range: NSRange(masked.startIndex..., in: masked)).compactMap { match in
            guard let range = Range(match.range, in: masked) else { return nil }
            let token = masked[range]
            guard !token.contains("[") && looksRandom(token) else { return nil }
            // Same offsets in the original: masking only swaps characters for spaces.
            return Range(match.range, in: text)
        }
    }

    /// Mixed case and digits, and more than 4.2 bits of entropy per character. Pure hex never qualifies.
    static func looksRandom(_ token: Substring) -> Bool {
        let hasUpper = token.contains(where: \.isUppercase)
        let hasLower = token.contains(where: \.isLowercase)
        let hasDigit = token.contains(where: \.isNumber)
        guard hasUpper && hasLower && hasDigit else { return false }
        return entropy(token) > 4.2
    }

    static func entropy(_ token: Substring) -> Double {
        var counts: [Character: Int] = [:]
        for c in token { counts[c, default: 0] += 1 }
        let n = Double(token.count)
        return counts.values.reduce(0) { sum, count in
            let p = Double(count) / n
            return sum - p * log2(p)
        }
    }

    private static func matches(_ regex: NSRegularExpression, in text: String) -> Bool {
        regex.firstMatch(in: text, range: NSRange(text.startIndex..., in: text)) != nil
    }

    /// Replaces every match of `regex` with spaces of the same UTF-16 length, so offsets still line up.
    private static func mask(_ regex: NSRegularExpression, in text: String) -> String {
        let ns = NSMutableString(string: text)
        for match in regex.matches(in: text, range: NSRange(location: 0, length: ns.length)).reversed() {
            ns.replaceCharacters(in: match.range, with: String(repeating: " ", count: match.range.length))
        }
        return ns as String
    }
}

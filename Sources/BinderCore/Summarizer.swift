import Foundation

/// Picks durable decisions out of what was said in a session. Only used after the user agreed.
public protocol Summarizer: Sendable {
    func proposals(from entries: [TranscriptEntry], project: String?) -> [Proposal]
}

/// Asks the user's own Claude Code (`claude -p`) on their own plan. Any failure means no proposals.
public struct ClaudeCLISummarizer: Summarizer {
    let runner: CommandRunner
    let session: String?

    public init(runner: CommandRunner, session: String?) {
        self.runner = runner
        self.session = session
    }

    static let limit = 12_000

    static let prompt = """
    You read part of a coding session and pick out durable things worth remembering in future sessions: \
    decisions, hosting and deploy choices, preferences the user stated, and access details. Never include secrets, \
    tokens or passwords. Ignore one-off experiments and anything obvious from the code itself. Reply with only a JSON \
    array and no other text: [{"kind":"decision|fact|preference|access","text":"one short sentence","confidence":"high|medium|low"}]. \
    Reply [] if nothing qualifies.

    SESSION:

    """

    public func proposals(from entries: [TranscriptEntry], project: String?) -> [Proposal] {
        let said = entries.compactMap { entry -> String? in
            switch entry {
            case let .user(text): "USER: \(text)"
            case let .assistant(text): "ASSISTANT: \(text)"
            default: nil
            }
        }
        guard !said.isEmpty else { return [] }
        let condensed = String(SecretFilter.redact(said.joined(separator: "\n")).suffix(Self.limit))
        guard let result = runner.run("claude", ["-p", "--model", "haiku", "--output-format", "text"],
                                      input: Self.prompt + condensed, timeout: 60),
              result.status == 0, !result.timedOut else { return [] }
        return parse(result.stdout, project: project)
    }

    func parse(_ reply: String, project: String?) -> [Proposal] {
        guard let start = reply.firstIndex(of: "["), let end = reply.lastIndex(of: "]"), start < end,
              let items = try? JSONSerialization.jsonObject(with: Data(reply[start...end].utf8)) as? [[String: Any]]
        else { return [] }
        return items.compactMap { item in
            guard let text = (item["text"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines), !text.isEmpty else { return nil }
            let kind = Proposal.Kind(rawValue: item["kind"] as? String ?? "") ?? .fact
            let confidence = Proposal.Confidence(rawValue: item["confidence"] as? String ?? "") ?? .medium
            let target = kind == .preference || project == nil ? "me.md" : "projects/\(project!).md"
            return Proposal.make(kind: kind, source: .chat, text: text, evidence: "said in session \(session ?? "?")",
                                 target: target, project: project, session: session, confidence: confidence)
        }
    }
}

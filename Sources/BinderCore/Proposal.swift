import Foundation

/// Something Binder caught and wants to remember, waiting for the user's Keep or Drop.
public struct Proposal: Codable, Equatable, Identifiable, Sendable {
    public enum Kind: String, Codable, Sendable { case decision, fact, preference, access, file }
    public enum Source: String, Codable, Sendable { case command, observed, chat }
    public enum Confidence: String, Codable, Sendable { case high, medium, low }

    public var id: String
    public var createdAt: Date
    public var session: String?
    public var agent: String
    public var project: String?
    public var kind: Kind
    public var source: Source
    public var evidence: String
    public var text: String
    /// Vault-relative path of the note it goes into, e.g. `projects/dawn.md`.
    public var target: String
    public var confidence: Confidence
    /// A few sentences on why, shown in detailed mode and written under the line on Keep.
    public var detail: String? = nil
    /// What was said in the session, rendered in English.
    public var quote: String? = nil
    /// Other notes the line also goes into on Keep (a server, a hosting provider), as vault paths.
    public var also: [String] = []
    /// For a `.file` catch: the waiting copy's name in `.binder/inbox/files`, and its SHA-256.
    public var file: String? = nil
    public var fileHash: String? = nil

    public static func make(kind: Kind, source: Source, text: String, evidence: String, target: String,
                            project: String?, session: String?, confidence: Confidence = .medium,
                            agent: String = "claude-code", now: Date = .now,
                            detail: String? = nil, quote: String? = nil, also: [String] = []) -> Proposal {
        let id = "p_" + (0..<8).map { _ in String("0123456789abcdef".randomElement()!) }.joined()
        return Proposal(id: id, createdAt: now, session: session, agent: agent, project: project, kind: kind,
                        source: source, evidence: evidence, text: text, target: target, confidence: confidence,
                        detail: detail, quote: quote, also: also)
    }

    /// What detailed mode shows under the line: the model's explanation, or for a command or a look around, its evidence.
    public var detailText: String? {
        if let detail { return detail }
        return source == .chat ? nil : evidence
    }

    /// How a kept proposal is labelled in the note's Decisions.
    var provenance: String {
        switch source {
        case .command: "kept from a command"
        case .chat: "kept from the chat"
        case .observed: "seen on this Mac"
        }
    }
}

extension Proposal {
    /// Fields added after 1.0 are optional, so inbox files written by an older version still load.
    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decode(String.self, forKey: .id)
        createdAt = try c.decode(Date.self, forKey: .createdAt)
        session = try c.decodeIfPresent(String.self, forKey: .session)
        agent = try c.decode(String.self, forKey: .agent)
        project = try c.decodeIfPresent(String.self, forKey: .project)
        kind = try c.decode(Kind.self, forKey: .kind)
        source = try c.decode(Source.self, forKey: .source)
        evidence = try c.decode(String.self, forKey: .evidence)
        text = try c.decode(String.self, forKey: .text)
        target = try c.decode(String.self, forKey: .target)
        confidence = try c.decode(Confidence.self, forKey: .confidence)
        detail = try c.decodeIfPresent(String.self, forKey: .detail)
        quote = try c.decodeIfPresent(String.self, forKey: .quote)
        also = try c.decodeIfPresent([String].self, forKey: .also) ?? []
        file = try c.decodeIfPresent(String.self, forKey: .file)
        fileHash = try c.decodeIfPresent(String.self, forKey: .fileHash)
    }
}

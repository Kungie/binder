import Foundation

/// Something Binder caught and wants to remember, waiting for the user's Keep or Drop.
public struct Proposal: Codable, Equatable, Identifiable, Sendable {
    public enum Kind: String, Codable, Sendable { case decision, fact, preference, access }
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

    public static func make(kind: Kind, source: Source, text: String, evidence: String, target: String,
                            project: String?, session: String?, confidence: Confidence = .medium,
                            agent: String = "claude-code", now: Date = .now) -> Proposal {
        let id = "p_" + (0..<8).map { _ in String("0123456789abcdef".randomElement()!) }.joined()
        return Proposal(id: id, createdAt: now, session: session, agent: agent, project: project, kind: kind,
                        source: source, evidence: evidence, text: text, target: target, confidence: confidence)
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

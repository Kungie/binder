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

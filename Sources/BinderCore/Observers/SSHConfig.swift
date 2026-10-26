import Foundation

public struct SSHHost: Equatable, Sendable {
    public var alias: String
    public var hostName: String?
    public var user: String?
    public var port: Int?
    public var identityFiles: [String] = []

    /// `~/.ssh/id_ed25519_hetzner` → `id_ed25519_hetzner`.
    public var keyNames: [String] { identityFiles.map { ($0 as NSString).lastPathComponent } }
}

/// A small `ssh_config` reader: Host blocks, HostName, User, Port, IdentityFile, and Include with globs.
/// Wildcard and negated patterns are skipped, `Match` blocks are ignored.
public enum SSHConfig {
    public static func parse(_ text: String, includeBase: URL) -> [SSHHost] {
        var files = 0
        return parse(text, includeBase: includeBase, filesRead: &files)
    }

    /// Reads `config` and everything it includes. `filesRead` counts each file opened.
    public static func load(_ config: URL) -> (hosts: [SSHHost], filesRead: Int) {
        guard let text = try? String(contentsOf: config, encoding: .utf8) else { return ([], 0) }
        var files = 1
        let hosts = parse(text, includeBase: config.deletingLastPathComponent(), filesRead: &files)
        return (hosts, files)
    }

    static func parse(_ text: String, includeBase: URL, filesRead: inout Int, depth: Int = 0) -> [SSHHost] {
        var hosts: [SSHHost] = []
        var current: [Int] = [] // indexes in `hosts` the active Host line applies to
        var inMatch = false

        for raw in text.split(separator: "\n", omittingEmptySubsequences: true) {
            let line = raw.trimmingCharacters(in: .whitespaces)
            guard !line.isEmpty, !line.hasPrefix("#") else { continue }
            let (keyword, value) = split(line)
            switch keyword {
            case "host":
                inMatch = false
                current = []
                for pattern in value.split(separator: " ").map(String.init) where isPlainAlias(pattern) {
                    hosts.append(SSHHost(alias: pattern))
                    current.append(hosts.count - 1)
                }
            case "match":
                inMatch = true
                current = []
            case "include" where depth < 4:
                for file in expand(value, base: includeBase) {
                    guard let included = try? String(contentsOf: file, encoding: .utf8) else { continue }
                    filesRead += 1
                    hosts += parse(included, includeBase: includeBase, filesRead: &filesRead, depth: depth + 1)
                }
            case "hostname" where !inMatch:
                for i in current where hosts[i].hostName == nil { hosts[i].hostName = value }
            case "user" where !inMatch:
                for i in current where hosts[i].user == nil { hosts[i].user = value }
            case "port" where !inMatch:
                for i in current where hosts[i].port == nil { hosts[i].port = Int(value) }
            case "identityfile" where !inMatch:
                for i in current { hosts[i].identityFiles.append(value) }
            default:
                continue
            }
        }
        return hosts
    }

    /// `Keyword value`, `Keyword=value` or `Keyword = value`; the keyword lower-cased.
    static func split(_ line: String) -> (String, String) {
        let separators = CharacterSet.whitespaces.union(CharacterSet(charactersIn: "="))
        guard let cut = line.rangeOfCharacter(from: separators) else { return (line.lowercased(), "") }
        let keyword = line[..<cut.lowerBound].lowercased()
        let value = line[cut.lowerBound...].trimmingCharacters(in: separators)
        return (keyword, value.trimmingCharacters(in: CharacterSet(charactersIn: "\"")))
    }

    static func isPlainAlias(_ pattern: String) -> Bool {
        !pattern.isEmpty && !pattern.contains("*") && !pattern.contains("?") && !pattern.hasPrefix("!")
    }

    /// Include paths are relative to ~/.ssh; the last component may be a glob.
    static func expand(_ value: String, base: URL) -> [URL] {
        value.split(separator: " ").flatMap { item -> [URL] in
            var path = String(item)
            if path.hasPrefix("~/") { path = base.deletingLastPathComponent().appendingPathComponent(String(path.dropFirst(2))).path }
            let url = path.hasPrefix("/") ? URL(fileURLWithPath: path) : base.appendingPathComponent(path)
            let pattern = url.lastPathComponent
            guard pattern.contains("*") || pattern.contains("?") else { return [url] }
            let dir = url.deletingLastPathComponent()
            let names = (try? FileManager.default.contentsOfDirectory(atPath: dir.path)) ?? []
            return names.sorted().filter { fnmatch(pattern, $0, 0) == 0 }.map { dir.appendingPathComponent($0) }
        }
    }
}

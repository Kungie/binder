import Foundation

public struct GraphNode: Equatable, Sendable, Identifiable {
    public enum Kind: String, Sendable {
        /// The centre: you (me.md, if it exists yet).
        case me
        /// A folder of notes: machines, access, servers, projects, handoffs.
        case folder
        case note
        /// Something several notes share, like a language or a deploy target.
        case facet
        /// A `[[wikilink]]` to a note that doesn't exist yet.
        case missing
    }

    public var id: String
    public var label: String
    public var kind: Kind
    /// The vault-relative path, for nodes that are files.
    public var path: String?
}

public struct GraphEdge: Hashable, Sendable {
    public enum Kind: String, Sendable { case structure, link, handoff, facet }

    /// The two ends, in sorted order.
    public var a: String
    public var b: String
    public var kind: Kind

    public init(_ x: String, _ y: String, kind: Kind) {
        a = min(x, y)
        b = max(x, y)
        self.kind = kind
    }
}

/// The binder as a network, Obsidian style: you in the middle, folders around you, notes on the folders,
/// and cross links from `[[wikilinks]]`, handoffs to their project, and facts several notes share.
public struct VaultGraph: Sendable {
    public private(set) var nodes: [GraphNode] = []
    public private(set) var edges: [GraphEdge] = []
    private var index: [String: Int] = [:]
    private var edgeIndex: [String: Int] = [:]

    static let folders = ["machines", "access", "servers", "projects", "handoffs"]
    /// Observed facts that become shared nodes, and how their values split.
    static let facetKeys = ["languages", "deploy", "domains"]

    public init() {}

    public func node(_ id: String) -> GraphNode? { index[id].map { nodes[$0] } }

    public func edge(_ x: String, _ y: String) -> GraphEdge? { edgeIndex[Self.key(x, y)].map { edges[$0] } }

    public func neighbours(of id: String) -> Set<String> {
        Set(edges.compactMap { $0.a == id ? $0.b : $0.b == id ? $0.a : nil })
    }

    static func key(_ x: String, _ y: String) -> String { min(x, y) + "\u{1F}" + max(x, y) }

    mutating func add(_ node: GraphNode) {
        guard index[node.id] == nil else { return }
        index[node.id] = nodes.count
        nodes.append(node)
    }

    mutating func connect(_ x: String, _ y: String, _ kind: GraphEdge.Kind) {
        let k = Self.key(x, y)
        guard x != y, edgeIndex[k] == nil else { return }
        edgeIndex[k] = edges.count
        edges.append(GraphEdge(x, y, kind: kind))
    }

    public static func build(vault: Vault) -> VaultGraph {
        var g = VaultGraph()
        let fm = FileManager.default
        g.add(GraphNode(id: "me", label: "ME", kind: .me, path: fm.fileExists(atPath: vault.url("me.md").path) ? "me.md" : nil))

        var notes: [(path: String, note: Note)] = []
        if let me = NoteIO.read(vault.url("me.md")) { notes.append(("me.md", me)) }
        func mdFiles(_ dir: String) -> [String] {
            ((try? fm.contentsOfDirectory(atPath: vault.url(dir).path)) ?? []).filter { $0.hasSuffix(".md") }.sorted()
        }

        for folder in folders where folder != "handoffs" {
            let files = mdFiles(folder)
            guard !files.isEmpty else { continue }
            let folderID = "folder:\(folder)"
            g.add(GraphNode(id: folderID, label: folder.uppercased(), kind: .folder))
            g.connect("me", folderID, .structure)
            for file in files {
                let path = "\(folder)/\(file)"
                let note = NoteIO.read(vault.url(path))
                g.add(GraphNode(id: path, label: note.flatMap(title) ?? String(file.dropLast(3)), kind: .note, path: path))
                g.connect(folderID, path, .structure)
                if let note { notes.append((path, note)) }
            }
        }

        let projects = ((try? fm.contentsOfDirectory(atPath: vault.url("handoffs").path)) ?? []).sorted()
        let handoffs = projects.flatMap { project in mdFiles("handoffs/\(project)").map { (project, $0) } }
        if !handoffs.isEmpty {
            g.add(GraphNode(id: "folder:handoffs", label: "HANDOFFS", kind: .folder))
            g.connect("me", "folder:handoffs", .structure)
            for (project, file) in handoffs {
                let path = "handoffs/\(project)/\(file)"
                g.add(GraphNode(id: path, label: "\(project) · \(file.dropLast(3))", kind: .note, path: path))
                g.connect("folder:handoffs", path, .structure)
                if g.node("projects/\(project).md") != nil { g.connect(path, "projects/\(project).md", .handoff) }
            }
        }

        // Wikilinks resolve by file name or title, ignoring case, like Obsidian.
        var byName: [String: String] = ["me": "me"]
        for node in g.nodes where node.kind == .note {
            if let path = node.path { byName[((path as NSString).lastPathComponent as NSString).deletingPathExtension.lowercased()] = node.id }
            byName[node.label.lowercased()] = byName[node.label.lowercased()] ?? node.id
        }
        for (path, note) in notes {
            let from = path == "me.md" ? "me" : path
            for target in CardBuilder.links(in: note.render()) {
                let name = target.trimmingCharacters(in: .whitespaces)
                if let to = byName[name.lowercased()] {
                    g.connect(from, to, .link)
                } else {
                    let id = "missing:\(name.lowercased())"
                    g.add(GraphNode(id: id, label: name, kind: .missing))
                    g.connect(from, id, .link)
                }
            }
        }

        // Facets: only values at least two notes share, so each one actually ties things together.
        var facets: [String: (label: String, notes: [String])] = [:]
        var order: [String] = []
        for (path, note) in notes {
            for line in note.observedLines {
                guard line.hasPrefix("- "), let colon = line.range(of: ": ") else { continue }
                let key = line[line.index(line.startIndex, offsetBy: 2)..<colon.lowerBound].lowercased()
                guard facetKeys.contains(key) else { continue }
                for value in line[colon.upperBound...].split(separator: ",").map({ $0.trimmingCharacters(in: .whitespaces) }) where !value.isEmpty {
                    let id = "facet:\(value.lowercased())"
                    if facets[id] == nil { order.append(id) }
                    facets[id, default: (value, [])].notes.append(path == "me.md" ? "me" : path)
                }
            }
        }
        for id in order {
            guard let facet = facets[id], Set(facet.notes).count >= 2 else { continue }
            g.add(GraphNode(id: id, label: facet.label, kind: .facet))
            for path in facet.notes { g.connect(id, path, .facet) }
        }
        return g
    }

    static func title(_ note: Note) -> String? {
        note.body.components(separatedBy: "\n").first { $0.hasPrefix("# ") }.map { String($0.dropFirst(2)) }
    }
}

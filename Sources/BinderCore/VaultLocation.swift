import Foundation

/// Where the binder folder lives, shared by the app and the CLI:
/// `~/Library/Application Support/Binder/config.json` holds `{"vault": "<path>"}`.
public enum VaultLocation {
    public static func configURL(home: URL) -> URL {
        home.appendingPathComponent("Library/Application Support/Binder/config.json")
    }

    public static func load(home: URL) -> URL? {
        guard let data = try? Data(contentsOf: configURL(home: home)),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let path = json["vault"] as? String, !path.isEmpty else { return nil }
        return URL(fileURLWithPath: path, isDirectory: true)
    }

    public static func save(_ root: URL, home: URL) throws {
        let url = configURL(home: home)
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        let data = try JSONSerialization.data(withJSONObject: ["vault": root.standardizedFileURL.path],
                                              options: [.prettyPrinted, .withoutEscapingSlashes])
        try data.write(to: url, options: .atomic)
    }

    /// `BINDER_VAULT`, then the pointer, then `~/Binder`.
    public static func resolve(env: [String: String], home: URL) -> URL {
        if let override = env["BINDER_VAULT"], !override.isEmpty { return URL(fileURLWithPath: override, isDirectory: true) }
        return load(home: home) ?? home.appendingPathComponent("Binder", isDirectory: true)
    }
}

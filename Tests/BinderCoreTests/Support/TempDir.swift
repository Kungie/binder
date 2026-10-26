import Foundation

/// A fresh, empty folder for one test. Not cleaned up: the OS clears the temp folder.
func makeTempDir(_ name: String = "binder-test") -> URL {
    let url = FileManager.default.temporaryDirectory
        .appendingPathComponent("\(name)-\(UUID().uuidString)", isDirectory: true)
    try! FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    return url
}

func write(_ text: String, to url: URL) {
    try! FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
    try! text.write(to: url, atomically: true, encoding: .utf8)
}

func read(_ url: URL) -> String? {
    try? String(contentsOf: url, encoding: .utf8)
}

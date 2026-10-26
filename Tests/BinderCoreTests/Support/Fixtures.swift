import Foundation

/// A file or folder under Tests/BinderCoreTests/Fixtures.
func fixture(_ path: String) -> URL {
    Bundle.module.resourceURL!.appendingPathComponent("Fixtures").appendingPathComponent(path)
}

import Testing
@testable import BinderCore

@Test func versionIsSet() {
    #expect(Binder.version.hasPrefix("1."))
}

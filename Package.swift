// swift-tools-version:6.0
import PackageDescription

let package = Package(
    name: "Binder",
    platforms: [.macOS(.v14)],
    products: [
        .executable(name: "binder", targets: ["BinderCLI"])
    ],
    targets: [
        .target(name: "BinderCore", path: "Sources/BinderCore"),
        .executableTarget(name: "BinderCLI", dependencies: ["BinderCore"], path: "Sources/BinderCLI"),
        .testTarget(
            name: "BinderCoreTests",
            dependencies: ["BinderCore", "BinderCLI"],
            path: "Tests/BinderCoreTests",
            resources: [.copy("Fixtures")]
        )
    ],
    swiftLanguageModes: [.v5]
)

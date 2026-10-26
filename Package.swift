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
        .target(
            name: "BinderUI",
            dependencies: ["BinderCore"],
            path: "Sources/BinderUI",
            resources: [.copy("Resources/Fonts")]
        ),
        .executableTarget(name: "BinderApp", dependencies: ["BinderUI"], path: "Sources/BinderApp"),
        .testTarget(
            name: "BinderCoreTests",
            dependencies: ["BinderCore", "BinderCLI", "BinderUI"],
            path: "Tests/BinderCoreTests",
            resources: [.copy("Fixtures")]
        )
    ],
    swiftLanguageModes: [.v5]
)

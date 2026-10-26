import Foundation

public struct CommandResult: Equatable, Sendable {
    public var status: Int32
    public var stdout: String
    public var stderr: String
    public var timedOut: Bool

    public init(status: Int32, stdout: String, stderr: String, timedOut: Bool) {
        self.status = status
        self.stdout = stdout
        self.stderr = stderr
        self.timedOut = timedOut
    }
}

/// Runs a command-line tool. Returns nil when the tool isn't installed.
public protocol CommandRunner: Sendable {
    func run(_ tool: String, _ args: [String], input: String?, timeout: TimeInterval) -> CommandResult?
}

/// Runs real processes. Looks tools up on PATH plus the usual Homebrew and user folders,
/// because apps and hooks often start with a bare PATH.
public struct SystemRunner: CommandRunner {
    /// Added to the child's environment, e.g. `BINDER_NESTED=1` so a nested `claude -p` doesn't re-enter Binder's hooks.
    let extraEnvironment: [String: String]

    public init(extraEnvironment: [String: String] = [:]) {
        self.extraEnvironment = extraEnvironment
    }

    static let extraPaths = ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin", "/usr/sbin", "/sbin",
                             NSHomeDirectory() + "/.local/bin", NSHomeDirectory() + "/.npm-global/bin", NSHomeDirectory() + "/.bun/bin"]

    static var searchPath: [String] {
        let env = (ProcessInfo.processInfo.environment["PATH"] ?? "").split(separator: ":").map(String.init)
        var seen = Set<String>()
        return (env + extraPaths).filter { seen.insert($0).inserted }
    }

    public static func resolve(_ tool: String) -> String? {
        let fm = FileManager.default
        if tool.contains("/") { return fm.isExecutableFile(atPath: tool) ? tool : nil }
        return searchPath.map { "\($0)/\(tool)" }.first { fm.isExecutableFile(atPath: $0) }
    }

    public func run(_ tool: String, _ args: [String], input: String?, timeout: TimeInterval) -> CommandResult? {
        guard let path = Self.resolve(tool) else { return nil }
        let process = Process()
        process.executableURL = URL(fileURLWithPath: path)
        process.arguments = args
        var env = ProcessInfo.processInfo.environment
        env["PATH"] = Self.searchPath.joined(separator: ":")
        env.merge(extraEnvironment) { _, new in new }
        process.environment = env

        let out = Pipe(), err = Pipe()
        process.standardOutput = out
        process.standardError = err
        let inPipe = input.map { _ in Pipe() }
        process.standardInput = inPipe ?? FileHandle.nullDevice

        let finished = DispatchSemaphore(value: 0)
        process.terminationHandler = { _ in finished.signal() }
        do { try process.run() } catch { return nil }

        // Drain both pipes in the background so a chatty tool can't block on a full pipe.
        let group = DispatchGroup()
        nonisolated(unsafe) var outData = Data(), errData = Data()
        DispatchQueue.global().async(group: group) { outData = out.fileHandleForReading.readDataToEndOfFile() }
        DispatchQueue.global().async(group: group) { errData = err.fileHandleForReading.readDataToEndOfFile() }
        if let inPipe, let input {
            try? inPipe.fileHandleForWriting.write(contentsOf: Data(input.utf8))
            try? inPipe.fileHandleForWriting.close()
        }

        var timedOut = false
        if finished.wait(timeout: .now() + timeout) == .timedOut {
            timedOut = true
            process.terminate()
            if finished.wait(timeout: .now() + 0.3) == .timedOut {
                kill(process.processIdentifier, SIGKILL)
                _ = finished.wait(timeout: .now() + 0.3)
            }
        }
        _ = group.wait(timeout: .now() + 0.5)
        return CommandResult(status: process.isRunning ? -1 : process.terminationStatus,
                             stdout: String(decoding: outData, as: UTF8.self),
                             stderr: String(decoding: errData, as: UTF8.self),
                             timedOut: timedOut)
    }
}

import BinderCore
import Foundation

let usage = """
binder \(Binder.version)
A memory for your AI agents.

Usage:
  binder observe [--lan]  Look around this Mac and update your binder
  binder card             Print the context card (called by the SessionStart hook)
  binder capture          Catch what a session decided (called by the Stop hook)
  binder handoff [--cwd <dir>]
                          Write a handoff card for a fresh session
  binder tidy [--all]     Move stale and repeated lines to Superseded
  binder install          Add Binder's hooks to Claude Code
  binder uninstall        Take them out again
  binder doctor           Check that everything is wired up
  binder help             Show this
"""

let args = Array(CommandLine.arguments.dropFirst())
let command = args.first ?? "help"
let paths = Paths.current()

/// Hook mode never gets in a session's way: a nested `claude -p` skips it entirely, a 2 s watchdog ends it,
/// it prints nothing but the hook reply, and it always exits 0.
func runHook(_ body: (Data) -> Void) -> Never {
    if ProcessInfo.processInfo.environment["BINDER_NESTED"] == "1" { exit(0) }
    let watchdog = DispatchSource.makeTimerSource(queue: .global())
    watchdog.schedule(deadline: .now() + 2)
    watchdog.setEventHandler { exit(0) }
    watchdog.resume()
    body(FileHandle.standardInput.readDataToEndOfFile())
    exit(0)
}

switch command {
case "card":
    runHook { Commands.card(paths, stdin: $0) }
case "capture":
    runHook { Commands.capture(paths, stdin: $0) }
case "summarize":
    Commands.summarize(paths, Array(args.dropFirst()))
    exit(0)
case "observe":
    exit(Commands.observe(paths, lan: args.contains("--lan")))
case "handoff":
    let cwd = args.firstIndex(of: "--cwd").flatMap { args.indices.contains($0 + 1) ? args[$0 + 1] : nil }
    exit(Commands.handoff(paths, cwd: cwd ?? FileManager.default.currentDirectoryPath))
case "tidy":
    exit(Commands.tidy(paths, everything: args.contains("--all")))
case "install":
    exit(Commands.install(paths, remove: false))
case "uninstall":
    exit(Commands.install(paths, remove: true))
case "doctor":
    exit(Commands.doctor(paths))
case "help", "--help", "-h":
    print(usage)
default:
    print(usage)
    exit(2)
}

import BinderCore

let usage = """
binder \(Binder.version)
A memory for your AI agents.

Usage:
  binder observe      Look around this Mac and update your binder
  binder card         Print the context card (called by the SessionStart hook)
  binder capture      Catch what a session decided (called by the Stop hook)
  binder handoff      Write a handoff card for a fresh session
  binder install      Add Binder's hooks to Claude Code
  binder uninstall    Take them out again
  binder doctor       Check that everything is wired up
  binder help         Show this
"""

print(usage)

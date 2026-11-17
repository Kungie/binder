import { useEffect, useRef } from "react";

/**
 * Calls `fn` every `ms` while the window is visible, and once when it shows again after being hidden: a hidden
 * Binder asks the server nothing. Returns the stop.
 */
export function whileVisible(fn: () => unknown, ms: number): () => void {
  const run = () => {
    if (!document.hidden) void fn();
  };
  const timer = setInterval(run, ms);
  document.addEventListener("visibilitychange", run);
  return () => {
    clearInterval(timer);
    document.removeEventListener("visibilitychange", run);
  };
}

/** Runs `fn` now, then every `ms` while the window is visible. Pauses while it's hidden and catches up on return. */
export function usePoll(fn: () => unknown, ms: number) {
  const ref = useRef(fn);
  ref.current = fn;
  useEffect(() => {
    if (!document.hidden) void ref.current();
    return whileVisible(() => ref.current(), ms);
  }, [ms]);
}

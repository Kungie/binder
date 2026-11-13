import { useEffect, useRef } from "react";

/** Runs `fn` now, then every `ms` while the window is visible. Pauses while it's hidden and catches up on return. */
export function usePoll(fn: () => unknown, ms: number) {
  const ref = useRef(fn);
  ref.current = fn;
  useEffect(() => {
    const run = () => {
      if (!document.hidden) void ref.current();
    };
    run();
    const timer = setInterval(run, ms);
    document.addEventListener("visibilitychange", run);
    return () => {
      clearInterval(timer);
      document.removeEventListener("visibilitychange", run);
    };
  }, [ms]);
}

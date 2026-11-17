import { afterEach, describe, expect, it, vi } from "vitest";
import { whileVisible } from "./usePoll";

const hide = (hidden: boolean) => {
  Object.defineProperty(document, "hidden", { configurable: true, get: () => hidden });
  document.dispatchEvent(new Event("visibilitychange"));
};

afterEach(() => {
  hide(false);
  vi.useRealTimers();
});

describe("whileVisible", () => {
  it("skips_while_the_window_is_hidden_and_catches_up_on_return", () => {
    vi.useFakeTimers();
    const fn = vi.fn();
    const stop = whileVisible(fn, 1000);
    vi.advanceTimersByTime(1000);
    expect(fn).toHaveBeenCalledTimes(1);
    hide(true);
    vi.advanceTimersByTime(5000);
    expect(fn).toHaveBeenCalledTimes(1);
    hide(false);
    expect(fn).toHaveBeenCalledTimes(2);
    stop();
    vi.advanceTimersByTime(5000);
    expect(fn).toHaveBeenCalledTimes(2);
  });
});

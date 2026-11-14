/** Whether to animate: off when macOS asks to reduce motion, and where there is no way to ask (tests). */
export function animates(): boolean {
  return typeof window.matchMedia === "function" && !window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

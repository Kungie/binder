import { ClipboardAddon } from "@xterm/addon-clipboard";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { useEffect, useRef } from "react";
import { terminalClipboard } from "../lib/clipboard";

/** The addon's runtime constructor is `(base64?, provider?)`; its 0.1.0 typings list only one argument. */
const Clipboard = ClipboardAddon as unknown as new (base64: undefined, provider: ReturnType<typeof terminalClipboard>) => ClipboardAddon;

const FONT = "Geist Mono, ui-monospace, monospace";

/** Binder's palette for the 16 ANSI colours; the bright ones a step lighter. */
const THEME = {
  background: "#141416",
  foreground: "#EDEDED",
  cursor: "#EDEDED",
  cursorAccent: "#141416",
  selectionBackground: "#242428",
  black: "#1C1C1F",
  red: "#EB5757",
  green: "#4CB782",
  yellow: "#F2C94C",
  blue: "#5E6AD2",
  magenta: "#8B7CF6",
  cyan: "#26B5CE",
  white: "#EDEDED",
  brightBlack: "#6B6B6B",
  brightRed: "#F07A7A",
  brightGreen: "#6FCB9C",
  brightYellow: "#F6D774",
  brightBlue: "#8089E0",
  brightMagenta: "#A89CF9",
  brightCyan: "#52C8DC",
  brightWhite: "#FFFFFF",
};

export interface XtermProps {
  onData(data: string): void;
  onResize(cols: number, rows: number): void;
  /** Puts text on the Mac clipboard. */
  onCopy(text: string): void;
  /** Called once the terminal exists, with a way to write output to it and to focus it. */
  register(write: (bytes: Uint8Array) => void, focus: () => void): void;
}

/** xterm.js in a box that fills its parent. The only file that imports xterm. */
export function XtermView({ onData, onResize, onCopy, register }: XtermProps) {
  const host = useRef<HTMLDivElement>(null);
  const handlers = useRef({ onData, onResize, onCopy });
  handlers.current = { onData, onResize, onCopy };

  useEffect(() => {
    const el = host.current;
    if (!el) return;
    let done = false;
    let cleanup = () => {};
    // xterm measures the font when it opens, so wait for Geist Mono first.
    void document.fonts
      .load(`12px "Geist Mono"`)
      .catch(() => [])
      .then(() => {
        if (done) return;
        const term = new Terminal({
          fontFamily: FONT,
          fontSize: 12,
          lineHeight: 1.3,
          cursorBlink: true,
          cursorStyle: "block",
          macOptionClickForcesSelection: true,
          scrollback: 0,
          theme: THEME,
        });
        const fit = new FitAddon();
        term.loadAddon(fit);
        term.loadAddon(new Clipboard(undefined, terminalClipboard((t) => handlers.current.onCopy(t))));
        term.open(el);
        // ⌘ keys belong to Binder and the Mac: ⌘C copies a native selection and never sends Ctrl-C,
        // ⌘V is the browser's paste, ⌘K, ⌘1 and ⌘2 reach Binder's own listeners.
        term.attachCustomKeyEventHandler((e) => {
          if (!e.metaKey) return true;
          if (e.type === "keydown" && e.key.toLowerCase() === "c" && term.hasSelection()) handlers.current.onCopy(term.getSelection());
          return false;
        });
        const sub = term.onData((d) => handlers.current.onData(d));
        let last = "";
        const resize = () => {
          if (!el.offsetWidth || !el.offsetHeight) return; // hidden: keep the last size
          fit.fit();
          const size = `${term.cols}x${term.rows}`;
          if (size !== last) {
            last = size;
            handlers.current.onResize(term.cols, term.rows);
          }
        };
        const observer = new ResizeObserver(resize);
        observer.observe(el);
        resize();
        register(
          (bytes) => term.write(bytes),
          () => term.focus(),
        );
        cleanup = () => {
          observer.disconnect();
          sub.dispose();
          term.dispose();
        };
      });
    return () => {
      done = true;
      cleanup();
    };
  }, [register]);

  return <div ref={host} className="h-full w-full" />;
}

/**
 * The terminals' clipboard for OSC 52. The server may set the Mac clipboard (a tmux selection lands there), so the
 * text goes through the app, which needs no click. It may never read it: the clipboard can hold a password.
 */
export function terminalClipboard(copy: (text: string) => void) {
  return {
    readText: (_selection: string) => "",
    // The addon passes "" for a payload it can't decode; that must not wipe the clipboard.
    writeText: (_selection: string, text: string) => {
      if (text) copy(text);
    },
  };
}

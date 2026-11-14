/** Release notes as plain text: Markdown's marks go, the words and line breaks stay. */
export function plainText(markdown: string): string {
  return markdown
    .split("\n")
    .map((line) =>
      line
        .replace(/^\s{0,3}#{1,6}\s+/, "")
        .replace(/^\s*(?:[-*+]|\d+[.)])\s+/, "")
        .replace(/!?\[([^\]]*)\]\([^)]*\)/g, "$1")
        .replace(/(\*\*|__|\*|_|`)/g, "")
        .trimEnd(),
    )
    .join("\n")
    .trim();
}

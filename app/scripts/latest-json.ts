// The updater's latest.json for a release: `node app/scripts/latest-json.ts <version> <notes file> <sig file>`.
import { readFileSync } from "node:fs";

const REPO = "https://github.com/Kungie/binder";

export function latestJson(version: string, notes: string, signature: string, date: Date) {
  return {
    version,
    notes,
    pub_date: date.toISOString(),
    platforms: {
      "darwin-aarch64": { signature, url: `${REPO}/releases/download/v${version}/Binder.app.tar.gz` },
    },
  };
}

if (import.meta.main) {
  const [version, notesFile, sigFile] = process.argv.slice(2);
  if (!version || !notesFile || !sigFile) {
    console.error("Usage: latest-json.ts <version> <notes file> <sig file>");
    process.exit(1);
  }
  const read = (f: string) => readFileSync(f, "utf8").trim();
  console.log(JSON.stringify(latestJson(version, read(notesFile), read(sigFile), new Date()), null, 2));
}

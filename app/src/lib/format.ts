/** Bytes the way Finder counts them (1000s): "840 MB", "2.3 GB", "2 TB". */
export function bytes(n: number): string {
  if (n < 1000) return `${n} B`;
  const units = ["KB", "MB", "GB", "TB", "PB"];
  let v = n;
  let i = -1;
  do {
    v /= 1000;
    i++;
  } while (v >= 1000 && i < units.length - 1);
  const shown = v < 10 ? String(Math.round(v * 10) / 10) : String(Math.round(v));
  return `${shown} ${units[i]}`;
}

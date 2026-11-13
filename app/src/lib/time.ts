/** "just now", "5 min ago", "3 hours ago", "1 day ago", "4 months ago", "2 years ago". */
export function ago(unix: number, now = Date.now() / 1000): string {
  const s = Math.max(0, now - unix);
  const unit = (n: number, one: string) => `${n} ${one}${n === 1 ? "" : "s"} ago`;
  if (s < 60) return "just now";
  if (s < 3600) return `${Math.floor(s / 60)} min ago`;
  if (s < 86400) return unit(Math.floor(s / 3600), "hour");
  if (s < 86400 * 30) return unit(Math.floor(s / 86400), "day");
  if (s < 86400 * 365) return unit(Math.floor(s / (86400 * 30)), "month");
  return unit(Math.floor(s / (86400 * 365)), "year");
}

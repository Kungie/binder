import type { Repeat } from "./types";

/** The rules in the order the picker shows them, with the words the owner sees. */
export const REPEATS: { value: Repeat; label: string; short: string; every: string }[] = [
  { value: "daily", label: "Every day", short: "Daily", every: "day" },
  { value: "weekdays", label: "Every weekday", short: "Weekdays", every: "weekday" },
  { value: "weekly", label: "Every week", short: "Weekly", every: "week" },
  { value: "monthly", label: "Every month", short: "Monthly", every: "month" },
  { value: "yearly", label: "Every year", short: "Yearly", every: "year" },
];

export const repeatInfo = (r: Repeat) => REPEATS.find((x) => x.value === r)!;

const iso = (d: Date) => `${d.getUTCFullYear()}-${String(d.getUTCMonth() + 1).padStart(2, "0")}-${String(d.getUTCDate()).padStart(2, "0")}`;
const parse = (s: string) => {
  const [y, m, d] = s.split("-").map(Number);
  return new Date(Date.UTC(y, m - 1, d));
};

function nth(r: Repeat, from: Date, n: number): Date {
  const d = new Date(from);
  if (r === "daily") d.setUTCDate(d.getUTCDate() + n);
  else if (r === "weekly") d.setUTCDate(d.getUTCDate() + 7 * n);
  else if (r === "monthly" || r === "yearly") {
    const months = r === "monthly" ? n : 12 * n;
    const target = new Date(Date.UTC(from.getUTCFullYear(), from.getUTCMonth() + months, 1));
    const last = new Date(Date.UTC(target.getUTCFullYear(), target.getUTCMonth() + 1, 0)).getUTCDate();
    return new Date(Date.UTC(target.getUTCFullYear(), target.getUTCMonth(), Math.min(from.getUTCDate(), last)));
  } else {
    for (let i = 0; i < n; i++) {
      d.setUTCDate(d.getUTCDate() + 1);
      while (d.getUTCDay() === 0 || d.getUTCDay() === 6) d.setUTCDate(d.getUTCDate() + 1);
    }
  }
  return d;
}

/** The same rule as the app's: the next day after the due date (or today), never on or before today. */
export function nextDue(r: Repeat, due: string | null, today: string): string {
  const from = parse(due ?? today);
  const t = parse(today);
  for (let n = 1; ; n++) {
    const d = nth(r, from, n);
    if (d > t || n > 100_000) return iso(d);
  }
}

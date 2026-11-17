import type { NewTask, Priority, Repeat } from "./types";

type ProjectRef = { id: number; name: string; prefix: string };

/** What quick add reads from one line: the rest is the title. */
export type Quick = Pick<NewTask, "title" | "due" | "due_time" | "project_id" | "priority" | "repeat">;

const B = "(?<=^|\\s)";
const E = "(?=$|\\s|[.,;])";
const re = (body: string) => new RegExp(`${B}(?:${body})${E}`, "iu");

const REPEATS: [Repeat, string][] = [
  ["weekdays", "every weekday|weekdays|hafta ?içi( her gün)?"],
  ["daily", "every day|daily|her ?gün"],
  ["weekly", "every week|weekly|her hafta"],
  ["monthly", "every month|monthly|her ay"],
  ["yearly", "every year|yearly|her yıl|her sene"],
];

const PRIORITIES: [Priority, string][] = [
  ["urgent", "!urgent|!1"],
  ["high", "!high|!2"],
  ["medium", "!medium|!3"],
  ["low", "!low|!4"],
];

/** Sunday first, as `Date.getDay` counts. Longer Turkish names first so cumartesi isn't read as cuma. */
const WEEKDAYS: [number, string][] = [
  [1, "monday|pazartesi"],
  [6, "saturday|cumartesi"],
  [2, "tuesday|salı"],
  [3, "wednesday|çarşamba"],
  [4, "thursday|perşembe"],
  [5, "friday|cuma"],
  [0, "sunday|pazar"],
];

const MONTHS = [
  "jan(?:uary)?|ocak",
  "feb(?:ruary)?|şubat",
  "mar(?:ch)?|mart",
  "apr(?:il)?|nisan",
  "may|mayıs",
  "jun(?:e)?|haziran",
  "jul(?:y)?|temmuz",
  "aug(?:ust)?|ağustos",
  "sep(?:tember)?|eylül",
  "oct(?:ober)?|ekim",
  "nov(?:ember)?|kasım",
  "dec(?:ember)?|aralık",
];

const pad = (n: number) => String(n).padStart(2, "0");
const iso = (d: Date) => `${d.getUTCFullYear()}-${pad(d.getUTCMonth() + 1)}-${pad(d.getUTCDate())}`;
const day = (s: string) => {
  const [y, m, d] = s.split("-").map(Number);
  return new Date(Date.UTC(y, m - 1, d));
};
const plus = (d: Date, n: number) => new Date(d.getTime() + n * 86_400_000);

/**
 * Reads one line the way the owner types it, in English or Turkish: a day (today, tomorrow, a weekday, "3 dec"), a
 * time ("at 9", "9:30", "5pm", "saat 9"), a repeat ("every month", "her hafta"), a project (`#orbit`) and a priority
 * (`!high`). Anything it doesn't read stays in the title; a line that would leave no title is all title.
 */
export function parseQuick(line: string, projects: ProjectRef[], today: string): Quick {
  let text = ` ${line.trim()} `;
  const out: Quick = { title: "" };
  const take = (pattern: RegExp): RegExpMatchArray | null => {
    const m = text.match(pattern);
    if (m) text = `${text.slice(0, m.index)} ${text.slice((m.index ?? 0) + m[0].length)}`;
    return m;
  };

  const tag = text.match(/(?<=^|\s)#([\p{L}\p{N}._-]+)(?=$|\s)/u);
  if (tag) {
    const p = projects.find((x) => x.name.toLowerCase() === tag[1].toLowerCase() || x.prefix.toLowerCase() === tag[1].toLowerCase());
    if (p) {
      take(new RegExp(`(?<=^|\\s)#${tag[1].replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}(?=$|\\s)`, "u"));
      out.project_id = p.id;
    }
  }
  for (const [p, words] of PRIORITIES) if (take(re(words))) out.priority = p;
  for (const [r, words] of REPEATS) {
    if (take(re(words))) {
      out.repeat = r;
      break;
    }
  }

  const base = day(today);
  if (take(re("today|bugün"))) out.due = iso(base);
  else if (take(re("tomorrow|yarın"))) out.due = iso(plus(base, 1));
  else {
    const date = take(re(`(?:on )?(\\d{1,2}) (${MONTHS.map((m) => `(?:${m})`).join("|")})`));
    if (date) {
      const month = MONTHS.findIndex((m) => new RegExp(`^(?:${m})$`, "iu").test(date[2]));
      let d = new Date(Date.UTC(base.getUTCFullYear(), month, Number(date[1])));
      if (d < base) d = new Date(Date.UTC(base.getUTCFullYear() + 1, month, Number(date[1])));
      out.due = iso(d);
    } else {
      for (const [n, words] of WEEKDAYS) {
        if (take(re(`(?:on )?(?:${words})`))) {
          const ahead = (n - base.getUTCDay() + 7) % 7 || 7;
          out.due = iso(plus(base, ahead));
          break;
        }
      }
    }
  }

  const ampm = take(re("(?:at |saat )?(\\d{1,2})(?::(\\d{2}))? ?(am|pm)"));
  const clock = ampm ? null : take(re("(?:at |saat )(\\d{1,2})(?::(\\d{2}))?|(\\d{1,2}):(\\d{2})"));
  let time: string | undefined;
  if (ampm) {
    const h = Number(ampm[1]) % 12 + (ampm[3].toLowerCase() === "pm" ? 12 : 0);
    if (h < 24 && Number(ampm[2] ?? 0) < 60) time = `${pad(h)}:${ampm[2] ?? "00"}`;
  } else if (clock) {
    const h = Number(clock[1] ?? clock[3]);
    const m = clock[2] ?? clock[4] ?? "00";
    if (h < 24 && Number(m) < 60) time = `${pad(h)}:${m}`;
  }
  if (time) out.due_time = time;
  if ((out.repeat || out.due_time) && !out.due) out.due = iso(base);

  const title = text.replace(/\s+/g, " ").trim();
  if (!title) return { title: line.trim() };
  out.title = title;
  const clean: Quick = { title };
  for (const k of ["repeat", "due", "due_time", "project_id", "priority"] as const) if (out[k] !== undefined) (clean as Record<string, unknown>)[k] = out[k];
  return clean;
}

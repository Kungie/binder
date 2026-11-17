import { describe, expect, it } from "vitest";
import { parseQuick } from "./quickadd";

const projects = [
  { id: 1, name: "orbit", prefix: "ORB" },
  { id: 2, name: "homelab", prefix: "HOME" },
];
// Monday 16 November 2026
const today = "2026-11-16";

describe("quick add", () => {
  it("plain_text_is_the_title", () => {
    expect(parseQuick("Water the plants", projects, today)).toEqual({ title: "Water the plants" });
  });

  it("reads_days_times_projects_and_priority_in_english", () => {
    expect(parseQuick("Call the bank tomorrow at 9:30 #orbit !high", projects, today)).toEqual({
      title: "Call the bank",
      due: "2026-11-17",
      due_time: "09:30",
      project_id: 1,
      priority: "high",
    });
    expect(parseQuick("Ship it friday 5pm", projects, today)).toEqual({ title: "Ship it", due: "2026-11-20", due_time: "17:00" });
    expect(parseQuick("Standup today 10am", projects, today)).toEqual({ title: "Standup", due: "2026-11-16", due_time: "10:00" });
    expect(parseQuick("Renew domain on 3 dec", projects, today)).toEqual({ title: "Renew domain", due: "2026-12-03" });
  });

  it("reads_turkish_too", () => {
    expect(parseQuick("Faturayı öde yarın saat 9", projects, today)).toEqual({ title: "Faturayı öde", due: "2026-11-17", due_time: "09:00" });
    expect(parseQuick("Kira her ay #homelab", projects, today)).toEqual({ title: "Kira", repeat: "monthly", due: "2026-11-16", project_id: 2 });
    expect(parseQuick("Spor hafta içi 07:00", projects, today)).toEqual({ title: "Spor", repeat: "weekdays", due: "2026-11-16", due_time: "07:00" });
    expect(parseQuick("Toplantı cuma 14:00", projects, today)).toEqual({ title: "Toplantı", due: "2026-11-20", due_time: "14:00" });
    expect(parseQuick("Yedek al her hafta pazar", projects, today)).toEqual({ title: "Yedek al", repeat: "weekly", due: "2026-11-22" });
  });

  it("reads_repeats", () => {
    expect(parseQuick("Backup every week", projects, today)).toEqual({ title: "Backup", repeat: "weekly", due: "2026-11-16" });
    expect(parseQuick("Pay rent every month on 1 dec", projects, today)).toEqual({ title: "Pay rent", repeat: "monthly", due: "2026-12-01" });
    expect(parseQuick("Stretch daily", projects, today)).toEqual({ title: "Stretch", repeat: "daily", due: "2026-11-16" });
  });

  it("an_unknown_project_or_a_lone_word_stays_in_the_title", () => {
    expect(parseQuick("Fix #42 in the parser", projects, today)).toEqual({ title: "Fix #42 in the parser" });
    expect(parseQuick("tomorrow", projects, today)).toEqual({ title: "tomorrow" });
  });

  it("a_weekday_named_today_means_next_week", () => {
    expect(parseQuick("Review monday", projects, today)).toEqual({ title: "Review", due: "2026-11-23" });
  });
});

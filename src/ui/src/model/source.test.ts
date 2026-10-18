import { describe, expect, it } from "vitest";
import { sourceFromForm, sourceToFields } from "./source";
import { formatDuration, formatDurationInput, parseDuration } from "./units";

describe("spec admin-ui: exchange-rate source screen", () => {
  it("Configure from the console: every 1 hour, stale after 1 day, timeout 10 seconds → seconds", () => {
    const r = sourceFromForm({ url: "https://api.frankfurter.app/latest?from=EUR", "interval.amount": "1", "interval.unit": "hours", "max_age.amount": "1", "max_age.unit": "days", "timeout.amount": "10", "timeout.unit": "seconds" });
    expect(r).toEqual({ ok: true, value: { url: "https://api.frankfurter.app/latest?from=EUR", interval_seconds: 3600, max_age_seconds: 86400, timeout_seconds: 10 } });
  });
  it("Bad interval: a maximum age below the interval and a non-URL are refused before sending", () => {
    const r = sourceFromForm({ url: "rates.example", "interval.amount": "2", "interval.unit": "hours", "max_age.amount": "1", "max_age.unit": "hours", "timeout.amount": "0", "timeout.unit": "seconds" });
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.errors.map((e) => e.field).sort()).toEqual(["max_age.amount", "timeout.amount", "url"]);
  });
  it("Fields round-trip through the largest even unit", () => {
    const s = { url: "https://x.example/r", interval_seconds: 900, max_age_seconds: 172800, timeout_seconds: 10 };
    const f = sourceToFields(s);
    expect(f).toMatchObject({ "interval.amount": "15", "interval.unit": "minutes", "max_age.amount": "2", "max_age.unit": "days", "timeout.amount": "10", "timeout.unit": "seconds" });
    expect(sourceFromForm(f)).toEqual({ ok: true, value: s });
    expect(sourceToFields(null)["interval.unit"]).toBe("hours");
  });
});

describe("spec admin-ui: human units for durations", () => {
  it("parses whole units only", () => {
    expect(parseDuration("2", "hours")).toBe(7200);
    expect(parseDuration("1.5", "hours")).toBeUndefined();
    expect(parseDuration("0", "days")).toBeUndefined();
    expect(parseDuration("3", "weeks")).toBeUndefined();
  });
  it("formats with the largest unit that divides evenly", () => {
    expect(formatDurationInput(90)).toEqual({ amount: "90", unit: "seconds" });
    expect(formatDuration(60)).toBe("1 minute");
    expect(formatDuration(7200)).toBe("2 hours");
    expect(formatDuration(86400 * 3)).toBe("3 days");
  });
});

// The exchange-rate source form (spec `admin-ui`, "Exchange-rate source screen"): the URL and
// three durations in human units, sent as seconds.
import type { ExchangeRateSource } from "../api/types";
import { Form, type Fields, type Parsed } from "./fields";
import { formatDurationInput, parseDuration } from "./units";

function duration(f: Form, fields: Fields, prefix: string, min: number): number {
  const amount = fields[`${prefix}.amount`] ?? "";
  const unit = fields[`${prefix}.unit`] ?? "";
  const seconds = parseDuration(amount, unit);
  if (seconds === undefined) {
    f.fail(`${prefix}.amount`, "whole number of units, at least 1");
    return 0;
  }
  if (seconds < min) f.fail(`${prefix}.amount`, `at least ${min} seconds`);
  return seconds;
}

export function sourceFromForm(fields: Fields): Parsed<ExchangeRateSource> {
  const f = new Form(fields);
  const url = f.str("url");
  if (url && !/^https?:\/\/\S+$/i.test(url)) f.fail("url", "an absolute http or https URL");
  const interval_seconds = duration(f, fields, "interval", 60);
  const max_age_seconds = duration(f, fields, "max_age", 60);
  if (interval_seconds && max_age_seconds && max_age_seconds < interval_seconds) f.fail("max_age.amount", "at least the interval");
  const timeout_seconds = duration(f, fields, "timeout", 1);
  return f.finish({ url, interval_seconds, max_age_seconds, timeout_seconds });
}

export function sourceToFields(s: ExchangeRateSource | null): Fields {
  const d = (prefix: string, seconds: number) => {
    const { amount, unit } = formatDurationInput(seconds);
    return { [`${prefix}.amount`]: amount, [`${prefix}.unit`]: unit };
  };
  if (!s) return { url: "", ...d("interval", 3600), ...d("max_age", 86400), ...d("timeout", 10) };
  return { url: s.url, ...d("interval", s.interval_seconds), ...d("max_age", s.max_age_seconds), ...d("timeout", s.timeout_seconds) };
}

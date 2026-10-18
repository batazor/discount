// PromotionMetadata form ↔ body. Labels are a free string map in the contract; the form
// takes one `key=value` line per label.
import type { PromotionMetadata } from "../api/types";
import { Form, type Fields, type Parsed } from "./fields";

export function labelsFromText(f: Form, key: string): Record<string, string> {
  const labels: Record<string, string> = {};
  for (const line of f.raw(key).split("\n")) {
    const t = line.trim();
    if (t === "") continue;
    const eq = t.indexOf("=");
    if (eq <= 0) {
      f.fail(key, `"${t}" is not key=value`);
      continue;
    }
    labels[t.slice(0, eq).trim()] = t.slice(eq + 1).trim();
  }
  return labels;
}

export function metadataFromForm(fields: Fields): Parsed<PromotionMetadata> {
  const f = new Form(fields);
  return f.finish({
    title: f.str("title"),
    description: f.raw("description"),
    source: f.optStr("source"),
    labels: labelsFromText(f, "labels"),
  });
}

export function metadataToFields(m: PromotionMetadata): Fields {
  return {
    title: m.title,
    description: m.description,
    source: m.source ?? "",
    labels: Object.entries(m.labels).map(([k, v]) => `${k}=${v}`).join("\n"),
  };
}

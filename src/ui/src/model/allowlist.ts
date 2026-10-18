// Allowlist form ↔ body: one customer id per line, deduplicated, order kept.
import type { Allowlist } from "../api/types";
import { Form, type Fields, type Parsed } from "./fields";

export function allowlistFromForm(fields: Fields): Parsed<Allowlist> {
  const f = new Form(fields);
  const ids = [...new Set(f.raw("customer_ids").split(/[\n,]/).map((s) => s.trim()).filter((s) => s !== ""))];
  return f.finish({ customer_ids: ids });
}

export function allowlistToFields(a: Allowlist): Fields {
  return { customer_ids: a.customer_ids.join("\n") };
}

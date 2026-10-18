import { describe, expect, it } from "vitest";
import { allowlistFromForm, allowlistToFields } from "./allowlist";

describe("allowlist", () => {
  it("one id per line, commas allowed, duplicates and blanks dropped", () => {
    expect(allowlistFromForm({ customer_ids: "c1\n c2 \n\nc1, c3" })).toEqual({ ok: true, value: { customer_ids: ["c1", "c2", "c3"] } });
  });
  it("round trips and an empty box clears the list", () => {
    expect(allowlistFromForm(allowlistToFields({ customer_ids: ["a", "b"] }))).toEqual({ ok: true, value: { customer_ids: ["a", "b"] } });
    expect(allowlistFromForm({ customer_ids: "" })).toEqual({ ok: true, value: { customer_ids: [] } });
  });
});

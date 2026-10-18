import { describe, expect, it } from "vitest";
import type { PromotionMetadata } from "../api/types";
import { metadataFromForm, metadataToFields } from "./metadata";

describe("spec admin-ui: promotion metadata", () => {
  it("Metadata round trip from the screen: title, source and one label line", () => {
    expect(metadataFromForm({ title: "Summer", description: "", source: "hotels", labels: "channel=app" })).toEqual({
      ok: true,
      value: { title: "Summer", description: "", source: "hotels", labels: { channel: "app" } },
    });
  });

  it("empty source is null, blank label lines are skipped, values may contain '='", () => {
    const r = metadataFromForm({ title: "T", description: "d", source: "", labels: "\n a = b=c \n\nk=\n" });
    expect(r).toEqual({ ok: true, value: { title: "T", description: "d", source: null, labels: { a: "b=c", k: "" } } });
  });

  it("a label line without '=' and an empty title are errors", () => {
    const r = metadataFromForm({ title: "", labels: "nokey" });
    expect(r.ok).toBe(false);
    if (r.ok) return;
    expect(r.errors.map((e) => e.field).sort()).toEqual(["labels", "title"]);
  });

  it("round trips", () => {
    const m: PromotionMetadata = { title: "Summer", description: "two\nlines", source: null, labels: { issuer: "ops", aim: "retention" } };
    expect(metadataFromForm(metadataToFields(m))).toEqual({ ok: true, value: m });
  });
});

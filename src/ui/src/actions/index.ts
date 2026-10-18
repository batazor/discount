// Every mutation of the console. Each action maps one form onto one documented operation
// of api/openapi.yaml (design.md, decision 2); the typed client refuses anything else.
import { defineAction } from "astro:actions";
import { api } from "../api/client";
import {
  couponFromForm,
  exchangeRateFromForm,
  generateRequestFromForm,
  grantRequestFromForm,
  grantRuleFromForm,
  groupFromForm,
  policyFromForm,
} from "../model/catalogue";
import { evaluateRequestFromForm, parseJson } from "../model/evaluate";
import { Form, type Fields, type Parsed } from "../model/fields";
import { allowlistFromForm } from "../model/allowlist";
import { metadataFromForm } from "../model/metadata";
import { sourceFromForm } from "../model/source";
import { promotionFromForm } from "../model/promotion";
import { fold, run, runPlain, type ActionOutcome } from "./run";
import { campaignDraftFromForm } from "../model/campaign";
import { applyBuilder, builderErrors } from "../model/builder";
import { call } from "../api/client";
import { fieldsOf } from "../model/fields";
import type { Budget, EvaluateRequest, Promotion } from "../api/types";

const form = { accept: "form" } as const;

function budgetFromForm(fields: Fields): Parsed<Budget> {
  const f = new Form(fields);
  // "no budget" is an explicit state: the checkbox clears it, the money fields set it.
  const remaining = f.bool("no_budget") ? null : f.money("remaining");
  return f.finish({ remaining });
}

export const server = {
  putPolicy: defineAction({ ...form, handler: (fd: FormData) => run(fd, policyFromForm, (body) => api.PUT("/v1/policy", { body })) }),

  putGroup: defineAction({
    ...form,
    handler: (fd: FormData) =>
      run(fd, groupFromForm, (body) => api.PUT("/v1/groups/{group_id}", { params: { path: { group_id: body.group_id } }, body })),
  }),
  deleteGroup: defineAction({
    ...form,
    handler: (fd: FormData) => runPlain(fd, (f) => api.DELETE("/v1/groups/{group_id}", { params: { path: { group_id: f.id ?? "" } } })),
  }),

  // Pasted JSON wins over the form when present and is sent as-is, so the API's own `400`
  // (an unknown key) is what the screen shows next to the untouched text.
  putPromotion: defineAction({
    ...form,
    handler: (fd: FormData) =>
      run(
        fd,
        (fields) => {
          if (fields.json?.trim()) return parseJson(fields.json) as Parsed<Promotion>;
          const errors = builderErrors(fields);
          return errors.length ? { ok: false, errors } : promotionFromForm(applyBuilder(fields));
        },
        (body) =>
          api.PUT("/v1/promotions/{promotion_id}", {
            params: { path: { promotion_id: String((body as { promotion_id?: unknown }).promotion_id ?? fd.get("promotion_id") ?? "") } },
            body,
          }),
      ),
  }),
  deletePromotion: defineAction({
    ...form,
    handler: (fd: FormData) =>
      runPlain(fd, (f) => api.DELETE("/v1/promotions/{promotion_id}", { params: { path: { promotion_id: f.id ?? "" } } })),
  }),
  putBudget: defineAction({
    ...form,
    handler: (fd: FormData) =>
      run(fd, budgetFromForm, (body) =>
        api.PUT("/v1/promotions/{promotion_id}/budget", { params: { path: { promotion_id: fd.get("promotion_id")?.toString() ?? "" } }, body }),
      ),
  }),

  putPromotionMetadata: defineAction({
    ...form,
    handler: (fd: FormData) =>
      run(fd, metadataFromForm, (body) =>
        api.PUT("/v1/promotions/{promotion_id}/metadata", { params: { path: { promotion_id: fd.get("promotion_id")?.toString() ?? "" } }, body }),
      ),
  }),
  deletePromotionMetadata: defineAction({
    ...form,
    handler: (fd: FormData) =>
      runPlain(fd, (f) => api.DELETE("/v1/promotions/{promotion_id}/metadata", { params: { path: { promotion_id: f.id ?? "" } } })),
  }),

  putAllowlist: defineAction({
    ...form,
    handler: (fd: FormData) =>
      run(fd, allowlistFromForm, (body) =>
        api.PUT("/v1/promotions/{promotion_id}/allowlist", { params: { path: { promotion_id: fd.get("promotion_id")?.toString() ?? "" } }, body }),
      ),
  }),
  clearAllowlist: defineAction({
    ...form,
    handler: (fd: FormData) =>
      runPlain(fd, (f) => api.DELETE("/v1/promotions/{promotion_id}/allowlist", { params: { path: { promotion_id: f.id ?? "" } } })),
  }),

  putCoupon: defineAction({
    ...form,
    handler: (fd: FormData) =>
      run(fd, couponFromForm, (body) => api.PUT("/v1/coupons/{code}", { params: { path: { code: body.code } }, body })),
  }),
  deleteCoupon: defineAction({
    ...form,
    handler: (fd: FormData) => runPlain(fd, (f) => api.DELETE("/v1/coupons/{code}", { params: { path: { code: f.id ?? "" } } })),
  }),
  generateCoupons: defineAction({
    ...form,
    handler: (fd: FormData) => run(fd, generateRequestFromForm, (body) => api.POST("/v1/coupons/generate", { body })),
  }),

  putGrantRule: defineAction({
    ...form,
    handler: (fd: FormData) =>
      run(fd, grantRuleFromForm, (body) => api.PUT("/v1/grant-rules/{rule_id}", { params: { path: { rule_id: body.rule_id } }, body })),
  }),
  deleteGrantRule: defineAction({
    ...form,
    handler: (fd: FormData) => runPlain(fd, (f) => api.DELETE("/v1/grant-rules/{rule_id}", { params: { path: { rule_id: f.id ?? "" } } })),
  }),
  grantCoupon: defineAction({
    ...form,
    handler: (fd: FormData) => run(fd, grantRequestFromForm, (body) => api.POST("/v1/coupons/grant", { body })),
  }),

  putExchangeRate: defineAction({
    ...form,
    handler: (fd: FormData) =>
      run(fd, exchangeRateFromForm, (r) =>
        api.PUT("/v1/exchange-rates/{from}/{to}", {
          params: { path: { from: r.from, to: r.to } },
          body: { numerator: r.numerator, denominator: r.denominator },
        }),
      ),
  }),
  putRateSource: defineAction({
    ...form,
    handler: (fd: FormData) => run(fd, sourceFromForm, (body) => api.PUT("/v1/exchange-rate-source", { body })),
  }),
  refreshRateSource: defineAction({
    ...form,
    handler: (fd: FormData) => runPlain(fd, () => api.POST("/v1/exchange-rate-source/refresh")),
  }),
  deleteRateSource: defineAction({
    ...form,
    handler: (fd: FormData) => runPlain(fd, () => api.DELETE("/v1/exchange-rate-source")),
  }),
  deleteExchangeRate: defineAction({
    ...form,
    handler: (fd: FormData) =>
      runPlain(fd, (f) => api.DELETE("/v1/exchange-rates/{from}/{to}", { params: { path: { from: f.from ?? "", to: f.to ?? "" } } })),
  }),

  confirmRedemption: defineAction({
    ...form,
    handler: (fd: FormData) =>
      runPlain(fd, (f) => api.POST("/v1/redemptions/{order_id}/confirm", { params: { path: { order_id: f.id ?? "" } } })),
  }),
  cancelRedemption: defineAction({
    ...form,
    handler: (fd: FormData) =>
      runPlain(fd, (f) => api.POST("/v1/redemptions/{order_id}/cancel", { params: { path: { order_id: f.id ?? "" } } })),
  }),

  // One flow to create a campaign (design.md, decision 3): the documented calls in order,
  // stopping at the first failure and naming it with what was already saved.
  createCampaign: defineAction({
    ...form,
    handler: async (fd: FormData, context): Promise<ActionOutcome<{ promotion_id: string; codes: string[]; saved: string[] }>> => {
      const fields = fieldsOf(fd);
      const draft = campaignDraftFromForm(fields);
      if (!draft.ok) return { kind: "invalid", errors: draft.errors, fields };
      const { promotion, metadata, budget, generate } = draft.value;
      const id = promotion.promotion_id;
      const saved: string[] = [];
      const fail = (step: string, o: Extract<ActionOutcome<unknown>, { kind: "api" | "unreachable" }>) => ({ ...o, step, saved: [...saved] });

      const p = fold(await call(() => api.PUT("/v1/promotions/{promotion_id}", { params: { path: { promotion_id: id } }, body: promotion })), fields);
      if (p.kind !== "ok") return p.kind === "invalid" ? p : fail("promotion", p);
      saved.push("promotion");
      if (budget) {
        const b = fold(await call(() => api.PUT("/v1/promotions/{promotion_id}/budget", { params: { path: { promotion_id: id } }, body: { remaining: budget } })), fields);
        if (b.kind !== "ok") return b.kind === "invalid" ? b : fail("budget", b);
        saved.push("budget");
      }
      if (metadata) {
        const m = fold(await call(() => api.PUT("/v1/promotions/{promotion_id}/metadata", { params: { path: { promotion_id: id } }, body: metadata })), fields);
        if (m.kind !== "ok") return m.kind === "invalid" ? m : fail("metadata", m);
        saved.push("metadata");
      }
      let codes: string[] = [];
      if (generate) {
        const g = fold(await call(() => api.POST("/v1/coupons/generate", { body: generate })), fields);
        if (g.kind !== "ok") return g.kind === "invalid" ? g : fail("codes", g);
        saved.push("codes");
        codes = g.data.codes;
        // Shown once on the campaign's codes tab after the redirect.
        await context.session?.set("generated", { promotion_id: id, codes });
      }
      return { kind: "ok", data: { promotion_id: id, codes, saved }, fields };
    },
  }),

  // The playground: a dry run, never /v1/redeem. Pasted JSON is sent as-is so the API's
  // own 400 (unknown key) or 422 (PricingError) is what the screen shows.
  evaluate: defineAction({
    ...form,
    handler: (fd: FormData) =>
      run(
        fd,
        (fields) => (fields.json?.trim() ? (parseJson(fields.json) as Parsed<EvaluateRequest>) : evaluateRequestFromForm(fields)),
        (body) => api.POST("/v1/evaluate", { body }),
      ),
  }),
};

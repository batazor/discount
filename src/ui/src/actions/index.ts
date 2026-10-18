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
import { metadataFromForm } from "../model/metadata";
import { promotionFromForm } from "../model/promotion";
import { run, runPlain } from "./run";
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
        (fields) => (fields.json?.trim() ? (parseJson(fields.json) as Parsed<Promotion>) : promotionFromForm(fields)),
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

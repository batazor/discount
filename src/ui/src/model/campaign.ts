// A campaign is the console's view of a promotion with its satellites (design.md,
// decision 1): nothing here is server state. The status is a display classification of
// returned fields and the clock, never a pricing result.
import type { Action, Coupon, MetadataEntry, Promotion, PromotionMetadata } from "../api/types";
import { formatAmount, formatBps } from "./units";

export type CampaignStatus = "scheduled" | "live" | "exhausted" | "ended";
export const CAMPAIGN_STATUSES = ["live", "scheduled", "exhausted", "ended"] as const;

export const CAMPAIGN_STATUS_HELP: Record<CampaignStatus, string> = {
  scheduled: "A date window starts in the future; nothing applies yet.",
  live: "Eligible carts get it now.",
  exhausted: "The budget is spent or the total-uses limit is reached; nothing more applies until it is raised.",
  ended: "A date window has closed.",
};

export type Campaign = {
  promotion: Promotion;
  metadata: PromotionMetadata | null;
  codes: Coupon[];
  status: CampaignStatus;
  startsAt: number | null;
  endsAt: number | null;
  /** Live and ending within `SOON_SECONDS`. */
  endingSoon: boolean;
  /** Why it is exhausted, when it is. */
  exhaustedBy: "budget" | "uses" | null;
};

export const SOON_SECONDS = 7 * 24 * 3600;

function windows(p: Promotion) {
  return p.rules.flatMap((r) => (r.type === "DateWindow" ? [r] : []));
}

/** Latest start among the windows (all must hold), or null. */
export function startsAt(p: Promotion): number | null {
  const s = windows(p).flatMap((w) => (w.starts_at === null ? [] : [w.starts_at]));
  return s.length ? Math.max(...s) : null;
}

/** Earliest end among the windows, or null. */
export function endsAt(p: Promotion): number | null {
  const e = windows(p).flatMap((w) => (w.ends_at === null ? [] : [w.ends_at]));
  return e.length ? Math.min(...e) : null;
}

export function exhaustedBy(p: Promotion): "budget" | "uses" | null {
  if (p.remaining_budget && p.remaining_budget.amount === 0) return "budget";
  const limit = p.rules.flatMap((r) => (r.type === "MaxUsesTotal" ? [r.max] : []));
  if (limit.length && p.uses_total >= Math.min(...limit)) return "uses";
  return null;
}

/** Precedence: ended, scheduled, exhausted, live. */
export function campaignStatus(p: Promotion, now: number): CampaignStatus {
  const e = endsAt(p);
  if (e !== null && e <= now) return "ended";
  const s = startsAt(p);
  if (s !== null && s > now) return "scheduled";
  if (exhaustedBy(p)) return "exhausted";
  return "live";
}

export function assembleCampaigns(promotions: Promotion[], metadata: MetadataEntry[], coupons: Coupon[], now: number): Campaign[] {
  const meta = new Map(metadata.map((m) => [m.promotion_id, m.metadata]));
  const codes = new Map<string, Coupon[]>();
  for (const c of coupons) codes.set(c.promotion_id, [...(codes.get(c.promotion_id) ?? []), c]);
  return promotions.map((p) => {
    const status = campaignStatus(p, now);
    const e = endsAt(p);
    return {
      promotion: p,
      metadata: meta.get(p.promotion_id) ?? null,
      codes: codes.get(p.promotion_id) ?? [],
      status,
      startsAt: startsAt(p),
      endsAt: e,
      endingSoon: status === "live" && e !== null && e - now <= SOON_SECONDS,
      exhaustedBy: exhaustedBy(p),
    };
  });
}

/** `10 % off the order` / `5.00 EUR off the order`. */
export function describeAction(a: Action): string {
  return a.type === "OrderPercentage" ? `${formatBps(a.bps)} off the order` : `${formatAmount(a.amount.amount, a.amount.currency)} off the order`;
}

export function campaignTitle(c: Pick<Campaign, "promotion" | "metadata">): string {
  return c.metadata?.title ?? c.promotion.promotion_id;
}

/** Plain names for the rule variants, for lists and the creation form. */
export const RULE_NAMES: Record<Promotion["rules"][number]["type"], string> = {
  DateWindow: "Dates",
  OrderTotalRange: "Order total",
  TagsMatch: "Cart tags",
  AttributeIn: "Cart attribute",
  AttributeDateWindow: "Cart date attribute",
  MaxCompletedOrders: "Completed orders",
  MaxUsesPerCustomer: "Uses per customer",
  MaxUsesTotal: "Uses in total",
  RequiresCoupon: "Needs a code",
  CustomerAttributeIn: "Customer attribute",
  CustomerAttributeRange: "Customer attribute range",
  RequiresAllowlist: "Allowlisted customers",
};

// ---- creation form (one flow, design.md decision 3) ----
import type { GenerateCouponsRequest, Money } from "../api/types";
import { Form, type Fields, type Parsed } from "./fields";
import { generateRequestFromForm } from "./catalogue";
import { metadataFromForm } from "./metadata";
import { applyBuilder } from "./builder";
import { promotionFromForm } from "./promotion";

export type CampaignDraft = {
  promotion: Promotion;
  metadata: PromotionMetadata | null;
  budget: Money | null;
  generate: GenerateCouponsRequest | null;
};

/** The creation form: promotion fields, optional title (→ metadata), budget and codes. */
export function campaignDraftFromForm(input: Fields): Parsed<CampaignDraft> {
  const fields = applyBuilder(input);
  const errors = [];
  const promotion = promotionFromForm(fields);
  if (!promotion.ok) errors.push(...promotion.errors);
  let metadata: PromotionMetadata | null = null;
  if ((fields.title ?? "").trim() !== "") {
    const m = metadataFromForm(fields);
    if (m.ok) metadata = m.value;
    else errors.push(...m.errors);
  }
  const f = new Form(fields);
  const budget = f.optMoney("budget");
  errors.push(...f.errors);
  let generate: GenerateCouponsRequest | null = null;
  if ((fields["codes.count"] ?? "").trim() !== "") {
    const g = generateRequestFromForm({
      promotion_id: fields.promotion_id ?? "",
      count: fields["codes.count"] ?? "",
      prefix: fields["codes.prefix"] ?? "",
      max_uses: fields["codes.max_uses"] ?? "",
      customer_id: "",
      starts_at: "",
      ends_at: "",
    });
    if (g.ok) generate = g.value;
    else errors.push(...g.errors.map((e) => ({ field: `codes.${e.field}`, message: e.message })));
  }
  if (errors.length) return { ok: false, errors };
  return { ok: true, value: { promotion: promotion.ok ? promotion.value : (null as never), metadata, budget, generate } };
}

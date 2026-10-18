// Short names for the generated contract types. Nothing here is a shape of its own.
import type { components } from "./schema";

export type S = components["schemas"];
export type Money = S["Money"];
export type RoundingPolicy = S["RoundingPolicy"];
export type StackingPolicy = S["StackingPolicy"];
export type StackingMode = S["StackingMode"];
export type OrderCap = S["OrderCap"];
export type PricingPolicy = S["PricingPolicy"];
export type StackingGroup = S["StackingGroup"];
export type Rule = S["Rule"];
export type Action = S["Action"];
export type Promotion = S["Promotion"];
export type Coupon = S["Coupon"];
export type ExchangeRate = S["ExchangeRate"];
export type RateBody = S["RateBody"];
export type GenerateCouponsRequest = S["GenerateCouponsRequest"];
export type GrantRule = S["GrantRule"];
export type GrantTrigger = S["GrantTrigger"];
export type GrantRequest = S["GrantRequest"];
export type CustomerCoupon = S["CustomerCoupon"];
export type Budget = S["Budget"];
export type CartLine = S["CartLine"];
export type CartSnapshot = S["CartSnapshot"];
export type CustomerContext = S["CustomerContext"];
export type EvaluateRequest = S["EvaluateRequest"];
export type PricingResult = S["PricingResult"];
export type PricingError = S["PricingError"];
export type SkipReason = S["SkipReason"];
export type ClampReason = S["ClampReason"];
export type Redemption = S["Redemption"];
export type PromotionMetadata = S["PromotionMetadata"];
export type MetadataEntry = S["MetadataEntry"];
export type Allowlist = S["Allowlist"];
export type CouponStatus = S["CouponStatus"];
export type RedemptionStatus = S["RedemptionStatus"];

/**
 * Compile-time completeness of a list over a union of the contract. Use as
 * `export const X_COMPLETE: Complete<Union, typeof LIST> = true;` — when the contract gains
 * a member the list lacks, the type becomes `{ missing_from_list: "<member>" }` and the
 * assignment fails with the member's name in the error.
 */
export type Complete<U extends string, L extends readonly string[]> =
  [Exclude<U, L[number]>] extends [never] ? true : { missing_from_list: Exclude<U, L[number]> };

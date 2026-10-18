// Help texts shown next to fields, options and badges. One place, worded after
// docs/ubiquitous-language.md so the console explains the engine's actual semantics.
// Every map is typed over the contract's union, so a new variant without a text fails
// `astro check`.
import type { Action, ClampReason, CouponStatus, GrantTrigger, RedemptionStatus, RoundingPolicy, Rule, SkipReason, StackingMode, StackingPolicy } from "../api/types";

export const ROUNDING_HELP: Record<RoundingPolicy["type"], string> = {
  HalfUp: "A remainder of exactly one half rounds away from zero.",
  HalfEven: "A remainder of exactly one half rounds to the even neighbour.",
  Down: "Toward zero: fractions of a minor unit are dropped.",
};

export const STACKING_HELP: Record<StackingPolicy["type"], string> = {
  Sequential: "A percentage is taken from the running subtotal, after the promotions applied before it.",
  Additive: "Every percentage is taken from the original subtotal, as if it applied alone.",
};

export const STACKING_MODE_HELP: Record<StackingMode["type"], string> = {
  Stackable: "Every eligible promotion in the group applies, in order.",
  FirstMatch: "Only the first eligible promotion of the group applies; the rest are skipped as FirstMatchTaken.",
};

export const ACTION_HELP: Record<Action["type"], string> = {
  OrderPercentage: "A share of the order subtotal, in basis points: 1000 = 10 %. The base depends on the stacking policy.",
  OrderFixedAmount: "A fixed amount off the order, converted to the cart currency when needed.",
};

export const RULE_HELP: Record<Rule["type"], string> = {
  DateWindow: "Holds from Starts (inclusive) until Ends (exclusive); either bound may be empty.",
  OrderTotalRange: "The order subtotal, in the cart currency, is within the bounds; either bound may be empty.",
  TagsMatch: "The cart carries every tag of All, at least one of Any (when Any is given), and none of None.",
  AttributeIn: "Some cart attribute has this key and one of the values.",
  AttributeDateWindow: "The cart attribute with this key is an integer timestamp within the window.",
  MaxCompletedOrders: "The customer has completed at most this many orders; 0 = first order only.",
  MaxUsesPerCustomer: "The customer has used this promotion fewer than this many times; stored redemptions count.",
  MaxUsesTotal: "The promotion has been used fewer than this many times in total.",
  RequiresCoupon: "The cart presents a stored coupon for this promotion that is within its window, not bound to another customer, and has uses left.",
  CustomerAttributeIn: "Some customer attribute has this key and one of the values.",
  CustomerAttributeRange: "The customer attribute with this key is an integer within the bounds, inclusive; either bound may be empty.",
  RequiresAllowlist: "The customer is on this promotion's allowlist.",
};

export const GRANT_TRIGGER_HELP: Record<GrantTrigger, string> = {
  cancel: "Issued when an order is cancelled.",
  review: "Issued when the customer leaves a review.",
};

export const COUPON_STATUS_HELP: Record<CouponStatus, string> = {
  active: "Can be used now.",
  scheduled: "Starts in the future.",
  expired: "Window closed, uses exhausted, or the promotion is gone.",
  applied: "The customer used it; the order links to that redemption.",
};

export const REDEMPTION_STATUS_HELP: Record<RedemptionStatus, string> = {
  reserved: "The discount is held and budgets are consumed; waiting for confirmation.",
  confirmed: "The order went through; budgets stay consumed.",
  cancelled: "Budgets and use counts were restored.",
};

export const CLAMP_HELP: Record<ClampReason["type"], string> = {
  Cap: "Cut to the promotion's cap.",
  Budget: "Cut to the remaining budget.",
  OrderCap: "Cut to what the order cap still allowed.",
  Running: "Cut to the remaining order subtotal.",
};

export const SKIP_HELP: Record<SkipReason["type"], string> = {
  NotEligible: "A rule did not hold; the index is the first failing rule of the promotion.",
  ExclusiveApplied: "An exclusive promotion applied alone.",
  FirstMatchTaken: "Another promotion of a first-match group applied before this one.",
};

/** What a destructive action does, for the confirm dialog. */
export const CONSEQUENCE = {
  archive: "The campaign leaves the catalogue and stops applying; its codes stop working. Redemptions already made keep their copy of it. There is no unarchive.",
  deleteCoupon: "The code stops working at once. Redemptions that used it keep their record.",
  deleteGroup: "Promotions in this group can no longer be priced until they are moved to another group.",
  deleteGrantRule: "Future events of this kind will not issue this coupon. Codes already issued stay.",
  deleteRate: "Carts and promotions in these two currencies can no longer be converted; evaluations needing the rate fail.",
  cancelRedemption: "The order's discount is released: budgets and use counts are restored. This cannot be confirmed again.",
  clearAllowlist: "Nobody is allowlisted any more; a RequiresAllowlist rule then matches no one.",
  removeMetadata: "Title, description, source and labels are removed; listings fall back to the id.",
};

export const TERM = {
  groupPriority: "Higher applies first. Groups are ordered by priority, then by id; promotions inside a group by their own priority, then by id.",
  promotionPriority: "Higher applies first within the group; ties by promotion id.",
  exclusive: "If any eligible promotion is exclusive, the first exclusive one in the order applies alone; every other promotion is skipped as ExclusiveApplied.",
  cap: "The most this promotion may grant on one order. Empty = unlimited.",
  promotionRounding: "Rounding for this promotion's percentage. Empty = the policy's rounding.",
  orderCap: "The most the whole order may be discounted: the smaller of the amount and the share of the subtotal, whichever is set. Both empty = unconstrained.",
  budget: "What this promotion may still grant across all orders. A reservation consumes it, a cancellation restores it. Empty = unbounded.",
  usesTotal: "Reserved and confirmed redemptions that applied this promotion. Read by MaxUsesTotal.",
  remainingBudget: "What is left of the budget after reserved and confirmed redemptions.",
  source: "A partition for listings, e.g. hotels or cars; lists can be filtered by it.",
  labels: "Free key=value pairs for reporting: issuer, business line, channel.",
  allowlist: "Customers this promotion is open to, one id per line. Read by the RequiresAllowlist rule; without that rule the list has no effect.",
  couponWindow: "The code works from Valid from (inclusive) until Valid until (exclusive); empty = open.",
  couponBound: "Only this customer may use the code. Empty = public: anyone may present it.",
  couponMaxUses: "How many redemptions may use the code in total. Empty = unlimited, 1 = single use.",
  couponUses: "Reserved and confirmed redemptions that used the code.",
  grantPriority: "Higher wins among rules for the same event; ties by rule id.",
  grantValidity: "The issued code is valid from the moment of the grant for this many seconds.",
  grantMaxUses: "Uses allowed on the issued code. Empty = unlimited.",
  rateFormula: "amount × numerator ÷ denominator, rounded by the policy.",
  completedOrders: "Orders the customer completed before this one. 0 = first order.",
  customerUses: "Extra use counts to add on top of the customer's stored redemptions.",
  customerAttributes: "Facts about the customer. Read by CustomerAttributeIn and CustomerAttributeRange.",
  allowlisted: "Promotions whose allowlist names the customer. The service adds the stored allowlists on its own.",
  cartTags: "Read by TagsMatch.",
  cartAttributes: "Read by AttributeIn and AttributeDateWindow.",
  couponCodes: "Read by RequiresCoupon; each code at most once.",
  applicationOrder: "Promotions are tried by group priority, then promotion priority, higher first; ties by id.",
  allocation: "How the discount is split over the lines, in minor units; the parts sum to the discount exactly.",
};

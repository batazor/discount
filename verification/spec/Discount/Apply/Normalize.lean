import Discount.Validate

/-!
# normalize (OpenSpec `multi-currency`)

Every promotion amount and the order cap in the cart currency. Runs after `validate`, which
guarantees a rate for every foreign amount.
-/
namespace Discount

/-- `m` in the cart currency; unchanged when no rate exists (validation guarantees one). -/
def normalizeMoney (rates : List ExchangeRate) (cur : Currency) (m : Money) : Money :=
  match convertAmount rates m.amount m.currency cur with
  | some a => { amount := a, currency := cur }
  | none => m

def normalizeRule (rates : List ExchangeRate) (cur : Currency) : Rule → Rule
  | .OrderTotalRange mn mx => .OrderTotalRange (mn.map (normalizeMoney rates cur)) (mx.map (normalizeMoney rates cur))
  | r => r

def normalizeAction (rates : List ExchangeRate) (cur : Currency) : Action → Action
  | .OrderFixedAmount m => .OrderFixedAmount (normalizeMoney rates cur m)
  | a => a

def normalizePromotion (rates : List ExchangeRate) (cur : Currency) (p : Promotion) : Promotion :=
  { p with
    cap := p.cap.map (normalizeMoney rates cur)
    remaining_budget := p.remaining_budget.map (normalizeMoney rates cur)
    rules := p.rules.map (normalizeRule rates cur)
    action := normalizeAction rates cur p.action }

/-- Every promotion amount and the order cap in the cart currency. Everything else is
unchanged. -/
def normalize (i : PricingInput) : PricingInput :=
  let cur := i.cart.currency
  let rates := i.exchange_rates
  { i with
    policy := { i.policy with
      order_cap := { i.policy.order_cap with amount := i.policy.order_cap.amount.map (normalizeMoney rates cur) } }
    promotions := i.promotions.map (normalizePromotion rates cur) }

end Discount

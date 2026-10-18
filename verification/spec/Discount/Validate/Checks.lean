import Discount.Types
import Discount.Rounding

/-!
# Validation building blocks (ADR 6)

Range predicates, `andThen` (sequencing two checks; only the first error survives), currency
conversion through `convertAmount` (OpenSpec `multi-currency`), money checks and the
duplicate scanners shared by the cart and catalogue checks.
-/
namespace Discount

def validCurrency (c : Currency) : Bool :=
  c.length == 3 && c.toList.all (fun ch => 'A' ≤ ch && ch ≤ 'Z')

def amountInRange (a : Int) : Bool := 0 ≤ a && a ≤ MAX_MINOR_UNITS

def bpsInRange (b : BasisPoints) : Bool := 0 ≤ b && b ≤ BPS_SCALE

def ratePartInRange (x : Int) : Bool := 1 ≤ x && x ≤ MAX_RATE_PART

def andThen (a b : Except PricingError Unit) : Except PricingError Unit :=
  match a with
  | .error e => .error e
  | .ok () => b

/-- The rate converting `from` into `to`, if listed. -/
def findRate (rates : List ExchangeRate) (from_ to : Currency) : Option ExchangeRate :=
  rates.find? (fun r => r.«from» == from_ && r.to == to)

/-- `amount` of currency `from` in currency `to`: unchanged for the same currency, else
`roundDiv Down (amount * numerator) denominator`; `none` without a rate. -/
def convertAmount (rates : List ExchangeRate) (amount : Int) (from_ to : Currency) : Option Int :=
  if from_ == to then some amount
  else match findRate rates from_ to with
    | some r => some (roundDiv .Down (amount * r.numerator) r.denominator)
    | none => none

/-- `InvalidCurrency`, `CurrencyMismatch`, `AmountOutOfRange` (original, then converted). -/
def checkMoney (m : Money) (cartCurrency : Currency) (rates : List ExchangeRate) (field : String) :
    Except PricingError Unit :=
  if !validCurrency m.currency then .error (.InvalidCurrency field)
  else match convertAmount rates m.amount m.currency cartCurrency with
    | none => .error (.CurrencyMismatch field)
    | some c =>
      if !amountInRange m.amount then .error (.AmountOutOfRange field)
      else if !amountInRange c then .error (.AmountOutOfRange field)
      else .ok ()

def checkOptMoney (m : Option Money) (cartCurrency : Currency) (rates : List ExchangeRate) (field : String) :
    Except PricingError Unit :=
  match m with
  | none => .ok ()
  | some m => checkMoney m cartCurrency rates field

/-- First element seen twice, scanning in order. -/
def firstDuplicate : List String → List String → Option String
  | [], _ => none
  | x :: xs, seen => if x ∈ seen then some x else firstDuplicate xs (x :: seen)

def checkDuplicates (ids : List String) (mk : String → PricingError) : Except PricingError Unit :=
  match firstDuplicate ids [] with
  | some x => .error (mk x)
  | none => .ok ()

/-- Index of the second occurrence of a repeated element, if any. -/
def secondOccurrence : List String → List String → Nat → Option Nat
  | [], _, _ => none
  | x :: xs, seen, k => if x ∈ seen then some k else secondOccurrence xs (x :: seen) (k + 1)

end Discount

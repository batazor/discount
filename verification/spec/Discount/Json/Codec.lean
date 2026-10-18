import Lean.Data.Json
import Discount.Types

/-!
# JSON helpers (docs/ubiquitous-language.md)

Handwritten encoders and decoders so that the wire format is exactly the documented one:
enums as `{"type": "<Variant>", ...fields}`, optional fields as `null`, extra keys rejected.
Output key order is alphabetical (Lean's `Json.obj` is a sorted map); the DRT compares parsed
values, not bytes.
-/
namespace Discount

open Lean (Json ToJson FromJson toJson fromJson?)

def objKeys : Json → List String
  | .obj kvs => kvs.keys
  | _ => []

/-- Fail if `j` is not an object or carries a key outside `allowed`. -/
def checkKeys (j : Json) (allowed : List String) : Except String Unit :=
  match j with
  | .obj _ =>
      match (objKeys j).find? (fun k => !(allowed.contains k)) with
      | some k => .error s!"unexpected key '{k}'"
      | none => .ok ()
  | _ => .error "expected object"

def getField [FromJson α] (j : Json) (k : String) : Except String α := do
  let v ← j.getObjVal? k
  fromJson? v

/-- Missing key and `null` both read as `none`. -/
def getOpt [FromJson α] (j : Json) (k : String) : Except String (Option α) :=
  match j.getObjVal? k with
  | .error _ => .ok none
  | .ok .null => .ok none
  | .ok v => some <$> fromJson? v

def optJson [ToJson α] : Option α → Json
  | none => .null
  | some a => toJson a

def variant (name : String) (fields : List (String × Json) := []) : Json :=
  Json.mkObj (("type", Json.str name) :: fields)

def getVariant (j : Json) : Except String String := getField j "type"

end Discount

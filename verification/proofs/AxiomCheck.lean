import Proofs

/-!
Prints the axioms every exported theorem depends on. CI fails if `sorryAx` appears
(ADR 2: no `sorry` in merged Lean code). Run with `lake env lean AxiomCheck.lean` in `verification/proofs`.
-/
open Discount

#print axioms final_total_bounds
#print axioms line_total_bounds
#print axioms discount_le_cap
#print axioms discount_le_budget
#print axioms total_discount_le_order_cap
#print axioms total_discount_le_subtotal
#print axioms total_discount_eq_sum
#print axioms exclusive_alone
#print axioms first_match_at_most_one
#print axioms allocation_conserves
#print axioms line_discount_sum
#print axioms eligible_depends_on_input
#print axioms allocate_sum
#print axioms allocate_le
#print axioms allocate_nonneg
#print axioms roundDiv_nonneg
#print axioms roundDiv_le
#print axioms validate_ok
#print axioms applyOne_inv
#print axioms sortPromotions_perm
#print axioms validate_perm
#print axioms apply_perm_invariant
#print axioms applyOne_unclamped
#print axioms commute_of_running_independent
#print axioms fixed_amounts_commute
#print axioms additive_percentages_commute
#print axioms sequential_percentages_commute_upto_rounding

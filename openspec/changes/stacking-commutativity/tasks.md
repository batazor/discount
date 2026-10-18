# Tasks

- [x] 1. `Proofs/Rounding.lean`: `roundDiv_err`, two-sided bound on `den * roundDiv p x den - x` for every policy
- [x] 2. `Proofs/Invariants/Commutativity.lean`: `applyOne_unclamped`, `commute_of_running_independent`, `fixed_amounts_commute`, `additive_percentages_commute`, `sequential_percentages_commute_upto_rounding`; verify `lake build` and AxiomCheck (no `sorryAx`)
- [x] 3. `docs/invariants.md` row 3 and §3, README row 3; verify `openspec validate stacking-commutativity`

## Workflow follow-up

- Archive the change after review: `openspec archive stacking-commutativity`.

import Proofs.Invariants.Bounds
import Proofs.Invariants.Caps
import Proofs.Invariants.Exclusivity
import Proofs.Invariants.Allocation
import Proofs.Invariants.Eligibility
import Proofs.Invariants.Determinism

/-!
# The invariants of `docs/invariants.md`, stated on `apply input = .ok result`

One file per numbered invariant; the numbering follows the document (`Determinism` is the
permutation part of 3; its commutativity part and 6 are not yet proved, see the status table
there). Everything is derived from `finalStep_inv` and the
stage lemmas; nothing is assumed about the input beyond `validate = ok`.
-/

import Proofs.Invariants.Bounds
import Proofs.Invariants.Caps
import Proofs.Invariants.Exclusivity
import Proofs.Invariants.Allocation
import Proofs.Invariants.Eligibility
import Proofs.Invariants.Determinism
import Proofs.Invariants.Commutativity

/-!
# The invariants of `docs/invariants.md`, stated on `apply input = .ok result`

One file per numbered invariant; the numbering follows the document (`Determinism` is the
permutation part of 3, `Commutativity` the part of 3 that holds; 6 is a service property, see
the status table there). Everything is derived from `finalStep_inv` and the
stage lemmas; nothing is assumed about the input beyond `validate = ok`.
-/

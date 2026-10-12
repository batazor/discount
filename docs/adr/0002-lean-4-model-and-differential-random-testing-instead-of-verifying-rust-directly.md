# 2. Lean 4 model and differential random testing instead of verifying Rust directly

Date: 2026-10-12

## Status

Accepted


Is depended on by [3. Integer money, basis points, explicit rounding policy, bounded amounts](0003-integer-money-basis-points-explicit-rounding-policy-bounded-amounts.md)


## Context

The pricing engine must satisfy a small set of invariants (`docs/invariants.md`) with a
degree of confidence that unit tests do not give: a wrong sign or a lost minor unit in a
discount is a money bug. The service itself is ordinary Rust (Axum, Postgres); only the
pure pricing core is worth proving.

Two families of approaches exist:

- Verify the Rust code itself (Aeneas, Creusot, Verus, Kani).
- Write an executable model in a proof assistant, prove the invariants about the model,
  and show the production code agrees with the model by differential testing. This is the
  approach AWS took with Cedar (Lean model + DRT against the Rust implementation).

Options considered:

1. Verify Rust directly (Creusot/Verus/Aeneas for everything). Rejected for now: the
   toolchains constrain how the Rust is written, are younger, and couple proof effort to
   implementation detail (serde derives, error enums). Kept as a later add-on for the two
   numeric functions where it pays off.
2. Lean model + DRT. Chosen.
3. Property-based tests only. Cheap, but gives no proof of invariants 1, 2, 5 and cannot
   catch a wrong specification, only disagreement between two guesses.

## Decision

1. The Lean 4 model is the primary artifact. Domain types, the `apply` function and the
   invariant theorems live in `spec/` as a `lake` project. The model is compiled into a
   CLI reference: JSON on stdin, JSON on stdout.
2. The Rust `core` crate mirrors the Lean model structurally: same type names, same
   function decomposition, same order of operations (ADR 8).
3. Differential random testing (DRT) in `drt/` generates carts, promotions and `now` with
   `proptest`, runs both implementations and asserts byte-identical JSON results,
   including error results (ADR 6). Communication is a child process over stdin/stdout.
   No FFI.
4. Aeneas is optional and later. If used, it is limited to the rounding and allocation
   functions, which stay inside Aeneas' supported subset: safe Rust, no async, no trait
   objects, no closures beyond what Aeneas documents.
5. Lean toolchain is pinned in `spec/lean-toolchain` (currently `v4.34.1`). No Mathlib.
   Integer bounds proofs use `omega`, `decide`, `simp` and core lemmas on `Int`/`Nat`.
   Mathlib would add a multi-GB dependency and tens of minutes to CI for lemmas we do not
   need.
6. `sorry` is forbidden in merged Lean code. CI runs `lake build`; a blocked proof is
   reported, not stubbed.

## Consequences

- Two implementations of the pricing engine must be kept in sync. DRT makes drift loud.
- Every domain concept is first a Lean type; the ubiquitous language is checked by the
  compiler in two languages.
- The Lean executable must build in CI, so the GitHub Actions workflow installs `elan`
  (`leanprover/lean-action`) and caches `.lake`.
- JSON is the shared contract. Field names and enum encodings in Lean (`ToJson`/`FromJson`)
  and Rust (`serde`) must agree exactly; the DRT harness fails fast on a schema mismatch.
- Proof obligations are stated over the Lean model. Anything the DRT does not exercise
  (e.g. amounts outside the generator bounds) is not covered by the equivalence argument;
  ADR 3 bounds the inputs so the generators cover the whole valid domain.

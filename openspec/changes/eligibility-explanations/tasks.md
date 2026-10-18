# Tasks

- [ ] 1. `api/openapi.yaml`: `explain` parameter, `Explanation`, `ExplainedResult`, `ExplainedRedeemResponse`; UL vocabulary of codes; verify lint and `openspec validate eligibility-explanations`
- [ ] 2. Application: pure `explain` function with a unit test per code
- [ ] 3. HTTP: flag handling on both routes; tests; contract test; smoke step (`explain=true` on the single-use code reuse shows `coupon.exhausted`)

## Workflow follow-up

- Archive the change after review: `openspec archive eligibility-explanations`.

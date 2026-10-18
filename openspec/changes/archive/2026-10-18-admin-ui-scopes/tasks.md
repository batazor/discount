# Tasks

## 1. Access model and navigation (`src/ui/src/model`, `src/ui/src/middleware.ts`, `src/ui/src/layouts`)

- [x] 1.1 `model/access.ts` with `accessOf` and `navigation`, vitest for "Support account" ordering and the no-sign-in case; role files removed; `locals.access` from the middleware; layout without the role switch; verify `npm test` and `astro check`

## 2. Overview and gated screens (`src/ui/src/pages`, `src/ui/src/components`)

- [x] 2.1 One overview by scope; read-only gating with a note on campaign tabs, settings and coupon pages; pricing gating on Explain, try-it and redemption transitions; customers gating; budget form back on the overview tab; verify "Support sees the catalogue read-only", "First run" and "Budget edited from the overview" against a local stack

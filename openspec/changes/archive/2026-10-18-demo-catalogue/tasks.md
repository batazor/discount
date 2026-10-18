# Tasks

## 1. Fixture loader (`src/infrastructure/`)

- [x] 1.1 Extend `CatalogFixture` in `persistence/memory.rs` with `as_of`, `metadata`, `allowlists`, `grant_rules` (all defaulted) and seed them in `with_fixture`; verify `cargo test -p discount-infrastructure --lib` passes
- [x] 1.2 Make `deploy/demo/catalogue.json` the default `DISCOUNT_PROMOTIONS` in `config.rs` and the file the `Dockerfile` copies; verify the config test and the image build

## 2. Demo catalogue (`deploy/demo/`)

- [x] 2.1 Write `deploy/demo/catalogue.json`: four groups, 21 promotions over every rule and action variant and every console status, codes, rates, titles, an allowlist, grant rules, `as_of`; verify with the consistency test in `persistence/memory.rs`
- [x] 2.2 Write `deploy/demo/seed.py` (stdlib only): load the catalogue with shifted windows, generate one batch of codes, play the order history, print what to look up; verify two runs against the in-memory server and the compose stack leave the same state

## 3. Documentation (`README.md`, `openspec/`)

- [x] 3.1 README paragraph under the local stack section and the layout line; `evaluate-http-api` spec names the new default; verify the paths quoted exist

# Design

See ADR 10 for the decision and alternatives. Mechanics: `src/client/build.rs` parses the
document with `openapiv3`, runs progenitor and writes `codegen.rs` into `OUT_DIR`;
`lib.rs` includes it. The contract test in `src/client/tests` starts the axum router on a
random port with the in-memory store and uses the generated client. The spec stays 3.0.3
because progenitor and most generators do not read 3.1 type arrays or `const`.

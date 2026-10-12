//! # discount-drt
//!
//! Differential random testing harness (ADR 2): a driver for the Lean reference
//! executable (`spec/.lake/build/bin/discount-spec`, one JSON document per line) and the
//! comparison of parsed results. Generators and the proptest suite are the next
//! deliverable; `tests/fixtures.rs` already replays the hand-written fixtures through both
//! implementations.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use discount_core::{PricingError, PricingInput, PricingResult, apply};
use serde_json::Value;

/// Where the Lean reference lives. `DISCOUNT_SPEC_BIN` overrides the default path, which
/// is relative to the workspace root.
pub fn spec_binary() -> PathBuf {
    if let Ok(p) = std::env::var("DISCOUNT_SPEC_BIN") {
        return PathBuf::from(p);
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    root.join("spec/.lake/build/bin/discount-spec")
}

/// A running Lean reference process.
pub struct Reference {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Reference {
    pub fn spawn() -> std::io::Result<Reference> {
        let mut child = Command::new(spec_binary())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        let stdin = child.stdin.take().expect("stdin");
        let stdout = BufReader::new(child.stdout.take().expect("stdout"));
        Ok(Reference {
            child,
            stdin,
            stdout,
        })
    }

    /// Send one input document (one line) and read the envelope back.
    pub fn evaluate_json(&mut self, line: &str) -> std::io::Result<Value> {
        self.stdin.write_all(line.as_bytes())?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;
        let mut out = String::new();
        self.stdout.read_line(&mut out)?;
        serde_json::from_str(&out)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    pub fn evaluate(&mut self, input: &PricingInput) -> std::io::Result<Value> {
        let line = serde_json::to_string(input).expect("serialize input");
        self.evaluate_json(&line)
    }
}

impl Drop for Reference {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The Rust side of the comparison: the same envelope the Lean executable prints.
pub fn rust_envelope(result: &Result<PricingResult, PricingError>) -> Value {
    match result {
        Ok(r) => serde_json::json!({ "ok": serde_json::to_value(r).expect("serialize result") }),
        Err(e) => serde_json::json!({ "error": serde_json::to_value(e).expect("serialize error") }),
    }
}

pub fn rust_evaluate(input: &PricingInput) -> Value {
    rust_envelope(&apply(input))
}

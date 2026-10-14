//! Spec-first: the client is generated from `api/openapi.yaml` at build time. Nothing in
//! this crate is written by hand except this file and `lib.rs`.

use std::path::Path;

fn main() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../api/openapi.yaml");
    println!("cargo:rerun-if-changed={}", src.display());
    let file = std::fs::File::open(&src).expect("api/openapi.yaml");
    let spec: openapiv3::OpenAPI =
        serde_yaml_ng::from_reader(file).expect("parse api/openapi.yaml");
    let mut generator = progenitor::Generator::default();
    let tokens = generator
        .generate_tokens(&spec)
        .expect("generate client from api/openapi.yaml");
    let ast = syn::parse2(tokens).expect("generated code parses");
    let content = prettyplease::unparse(&ast);
    let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join("codegen.rs");
    std::fs::write(out, content).expect("write codegen.rs");
}

//! Driver-level fixed-point: each self-hosted stage source, compiled end to end
//! through the self-hosted pipeline (`selfhost::self_host_compile`), emits bytecode
//! byte-identical to the Rust-hosted reference compiler. This is the same property
//! the parent crate's `tests/selfhost_codegen.rs` proves per stage, exercised here
//! through the compiler subproject's library so the `compile` command and this test
//! share one implementation. The three stages together are the self-hosting fixed
//! point precondition: the self-hosted compiler reproduces its own three sources.
//!
//! Reconstruction remains host-side Rust and the module scaffold is taken from the
//! reference (see `src/selfhost.rs` and `MILESTONES.md`); this test pins the ops,
//! constant pool, and local-frame size the stages themselves produce.

use keleusma_selfhost::selfhost::{compile_src, self_host_compile};

fn assert_stage_self_compiles(rel: &str) {
    let src = std::fs::read_to_string(rel)
        .or_else(|_| std::fs::read_to_string(format!("compiler/{rel}")))
        .unwrap_or_else(|e| panic!("cannot read {rel}: {e}"));
    let module = self_host_compile(&src);
    let reference = compile_src(&src);
    assert_eq!(
        module.chunks.len(),
        reference.chunks.len(),
        "chunk count for {rel}"
    );
    for (m, r) in module.chunks.iter().zip(reference.chunks.iter()) {
        assert_eq!(m.name, r.name, "chunk order in {rel}");
        assert_eq!(m.ops, r.ops, "ops for `{}` in {rel}", r.name);
        assert_eq!(m.constants, r.constants, "pool for `{}` in {rel}", r.name);
        assert_eq!(
            m.local_count, r.local_count,
            "local_count for `{}` in {rel}",
            r.name
        );
    }
}

#[test]
fn lexer_kel_self_compiles_byte_identically() {
    assert_stage_self_compiles("kel/lexer.kel");
}

#[test]
fn codegen_kel_self_compiles_byte_identically() {
    assert_stage_self_compiles("kel/codegen.kel");
}

#[test]
fn parse_kel_self_compiles_byte_identically() {
    assert_stage_self_compiles("kel/parse.kel");
}

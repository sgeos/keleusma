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

/// Read a stage source by basename. The four pipeline stages moved into the parent
/// crate at `src/selfhost/kel/`; the subproject-only sources stay in `compiler/kel/`.
fn read_stage(rel: &str) -> String {
    let base = rel.rsplit('/').next().unwrap_or(rel);
    let relocated = matches!(
        base,
        "lexer.kel"
            | "parse.kel"
            | "reconstruct.kel"
            | "codegen.kel"
            | "analyze.kel"
            | "verify_structural.kel"
            | "verify_yield.kel"
            | "verify_depth.kel"
            | "verify_typed.kel"
            | "verify_datalayout.kel"
    );
    let candidates: [String; 6] = if relocated {
        [
            format!("src/selfhost/kel/{base}"),
            format!("../src/selfhost/kel/{base}"),
            format!("compiler/{rel}"),
            rel.to_string(),
            format!("kel/{base}"),
            format!("compiler/kel/{base}"),
        ]
    } else {
        [
            format!("compiler/kel/{base}"),
            format!("kel/{base}"),
            format!("../compiler/kel/{base}"),
            format!("compiler/{rel}"),
            rel.to_string(),
            format!("src/selfhost/kel/{base}"),
        ]
    };
    for cand in &candidates {
        if let Ok(s) = std::fs::read_to_string(cand) {
            return s;
        }
    }
    panic!("cannot read stage `{rel}` (tried {candidates:?})");
}

fn assert_stage_self_compiles(rel: &str) {
    let src = read_stage(rel);
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

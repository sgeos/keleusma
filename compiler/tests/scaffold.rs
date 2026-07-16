//! Driver-level scaffold fixed point: each self-hosted stage source, compiled through
//! the from-scratch-scaffold entry (`selfhost::self_host_compile_full`), serializes
//! byte-for-byte identically to the Rust-hosted reference compiler. Unlike
//! `fixed_point.rs`, which pins only the ops/constants/local_count the stages emit,
//! this pins the whole wire encoding: the data layout, enum-layout table, chunk
//! signatures, schema hash, and declared WCET/WCMU header are assembled from the
//! pipeline output rather than borrowed from the reference. The reference module is the
//! comparison oracle only; a single differing byte means the assembly is wrong.
//!
//! Stage sources are read with the compiler-relative `kel/<stage>.kel` path.

use keleusma_selfhost::selfhost::{compile_src, self_host_compile_full};

fn assert_scaffold_byte_identical(rel: &str) {
    let src = std::fs::read_to_string(rel)
        .or_else(|_| std::fs::read_to_string(format!("compiler/{rel}")))
        .unwrap_or_else(|e| panic!("cannot read {rel}: {e}"));
    let self_bytes = self_host_compile_full(&src)
        .to_bytes()
        .unwrap_or_else(|e| panic!("serialize self-assembled module for {rel}: {e:?}"));
    let ref_bytes = compile_src(&src)
        .to_bytes()
        .unwrap_or_else(|e| panic!("serialize reference module for {rel}: {e:?}"));
    assert_eq!(self_bytes, ref_bytes, "serialized module for {rel}");
}

#[test]
fn lexer_kel_scaffold_serializes_byte_identically() {
    assert_scaffold_byte_identical("kel/lexer.kel");
}

#[test]
fn parse_kel_scaffold_serializes_byte_identically() {
    assert_scaffold_byte_identical("kel/parse.kel");
}

#[test]
fn reconstruct_kel_scaffold_serializes_byte_identically() {
    assert_scaffold_byte_identical("kel/reconstruct.kel");
}

#[test]
fn codegen_kel_scaffold_serializes_byte_identically() {
    assert_scaffold_byte_identical("kel/codegen.kel");
}

#[test]
fn analyze_kel_scaffold_serializes_byte_identically() {
    assert_scaffold_byte_identical("kel/analyze.kel");
}

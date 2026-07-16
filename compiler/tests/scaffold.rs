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

// The self-hosted bookkeeping (`self_host_module_bookkeeping`) derives `FLAG_EPHEMERAL` from the
// absence of private data. The five stage files all have private data, so they exercise only the
// non-ephemeral path (flags 0). This private-data-free program is marked ephemeral by the
// reference, so byte-identity here confirms the self-hosted flags computation genuinely sets the
// bit -- and the `expect_ephemeral` check keeps the test non-vacuous.
#[test]
fn ephemeral_module_scaffold_serializes_byte_identically() {
    let src = "require word >= 32;\n\
               shared data io { out: Word }\n\
               loop main(r: Word) -> Word { io.out = r + 1; yield io.out }";
    let self_module = self_host_compile_full(src);
    let ref_module = compile_src(src);
    assert!(
        ref_module.flags & keleusma::bytecode::FLAG_EPHEMERAL != 0,
        "the reference must mark this private-data-free program ephemeral for the test to bind"
    );
    assert_eq!(
        self_module.to_bytes().unwrap(),
        ref_module.to_bytes().unwrap(),
        "serialized ephemeral module (flags path)"
    );
}

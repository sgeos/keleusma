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

use keleusma_selfhost::selfhost::{compile_src, self_host_compile_full, self_host_compile_scratch};

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

/// The private-data-free program the ephemeral scaffold test uses, kept in one place so the
/// full-scaffold and from-scratch tests exercise the same source.
const EPHEMERAL_SRC: &str = "require word >= 32;\n\
                             shared data io { out: Word }\n\
                             loop main(r: Word) -> Word { io.out = r + 1; yield io.out }";

/// The from-scratch entry (`self_host_compile_scratch`) builds every module field from the
/// pipeline output with no reference borrow of the user program, yet serializes byte-for-byte
/// identically to the reference. Before trusting `to_bytes`, compare the chunks and the scalar
/// header fields field by field so a mismatch names the offending field rather than a byte
/// offset.
fn assert_scratch_byte_identical(rel_or_src: &str, is_path: bool) {
    let src = if is_path {
        std::fs::read_to_string(rel_or_src)
            .or_else(|_| std::fs::read_to_string(format!("compiler/{rel_or_src}")))
            .unwrap_or_else(|e| panic!("cannot read {rel_or_src}: {e}"))
    } else {
        rel_or_src.to_string()
    };
    let scratch = self_host_compile_scratch(&src);
    let reference = compile_src(&src);

    // Field-by-field diagnosis: chunks first, then the scalar header fields.
    assert_eq!(
        scratch.chunks.len(),
        reference.chunks.len(),
        "chunk count for {rel_or_src}"
    );
    for (s, r) in scratch.chunks.iter().zip(reference.chunks.iter()) {
        assert_eq!(s.name, r.name, "chunk name for {rel_or_src}");
        assert_eq!(s.ops, r.ops, "chunk `{}` ops for {rel_or_src}", r.name);
        assert_eq!(
            s.constants, r.constants,
            "chunk `{}` constants for {rel_or_src}",
            r.name
        );
        assert_eq!(
            s.local_count, r.local_count,
            "chunk `{}` local_count for {rel_or_src}",
            r.name
        );
        assert_eq!(
            s.param_count, r.param_count,
            "chunk `{}` param_count for {rel_or_src}",
            r.name
        );
        assert_eq!(
            s.block_type, r.block_type,
            "chunk `{}` block_type for {rel_or_src}",
            r.name
        );
        assert_eq!(
            s.param_types, r.param_types,
            "chunk `{}` param_types for {rel_or_src}",
            r.name
        );
    }
    assert_eq!(
        scratch.native_names, reference.native_names,
        "native_names for {rel_or_src}"
    );
    assert_eq!(
        scratch.entry_point, reference.entry_point,
        "entry_point for {rel_or_src}"
    );
    assert_eq!(
        scratch.word_bits_log2, reference.word_bits_log2,
        "word_bits_log2 for {rel_or_src}"
    );
    assert_eq!(
        scratch.addr_bits_log2, reference.addr_bits_log2,
        "addr_bits_log2 for {rel_or_src}"
    );
    assert_eq!(
        scratch.float_bits_log2, reference.float_bits_log2,
        "float_bits_log2 for {rel_or_src}"
    );
    assert_eq!(
        scratch.wcet_cycles, reference.wcet_cycles,
        "wcet_cycles for {rel_or_src}"
    );
    assert_eq!(
        scratch.wcmu_bytes, reference.wcmu_bytes,
        "wcmu_bytes for {rel_or_src}"
    );
    assert_eq!(
        scratch.aux_arena_bytes, reference.aux_arena_bytes,
        "aux_arena_bytes for {rel_or_src}"
    );
    assert_eq!(
        scratch.persistent_composite_bytes, reference.persistent_composite_bytes,
        "persistent_composite_bytes for {rel_or_src}"
    );
    assert_eq!(scratch.flags, reference.flags, "flags for {rel_or_src}");
    assert_eq!(
        scratch.shared_data_bytes, reference.shared_data_bytes,
        "shared_data_bytes for {rel_or_src}"
    );
    assert_eq!(
        scratch.private_data_bytes, reference.private_data_bytes,
        "private_data_bytes for {rel_or_src}"
    );
    assert_eq!(
        scratch.schema_hash, reference.schema_hash,
        "schema_hash for {rel_or_src}"
    );
    assert_eq!(
        scratch.native_return_shapes, reference.native_return_shapes,
        "native_return_shapes for {rel_or_src}"
    );

    // The whole wire encoding must match, byte for byte.
    let scratch_bytes = scratch
        .to_bytes()
        .unwrap_or_else(|e| panic!("serialize from-scratch module for {rel_or_src}: {e:?}"));
    let ref_bytes = reference
        .to_bytes()
        .unwrap_or_else(|e| panic!("serialize reference module for {rel_or_src}: {e:?}"));
    assert_eq!(
        scratch_bytes, ref_bytes,
        "serialized from-scratch module for {rel_or_src}"
    );
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
    let self_module = self_host_compile_full(EPHEMERAL_SRC);
    let ref_module = compile_src(EPHEMERAL_SRC);
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

// The from-scratch entry (`self_host_compile_scratch`) builds the module with NO reference
// borrow of the user program (no `compile_src(src)` on the user program), yet is byte-identical
// to the reference for the five stage sources and the ephemeral program.

#[test]
fn lexer_kel_scratch_serializes_byte_identically() {
    assert_scratch_byte_identical("kel/lexer.kel", true);
}

#[test]
fn parse_kel_scratch_serializes_byte_identically() {
    assert_scratch_byte_identical("kel/parse.kel", true);
}

#[test]
fn reconstruct_kel_scratch_serializes_byte_identically() {
    assert_scratch_byte_identical("kel/reconstruct.kel", true);
}

#[test]
fn codegen_kel_scratch_serializes_byte_identically() {
    assert_scratch_byte_identical("kel/codegen.kel", true);
}

#[test]
fn analyze_kel_scratch_serializes_byte_identically() {
    assert_scratch_byte_identical("kel/analyze.kel", true);
}

#[test]
fn ephemeral_module_scratch_serializes_byte_identically() {
    let ref_module = compile_src(EPHEMERAL_SRC);
    assert!(
        ref_module.flags & keleusma::bytecode::FLAG_EPHEMERAL != 0,
        "the reference must mark this private-data-free program ephemeral for the test to bind"
    );
    assert_scratch_byte_identical(EPHEMERAL_SRC, false);
}

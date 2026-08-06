//! Differential validation of the wire-format v2 schema against the largest real
//! Keleusma programs that exist: the ten self-hosted compiler stage sources.
//!
//! # Why this test rather than more hand-built cases
//!
//! Every other test of `wire_schema` uses data I constructed, which means it
//! exercises the shapes I thought of. These are real compiler output —
//! thousands of lines each, with whatever constant trees, struct templates,
//! signatures and parameter types the compiler actually emits. `parse.kel` alone
//! is over six thousand lines.
//!
//! This is the evidence that the codec is ready to be routed into the runtime.
//! A cutover justified only by hand-built round trips would be resting on my
//! imagination of what a module looks like.

#![cfg(feature = "compile")]

use keleusma::bytecode::Module;
use keleusma::wire_format::{WireAuxBody, WireChunk};
use keleusma::wire_schema::{decode_aux_body, encode_aux_body};

/// All ten stage sources, embedded the same way the self-host driver embeds them.
///
/// These were briefly split, with the two largest behind `#[ignore]`, because the
/// full corpus took **782 seconds**. That turned out to be a quadratic interner
/// in the encoder rather than an inherent cost — see `Names` in
/// `src/wire_schema.rs`. With that fixed the whole corpus runs in about two and a
/// half seconds, so the split was removed: it would have hidden the two most
/// valuable inputs behind a flag nobody passes, to dodge a defect that no longer
/// exists.
const CORPUS: &[(&str, &str)] = &[
    ("lexer", include_str!("../src/selfhost/kel/lexer.kel")),
    ("parse", include_str!("../src/selfhost/kel/parse.kel")),
    ("codegen", include_str!("../src/selfhost/kel/codegen.kel")),
    (
        "reconstruct",
        include_str!("../src/selfhost/kel/reconstruct.kel"),
    ),
    ("analyze", include_str!("../src/selfhost/kel/analyze.kel")),
    (
        "verify_structural",
        include_str!("../src/selfhost/kel/verify_structural.kel"),
    ),
    (
        "verify_typed",
        include_str!("../src/selfhost/kel/verify_typed.kel"),
    ),
    (
        "verify_yield",
        include_str!("../src/selfhost/kel/verify_yield.kel"),
    ),
    (
        "verify_depth",
        include_str!("../src/selfhost/kel/verify_depth.kel"),
    ),
    (
        "verify_datalayout",
        include_str!("../src/selfhost/kel/verify_datalayout.kel"),
    ),
];

/// Builds the auxiliary body a module would be serialised from.
///
/// `op_byte_offset` is left zero: it is assigned by the opcode-stream layout,
/// which is not what this test exercises. Everything else is the compiler's
/// real output.
fn aux_of(module: &Module) -> WireAuxBody {
    WireAuxBody {
        chunks: module
            .chunks
            .iter()
            .map(|c| WireChunk {
                name: c.name.clone(),
                constants: c.constants.clone(),
                struct_templates: c.struct_templates.clone(),
                local_count: c.local_count,
                param_count: c.param_count,
                block_type: c.block_type,
                param_types: c.param_types.clone(),
                op_byte_offset: 0,
                op_record_count: c.ops.len() as u32,
                debug_pool_bytes: None,
            })
            .collect(),
        native_names: module.native_names.clone(),
        entry_point: module.entry_point,
        data_layout: module.data_layout.clone(),
        word_bits_log2: module.word_bits_log2,
        addr_bits_log2: module.addr_bits_log2,
        float_bits_log2: module.float_bits_log2,
        wcet_cycles: module.wcet_cycles,
        wcmu_bytes: module.wcmu_bytes,
        flags: module.flags,
        shared_data_bytes: module.shared_data_bytes,
        private_data_bytes: module.private_data_bytes,
        schema_hash: module.schema_hash,
        enum_layouts: module.enum_layouts.clone(),
        signatures: module.signatures.clone(),
        native_return_shapes: module.native_return_shapes.clone(),
    }
}

fn compile_stage(src: &str) -> Module {
    let tokens = keleusma::lexer::tokenize(src).expect("lex");
    let program = keleusma::parser::parse(&tokens).expect("parse");
    keleusma::compiler::compile(&program).expect("compile")
}

/// Structural equality including the enum discriminant.
///
/// `ConstValue`'s own `PartialEq` ignores `discriminant`, so `assert_eq!` on a
/// round trip is blind to whether it survived. Duplicated from the unit suite
/// deliberately: an integration test that silently stopped checking the
/// discriminant would look exactly like one that checks it.
fn deep_eq(a: &keleusma::bytecode::ConstValue, b: &keleusma::bytecode::ConstValue) -> bool {
    use keleusma::bytecode::ConstValue as C;
    match (a, b) {
        (
            C::Enum {
                type_name: na,
                variant: va,
                discriminant: da,
                fields: fa,
            },
            C::Enum {
                type_name: nb,
                variant: vb,
                discriminant: db,
                fields: fb,
            },
        ) => {
            na == nb
                && va == vb
                && da == db
                && fa.len() == fb.len()
                && fa.iter().zip(fb).all(|(x, y)| deep_eq(x, y))
        }
        (C::Tuple(x), C::Tuple(y)) | (C::Array(x), C::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| deep_eq(p, q))
        }
        (
            C::Struct {
                type_name: na,
                fields: fa,
            },
            C::Struct {
                type_name: nb,
                fields: fb,
            },
        ) => {
            na == nb
                && fa.len() == fb.len()
                && fa
                    .iter()
                    .zip(fb)
                    .all(|((kn, kv), (ln, lv))| kn == ln && deep_eq(kv, lv))
        }
        _ => a == b,
    }
}

fn assert_round_trip(stage: &str, want: &WireAuxBody) {
    let bytes = encode_aux_body(want).unwrap_or_else(|e| panic!("{stage}: encode failed: {e}"));
    let got = decode_aux_body(&bytes).unwrap_or_else(|e| panic!("{stage}: decode failed: {e:?}"));

    assert_eq!(got.chunks.len(), want.chunks.len(), "{stage}: chunk count");
    for (g, w) in got.chunks.iter().zip(&want.chunks) {
        let at = || format!("{stage}::{}", w.name);
        assert_eq!(g.name, w.name, "{}", at());
        assert_eq!(g.local_count, w.local_count, "{}", at());
        assert_eq!(g.param_count, w.param_count, "{}", at());
        assert_eq!(g.block_type, w.block_type, "{}", at());
        assert_eq!(g.param_types, w.param_types, "{}", at());
        assert_eq!(g.op_record_count, w.op_record_count, "{}", at());
        assert_eq!(
            g.constants.len(),
            w.constants.len(),
            "{}: constant count",
            at()
        );
        for (i, (gc, wc)) in g.constants.iter().zip(&w.constants).enumerate() {
            assert!(
                deep_eq(gc, wc),
                "{}: constant {i}\n got {gc:?}\nwant {wc:?}",
                at()
            );
        }
        assert_eq!(
            g.struct_templates.len(),
            w.struct_templates.len(),
            "{}: template count",
            at()
        );
        for (gt, wt) in g.struct_templates.iter().zip(&w.struct_templates) {
            assert_eq!(gt.type_name, wt.type_name, "{}", at());
            assert_eq!(gt.field_names, wt.field_names, "{}", at());
        }
    }

    assert_eq!(got.native_names, want.native_names, "{stage}: natives");
    assert_eq!(
        got.native_return_shapes, want.native_return_shapes,
        "{stage}: native return shapes"
    );
    assert_eq!(got.entry_point, want.entry_point, "{stage}: entry point");
    assert_eq!(got.schema_hash, want.schema_hash, "{stage}: schema hash");
    assert_eq!(got.wcet_cycles, want.wcet_cycles, "{stage}: wcet");
    assert_eq!(got.wcmu_bytes, want.wcmu_bytes, "{stage}: wcmu");
    assert_eq!(got.flags, want.flags, "{stage}: flags");
    assert_eq!(
        got.shared_data_bytes, want.shared_data_bytes,
        "{stage}: shared bytes"
    );
    assert_eq!(
        got.private_data_bytes, want.private_data_bytes,
        "{stage}: private bytes"
    );

    assert_eq!(
        got.signatures.len(),
        want.signatures.len(),
        "{stage}: signature count"
    );
    for (i, (g, w)) in got.signatures.iter().zip(&want.signatures).enumerate() {
        assert_eq!(g.params, w.params, "{stage}: signature {i} params");
        assert_eq!(g.ret, w.ret, "{stage}: signature {i} ret");
        assert_eq!(g.resume, w.resume, "{stage}: signature {i} resume");
    }

    assert_eq!(
        got.enum_layouts.len(),
        want.enum_layouts.len(),
        "{stage}: enum layout count"
    );
    for (g, w) in got.enum_layouts.iter().zip(&want.enum_layouts) {
        assert_eq!(g.type_name, w.type_name, "{stage}");
        assert_eq!(g.min_payload, w.min_payload, "{stage}");
        assert_eq!(g.variants.len(), w.variants.len(), "{stage}");
        for (gv, wv) in g.variants.iter().zip(&w.variants) {
            assert_eq!(gv.name, wv.name, "{stage}");
            assert_eq!(gv.disc, wv.disc, "{stage}: discriminant");
        }
    }

    match (&got.data_layout, &want.data_layout) {
        (None, None) => {}
        (Some(g), Some(w)) => {
            assert_eq!(g.slots.len(), w.slots.len(), "{stage}: data slots");
            for (gs, ws) in g.slots.iter().zip(&w.slots) {
                assert_eq!(gs.name, ws.name, "{stage}");
                assert_eq!(gs.visibility, ws.visibility, "{stage}");
            }
            assert_eq!(g.shared_layout, w.shared_layout, "{stage}");
            assert_eq!(
                g.private_composite_layout, w.private_composite_layout,
                "{stage}"
            );
            assert_eq!(
                g.private_init.len(),
                w.private_init.len(),
                "{stage}: private init"
            );
            for (gi, wi) in g.private_init.iter().zip(&w.private_init) {
                assert!(deep_eq(gi, wi), "{stage}: private init value");
            }
        }
        (g, w) => panic!(
            "{stage}: data layout presence differs: {:?} vs {:?}",
            g.is_some(),
            w.is_some()
        ),
    }
}

#[test]
fn every_self_hosted_stage_round_trips_through_the_new_schema() {
    // Totals are printed and asserted non-trivial, so this cannot pass by
    // round-tripping a corpus that turned out to be empty.
    let mut total_chunks = 0usize;
    let mut total_consts = 0usize;
    let mut total_templates = 0usize;
    let mut total_sigs = 0usize;
    let mut with_data_layout = 0usize;

    for (stage, src) in CORPUS {
        let module = compile_stage(src);
        let aux = aux_of(&module);

        total_chunks += aux.chunks.len();
        total_consts += aux.chunks.iter().map(|c| c.constants.len()).sum::<usize>();
        total_templates += aux
            .chunks
            .iter()
            .map(|c| c.struct_templates.len())
            .sum::<usize>();
        total_sigs += aux.signatures.len();
        if aux.data_layout.is_some() {
            with_data_layout += 1;
        }

        assert_round_trip(stage, &aux);
    }

    println!(
        "corpus: {} stages, {total_chunks} chunks, {total_consts} constants, \
         {total_templates} templates, {total_sigs} signatures, \
         {with_data_layout} with a data layout",
        CORPUS.len()
    );

    // Coverage floors, below the measured values, so the test fails loudly if
    // the corpus stops exercising a table rather than passing on nothing.
    assert!(
        total_chunks > 100,
        "corpus too small: {total_chunks} chunks"
    );
    assert!(total_consts > 100, "few constants: {total_consts}");
    assert!(total_sigs > 100, "few signatures: {total_sigs}");
    assert!(
        with_data_layout > 0,
        "no stage exercised the data layout path"
    );

    // WHAT THIS CORPUS DOES NOT COVER, measured rather than assumed: the
    // self-hosted stages emit ZERO struct templates. Across all ten stages the
    // count is 0, so "the real corpus round-trips" says nothing about the
    // template table -- that is covered only by hand-built cases in
    // tests/wire_schema.rs. Asserted so the claim stays honest if the compiler
    // ever starts emitting them and someone reads this comment as still true.
    assert_eq!(
        total_templates, 0,
        "the corpus now emits struct templates ({total_templates}); \
         drop the coverage caveat in this test and in REVERSE_PROMPT.md"
    );
}

//! The (72,64) SECDED parity plane, end to end on real compiler output.
//!
//! # Why this file exists
//!
//! `keleusma-wire/src/ecc.rs` has had a complete SECDED implementation with
//! exhaustive single-bit-correction unit tests since the wire format landed,
//! and **no artifact the shipping encoder produced ever carried a plane**.
//! Every test of the mechanism built its own bytes.
//!
//! Radiation hardness is this project's stated value proposition. A correction
//! code proven only against inputs written to exercise it is the weakest part of
//! that claim: it shows the arithmetic is right, not that the artifact a host
//! actually loads is protected, nor that protecting it leaves the artifact
//! readable.
//!
//! So these tests use the real encoder over real stage sources, flip bits in the
//! encoded bytes, and check what the reader reports.
//!
//! # The control that matters most
//!
//! Every corruption case is paired with the SAME corruption applied to an
//! artifact built WITHOUT planes, asserting it goes undetected. Without that
//! pairing, a test showing "the flip was caught" cannot distinguish the parity
//! plane from some other check — the CRC, a length field, a structural
//! validation — happening to notice.
#![cfg(feature = "compile")]

use keleusma::bytecode::Module;
use keleusma::wire_format::{WireAuxBody, WireChunk};
use keleusma::wire_schema::{decode_aux_body, encode_aux_body, encode_aux_body_with_ecc};
use keleusma_wire::WireView;

/// Two stages, deliberately different in size and shape.
const STAGES: &[(&str, &str)] = &[
    (
        "verify_datalayout",
        include_str!("../src/selfhost/kel/verify_datalayout.kel"),
    ),
    ("codegen", include_str!("../src/selfhost/kel/codegen.kel")),
];

fn compile_stage(src: &str) -> Module {
    let tokens = keleusma::lexer::tokenize(src).expect("lex");
    let program = keleusma::parser::parse(&tokens).expect("parse");
    keleusma::compiler::compile(&program).expect("compile")
}

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

/// The byte offset of the first payload (non-plane, ECC-protected) region, and
/// its length. This is where a fault is injected.
fn first_protected_span(bytes: &[u8]) -> (usize, usize) {
    let view = WireView::parse(bytes).expect("parses");
    for i in 0..view.region_count() {
        let r = view.region_at(i).expect("region");
        if r.is_ecc_plane() || !r.has_ecc() {
            continue;
        }
        let (Some(base), Ok(payload)) = (r.byte_offset(), view.region_bytes(&r)) else {
            continue;
        };
        if !payload.is_empty() {
            return (base, payload.len());
        }
    }
    panic!("no protected non-empty region found");
}

#[test]
fn a_protected_artifact_still_decodes_through_the_ordinary_path() {
    for (name, src) in STAGES {
        let module = compile_stage(src);
        let aux = aux_of(&module);

        let plain = encode_aux_body(&aux).expect("encode");
        let armoured = encode_aux_body_with_ecc(&aux).expect("encode with ecc");

        // MUST-FIRE: the planes must actually have been added. Without this the
        // whole file passes against an encoder that ignored `with_ecc`.
        assert!(
            armoured.len() > plain.len(),
            "{name}: the ECC artifact is not larger, so no plane was emitted"
        );
        let pv = WireView::parse(&plain).expect("plain parses");
        let av = WireView::parse(&armoured).expect("armoured parses");
        assert!(
            av.region_count() > pv.region_count(),
            "{name}: no extra regions, so no plane was emitted"
        );
        assert_eq!(
            av.region_count(),
            pv.region_count() * 2,
            "{name}: expected exactly one plane per region"
        );

        // ADDITIVITY, which is the compatibility claim: the ordinary decode path
        // is untouched by the presence of planes. Asserted on the decoded value
        // rather than the bytes, since the bytes necessarily differ.
        let want = decode_aux_body(&plain).expect("decode plain");
        let got = decode_aux_body(&armoured).expect("decode armoured");
        assert_eq!(
            got.chunks.len(),
            want.chunks.len(),
            "{name}: chunk count differs"
        );
        assert_eq!(got.entry_point, want.entry_point);
        assert_eq!(got.schema_hash, want.schema_hash);
        assert_eq!(got.native_names, want.native_names);
        for (g, w) in got.chunks.iter().zip(&want.chunks) {
            assert_eq!(g.name, w.name, "{name}: chunk name differs");
            assert_eq!(g.param_types, w.param_types);
            assert_eq!(g.local_count, w.local_count);
        }

        // The whole-value check. `DataLayout` and `EnumLayout` carry no
        // `PartialEq`, so rather than compare the fields that happen to have one
        // -- which would quietly stop covering the data layout, the very table
        // this session has been changing -- re-encode both decoded bodies and
        // compare the bytes. Two values that encode identically are equal for
        // every purpose this format has.
        let re_plain = encode_aux_body(&want).expect("re-encode plain");
        let re_armoured = encode_aux_body(&got).expect("re-encode armoured");
        assert_eq!(
            re_armoured, re_plain,
            "{name}: decoding an ECC artifact produced a DIFFERENT body than \
             decoding the plain one, so planes are not additive after all"
        );

        // A clean artifact must scan clean, and `verify_all` must report SOME
        // report rather than None.
        let report = av
            .verify_all()
            .unwrap_or_else(|| panic!("{name}: verify_all found no planes"));
        assert!(
            report.is_clean(),
            "{name}: a freshly encoded artifact is not clean: {report:?}"
        );
        assert!(
            report.words > 0,
            "{name}: verify_all examined zero words, so it measured nothing"
        );

        // And the unprotected artifact must report None, not a clean report.
        // Treating "no ECC" as "verified" would call an unprotected artifact
        // sound.
        assert!(
            pv.verify_all().is_none(),
            "{name}: an artifact with no planes reported an ECC result"
        );
    }
}

#[test]
fn a_single_flipped_bit_in_a_protected_region_is_corrected() {
    for (name, src) in STAGES {
        let module = compile_stage(src);
        let aux = aux_of(&module);
        let armoured = encode_aux_body_with_ecc(&aux).expect("encode with ecc");
        let (base, len) = first_protected_span(&armoured);

        // Several positions, including the first and last byte of the region and
        // every bit of one byte, so this is not one lucky offset.
        let mut offsets: Vec<usize> = vec![0, 1, len / 2, len - 1];
        offsets.dedup();
        for off in offsets {
            for bit in 0..8u32 {
                let mut damaged = armoured.clone();
                damaged[base + off] ^= 1 << bit;

                let dv = WireView::parse(&damaged).expect("a damaged artifact still parses");
                let report = dv
                    .verify_all()
                    .unwrap_or_else(|| panic!("{name}: no planes after damage"));

                assert_eq!(
                    report.corrected, 1,
                    "{name}: byte {off} bit {bit}: expected exactly one corrected word, got {report:?}"
                );
                assert_eq!(
                    report.uncorrectable, 0,
                    "{name}: byte {off} bit {bit}: a single flip must be correctable, got {report:?}"
                );
                assert!(
                    report.needs_scrub(),
                    "{name}: a corrected word must ask for a scrub"
                );
            }
        }
    }
}

#[test]
fn two_flipped_bits_in_one_word_are_detected_as_uncorrectable() {
    for (name, src) in STAGES {
        let module = compile_stage(src);
        let aux = aux_of(&module);
        let armoured = encode_aux_body_with_ecc(&aux).expect("encode with ecc");
        let (base, _len) = first_protected_span(&armoured);

        // Two bits inside the SAME 64-bit word. SECDED detects but cannot
        // correct this, and reporting it as corrected would be the dangerous
        // failure: a silently wrong repair.
        for (b0, b1) in [(0u32, 1u32), (0, 7), (3, 4)] {
            let mut damaged = armoured.clone();
            damaged[base] ^= 1 << b0;
            damaged[base] ^= 1 << b1;

            let dv = WireView::parse(&damaged).expect("parses");
            let report = dv.verify_all().expect("planes present");
            assert_eq!(
                report.uncorrectable, 1,
                "{name}: bits {b0},{b1}: a double fault must be reported uncorrectable, got {report:?}"
            );
            assert_eq!(
                report.corrected, 0,
                "{name}: bits {b0},{b1}: a double fault must NOT be reported as corrected, which \
                 would mean a silently wrong repair, got {report:?}"
            );
        }
    }
}

/// THE CONTROL. The same corruption must go UNDETECTED without a plane.
///
/// Every assertion above shows the reader noticing a flipped bit. None of them,
/// alone, shows that the PARITY PLANE is what noticed. The container also
/// carries a CRC, region lengths, and structural validation, any of which might
/// have caught the same damage — in which case the plane would be decoration
/// and these tests would still pass.
#[test]
fn the_same_corruption_is_invisible_without_a_plane() {
    for (name, src) in STAGES {
        let module = compile_stage(src);
        let aux = aux_of(&module);

        let plain = encode_aux_body(&aux).expect("encode");
        // Find the same first non-empty region, this time without ECC flags.
        let base = {
            let view = WireView::parse(&plain).expect("parses");
            let mut found = None;
            for i in 0..view.region_count() {
                let r = view.region_at(i).expect("region");
                let (Some(b), Ok(p)) = (r.byte_offset(), view.region_bytes(&r)) else {
                    continue;
                };
                if !p.is_empty() {
                    found = Some(b);
                    break;
                }
            }
            found.expect("a non-empty region")
        };

        let mut damaged = plain.clone();
        damaged[base] ^= 1;

        // The artifact still parses: the prologue CRC covers twelve bytes of
        // prologue, NOT the body, so body damage is invisible to it.
        let dv = WireView::parse(&damaged).expect("{name}: a body flip must not break parsing");
        assert!(
            dv.verify_all().is_none(),
            "{name}: an artifact without planes reported an ECC result"
        );

        // And the damage is genuinely undetected: it decodes, and it decodes to
        // something DIFFERENT from the truth. If this ever fails because the
        // decode errors instead, the corruption is being caught by structural
        // validation and this control has stopped isolating the plane.
        assert_ne!(
            damaged, plain,
            "{name}: the corruption did not change the bytes"
        );
        match decode_aux_body(&damaged) {
            Ok(_) => { /* undetected, which is the point */ }
            Err(_) => panic!(
                "{name}: the unprotected artifact REJECTED the flip through some other check, so \
                 the corrected-bit tests above do not isolate the parity plane. Move the injection \
                 to a region where structural validation is not what notices."
            ),
        }
    }
}

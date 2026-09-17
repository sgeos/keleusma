//! **A BYTE QUOTIENT STORED IN A COMPOSITE FIELD, AND ITS NEIGHBOUR.**
//!
//! # Why this exists, and why it is about my own change
//!
//! `Op::Div` and `Op::Mod` now propagate `Width::Scalar(1)` for a matched `Byte`
//! pair. **The refusal that was removed existed to prevent a silent wrong answer
//! in composite PACKING**, and the evidence for the change was 1500 generated
//! scalar trees — which never pack anything, because a returned value is not a
//! value stored at an offset.
//!
//! So the riskiest consequence of that change was exactly the one nothing tested.
//! This file tests it.
//!
//! # The neighbour is the detector
//!
//! A single-field struct returns the right answer even under a wrong width,
//! because nothing follows the field. **A wrong width shifts every SUBSEQUENT
//! field's offset**, so the symptom appears in a neighbour rather than in the
//! field itself. Every subject here therefore places the byte-derived field
//! beside at least one other field and reads both.
//!
//! # Scope
//!
//! `Byte` fields fed by byte arithmetic, including the newly reachable division
//! and modulo. **Not** a general claim about composite layout, which
//! `flat_value`, the corpus differential and the typed verifier cover from other
//! directions.

mod common;

/// `(label, source)`. Each reads a byte-derived field AND a neighbour, so a
/// shifted offset cannot hide.
const SUBJECTS: &[(&str, &str)] = &[
    (
        "byte quotient beside a word",
        "struct P { q: Byte, w: Word }\n\
         fn main(a: Word, b: Word) -> Word { \
           let p: P = P { q: ((a as Byte) / (3 as Byte)), w: b }; \
           ((p.q as Word) * 1000) + p.w }",
    ),
    (
        "byte remainder beside a word",
        "struct P { r: Byte, w: Word }\n\
         fn main(a: Word, b: Word) -> Word { \
           let p: P = P { r: ((a as Byte) % (3 as Byte)), w: b }; \
           ((p.r as Word) * 1000) + p.w }",
    ),
    (
        "a byte quotient between two neighbours",
        "struct P { lo: Word, q: Byte, hi: Word }\n\
         fn main(a: Word, b: Word) -> Word { \
           let p: P = P { lo: a, q: ((b as Byte) / (2 as Byte)), hi: b }; \
           (p.lo * 100) + ((p.q as Word) * 10) + p.hi }",
    ),
    (
        "a composed byte expression, quotient feeding a subtract",
        "struct P { v: Byte, w: Word }\n\
         fn main(a: Word, b: Word) -> Word { \
           let p: P = P { v: (((a as Byte) / (2 as Byte)) - (b as Byte)), w: a }; \
           ((p.v as Word) * 1000) + p.w }",
    ),
    (
        "two byte fields adjacent",
        "struct P { x: Byte, y: Byte, w: Word }\n\
         fn main(a: Word, b: Word) -> Word { \
           let p: P = P { x: ((a as Byte) / (2 as Byte)), y: ((b as Byte) % (3 as Byte)), w: a }; \
           ((p.x as Word) * 10000) + ((p.y as Word) * 100) + p.w }",
    ),
];

/// **The packing, compared against the reference.**
#[test]
fn byte_derived_fields_pack_where_the_reference_packs_them() {
    for (label, src) in SUBJECTS {
        let refusals = keleusma_native::module_refusals(
            &common::build(src),
            keleusma_native::LowerOptions::default(),
        );
        assert!(
            refusals.is_empty(),
            "`{label}` no longer lowers: {refusals:?}. If the byte width \
             propagation was reverted, this file's subject is gone and it should \
             be retired with it rather than left failing."
        );
        let (vm, native) = common::vm_and_native_two_arg(src, 9, 5);
        assert_eq!(
            vm, native,
            "`{label}` MISPACKS: the reference gives {vm}, native gives {native}.\n\
             \n\
             This is the failure the removed refusal existed to prevent. **The \
             correct response is to revert the byte width propagation in \
             `Op::Div`/`Op::Mod` and restore the refusal**, not to patch the \
             packing: refusing was the conservative position, and this line moved \
             off it on the strength of a scalar differential that never packed \
             anything."
        );
    }
}

/// **Non-vacuity: the neighbour must actually be read**, or a shifted offset
/// would not show. Asserted on the source rather than trusted.
#[test]
fn every_subject_reads_more_than_the_byte_field() {
    for (label, src) in SUBJECTS {
        let reads = src.matches("p.").count();
        assert!(
            reads >= 2,
            "`{label}` reads {reads} field(s). A single-field read returns the \
             right answer under a wrong width, because nothing follows it; the \
             neighbour is the detector and this subject has none."
        );
    }
}

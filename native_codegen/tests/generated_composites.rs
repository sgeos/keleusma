//! **GENERATED FLAT COMPOSITES, BUILT FROM COMPUTED VALUES AND READ BACK.**
//!
//! # Why this surface
//!
//! **Layout is where this backend has actually been wrong.** The only genuine
//! codegen defect this line has found was composite-return aliasing, and three of
//! the ten recorded defects were layout faults: a slot storing a body ADDRESS, a
//! slot read before write answering 0, a declared initializer never applied.
//!
//! `byte_field_packing.rs` covers the shape with **five hand-written subjects** —
//! chosen by one person, covering the field counts and orders they thought of.
//! That is the limitation the scalar matrices had, one level up.
//!
//! # Why every field is read, with distinct multipliers
//!
//! A wrong packed width shifts every SUBSEQUENT field, so **the defect shows in a
//! neighbour rather than in the field itself**. An unread field cannot report a
//! shifted offset, and summing fields hides two of them exchanging values.
//! Distinct powers of ten make position observable in the result.
//!
//! # ⚠ SCOPE — WHAT IS NOT GENERATED
//!
//! Flat structs of scalars, built and read in one function. **Nested composites,
//! arrays, composite returns across calls, and composites crossing a yield are
//! all out of scope**, and each is covered — where it is covered — by a different
//! file. A green run here says nothing about them.

mod common;

use std::collections::BTreeSet;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// A `Byte`-valued expression. Bounds as in `generated_expressions.rs`: leaves at
/// most 3 and depth at most 2, so a checked byte multiply cannot overflow and
/// kill the binary with `SIGTRAP`.
fn byte_value(rng: &mut Rng, depth: u32) -> String {
    if depth == 0 {
        return match rng.below(4) {
            0 => "(a as Byte)".into(),
            1 => "(b as Byte)".into(),
            n => format!("({} as Byte)", n - 1),
        };
    }
    match rng.below(6) {
        0 => format!("({} / (2 as Byte))", byte_value(rng, depth - 1)),
        1 => format!("({} % (3 as Byte))", byte_value(rng, depth - 1)),
        2 => format!(
            "({} + {})",
            byte_value(rng, depth - 1),
            byte_value(rng, depth - 1)
        ),
        3 => format!(
            "({} band {})",
            byte_value(rng, depth - 1),
            byte_value(rng, depth - 1)
        ),
        4 => format!(
            "({} bxor {})",
            byte_value(rng, depth - 1),
            byte_value(rng, depth - 1)
        ),
        _ => format!(
            "({} - {})",
            byte_value(rng, depth - 1),
            byte_value(rng, depth - 1)
        ),
    }
}

/// A `Word`-valued expression, bounded so checked arithmetic cannot overflow.
fn word_value(rng: &mut Rng, depth: u32) -> String {
    if depth == 0 {
        return match rng.below(4) {
            0 => "a".into(),
            1 => "b".into(),
            n => format!("{}", n),
        };
    }
    match rng.below(4) {
        0 => format!("({} / 3)", word_value(rng, depth - 1)),
        1 => format!("({} % 7)", word_value(rng, depth - 1)),
        2 => format!(
            "({} + {})",
            word_value(rng, depth - 1),
            word_value(rng, depth - 1)
        ),
        _ => format!(
            "({} bxor {})",
            word_value(rng, depth - 1),
            word_value(rng, depth - 1)
        ),
    }
}

/// Build a struct of `n` fields with mixed types, and a body reading every field
/// with a distinct multiplier.
fn composite_program(rng: &mut Rng, fields: usize) -> String {
    let mut decl = String::new();
    let mut init = String::new();
    let mut read = String::new();
    for i in 0..fields {
        let is_byte = rng.below(2) == 0;
        let (ty, val) = if is_byte {
            ("Byte", byte_value(rng, 2))
        } else {
            ("Word", word_value(rng, 2))
        };
        decl.push_str(&format!("{}f{i}: {ty}", if i > 0 { ", " } else { "" }));
        init.push_str(&format!("{}f{i}: {val}", if i > 0 { ", " } else { "" }));
        // A distinct multiplier per position, so two fields exchanging values
        // changes the result. Kept small so the sum cannot overflow.
        let mult = 10u64.pow(i as u32);
        let field = if is_byte {
            format!("(p.f{i} as Word)")
        } else {
            format!("(p.f{i} % 9)")
        };
        read.push_str(&format!(
            "{}({field} * {mult})",
            if i > 0 { " + " } else { "" }
        ));
    }
    format!(
        "struct P {{ {decl} }}\nfn main(a: Word, b: Word) -> Word {{ let p: P = P {{ {init} }}; {read} }}"
    )
}

const SEEDS: &[u64] = &[0xC0FF_EE00_1234_5678, 0x0BAD_F00D_DEAD_1111];
const PER_SEED: usize = 120;
/// Field counts swept, so offsets vary rather than being fixed.
const FIELD_COUNTS: &[usize] = &[2, 3, 4];

/// **The comparison.**
#[test]
fn generated_composites_agree_with_the_reference() {
    let mut compared = 0usize;
    let mut seen: BTreeSet<String> = BTreeSet::new();

    for seed in SEEDS {
        let mut rng = Rng(*seed);
        for i in 0..PER_SEED {
            let fields = FIELD_COUNTS[i % FIELD_COUNTS.len()];
            let src = composite_program(&mut rng, fields);
            seen.insert(src.clone());

            let refusals = keleusma_native::module_refusals(
                &common::build(&src),
                keleusma_native::LowerOptions::default(),
            );
            assert!(
                refusals.is_empty(),
                "generated composite {i} is REFUSED:\n  {src}\n  {refusals:?}\n\n\
                 A refusal is not a wrong answer, but a shape this generator emits \
                 and the backend declines is a capability gap worth naming."
            );

            let (vm, native) = common::vm_and_native_two_arg(&src, 9, 5);
            compared += 1;
            assert_eq!(
                vm, native,
                "generated composite {i} MISPACKS:\n  {src}\n  reference = {vm}\n  \
                 native    = {native}\n\nEvery field is read with a distinct \
                 multiplier, so a wrong packed width shows as a shifted neighbour. \
                 This establishes that the two implementations disagree, NOT which \
                 is right."
            );
        }
    }

    assert_eq!(
        compared,
        PER_SEED * SEEDS.len(),
        "only {compared} composites compared"
    );
    assert!(
        seen.len() >= PER_SEED,
        "the generator produced only {} distinct composites; it has collapsed and \
         is re-testing the five hand-written subjects",
        seen.len()
    );
}

/// **Field counts and both types must actually appear**, or the sweep is
/// narrower than it claims and offsets never move.
#[test]
fn the_generated_composites_vary_in_shape() {
    let mut rng = Rng(SEEDS[0]);
    let (mut byte_fields, mut word_fields) = (0usize, 0usize);
    let mut counts: BTreeSet<usize> = BTreeSet::new();
    for i in 0..PER_SEED {
        let fields = FIELD_COUNTS[i % FIELD_COUNTS.len()];
        counts.insert(fields);
        let src = composite_program(&mut rng, fields);
        byte_fields += src.matches(": Byte").count();
        word_fields += src.matches(": Word,").count() + src.matches(": Word }").count();
    }
    assert_eq!(
        counts.len(),
        FIELD_COUNTS.len(),
        "only {counts:?} field counts appeared; offsets are not moving"
    );
    assert!(
        byte_fields > 0 && word_fields > 0,
        "the sweep produced {byte_fields} byte field(s) and {word_fields} word \
         field(s). A uniform struct hides offset errors that cancel, which is the \
         whole reason the types are mixed."
    );
}

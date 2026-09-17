//! **NESTED COMPOSITES, WHERE THIS FAMILY'S MISTAKE IS A WRONG ANSWER.**
//!
//! # Why this surface differs from every other one repaired on 2026-09-17
//!
//! The four width gaps found that day were all **refusals**: the backend declined
//! to lower rather than lowering wrongly. Nested composites are where that stops
//! being true. From the emitter's own `Width` documentation:
//!
//! > *An eight-byte nested composite body and a `Word` are both eight bytes, and a
//! > single-field struct wrapping a word is exactly that shape — 80 of the
//! > corpus's constructions are `(8, 1)`. Storing one as a scalar would write the
//! > POINTER into the parent body while every downstream field offset still looked
//! > correct, which is **a silent wrong answer rather than a fault**.*
//!
//! And one of the ten recorded defects on this line was exactly that: a composite
//! data slot stored as the body's ADDRESS.
//!
//! # The neighbour is still the detector
//!
//! If an outer struct held nothing but the nested body, an address written in its
//! place could still read back correctly through the same wrong indirection. **A
//! scalar after the nested field is what makes a wrong size observable.**
//!
//! # ⚠ AND THESE SUBJECTS DO NOT CATCH THAT FAILURE. MEASURED, NOT ASSUMED.
//!
//! The brief for this file demanded the decisive perturbation: make a nested
//! body's width be a `Scalar` of the same size, and see whether the comparison
//! notices. **It does not.** All three sites that produce a `Width::Body` were
//! turned into `Width::Scalar` in turn, and **every test in this file stayed
//! green each time**, as did the composite-field and array-element suites.
//!
//! **What DOES catch it is `composite_return_aliasing.rs`** — three of its tests
//! fail on that perturbation. That file pins the one genuine codegen defect this
//! line has ever found, and it earns its keep here.
//!
//! **So the `Body`/`Scalar` guarantee does not rest on this file**, and saying so
//! is the point of this note. The distinction matters where a composite body is
//! passed or returned ACROSS A CALL, not where one is built and read inside a
//! single function — which is all these subjects do.
//!
//! # What this file is actually worth
//!
//! It exercises the widths repaired on 2026-09-17 **one level down**, inside a
//! nested body, across 180 generated shapes. That is real and was untested. It is
//! not evidence about body-versus-scalar confusion, and a reader looking for that
//! guarantee should read `composite_return_aliasing.rs` instead.
//!
//! # ⚠ SCOPE
//!
//! Structs containing structs, built and read in one function. Composite returns,
//! composites crossing a yield, and arrays of composites are all out of scope.

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

/// A bounded `Word` expression, so the arithmetic inside a nested body exercises
/// the widths repaired today without overflowing a checked operation.
fn value(rng: &mut Rng, depth: u32) -> String {
    if depth == 0 {
        return match rng.below(4) {
            0 => "a".into(),
            1 => "b".into(),
            n => format!("{n}"),
        };
    }
    match rng.below(6) {
        0 => format!("({} / 3)", value(rng, depth - 1)),
        1 => format!("({} % 7)", value(rng, depth - 1)),
        2 => format!("({} band 15)", value(rng, depth - 1)),
        3 => format!("({} lsr 1)", value(rng, depth - 1)),
        4 => format!("({} + {})", value(rng, depth - 1), value(rng, depth - 1)),
        _ => format!("({} bxor {})", value(rng, depth - 1), value(rng, depth - 1)),
    }
}

/// `(label, source)` for a generated nesting.
///
/// **`Inner` is deliberately single-field**, so its body is eight bytes — the
/// same size as the `Word` it could be mistaken for, which is the shape the
/// documentation names as ambiguous.
fn nesting_program(rng: &mut Rng, shape: usize) -> String {
    let v1 = value(rng, 2);
    let v2 = value(rng, 2);
    let v3 = value(rng, 2);
    match shape % 3 {
        // One level, with a scalar AFTER the nested body.
        0 => format!(
            "struct I {{ v: Word }}\n\
             struct O {{ i: I, w: Word }}\n\
             fn main(a: Word, b: Word) -> Word {{ \
               let o: O = O {{ i: I {{ v: {v1} }}, w: {v2} }}; \
               ((o.i.v % 9) * 100) + (o.w % 9) }}"
        ),
        // A scalar BEFORE and after, so a shift in either direction shows.
        1 => format!(
            "struct I {{ v: Word }}\n\
             struct O {{ lo: Word, i: I, hi: Word }}\n\
             fn main(a: Word, b: Word) -> Word {{ \
               let o: O = O {{ lo: {v1}, i: I {{ v: {v2} }}, hi: {v3} }}; \
               ((o.lo % 9) * 10000) + ((o.i.v % 9) * 100) + (o.hi % 9) }}"
        ),
        // Two levels of nesting, each single-field, with a trailing scalar.
        _ => format!(
            "struct A {{ v: Word }}\n\
             struct B {{ a: A }}\n\
             struct C {{ b: B, w: Word }}\n\
             fn main(a: Word, b: Word) -> Word {{ \
               let c: C = C {{ b: B {{ a: A {{ v: {v1} }} }}, w: {v2} }}; \
               ((c.b.a.v % 9) * 100) + (c.w % 9) }}"
        ),
    }
}

const SEEDS: &[u64] = &[0x1357_9BDF_2468_ACE0, 0xFEDC_BA98_7654_3210];
const PER_SEED: usize = 90;

#[test]
fn generated_nestings_agree_with_the_reference() {
    let mut compared = 0usize;
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut refused: Vec<(usize, String, String)> = Vec::new();

    for seed in SEEDS {
        let mut rng = Rng(*seed);
        for i in 0..PER_SEED {
            let src = nesting_program(&mut rng, i);
            seen.insert(src.clone());

            let r = keleusma_native::module_refusals(
                &common::build(&src),
                keleusma_native::LowerOptions::default(),
            );
            if !r.is_empty() {
                refused.push((i, src.clone(), format!("{:?}", r[0].1)));
                continue;
            }

            let (vm, native) = common::vm_and_native_two_arg(&src, 9, 5);
            compared += 1;
            assert_eq!(
                vm, native,
                "generated nesting {i} DIVERGES:\n  {src}\n  reference = {vm}\n  \
                 native    = {native}\n\n**This is the surface where a body stored \
                 as a scalar is a WRONG ANSWER rather than a refusal**, and a \
                 single-field struct wrapping a word is the ambiguous shape. \
                 This establishes that the two implementations disagree, NOT which \
                 is right."
            );
        }
    }

    assert!(
        refused.is_empty(),
        "{} generated nesting(s) were REFUSED, first: {:?}.\n\n**Read the message \
         before concluding.** A refusal here may be the conservative design \
         working rather than a gap — and repairing a refusal into a mispack on \
         this surface is exactly the silent wrong answer the `Body`/`Scalar` split \
         exists to prevent.",
        refused.len(),
        refused.first()
    );
    assert_eq!(
        compared,
        PER_SEED * SEEDS.len(),
        "only {compared} nestings compared"
    );
    assert!(
        seen.len() >= PER_SEED,
        "the generator produced only {} distinct nestings; it has collapsed",
        seen.len()
    );
}

/// **All three shapes must appear**, including the two-level one, or the
/// "nested" claim is carried by one-level subjects alone.
#[test]
fn all_three_nesting_shapes_are_generated() {
    let mut rng = Rng(SEEDS[0]);
    let (mut one, mut two, mut sandwich) = (0usize, 0usize, 0usize);
    for i in 0..PER_SEED {
        let src = nesting_program(&mut rng, i);
        if src.contains("struct C") {
            two += 1;
        } else if src.contains("lo: Word") {
            sandwich += 1;
        } else {
            one += 1;
        }
    }
    assert!(
        one > 0 && two > 0 && sandwich > 0,
        "shapes generated: one-level {one}, sandwich {sandwich}, two-level {two}. \
         A missing shape means the nesting claim rests on fewer forms than it says."
    );
}

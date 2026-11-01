//! **BOOLEANS AND FIXED-POINT VALUES INSIDE COMPOSITE BODIES.**
//!
//! # Why these were refused, and why the refusal was in an unexpected place
//!
//! `let xs = [true, false]` compiled and was then refused at `NewComposite` for
//! *an operand of unknown packed width*. The gap was not in composite
//! construction at all: it was in **two producers**. `Op::PushImmediate` and
//! `Op::WordToFixed` pushed their values with no width, while `Op::Const` —
//! which is what an integer literal uses — already carried one. That is why
//! every integer composite in the corpus worked and nothing noticed.
//!
//! See `docs/decisions/OPERAND_WIDTH_GAP_BRIEF.md`.
//!
//! # The neighbour is the case that matters
//!
//! A one-byte value written eight bytes wide overwrites whatever follows it, and
//! **no test that reads only the narrow field itself would see that**. So the
//! struct case here reads the NEIGHBOUR back.
//!
//! # Related
//!
//! The `Byte` composite witnesses live in `float_composite.rs` beside the float
//! cases, because they were found by the same census and share its harness.

mod common;

use common::vm_and_native_two_arg as both;

/// A boolean field beside a word, with the WORD read back.
#[test]
fn a_bool_struct_field_does_not_disturb_its_neighbour() {
    // `b` is one byte at offset 0 and `n` is eight bytes after it. A boolean
    // stored eight bytes wide would overwrite `n` entirely, which is invisible
    // to any test that returns the boolean.
    let src = "struct P { b: bool, n: Word }
         fn main(a: Word, b: Word) -> Word { let p = P { b: true, n: b }; p.n }";
    for (a, b) in [(0, 7), (0, -1), (0, i64::MIN), (0, i64::MAX)] {
        let (vm, nat) = both(src, a, b);
        assert_eq!(vm, nat, "bool field disturbed its neighbour for ({a}, {b})");
    }
}

/// The boolean itself, on both paths, so a value that is always true or always
/// false cannot pass by accident.
#[test]
fn a_bool_struct_field_reads_back_on_both_paths() {
    let src = "struct P { b: bool, n: Word }
         fn main(a: Word, b: Word) -> Word {
             let p = P { b: a > 0, n: b };
             if p.b { p.n } else { 0 - p.n }
         }";
    for (a, b) in [(1, 7), (-1, 7), (0, 5), (1, -3)] {
        let (vm, nat) = both(src, a, b);
        assert_eq!(vm, nat, "bool field read disagrees for ({a}, {b})");
    }
}

/// A boolean array, which reaches the element read and therefore a STRIDE. A
/// one-byte stride read as eight would index outside the body.
#[test]
fn a_bool_array_element_agrees_with_the_vm() {
    let src = "fn main(a: Word, b: Word) -> Word {
             let xs = [a > 0, a > 5, a > 10];
             if xs[1] { b } else { 0 - b }
         }";
    for (a, b) in [(0, 3), (1, 3), (7, 3), (20, 3), (-4, 9)] {
        let (vm, nat) = both(src, a, b);
        assert_eq!(vm, nat, "bool array element disagrees for ({a}, {b})");
    }
}

/// A fixed-point tuple member. **The value discriminates**: `a as Fixed<16>`
/// shifts left by sixteen, so the stored bit pattern is not the integer reading,
/// and a lowering that confused the two would return a number 65536 times wrong.
#[test]
fn a_fixed_tuple_member_agrees_with_the_vm() {
    let src = "fn main(a: Word, b: Word) -> Word {
             let t = (a as Fixed<16>, b); ((t.0) as Word) + t.1
         }";
    for (a, b) in [(3, 5), (-3, 5), (0, 0), (1, -1), (12345, 7)] {
        let (vm, nat) = both(src, a, b);
        assert_eq!(vm, nat, "fixed tuple member disagrees for ({a}, {b})");
    }
}

/// A fixed-point array, reading a LATER element so the stride is exercised.
#[test]
fn a_fixed_array_element_agrees_with_the_vm() {
    let src = "fn main(a: Word, b: Word) -> Word {
             let xs = [a as Fixed<16>, b as Fixed<16>]; ((xs[1]) as Word) + a
         }";
    for (a, b) in [(3, 5), (-3, 5), (0, 0), (7, -9), (12345, 6789)] {
        let (vm, nat) = both(src, a, b);
        assert_eq!(vm, nat, "fixed array element disagrees for ({a}, {b})");
    }
}

/// **THE UNIT IMMEDIATE STAYS WIDTHLESS, AND THAT IS A DECISION.**
///
/// `PushImmediate(0)` pushes a placeholder zero for a value whose flat width is
/// ZERO and which nothing consumes. Giving it a scalar width alongside its
/// neighbours in the same match arm would invent a representation. This test
/// pins the consequence rather than the implementation: a composite cannot be
/// built out of unit values, and the refusal is loud.
#[test]
fn a_unit_valued_composite_is_refused_rather_than_given_a_width() {
    use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

    let src = "fn main(a: Word, b: Word) -> Word { let t = ((), b); t.1 }";
    match tokenize(src)
        .ok()
        .and_then(|t| parse(&t).ok())
        .and_then(|x| compile(&x).ok())
    {
        None => {
            // The reference compiler declines the shape, so the backend never
            // sees it. Recorded rather than treated as a pass for the backend.
            println!("  the reference compiler refuses a unit tuple member; the");
            println!("  backend's width decision is unreachable from this source");
        }
        Some(m) => {
            let r = keleusma_native::module_refusals(&m, keleusma_native::LowerOptions::default());
            assert!(
                !r.is_empty(),
                "a composite carrying a unit value LOWERS, which means the unit \
                 immediate acquired a packed width. Its flat width is zero and \
                 the pushed value is a placeholder -- if this is now deliberate, \
                 the reasoning in the PushImmediate arm must be rewritten rather \
                 than this test deleted"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The last reachable residue of the kind-arm census. Each of these arms exists,
// is reachable from ordinary source, and was exercised by nothing.
// `probe_float_composite::which_of_the_last_residue_is_reachable` measured which
// ones lower before any of them was written.
// ---------------------------------------------------------------------------

/// A boolean TUPLE member, reaching the arm through a different operand shape
/// than the struct case.
#[test]
fn a_bool_tuple_member_agrees_with_the_vm() {
    let src =
        "fn main(a: Word, b: Word) -> Word { let t = (a > 0, b); if t.0 { t.1 } else { 0 - t.1 } }";
    for (a, b) in [(1, 7), (-1, 7), (0, 5), (3, -3)] {
        let (vm, nat) = both(src, a, b);
        assert_eq!(vm, nat, "bool tuple member disagrees for ({a}, {b})");
    }
}

/// A boolean ENUM PAYLOAD, whose offset is measured PAST the discriminant word,
/// so a mistake here is an offset error rather than a width error.
#[test]
fn a_bool_enum_payload_agrees_with_the_vm() {
    let src = "enum E { A(bool), B }
         fn main(a: Word, b: Word) -> Word {
             let e = E::A(a > 0);
             match e { E::A(x) => if x { b } else { 0 - b }, E::B => 0 }
         }";
    for (a, b) in [(1, 7), (-1, 7), (0, 5), (9, -2)] {
        let (vm, nat) = both(src, a, b);
        assert_eq!(vm, nat, "bool enum payload disagrees for ({a}, {b})");
    }
}

/// A fixed-point ENUM PAYLOAD. The value differs from its integer reading by a
/// factor of 65536, so a lowering confusing the two returns a wildly wrong
/// number rather than a plausible one.
#[test]
fn a_fixed_enum_payload_agrees_with_the_vm() {
    let src = "enum E { A(Fixed<16>), B }
         fn main(a: Word, b: Word) -> Word {
             let e = E::A(a as Fixed<16>);
             match e { E::A(x) => (x as Word) + b, E::B => 0 }
         }";
    for (a, b) in [(3, 5), (-3, 5), (0, 0), (12345, 7)] {
        let (vm, nat) = both(src, a, b);
        assert_eq!(vm, nat, "fixed enum payload disagrees for ({a}, {b})");
    }
}

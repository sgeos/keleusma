//! **DOES A FLOAT INSIDE A COMPOSITE COMPUTE THE SAME NUMBER AS THE REFERENCE?**
//!
//! # Why this needed no ruling
//!
//! A composite body field is INTERNAL. The compiler packs it, the same program
//! reads it back, and nothing outside the running program observes the layout,
//! so the backend agreeing with the reference is a **fact to be measured**
//! rather than an application binary interface to be chosen. That is the same
//! ground on which `Fixed` is lowered in a body and refused in a shared slot,
//! and it is why this file exists while `Fixed`, `Text`, `Opaque` and `Unit`
//! shared slots stay refused pending the operator.
//!
//! # Why the values are the ones they are
//!
//! A mispacked body returns a plausible number, so acceptance proves nothing.
//! Every case below is driven from RUNTIME arguments — nothing is
//! constant-folded before it reaches the target's instructions — and the values
//! are chosen to discriminate: both infinities, a negative zero and a NaN, whose
//! bit patterns a lowering that rounded, truncated or reinterpreted would not
//! reproduce.
//!
//! # What is NOT covered, stated so a green file is not read as more than it is
//!
//! A `Float` of any width other than eight bytes, which is refused; a float
//! inside a NESTED composite body, which goes through the `FlatNested` arms and
//! is untouched here; and a float in a composite that reaches a data slot.

mod common;

use common::vm_and_native_two_arg as both;

/// A float field packed into a struct beside a word, and read back.
#[test]
fn a_float_struct_field_agrees_with_the_vm() {
    // The word field is returned in the FIRST case so a mispacked float that
    // overran its slot would corrupt an answer the test actually reads.
    let neighbour = "struct P { x: Float, n: Word }
         fn main(a: Word, b: Word) -> Word {
             let p = P { x: (a as Float) / (b as Float), n: b }; p.n
         }";
    let read_back = "struct P { x: Float, n: Word }
         fn main(a: Word, b: Word) -> Word {
             let p = P { x: (a as Float) / (b as Float), n: b }; (p.x) as Word
         }";
    for src in [neighbour, read_back] {
        for (a, b) in [
            (7, 2),
            (-7, 2),
            (1, 0),  // +inf
            (-1, 0), // -inf
            (0, 0),  // NaN
            (0, -1), // negative zero
            (i64::MAX, 3),
        ] {
            let (vm, nat) = both(src, a, b);
            assert_eq!(vm, nat, "float struct field disagrees for ({a}, {b})");
        }
    }
}

/// **THE TAG, WHICH LAYOUT ALONE DOES NOT ESTABLISH.** A field read that packs
/// and loads correctly but pushes an untagged operand produces a module that
/// LOWERS and then refuses at the first float operation — the miss made once by
/// the entry ABI and once by the shared slot. Using the value in float
/// arithmetic rather than only returning it is what distinguishes the two.
#[test]
fn a_float_read_out_of_a_struct_is_usable_in_float_arithmetic() {
    let src = "struct P { x: Float, n: Word }
         fn main(a: Word, b: Word) -> Word {
             let p = P { x: a as Float, n: b };
             (p.x * (b as Float) + p.x) as Word
         }";
    for (a, b) in [(3, 4), (-3, 4), (0, 0), (7, -2), (i64::MAX, 1)] {
        let (vm, nat) = both(src, a, b);
        assert_eq!(
            vm, nat,
            "float arithmetic on a field read disagrees for ({a}, {b})"
        );
    }
}

/// A tuple, whose body is the same flat packing reached by a different opcode
/// shape.
#[test]
fn a_float_in_a_tuple_agrees_with_the_vm() {
    let src = "fn main(a: Word, b: Word) -> Word {
             let t = ((a as Float) / (b as Float), b); (t.0) as Word + t.1
         }";
    for (a, b) in [(7, 2), (1, 0), (0, 0), (-9, 4)] {
        let (vm, nat) = both(src, a, b);
        assert_eq!(vm, nat, "float tuple disagrees for ({a}, {b})");
    }
}

/// An array of floats, which reaches `GetIndex` and therefore a STRIDE. An
/// element width of anything but eight would read the wrong element, so the
/// second element is the one returned.
#[test]
fn an_array_of_floats_indexes_at_the_right_stride() {
    let src = "fn main(a: Word, b: Word) -> Word {
             let xs = [a as Float, b as Float, (a as Float) / (b as Float)];
             (xs[1] + xs[2]) as Word
         }";
    for (a, b) in [(7, 2), (1, 0), (0, 0), (-5, 3), (i64::MIN, 1)] {
        let (vm, nat) = both(src, a, b);
        assert_eq!(vm, nat, "float array disagrees for ({a}, {b})");
    }
}

/// **THE MUST-FIRE WIDTH REFUSAL.** This build's `Float` is eight bytes, so the
/// refusal is unreachable by compiling a program; the module's declared width is
/// overwritten after compilation, which is the technique the signature and data
/// slot routes both use for the same reason — an unreachable claim is not a
/// guard.
#[test]
fn a_float_field_at_any_other_width_is_refused() {
    use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

    let src = "struct P { x: Float, n: Word }
         fn main(a: Word, b: Word) -> Word {
             let p = P { x: a as Float, n: b }; (p.x) as Word
         }";
    let mut m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    assert_eq!(
        1u32 << m.float_bits_log2 >> 3,
        8,
        "this build's Float is not 8 bytes, so both halves of this test describe \
         a different build than the one running it"
    );
    assert!(
        keleusma_native::module_refusals(&m, keleusma_native::LowerOptions::default()).is_empty(),
        "an 8-byte float field refuses again; either this lowering was removed \
         or another guard now fires first"
    );

    m.float_bits_log2 = 5;
    let refusals = keleusma_native::module_refusals(&m, keleusma_native::LowerOptions::default());
    assert!(
        !refusals.is_empty(),
        "a 4-byte Float field must refuse: the reference sizes the body by the \
         float width, so lowering it at eight would mispack silently"
    );
    let why = format!("{}", refusals[0].1);
    assert!(
        why.to_lowercase().contains("float"),
        "refused, but not in a way naming the float: {why}"
    );
}

#![cfg(feature = "compile")]
//! Targets declaring a width NARROWER than anything the runtime implements.
//!
//! # Why this file exists
//!
//! [`Target::validate_against_runtime`] checked only that the word, address and
//! float widths did not EXCEED the runtime's. Nothing checked the other end, so
//! a target could declare a width that is not a format at all and compile
//! without complaint.
//!
//! **The floor argument was already written down in the tree, and applied to
//! one of the three widths.** `validate_program_for_target` refuses
//! `float_bits_log2` below 5 with the reasoning that such widths "are not
//! formats, and the runtime's implemented-width predicate does not admit them,
//! so a target declaring one produces bytecode nothing will run". That argument
//! is not about floats. It holds verbatim for the word and the address, which
//! sit beside `float_bits_log2` in the same struct and are validated in the
//! same call.
//!
//! # It was found by the derivation that produced one
//!
//! `tests/composite_width_skew.rs` derives a skewed target by taking the
//! build's word width and clamping one step below it. Its floor was 2. Under
//! `narrow-word-8` the word is 3, so the clamp produced `addr_bits_log2 = 2` --
//! a FOUR-BIT address. The layout sizes an opaque by the address width, four
//! bits is zero bytes, and the failure surfaced at RUN time as
//! `InvalidBytecode("NewComposite flat operand on non-flat values")`, naming
//! neither the width nor the target that caused it.
//!
//! Both the derivation and the missing check are repaired. This file pins the
//! check; the premise guard in the skew corpus pins the derivation.

extern crate alloc;

use keleusma::compiler::compile_with_target;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::target::Target;

/// The narrowest widths any runtime implements, taken from the trait impls
/// rather than written as literals. A test asserting a floor must not carry its
/// own copy of the number the floor is supposed to track.
const WORD_FLOOR: u8 = <i8 as keleusma::word::Word>::BITS_LOG2;
const ADDR_FLOOR: u8 = <u8 as keleusma::address::Address>::BITS_LOG2;

const TRIVIAL: &str = "fn main() -> Word { 1 }";

fn compile_at(target: &Target) -> Result<(), alloc::string::String> {
    let program = parse(&tokenize(TRIVIAL).expect("lex")).expect("parse");
    compile_with_target(&program, target)
        .map(|_| ())
        .map_err(|e| e.message)
}

/// An eight-bit word paired with an eight-bit address: the narrowest target
/// that is entirely made of implemented widths. It must be ACCEPTED, or the
/// floor is refusing something real rather than something absent.
#[test]
fn the_narrowest_implemented_widths_are_accepted() {
    let t = Target {
        word_bits_log2: WORD_FLOOR,
        addr_bits_log2: ADDR_FLOOR,
        has_floats: false,
        float_bits_log2: 0,
        ..Target::embedded_8()
    };
    assert_eq!(
        compile_at(&t),
        Ok(()),
        "the floor must admit the width it is set to; refusing this would make the check a \
         ceiling in disguise"
    );
}

#[test]
fn an_address_below_the_floor_is_refused_at_compile_time() {
    let t = Target {
        addr_bits_log2: ADDR_FLOOR - 1,
        ..Target::embedded_8()
    };
    let err = compile_at(&t).expect_err(
        "a four-bit address is not a format; accepting it produces a module whose opaque scalar \
         is zero bytes wide",
    );
    assert!(
        err.contains("addr_bits_log2"),
        "the refusal must name the field that is wrong, because the symptom it replaces named \
         nothing: got {err}"
    );
}

#[test]
fn a_word_below_the_floor_is_refused_at_compile_time() {
    let t = Target {
        word_bits_log2: WORD_FLOOR - 1,
        ..Target::embedded_8()
    };
    let err = compile_at(&t).expect_err("a four-bit word is not a format any runtime implements");
    assert!(
        err.contains("word_bits_log2"),
        "the refusal must name the field that is wrong: got {err}"
    );
}

/// The MECHANISM, pinned separately from the check.
///
/// Without this property a sub-floor address would be merely unusual rather
/// than corrupting. It is what makes the refusal necessary: the layout sizes
/// [`keleusma::value_layout::ScalarKind::Opaque`] by the address width in
/// BYTES, and every width below the floor rounds to zero.
#[test]
fn every_width_below_the_floor_is_zero_bytes() {
    for below in 0..ADDR_FLOOR {
        let t = Target {
            addr_bits_log2: below,
            ..Target::embedded_8()
        };
        assert_eq!(
            t.address_bits() / 8,
            0,
            "addr_bits_log2 = {below} gives {} bits, which is not sub-byte and so does not \
             explain the zero-width opaque this check exists to prevent",
            t.address_bits()
        );
    }
}

/// The third width, checked here so the family cannot lose a member quietly.
///
/// The float floor is enforced in a DIFFERENT function, conditional on
/// `has_floats`. That split is deliberate, and it is also how the other two
/// floors came to be missing for as long as they were: a reader looking at the
/// width checks does not see the float one beside them.
#[cfg(feature = "floats")]
#[test]
fn the_float_floor_beside_them_still_holds() {
    let t = Target {
        has_floats: true,
        float_bits_log2: 4,
        ..Target::host()
    };
    let err = compile_at(&t).expect_err("binary16 needs software rounding that does not exist");
    assert!(err.contains("float_bits_log2"), "got {err}");
}

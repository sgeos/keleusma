#![cfg(all(feature = "compile", feature = "verify"))]
//! A composite bearing an opaque field, on runtimes whose WORD and ADDRESS
//! widths differ.
//!
//! # Why this file exists, and why it needs no feature selection
//!
//! `ScalarKind::Opaque` is sized by the ADDRESS width. The compiler bakes every
//! field offset from that layout, so a runtime that constructs or reads the
//! field at some OTHER width puts each following field where the baked access
//! does not look for it. Four runtime sites assumed a WORD; each now asks the
//! layout.
//!
//! **The bundled runtime makes the two widths equal, so it cannot show the
//! disagreement.** But `GenericVm<W, A, F>` is generic over the word and
//! address types independently, and every `Word` (`i8`, `i16`, `i32`, `i64`)
//! and every `Address` (`u8`, `u16`, `u32`, `u64`) is implemented
//! unconditionally. **A host-defined alias therefore reaches a skewed pair in
//! the DEFAULT build**, with no `narrow-*` feature, which is why these tests
//! run wherever the suite runs rather than in a configuration nothing builds.
//!
//! # A skewed pair is a shipped shape, not a contrivance
//!
//! `Target::embedded_8` declares an eight-bit word with a sixteen-bit address,
//! and the `addr_bits_log2` field's own documentation names the 6502 as the
//! machine it stands for. The first test below drives exactly that target.
//!
//! # Measured coverage: four sites, each caught, and one test that catches nothing
//!
//! Established by MUTATION, not by inspection. Each of the four sites that
//! assumed a word was reverted in turn and this file re-run.
//!
//! | reverted site | tests that caught it |
//! |---|---|
//! | the construction path collapsing the index to a one-word `Int` | 3 |
//! | the arena packer advancing by a word | 5 |
//! | the flat scalar read taking a word | **1** |
//! | the host decode asking for a word | **1** |
//!
//! **Two sites are held by a single test each**, which is thin, and the two
//! are named above so a future edit knows what it is removing. The read side
//! is observable only through `the_opaque_itself_survives_a_skewed_composite`,
//! because with a handful of opaques the registry index fits in one byte and
//! every other test reads the same number at either width.
//!
//! **`a_6502_class_runtime_compares_a_differing_word_field_unequal` was caught
//! by NO mutation.** At that skew the opaque field's own read absorbs the
//! displacement -- each side reads a different wrong index, so the comparison
//! still reports "unequal" for the wrong reason. It is kept because it pins the
//! exact shape that was observed returning a wrong answer, but it is a witness
//! to the reported symptom rather than a discriminating guard, and it must not
//! be counted as covering anything.

extern crate alloc;

use alloc::string::String;
use keleusma::bytecode::GenericValue;
use keleusma::compiler::compile_with_target;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::target::Target;
use keleusma::vm::{GenericVm, GenericVmState};
use keleusma::{Arena, HostOpaque, KeleusmaType, host_arc};

struct Handle {
    /// Read back by `the_opaque_itself_survives_a_skewed_composite`, which is
    /// how that test tells the right handle from a resolvable wrong one.
    label: String,
}
impl HostOpaque for Handle {
    fn type_name(&self) -> &'static str {
        "Handle"
    }
}

/// 6502 class: an eight-bit word with a sixteen-bit address. The address is
/// WIDER than the word.
type SixFiveOhTwo<'a, 'arena> = GenericVm<'a, 'arena, i8, u16, f32>;

/// The opposite skew: a sixty-four-bit word with a sixteen-bit address. Not a
/// machine anyone builds, but it is the configuration in which the defect was
/// first measured, and the two skews exercise the disagreement from either
/// side.
type WideWordNarrowAddress<'a, 'arena> = GenericVm<'a, 'arena, i64, u16, f64>;

/// A sixty-four-bit word with a sixteen-bit address, floats included so the
/// descriptor is admissible on the default runtime.
fn wide_word_narrow_address() -> Target {
    Target {
        word_bits_log2: 6,
        addr_bits_log2: 4,
        ..Target::host()
    }
}

/// Reads the field that sits AFTER the opaque, which is the one a wrong opaque
/// width displaces.
const READ_AFTER: &str = "use make_handle\n\
                          struct P { h: Handle, n: Word }\n\
                          fn main() -> Word { let h = make_handle(); let p = P { h: h, n: 7 }; p.n }";

/// Two structures sharing an opaque and differing only in the word field. This
/// is the shape that returned a WRONG ANSWER rather than a fault: with the
/// field after the opaque read out of padding, both sides read the same zero
/// and the comparison reported equality.
const COMPARE: &str = "use make_handle\n\
                       struct P { h: Handle, n: Word }\n\
                       fn main() -> bool { let h = make_handle(); P { h: h, n: 1 } == P { h: h, n: 2 } }";

#[test]
fn a_6502_class_runtime_reads_the_field_after_an_opaque() {
    let program = parse(&tokenize(READ_AFTER).expect("lex")).expect("parse");
    let module = compile_with_target(&program, &Target::embedded_8()).expect("compile");
    let arena = Arena::with_capacity(4096);
    let mut vm: SixFiveOhTwo<'_, '_> = SixFiveOhTwo::new(module, &arena).expect("verify");
    vm.register_native("make_handle", |_args| {
        Ok(GenericValue::Opaque(host_arc(Handle {
            label: String::from("id"),
        })))
    });
    match vm.call(&[]).expect("call") {
        GenericVmState::Finished(GenericValue::Int(n)) => assert_eq!(
            n, 7_i8,
            "the word field after an opaque must read back what was stored; a 0 here means the \
             field was read out of the padding left by a short body"
        ),
        other => panic!("expected a word, got {:?}", other),
    }
}

#[test]
fn a_6502_class_runtime_compares_a_differing_word_field_unequal() {
    let program = parse(&tokenize(COMPARE).expect("lex")).expect("parse");
    let module = compile_with_target(&program, &Target::embedded_8()).expect("compile");
    let arena = Arena::with_capacity(4096);
    let mut vm: SixFiveOhTwo<'_, '_> = SixFiveOhTwo::new(module, &arena).expect("verify");
    vm.register_native("make_handle", |_args| {
        Ok(GenericValue::Opaque(host_arc(Handle {
            label: String::from("id"),
        })))
    });
    match vm.call(&[]).expect("call") {
        GenericVmState::Finished(GenericValue::Bool(b)) => assert!(
            !b,
            "two structures differing in a word field must not compare equal; a `true` here is a \
             WRONG ANSWER rather than a fault, which is why this case is pinned separately"
        ),
        other => panic!("expected a bool, got {:?}", other),
    }
}

#[test]
fn a_wide_word_narrow_address_runtime_reads_the_field_after_an_opaque() {
    let program = parse(&tokenize(READ_AFTER).expect("lex")).expect("parse");
    let module = compile_with_target(&program, &wide_word_narrow_address()).expect("compile");
    let arena = Arena::with_capacity(4096);
    let mut vm: WideWordNarrowAddress<'_, '_> =
        WideWordNarrowAddress::new(module, &arena).expect("verify");
    vm.register_native("make_handle", |_args| {
        Ok(GenericValue::Opaque(host_arc(Handle {
            label: String::from("id"),
        })))
    });
    match vm.call(&[]).expect("call") {
        GenericVmState::Finished(GenericValue::Int(n)) => assert_eq!(n, 7_i64),
        other => panic!("expected a word, got {:?}", other),
    }
}

#[test]
fn a_wide_word_narrow_address_runtime_compares_a_differing_word_field_unequal() {
    let program = parse(&tokenize(COMPARE).expect("lex")).expect("parse");
    let module = compile_with_target(&program, &wide_word_narrow_address()).expect("compile");
    let arena = Arena::with_capacity(4096);
    let mut vm: WideWordNarrowAddress<'_, '_> =
        WideWordNarrowAddress::new(module, &arena).expect("verify");
    vm.register_native("make_handle", |_args| {
        Ok(GenericValue::Opaque(host_arc(Handle {
            label: String::from("id"),
        })))
    });
    match vm.call(&[]).expect("call") {
        GenericVmState::Finished(GenericValue::Bool(b)) => assert!(!b),
        other => panic!("expected a bool, got {:?}", other),
    }
}

/// Reads the OPAQUE back out of the composite, rather than the field after it.
///
/// The four tests above do not exercise the read side's width at all: with a
/// handful of opaques the registry index fits in one byte, so a narrower read
/// of the field yields the same number, and a wider read yields a number that
/// is wrong but still compares unequal in the same direction. **Only resolving
/// the index back to its host object makes the read width observable**, which
/// is why this case exists as well as the two above.
#[test]
fn the_opaque_itself_survives_a_skewed_composite() {
    let src = "use make_handle\n\
               struct P { h: Handle, n: Word }\n\
               fn main() -> Handle { let h = make_handle(); let p = P { h: h, n: 7 }; p.h }";
    let program = parse(&tokenize(src).expect("lex")).expect("parse");
    let module = compile_with_target(&program, &wide_word_narrow_address()).expect("compile");
    let arena = Arena::with_capacity(4096);
    let mut vm: WideWordNarrowAddress<'_, '_> =
        WideWordNarrowAddress::new(module, &arena).expect("verify");
    vm.register_native("make_handle", |_args| {
        Ok(GenericValue::Opaque(host_arc(Handle {
            label: String::from("survivor"),
        })))
    });
    let val = match vm.call(&[]).expect("call") {
        GenericVmState::Finished(v) => v,
        other => panic!("expected finished, got {:?}", other),
    };
    let opaque = match val {
        GenericValue::Opaque(o) => o,
        other => panic!("expected an opaque, got {:?}", other),
    };
    let typed: &Handle = opaque
        .as_ref()
        .downcast_ref::<Handle>()
        .expect("downcast Handle");
    assert_eq!(
        typed.label, "survivor",
        "the opaque read back out of a skewed composite must be the one that was stored"
    );
}

/// The host DECODE of a skewed composite, which is a fourth width entirely.
///
/// `Vm::decode` walks the body with the field sizes the marshalling layer
/// reports, so it must ask for the same width the layout gave the field. This
/// is the only case here that crosses the host boundary, and none of the tests
/// above touches that path.
#[derive(KeleusmaType)]
struct Carrier {
    h: alloc::sync::Arc<dyn HostOpaque>,
    n: i64,
}

#[test]
fn a_skewed_composite_decodes_at_the_host_boundary() {
    let src = "use make_handle\n\
               struct Carrier { h: Handle, n: Word }\n\
               fn main() -> Carrier { Carrier { h: make_handle(), n: 9 } }";
    let program = parse(&tokenize(src).expect("lex")).expect("parse");
    let module = compile_with_target(&program, &wide_word_narrow_address()).expect("compile");
    let arena = Arena::with_capacity(4096);
    let mut vm: WideWordNarrowAddress<'_, '_> =
        WideWordNarrowAddress::new(module, &arena).expect("verify");
    vm.register_native("make_handle", |_args| {
        Ok(GenericValue::Opaque(host_arc(Handle {
            label: String::from("decoded"),
        })))
    });
    let val = match vm.call(&[]).expect("call") {
        GenericVmState::Finished(v) => v,
        other => panic!("expected finished, got {:?}", other),
    };
    let c: Carrier = vm.decode(&val).expect("decode");
    assert_eq!(
        c.n, 9,
        "the word field must survive the decode of a skewed body"
    );
    let typed: &Handle = c.h.as_ref().downcast_ref::<Handle>().expect("downcast");
    assert_eq!(typed.label, "decoded");
}

/// The control. The same programs on a runtime whose widths AGREE must behave
/// identically, so a failure above is attributable to the skew rather than to
/// the programs or to the narrow word.
#[test]
fn the_same_programs_behave_identically_where_the_widths_agree() {
    let program = parse(&tokenize(READ_AFTER).expect("lex")).expect("parse");
    let module = compile_with_target(&program, &Target::host()).expect("compile");
    let arena = Arena::with_capacity(4096);
    let mut vm = keleusma::vm::Vm::new(module, &arena).expect("verify");
    vm.register_native("make_handle", |_args| {
        Ok(GenericValue::Opaque(host_arc(Handle {
            label: String::from("id"),
        })))
    });
    match vm.call(&[]).expect("call") {
        GenericVmState::Finished(GenericValue::Int(n)) => assert_eq!(n, 7_i64),
        other => panic!("expected a word, got {:?}", other),
    }
}

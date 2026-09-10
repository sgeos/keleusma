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
    // **THE SKEW IS THE SUBJECT, SO BOTH ENDS MOVE WITH THE BUILD.** A literal 6-against-4 is not
    // emittable under `narrow-word-16`, where the compiler's maximum is 4 — the file could not run
    // at a narrow width. Pinning the word to the build's maximum and the address one step below it
    // keeps the ADDRESS NARROWER THAN THE WORD, which is the configuration the opaque defect was
    // first measured in. A particular pair of widths is not the property; their ordering is.
    // The address is capped by the RUNTIME ALIAS too, not only by the word. `WideWordNarrowAddress`
    // fixes it at `u16`, so a module declaring wider is refused at load — which a first attempt at
    // this change did, breaking the DEFAULT build by asking for 5 against the alias's 4. Caught by
    // running rather than by reading the edit.
    // **THE FLOOR OF 2 WAS WRONG, AND IT WAS THE CLASS UNDER REPAIR APPEARING INSIDE THE REPAIR.**
    // This clamp read `.clamp(2, ..)` until 2026-09-10. Under `narrow-word-8` the word is 3, one
    // step below it is 2, and 2 is a FOUR-BIT address -- not a width any runtime implements, and
    // one the layout sizes an opaque at zero bytes. The target compiled anyway and the fault
    // surfaced much later as a runtime `InvalidBytecode` about non-flat operands. The floor is now
    // taken from the narrowest type carrying `Address`, so it cannot drift below what exists.
    const ALIAS_ADDR_BITS_LOG2: u8 = 4; // `u16`, the alias below
    let floor = <u8 as keleusma::address::Address>::BITS_LOG2;
    let word = keleusma::bytecode::RUNTIME_WORD_BITS_LOG2;
    Target {
        word_bits_log2: word,
        addr_bits_log2: word
            .saturating_sub(1)
            .clamp(floor, ALIAS_ADDR_BITS_LOG2.max(floor)),
        ..Target::host()
    }
}

/// **THE PREMISE OF THIS WHOLE FILE, ASSERTED RATHER THAN ASSUMED.**
///
/// Every test below is about a runtime whose word and address widths DIFFER.
/// Both targets are derived from the build, and a derivation can collapse: if
/// the address ends up equal to the word, the tests still pass and test
/// NOTHING, which is the quietest possible failure. This states the premise so
/// a build that cannot express the skew says so instead of going vacuous.
///
/// It is expected to FAIL under `narrow-word-8`, and that failure is the
/// correct report: with the word already at the narrowest implemented width
/// there is no narrower address to pair it with, so the file has no subject
/// there. That is a build the file cannot cover, not a defect it has found.
#[test]
fn the_skew_this_file_depends_on_is_not_degenerate() {
    let skewed = wide_word_narrow_address();
    assert!(
        skewed.addr_bits_log2 < skewed.word_bits_log2,
        "the derived target has a {}-bit word and a {}-bit address, which is NOT a skew; every \
         test in this file would pass while exercising nothing. With the word at the narrowest \
         implemented width there is no narrower address, so this build cannot host the subject",
        skewed.word_bits(),
        skewed.address_bits(),
    );
    let six = Target::embedded_8();
    assert!(
        six.addr_bits_log2 > six.word_bits_log2,
        "the 6502-class target is meant to skew the OTHER way, a narrow word with a wider \
         address; it now reads {}-bit word against {}-bit address",
        six.word_bits(),
        six.address_bits(),
    );
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

// ---------------------------------------------------------------------------
// A differential corpus over composite SHAPES
// ---------------------------------------------------------------------------
//
// The tests above cover ONE shape: a flat struct with an opaque followed by a
// word. The packer has a separate branch for a nested composite child, an enum
// body carries a discriminant word ahead of its payload, an array strides by
// element size, and a tuple's access form is recovered by a different inference
// path than a struct's. None of that is exercised by the tests above.
//
// # The design, and why a difference here is attributable
//
// Each program runs on the DEFAULT runtime and on one whose ADDRESS width is
// sixteen bits, with the WORD held at sixty-four bits on both. Arithmetic,
// constants, and word-sized field reads are then bit-identical by construction,
// so any difference in result is an address-width confusion rather than a
// narrow-word artefact.
//
// **`ScalarKind::Opaque` is the only kind the address width sizes**, so this
// corpus is about opaque-bearing composites specifically. It is not a sweep of
// the value surface, and holding the word fixed means it says nothing at all
// about narrow-word behaviour.
//
// # Each entry proves it built the shape it names
//
// An entry whose composite is BOXED rather than flat would agree on both
// runtimes while testing nothing, because the boxed body carries no packed
// offsets. Every entry therefore also runs a program returning the composite
// itself and asserts the body is flat, on both runtimes.
//
// # Measured coverage, and an honest negative
//
// Reverting each of the four repaired sites in turn: the corpus catches the
// construction path and the arena packer, on shapes the tests above never
// build. It does NOT catch the read side or the host decode, both of which are
// held by their own tests above.
//
// **No mutation tried is caught by the corpus ALONE.** A nested child's stride
// was shortened by one byte specifically to demonstrate unique value, on the
// reasoning that no test above builds a nested composite. It failed as a
// demonstration: **seventeen tests across the suite caught it**, because a
// broken nested stride is wrong at every width and the suite already covers
// nesting well.
//
// So the case for this corpus is NOT that it currently catches something
// nothing else does. It is that it exercises the four repaired sites through
// the nested, enum, array and tuple construction paths, so a future defect
// confined to one of those shapes would be seen. **That is a hypothesis about
// defects not yet found, not a measurement**, and it should not be counted as
// coverage of anything today.
//
// One claim here IS made nowhere else: `every_corpus_shape_is_actually_built_flat`
// pins that these four shapes flatten at a skewed address width rather than
// falling back to a boxed body.

/// One corpus entry: a program returning a `Word`, the value it must produce,
/// and a program returning the composite so flatness can be checked.
struct Shape {
    name: &'static str,
    src: &'static str,
    shape_src: &'static str,
    expect: i64,
}

const PRELUDE: &str = "use make_handle() -> Handle\nuse handle_val(Handle) -> Word\n";

fn corpus() -> alloc::vec::Vec<Shape> {
    alloc::vec![
        Shape {
            name: "nested composite child",
            src: "struct Inner { h: Handle, a: Word }\n\
                  struct Outer { i: Inner, b: Word }\n\
                  fn main() -> Word { let o = Outer { i: Inner { h: make_handle(), a: 3 }, b: 5 }; \
                   handle_val(o.i.h) + o.i.a * 10 + o.b }",
            shape_src: "struct Inner { h: Handle, a: Word }\n\
                        struct Outer { i: Inner, b: Word }\n\
                        fn main() -> Outer { Outer { i: Inner { h: make_handle(), a: 3 }, b: 5 } }",
            expect: 100 + 30 + 5,
        },
        Shape {
            name: "enum payload after a discriminant word",
            src: "enum Held { Wrapped(Handle, Word), Empty }\n\
                  fn main() -> Word { let e = Held::Wrapped(make_handle(), 9); \
                   match e { Held::Wrapped(h, n) => handle_val(h) + n, Held::Empty => 0 } }",
            shape_src: "enum Held { Wrapped(Handle, Word), Empty }\n\
                        fn main() -> Held { Held::Wrapped(make_handle(), 9) }",
            expect: 109,
        },
        Shape {
            name: "array striding over opaque-bearing elements",
            src: "struct P { h: Handle, n: Word }\n\
                  fn main() -> Word { let a = [P { h: make_handle(), n: 1 }, P { h: make_handle(), n: 2 }]; \
                   a[1].n * 10 + a[0].n }",
            shape_src: "struct P { h: Handle, n: Word }\n\
                        fn main() -> [P; 2] { [P { h: make_handle(), n: 1 }, P { h: make_handle(), n: 2 }] }",
            expect: 21,
        },
        Shape {
            name: "tuple with two fields after the opaque",
            src: "fn main() -> Word { let t = (make_handle(), 3, 4); handle_val(t.0) + t.1 * 2 + t.2 }",
            shape_src: "fn main() -> (Handle, Word, Word) { (make_handle(), 3, 4) }",
            expect: 100 + 6 + 4,
        },
    ]
}

/// Register the two natives every corpus program uses, on any runtime width.
macro_rules! register_corpus_natives {
    ($vm:expr) => {{
        $vm.register_native("make_handle", |_args| {
            Ok(GenericValue::Opaque(host_arc(Handle {
                label: String::from("corpus"),
            })))
        });
        $vm.register_native("handle_val", |_args| Ok(GenericValue::Int(100)));
    }};
}

/// Whether a returned value carries a FLAT composite body.
fn is_flat<W: keleusma::word::Word, F: keleusma::float::Float>(v: &GenericValue<W, F>) -> bool {
    use keleusma::bytecode::{ArrayBody, EnumBody, StructBody, TupleBody};
    matches!(
        v,
        GenericValue::Struct(StructBody::Flat(_))
            | GenericValue::Tuple(TupleBody::Flat(_))
            | GenericValue::Array(ArrayBody::Flat(_))
            | GenericValue::Enum(EnumBody::Flat(_))
    )
}

#[test]
fn the_corpus_agrees_across_an_address_width_skew() {
    for shape in corpus() {
        let full = alloc::format!("{PRELUDE}{}", shape.src);
        let program = parse(&tokenize(&full).expect("lex")).expect("parse");

        // Default widths.
        let wide = compile_with_target(&program, &Target::host()).expect("compile wide");
        let arena_w = Arena::with_capacity(8192);
        let mut vm_w = keleusma::vm::Vm::new(wide, &arena_w).expect("verify wide");
        register_corpus_natives!(vm_w);
        let got_w = match vm_w.call(&[]).expect("call wide") {
            GenericVmState::Finished(GenericValue::Int(n)) => n,
            other => panic!("{}: expected a word, got {:?}", shape.name, other),
        };

        // Same word, sixteen-bit address.
        let skew =
            compile_with_target(&program, &wide_word_narrow_address()).expect("compile skewed");
        let arena_s = Arena::with_capacity(8192);
        let mut vm_s: WideWordNarrowAddress<'_, '_> =
            WideWordNarrowAddress::new(skew, &arena_s).expect("verify skewed");
        register_corpus_natives!(vm_s);
        let got_s = match vm_s.call(&[]).expect("call skewed") {
            GenericVmState::Finished(GenericValue::Int(n)) => n,
            other => panic!("{}: expected a word, got {:?}", shape.name, other),
        };

        assert_eq!(
            got_w, shape.expect,
            "{}: the default runtime already disagrees with the expected value, so the \
             cross-width comparison below would be meaningless",
            shape.name
        );
        assert_eq!(
            got_s, got_w,
            "{}: the two runtimes differ only in address width, so a difference here is an \
             address-width confusion",
            shape.name
        );
    }
}

#[test]
fn every_corpus_shape_is_actually_built_flat() {
    for shape in corpus() {
        let full = alloc::format!("{PRELUDE}{}", shape.shape_src);
        let program = parse(&tokenize(&full).expect("lex")).expect("parse");

        let wide = compile_with_target(&program, &Target::host()).expect("compile wide");
        let arena_w = Arena::with_capacity(8192);
        let mut vm_w = keleusma::vm::Vm::new(wide, &arena_w).expect("verify wide");
        register_corpus_natives!(vm_w);
        let val_w = match vm_w.call(&[]).expect("call wide") {
            GenericVmState::Finished(v) => v,
            other => panic!("{}: {:?}", shape.name, other),
        };
        assert!(
            is_flat(&val_w),
            "{}: the composite is BOXED at the default widths, so the corpus entry above \
             compares two boxed bodies and tests no packed offset at all",
            shape.name
        );

        let skew =
            compile_with_target(&program, &wide_word_narrow_address()).expect("compile skewed");
        let arena_s = Arena::with_capacity(8192);
        let mut vm_s: WideWordNarrowAddress<'_, '_> =
            WideWordNarrowAddress::new(skew, &arena_s).expect("verify skewed");
        register_corpus_natives!(vm_s);
        let val_s = match vm_s.call(&[]).expect("call skewed") {
            GenericVmState::Finished(v) => v,
            other => panic!("{}: {:?}", shape.name, other),
        };
        assert!(
            is_flat(&val_s),
            "{}: the composite is BOXED at the skewed widths but flat at the default ones, \
             which is itself a width-dependent difference",
            shape.name
        );
    }
}

#![cfg(feature = "floats")]
//! Is a flat FLOAT field sized by the float width at every site?
//!
//! # Why this exists: an audit excluded this class by an argument that is false
//!
//! `FLAT_FIELD_WIDTH_AUDIT.md` audited the sites that size an **opaque** flat field, after one of
//! them returned a silently wrong answer. It justified stopping there:
//!
//! > `ScalarKind::Opaque` is the **only** kind the address width sizes. Every other kind is a
//! > function of the word width or the float width, so a site assuming a word is correct for them.
//!
//! **The second sentence is false for `Float`.** A float field is sized by the FLOAT width, which
//! is selected independently of the word width — `narrow-float-32` exists, and
//! `GenericVm<i64, u64, f32>` reaches the skew in the default build. A site assuming a word for a
//! float field is correct only where the two widths happen to be equal, which is the precise
//! coincidence that hid the opaque defect for as long as it hid.
//!
//! The argument being unsound does not make the code wrong. **It is not wrong**, and this file is
//! the evidence rather than the assertion.
//!
//! # The configuration that matters, and why it is not the obvious one
//!
//! The first probe written for this compiled and ran at a matched width — module `f32` on an `f32`
//! runtime — and passed. It proved nothing: every site derives from one layout, so a matched pair
//! agrees however the layout is written. Mis-sizing the layout itself was an **equivalent
//! mutation**, invisible in every case, because the compiler's baked offsets and the runtime's
//! strides moved together.
//!
//! The defect shape requires **two authorities**, which needs the module's declared float width to
//! differ from the runtime's. The load check refuses a module whose float is WIDER than the
//! runtime and admits one that is NARROWER, so a module compiled for a 32-bit-float target running
//! on a 64-bit-float runtime is a supported configuration and is the one exercised here.
//!
//! # This corpus has demonstrated reach, which is the part that is easy to skip
//!
//! Made to fail by taking the VM's float width from the runtime type rather than the module
//! header — one site disagreeing with the layout, the opaque defect transposed. **Three of the six
//! cases below catch it**, reporting a silently wrong integer rather than a fault, exactly as the
//! opaque defect did. The three that catch it are the three that read a field POSITIONED AFTER a
//! float; the three that do not are reading the float itself, where an offset error is invisible.
//!
//! That is the property the corpus needs, and `the_corpus_reads_past_a_float` asserts it — not
//! that the corpus contains floats, which would be a proxy for coverage rather than coverage.

use keleusma::Arena;
use keleusma::bytecode::GenericValue;
use keleusma::compiler::compile_with_target;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::target::Target;
use keleusma::vm::{GenericVm, GenericVmState};

/// A runtime whose float is 64 bits, for modules declaring 32.
type WideFloatVm<'a, 'arena> = GenericVm<'a, 'arena, i64, u64, f64>;
/// A runtime whose float is 32 bits, matching a module that declares 32.
type NarrowFloatVm<'a, 'arena> = GenericVm<'a, 'arena, i64, u64, f32>;

/// One case: a program, the value it must produce, and whether it reads a field
/// positioned after a float.
struct Case {
    label: &'static str,
    src: &'static str,
    expect: &'static str,
    /// **The distinguishing property.** An offset error caused by mis-sizing a float is only
    /// observable from something stored AFTER the float. Reading the float itself lands at the
    /// same place either way.
    reads_past_a_float: bool,
}

const CASES: &[Case] = &[
    Case {
        label: "a word field positioned after a float",
        src: "struct P { f: Float, n: Word }\nfn main() -> Word { let p = P { f: 1.5, n: 42 }; p.n }",
        expect: "Int(42)",
        reads_past_a_float: true,
    },
    Case {
        label: "the float itself, float first",
        src: "struct P { f: Float, n: Word }\nfn main() -> Float { let p = P { f: 1.5, n: 42 }; p.f }",
        expect: "Float(1.5)",
        reads_past_a_float: false,
    },
    Case {
        label: "a float positioned after a word",
        src: "struct Q { n: Word, f: Float }\nfn main() -> Float { let q = Q { n: 42, f: 1.5 }; q.f }",
        expect: "Float(1.5)",
        reads_past_a_float: false,
    },
    Case {
        label: "a word after TWO floats, so the error doubles",
        src: "struct R { a: Float, b: Float, n: Word }\nfn main() -> Word { let r = R { a: 1.5, b: 2.5, n: 7 }; r.n }",
        expect: "Int(7)",
        reads_past_a_float: true,
    },
    Case {
        label: "the last element of a float array",
        src: "fn main() -> Float { let a = [1.5, 2.5, 3.5]; a[2] }",
        expect: "Float(3.5)",
        reads_past_a_float: false,
    },
    Case {
        label: "a word after a float in a tuple",
        src: "fn main() -> Word { let t = (1.5, 9); t.1 }",
        expect: "Int(9)",
        reads_past_a_float: true,
    },
];

fn module_for(src: &str, float_bits_log2: u8) -> keleusma::bytecode::Module {
    let target = Target {
        word_bits_log2: 6,
        addr_bits_log2: 6,
        float_bits_log2,
        has_floats: true,
        has_strings: false,
    };
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    compile_with_target(&program, &target).expect("compile")
}

fn finished<W, F>(state: GenericVmState<W, F>) -> GenericValue<W, F>
where
    W: keleusma::word::Word,
    F: keleusma::float::Float,
{
    match state {
        GenericVmState::Finished(v) => v,
        other => panic!("the program did not run to completion: {other:?}"),
    }
}

/// The module declares a 32-bit float; the runtime provides 64. **Two widths, and the layout must
/// be the only authority.**
#[test]
fn a_module_declaring_a_narrower_float_than_the_runtime_reads_every_field_correctly() {
    for case in CASES {
        let module = module_for(case.src, 5);
        let arena = Arena::with_capacity(65536);
        let mut vm: WideFloatVm<'_, '_> = WideFloatVm::new(module, &arena).expect("new");
        let got = format!("{:?}", finished(vm.call(&[]).expect("call")));
        assert_eq!(
            got, case.expect,
            "`{}` read the wrong value on a 64-bit-float runtime running a 32-bit-float module. A \
             WRONG VALUE rather than a fault is the opaque defect's signature: some site took the \
             float width from somewhere other than the module's layout.",
            case.label
        );
    }
}

/// The matched pair, as a control. **On its own this test would establish nothing** — every site
/// derives from one layout, so a matched configuration agrees however that layout is written. It
/// is here to show the corpus itself is sound, not to check the width.
#[test]
fn the_same_corpus_is_correct_when_the_module_and_the_runtime_agree() {
    for case in CASES {
        let module = module_for(case.src, 5);
        let arena = Arena::with_capacity(65536);
        let mut vm: NarrowFloatVm<'_, '_> = NarrowFloatVm::new(module, &arena).expect("new");
        let got = format!("{:?}", finished(vm.call(&[]).expect("call")));
        assert_eq!(
            got, case.expect,
            "`{}` is wrong even at a matched width, so the case is broken rather than the widths",
            case.label
        );
    }
}

/// **The corpus must READ PAST A FLOAT, not merely contain one.**
///
/// A mis-sized float field displaces everything stored after it. A case that reads the float
/// itself lands in the same place under either width and cannot separate the two readings, so a
/// corpus of only such cases would pass against a broken tree. Three of the six cases carry the
/// property; the mutation that proves it is described in this file's header.
///
/// This assertion names the property that distinguishes the competing readings rather than
/// inventorying the constructs present. A construct list is a proxy for coverage, and this
/// repository has twice shipped a coverage assertion that was itself vacuous.
#[test]
fn the_corpus_reads_past_a_float() {
    let past = CASES.iter().filter(|c| c.reads_past_a_float).count();
    assert!(
        past >= 3,
        "only {past} case(s) read a field positioned after a float. Below three the corpus stops \
         separating a correct float width from a word-sized one, which is the only thing this \
         file checks."
    );
    assert!(
        CASES.iter().any(|c| !c.reads_past_a_float),
        "every case now reads past a float, so the control cases that pin the float's OWN value \
         are gone and a width error that shifted every field equally could pass unnoticed"
    );
}

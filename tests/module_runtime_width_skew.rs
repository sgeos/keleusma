#![cfg(feature = "floats")]
//! A module may declare a NARROWER width than the runtime provides. The layout must still be the
//! only authority.
//!
//! # The general property, of which the opaque defect was one instance
//!
//! Three widths are carried independently: the word, the float, and the address. Each has **two
//! possible authorities** — the module header, which the compiler baked every field offset from,
//! and the runtime type parameter of `GenericVm`. They must never disagree, and the load check
//! does not make them agree: it refuses a module whose width is **wider** than the runtime and
//! admits one that is **narrower**. A module compiled for a small target and run on a large host is
//! therefore a supported configuration, and it is the one in which a second authority becomes
//! visible.
//!
//! **This is exactly how the opaque defect presented.** Four sites took the address width from
//! somewhere other than the layout; the default build made the widths equal, so they agreed by
//! coincidence, and a composite compared equal to one differing from it in a `Word` field.
//!
//! This file covers the **word** and **address** axes. The float axis is covered by
//! `tests/flat_float_field_width.rs`, which records why the obvious probe for it proved nothing.
//!
//! # What the existing skew tests do NOT cover, which is why this one exists
//!
//! `tests/composite_width_skew.rs` drives narrow and skewed runtimes, and every configuration in it
//! is **matched**: a module compiled for a sixteen-bit address runs on a `u16` runtime. A matched
//! pair cannot expose a second authority, because both readings coincide. The skew here is between
//! the module and the runtime rather than between two widths of one target.
//!
//! # Reach, established rather than assumed
//!
//! A corpus that passes says nothing until it has been made to fail. Each axis was mutated by
//! taking the VM's width for that axis from the runtime type instead of the module header — one
//! site disagreeing with the layout, the opaque defect transposed:
//!
//! | axis | cases | caught the mutation |
//! |---|---|---|
//! | word | 6 | 3 |
//! | address | 3 | 2 |
//!
//! Every catch reported a **silently wrong integer** rather than a fault, which is the signature
//! that made the original defect worse than any refusal in this tree.
//!
//! **The cases that do NOT catch it are kept deliberately and are marked.** A case reading a field
//! that sits BEFORE the skewed-width field, or reading that field itself, lands in the same place
//! under either width. Those are controls: they prove the corpus is not simply broken.
//!
//! # WHICH SHAPES SEPARATE, MEASURED — AND THREE EXCLUDED EXPLANATIONS
//!
//! Two word cases — an array element and a byte-then-word struct — read correctly under the word
//! mutation. **The mechanism is not established.** Three hypotheses have been tested and excluded,
//! recorded because an unexplained survivor reads as a missing guard and sends the next reader
//! hunting for one:
//!
//! | hypothesis | verdict |
//! |---|---|
//! | the read is constant-folded, so no flat body is touched | **excluded** — an array built from a host call survives identically |
//! | the composite is BOXED, so both widths agree | **excluded** — every shape reports a FLAT body at both module widths |
//! | an over-wide read pulls in ZERO neighbours and is right by luck | **excluded** — adding a non-zero field after the one read changes nothing |
//!
//! The second was the likely one: `tests/composite_width_skew.rs` records that a boxed composite
//! agrees on both runtimes and asserts flatness for exactly that reason. It does not apply here.
//! All three survivors and separators compile to the **same** opcode, a flat `GetField` or
//! `GetIndex` carrying an `Int` kind, so the path is not the difference either.
//!
//! **What IS established is a characterization, and it covers every case measured:**
//!
//! | shape | separates the two readings |
//! |---|---|
//! | all-`Word` composite, field not first | **yes** |
//! | all-`Word` composite, FIRST field | no — offset zero is the same under any width |
//! | composite with a leading `Byte` | no |
//! | array element, any position | no |
//!
//! **That is a stated LIMIT, not a defect.** Every shape answers correctly on the unmutated tree.
//! What the table says is that this corpus's word-axis sensitivity comes entirely from homogeneous
//! word composites read past their first field, so **arrays and byte-leading structs are not
//! checked against a word-width divergence by this file**. A later reader wanting that coverage
//! needs a different instrument, not more cases of the same shape.

use keleusma::bytecode::GenericValue;
use keleusma::compiler::compile_with_target;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::target::Target;
use keleusma::vm::{GenericVm, GenericVmState};
use keleusma::{Arena, HostOpaque, host_arc};

#[derive(Debug)]
struct Handle;
impl HostOpaque for Handle {
    fn type_name(&self) -> &'static str {
        "Handle"
    }
}

/// The wide runtime every case below runs on: sixty-four bits in all three widths.
type WideVm<'a, 'arena> = GenericVm<'a, 'arena, i64, u64, f64>;

/// One case, with the property that decides whether it can separate the two readings.
struct Case {
    label: &'static str,
    src: &'static str,
    expect: i64,
    /// **Measured, not predicted.** True where the width mutation for this axis actually produced
    /// a wrong value. A case that cannot separate the readings is a control, not coverage, and
    /// counting it as coverage is how a guard comes to assert nothing.
    separates: bool,
}

/// One step narrower than the build emits, which is what "narrower than the runtime" has to mean
/// when the build itself is narrow.
///
/// **A literal 4-against-6 does not survive a narrow build.** Under `narrow-word-16` the compiler
/// refuses `word_bits_log2 = 6` outright — runtime maximum 4 — so this file could not run there,
/// which is the defect this session repaired in `tests/narrow_vm.rs` and introduced here while
/// auditing it. Deriving both ends from the build keeps the SKEW, which is the property, rather
/// than a particular pair of widths, which is not.
fn one_step_narrower(build: u8) -> u8 {
    build.saturating_sub(1).max(2)
}

fn module_narrower(src: &str, word_log2: u8, addr_log2: u8) -> keleusma::bytecode::Module {
    let target = Target {
        word_bits_log2: word_log2,
        addr_bits_log2: addr_log2,
        float_bits_log2: keleusma::bytecode::RUNTIME_FLOAT_BITS_LOG2,
        has_floats: true,
        has_strings: false,
    };
    let program = parse(&tokenize(src).expect("lex")).expect("parse");
    compile_with_target(&program, &target).expect("compile")
}

fn word_of(state: GenericVmState<i64, f64>, label: &str) -> i64 {
    match state {
        GenericVmState::Finished(GenericValue::Int(n)) => n,
        other => panic!("`{label}` did not finish with a word: {other:?}"),
    }
}

const WORD_CASES: &[Case] = &[
    Case {
        label: "the second of two word fields",
        src: "struct P { a: Word, b: Word }\nfn main() -> Word { let p = P { a: 11, b: 22 }; p.b }",
        expect: 22,
        separates: true,
    },
    Case {
        label: "the first of two word fields",
        src: "struct P { a: Word, b: Word }\nfn main() -> Word { let p = P { a: 11, b: 22 }; p.a }",
        expect: 11,
        separates: false,
    },
    Case {
        label: "the third of three word fields",
        src: "struct R { a: Word, b: Word, c: Word }\nfn main() -> Word { let r = R { a: 1, b: 2, c: 33 }; r.c }",
        expect: 33,
        separates: true,
    },
    Case {
        label: "a word field after a byte field",
        src: "struct Q { f: Byte, n: Word }\nfn main() -> Word { let q = Q { f: 1 as Byte, n: 44 }; q.n }",
        expect: 44,
        separates: false,
    },
    Case {
        label: "the last element of a word array",
        src: "fn main() -> Word { let a = [1, 2, 55]; a[2] }",
        expect: 55,
        separates: false,
    },
    Case {
        label: "the second element of a word tuple",
        src: "fn main() -> Word { let t = (7, 66); t.1 }",
        expect: 66,
        separates: true,
    },
];

const ADDRESS_CASES: &[Case] = &[
    Case {
        label: "a word field after an opaque",
        src: "use make_handle\nstruct P { h: Handle, n: Word }\n\
               fn main() -> Word { let h = make_handle(); let p = P { h: h, n: 42 }; p.n }",
        expect: 42,
        separates: true,
    },
    Case {
        label: "the second word field after an opaque",
        src: "use make_handle\nstruct R { h: Handle, a: Word, b: Word }\n\
               fn main() -> Word { let h = make_handle(); let r = R { h: h, a: 1, b: 77 }; r.b }",
        expect: 77,
        separates: true,
    },
    Case {
        label: "a word field BEFORE an opaque",
        src: "use make_handle\nstruct Q { n: Word, h: Handle }\n\
               fn main() -> Word { let h = make_handle(); let q = Q { n: 42, h: h }; q.n }",
        expect: 42,
        separates: false,
    },
];

/// The module declares a sixteen-bit **word**; the runtime provides sixty-four.
#[test]
fn a_module_declaring_a_narrower_word_than_the_runtime_reads_every_field_correctly() {
    for case in WORD_CASES {
        let module = module_narrower(
            case.src,
            one_step_narrower(keleusma::bytecode::RUNTIME_WORD_BITS_LOG2),
            keleusma::bytecode::RUNTIME_ADDRESS_BITS_LOG2,
        );
        let arena = Arena::with_capacity(65536);
        let mut vm: WideVm<'_, '_> = WideVm::new(module, &arena).expect("verify");
        let got = word_of(vm.call(&[]).expect("call"), case.label);
        assert_eq!(
            got, case.expect,
            "`{}` read the wrong value on a sixty-four-bit-word runtime running a sixteen-bit-word \
             module. A WRONG VALUE rather than a fault means some site took the word width from \
             somewhere other than the module's layout.",
            case.label
        );
    }
}

/// The module declares a sixteen-bit **address**; the runtime provides sixty-four. This is the axis
/// the original defect lived on, in the configuration the existing skew tests do not build.
#[test]
fn a_module_declaring_a_narrower_address_than_the_runtime_reads_every_field_correctly() {
    for case in ADDRESS_CASES {
        let module = module_narrower(
            case.src,
            keleusma::bytecode::RUNTIME_WORD_BITS_LOG2,
            one_step_narrower(keleusma::bytecode::RUNTIME_ADDRESS_BITS_LOG2),
        );
        let arena = Arena::with_capacity(65536);
        let mut vm: WideVm<'_, '_> = WideVm::new(module, &arena).expect("verify");
        vm.register_native("make_handle", |_args| {
            Ok(GenericValue::Opaque(host_arc(Handle)))
        });
        let got = word_of(vm.call(&[]).expect("call"), case.label);
        assert_eq!(
            got, case.expect,
            "`{}` read the wrong value on a sixty-four-bit-address runtime running a \
             sixteen-bit-address module. This is the shape that once compared two differing \
             composites equal.",
            case.label
        );
    }
}

/// **Each axis must keep cases that actually separate the two readings, and keep controls too.**
///
/// The separating count is measured against the width mutation described in this file's header,
/// not predicted from the source. Asserting the corpus merely *contains* structs would be a proxy
/// for coverage, and this repository has twice shipped a coverage assertion that was itself
/// vacuous.
#[test]
fn each_axis_keeps_both_separating_cases_and_controls() {
    for (axis, cases, minimum) in [("word", WORD_CASES, 3), ("address", ADDRESS_CASES, 2)] {
        let separating = cases.iter().filter(|c| c.separates).count();
        assert!(
            separating >= minimum,
            "the {axis} axis now has only {separating} case(s) that separate a correct width from \
             a runtime-derived one, below the {minimum} measured against the mutation. Below that \
             the axis stops being checked."
        );
        assert!(
            cases.iter().any(|c| !c.separates),
            "the {axis} axis has lost its controls. A corpus of only separating cases cannot show \
             it is reading the right thing when the widths agree."
        );
    }
}

#![cfg(all(feature = "compile", feature = "verify"))]
//! Every target descriptor the compiler ACCEPTS, against a corpus of ordinary
//! programs, asking whether any of them reaches a run-time `InvalidBytecode`.
//!
//! # The axis this exists to vary
//!
//! `docs/decisions/INVALID_BYTECODE_CENSUS.md` asks at each site whether a
//! module that a supported producer emitted, and that `verify()` accepted, can
//! reach it. It derived its population from source and reasoned about each group
//! from the shapes a PROGRAM can express.
//!
//! **It never varied the TARGET DESCRIPTOR.** On 2026-09-10 an ordinary program
//! compiled for a degenerate target reached a site that census does not name.
//! The route is closed, but the assumption it rested on -- that the target is
//! well-formed -- was unstated and load-bearing. `Target` is public,
//! `compile_with_target` is public, and `embedded_8` and `embedded_16` are
//! shipped presets, so choosing a target is a SUPPORTED use.
//!
//! # What a cell means
//!
//! `InvalidBytecode` means *this artefact should never have been produced*. It
//! is the class `verify()` exists to exclude. So the outcomes are not equally
//! interesting:
//!
//! | outcome | reading |
//! |---|---|
//! | refused at compile time | SAFE, and the expected majority |
//! | refused at load | SAFE |
//! | ran to a value | the module was well formed for that target |
//! | run-time `InvalidBytecode` | **a hole in the load-time guarantee** |
//! | other run-time fault | classified separately, not lumped in |
//!
//! A legitimate program error at a narrow width -- an overflow that genuinely
//! does not fit -- is NOT an artefact defect and is counted apart. Conflating
//! the two would turn the sweep into noise.
//!
//! # Bounds are derived, never written
//!
//! Every width bound comes from the runtime's own constants and trait impls.
//! Three increments of this line have been repairing hard-coded widths, and a
//! matrix carrying one would be the same defect a seventh time.

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use keleusma::bytecode::GenericValue;
use keleusma::compiler::compile_with_target;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::target::Target;
use keleusma::vm::{VmError, VmState};
use keleusma::{Arena, HostOpaque, host_arc};

struct Handle;
impl HostOpaque for Handle {
    fn type_name(&self) -> &'static str {
        "Handle"
    }
}

/// The narrowest widths any runtime implements, from the trait impls.
const WORD_FLOOR: u8 = <i8 as keleusma::word::Word>::BITS_LOG2;
const ADDR_FLOOR: u8 = <u8 as keleusma::address::Address>::BITS_LOG2;

const PRELUDE: &str = "use make_handle() -> Handle\nuse handle_val(Handle) -> Word\n";

/// A program and the value it must produce wherever it runs at all.
///
/// **Every expected value is small enough to exist in an EIGHT-BIT word.** A
/// corpus whose answers overflow at the narrow end would report a legitimate
/// arithmetic trap as though it were an artefact defect, which is the failure
/// mode that makes a sweep like this useless.
struct Shape {
    name: &'static str,
    src: &'static str,
    expect: i64,
}

fn corpus() -> Vec<Shape> {
    alloc::vec![
        Shape {
            name: "scalar arithmetic",
            src: "fn main() -> Word { 1 + 2 }",
            expect: 3,
        },
        Shape {
            name: "word field after an opaque",
            src: "struct P { h: Handle, n: Word }\n\
                  fn main() -> Word { let p = P { h: make_handle(), n: 7 }; p.n }",
            expect: 7,
        },
        Shape {
            name: "nested composite child",
            src: "struct Inner { h: Handle, a: Word }\n\
                  struct Outer { i: Inner, b: Word }\n\
                  fn main() -> Word { let o = Outer { i: Inner { h: make_handle(), a: 3 }, b: 5 }; \
                   o.i.a * 10 + o.b }",
            expect: 35,
        },
        Shape {
            name: "array stride over opaque-bearing elements",
            src: "struct P { h: Handle, n: Word }\n\
                  fn main() -> Word { let a = [P { h: make_handle(), n: 1 }, \
                   P { h: make_handle(), n: 2 }]; a[1].n * 10 + a[0].n }",
            expect: 21,
        },
        Shape {
            name: "enum payload after a discriminant",
            src: "enum Held { Wrapped(Handle, Word), Empty }\n\
                  fn main() -> Word { let e = Held::Wrapped(make_handle(), 9); \
                   match e { Held::Wrapped(h, n) => n, Held::Empty => 0 } }",
            expect: 9,
        },
        Shape {
            name: "tuple with fields after an opaque",
            src: "fn main() -> Word { let t = (make_handle(), 3, 4); t.1 * 2 + t.2 }",
            expect: 10,
        },
    ]
}

/// What one (target, program) cell did.
#[derive(Debug, PartialEq, Eq)]
enum Cell {
    /// Refused before a module existed. Safe.
    CompileRefused,
    /// A module existed and the loader refused it. Safe.
    LoadRefused,
    /// Ran and produced the expected value.
    Ran,
    /// Ran and produced a DIFFERENT value. Not `InvalidBytecode`, but a
    /// silent disagreement is worse than a fault, so it is its own category.
    WrongAnswer(i64),
    /// **The finding this sweep exists to detect.**
    InvalidBytecodeAtRuntime(String),
    /// A run-time fault that is not an artefact defect: an overflow that
    /// genuinely does not fit the declared width, a host contract error.
    OtherFault(String),
}

fn run_cell(target: &Target, shape: &Shape) -> Cell {
    let full = format!("{PRELUDE}{}", shape.src);
    let Ok(tokens) = tokenize(&full) else {
        return Cell::CompileRefused;
    };
    let Ok(program) = parse(&tokens) else {
        return Cell::CompileRefused;
    };
    let Ok(module) = compile_with_target(&program, target) else {
        return Cell::CompileRefused;
    };
    let arena = Arena::with_capacity(8192);
    let Ok(mut vm) = keleusma::vm::Vm::new(module, &arena) else {
        return Cell::LoadRefused;
    };
    vm.register_native("make_handle", |_args| {
        Ok(GenericValue::Opaque(host_arc(Handle)))
    });
    vm.register_native("handle_val", |_args| Ok(GenericValue::Int(100)));
    match vm.call(&[]) {
        Ok(VmState::Finished(GenericValue::Int(n))) => {
            // `Word::to_i64` rather than `i64::from`: the runtime word is `i64`
            // at the default build and NARROWER at every `narrow-word-*` one, so
            // the `From` form is a useless conversion in one configuration and
            // the only correct one in the others. The trait method is right in
            // all of them and needs no lint exemption to say so.
            let got = keleusma::word::Word::to_i64(n);
            if got == shape.expect {
                Cell::Ran
            } else {
                Cell::WrongAnswer(got)
            }
        }
        Ok(other) => Cell::OtherFault(format!("unexpected final state: {other:?}")),
        Err(VmError::InvalidBytecode(m)) => Cell::InvalidBytecodeAtRuntime(m),
        Err(e) => Cell::OtherFault(format!("{e:?}")),
    }
}

/// Every descriptor the compiler could accept, derived from the runtime's own
/// maxima. `has_floats` is swept too, because the float floor is enforced in a
/// different function from the other two and the flag gates it.
fn targets() -> Vec<(String, Target)> {
    let mut out = Vec::new();
    let word_max = keleusma::bytecode::RUNTIME_WORD_BITS_LOG2;
    let addr_max = keleusma::bytecode::RUNTIME_ADDRESS_BITS_LOG2;
    let float_max = keleusma::bytecode::RUNTIME_FLOAT_BITS_LOG2;
    for word in WORD_FLOOR..=word_max {
        for addr in ADDR_FLOOR..=addr_max {
            out.push((
                format!("w{word}/a{addr}/nofloat"),
                Target {
                    word_bits_log2: word,
                    addr_bits_log2: addr,
                    has_floats: false,
                    float_bits_log2: 0,
                    ..Target::host()
                },
            ));
            // 5 is the narrowest float format the runtime implements; anything
            // narrower is refused by the floor beside the other two.
            for float in 5..=float_max {
                out.push((
                    format!("w{word}/a{addr}/f{float}"),
                    Target {
                        word_bits_log2: word,
                        addr_bits_log2: addr,
                        has_floats: true,
                        float_bits_log2: float,
                        ..Target::host()
                    },
                ));
            }
        }
    }
    out
}

/// **THE SWEEP.**
///
/// The assertion is narrow on purpose: a run-time `InvalidBytecode` is a
/// finding, and every other outcome is reported but permitted. Refusal is the
/// correct and expected majority outcome, and a sweep that treated it as a
/// defect would be wrong about its own subject.
#[test]
fn no_accepted_target_lets_an_ordinary_program_reach_invalid_bytecode() {
    let mut findings: Vec<String> = Vec::new();
    let mut wrong: Vec<String> = Vec::new();
    let (mut ran, mut compile_refused, mut load_refused, mut other) = (0, 0, 0, 0);

    for (label, target) in targets() {
        for shape in corpus() {
            match run_cell(&target, &shape) {
                Cell::Ran => ran += 1,
                Cell::CompileRefused => compile_refused += 1,
                Cell::LoadRefused => load_refused += 1,
                Cell::OtherFault(_) => other += 1,
                Cell::WrongAnswer(got) => wrong.push(format!(
                    "{label} / {}: expected {}, got {got}",
                    shape.name, shape.expect
                )),
                Cell::InvalidBytecodeAtRuntime(m) => {
                    findings.push(format!("{label} / {}: {m}", shape.name))
                }
            }
        }
    }

    // **NON-VACUITY.** A sweep in which nothing ran proves nothing about the
    // runtime, and one whose descriptor set has collapsed to a single point is
    // not varying what it claims to vary.
    //
    // The first draft ALSO required at least one refusal, on the assumption that
    // some admissible descriptor would be rejected for these programs. **That was
    // wrong, and the sweep said so on its first run**: every descriptor the
    // compiler accepts compiles and loads every shape in this corpus. Requiring a
    // refusal asserted a property that had not been measured, in a test written
    // to measure properties -- so the check now constrains the SWEEP's spread,
    // which is what non-vacuity actually needs, and the refusal counts are
    // reported rather than demanded.
    assert!(
        ran > 0,
        "no cell ran a program to completion, so this sweep measured nothing about the runtime"
    );
    let spread = targets();
    assert!(
        spread.len() > 1
            && spread.iter().any(|(_, t)| t.word_bits_log2 == WORD_FLOOR)
            && spread
                .iter()
                .any(|(_, t)| t.word_bits_log2 == keleusma::bytecode::RUNTIME_WORD_BITS_LOG2),
        "the descriptor set does not span the runtime's width range, so the sweep is not \
         varying the axis it names ({} descriptors)",
        spread.len()
    );

    assert!(
        findings.is_empty(),
        "a module that COMPILED and LOADED raised InvalidBytecode, which is the class verify() \
         exists to exclude:\n{}\n(ran {ran}, compile-refused {compile_refused}, load-refused \
         {load_refused}, other {other})",
        findings.join("\n")
    );
    assert!(
        wrong.is_empty(),
        "a module that compiled and loaded returned the WRONG ANSWER, which is worse than a \
         fault because nothing reports it:\n{}",
        wrong.join("\n")
    );

    // **THE RESULT, PINNED RATHER THAN DESCRIBED.** Measured 2026-09-10 at the default build:
    // every one of the 288 cells RAN and returned the expected value. Nothing was refused at
    // either boundary and nothing faulted.
    //
    // A refusal is a SAFE outcome and this is not asserting that refusals are wrong. It asserts
    // that the sweep's shape has not changed silently: if a descriptor starts being refused, or a
    // shape stops running, that is worth a reader's attention even though neither is a defect.
    // The figure is derived from the corpus and descriptor set rather than written as a literal,
    // so adding either moves it.
    assert_eq!(
        ran,
        targets().len() * corpus().len(),
        "not every accepted descriptor ran every shape: {ran} of {} cells ran ({compile_refused} \
         compile-refused, {load_refused} load-refused, {other} other). None of those outcomes is \
         unsafe, but the sweep's shape has changed and the document describing it is now stale",
        targets().len() * corpus().len()
    );
}

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
        // ------------------------------------------------------------------
        // ADDED 2026-09-10. The six above caught the only defect known on this
        // axis in exactly ONE of their number -- the array stride, because
        // striding MULTIPLIES an element size, so a zero-byte scalar surfaces
        // there and is absorbed everywhere else. Each shape below is here for a
        // layout property the first six do not stress, named in its comment.
        // Variety is not the criterion; a second plain struct would add cells
        // and no information.
        // ------------------------------------------------------------------
        Shape {
            // A scalar whose width is the WORD but whose value is a Q-format
            // encoding, sitting between the opaque and the word that follows it.
            // The default fraction count is DERIVED from the word width, so this
            // is the one shape whose semantics move with the descriptor.
            name: "word field after an opaque and a Fixed",
            src: "struct P { h: Handle, f: Fixed<4>, n: Word }\n\
                  fn main() -> Word { let p = P { h: make_handle(), f: 2Fixed<4>, n: 7 }; p.n }",
            expect: 7,
        },
        Shape {
            // A scalar that is ONE BYTE at every word width. Every other scalar
            // in the corpus is sized by a width the descriptor moves, so this is
            // the only field whose offset contribution is descriptor-invariant
            // while its neighbours' are not.
            name: "word field after an opaque and a Byte",
            src: "struct P { h: Handle, b: Byte, n: Word }\n\
                  fn main() -> Word { let p = P { h: make_handle(), b: 5Byte, n: 7 }; p.n }",
            expect: 7,
        },
        Shape {
            // Stride COMPOSED with a field offset: the array's element size is
            // multiplied inside a body that already has an opaque ahead of it.
            name: "array inside a struct after an opaque",
            src: "struct S { h: Handle, a: [Word; 2], n: Word }\n\
                  fn main() -> Word { let s = S { h: make_handle(), a: [4, 5], n: 6 }; \
                   s.a[1] * 10 + s.n }",
            expect: 56,
        },
        Shape {
            // A COMPOSITE payload behind a discriminant word: the enum body's
            // own offsets are computed relative to a header the descriptor sizes.
            name: "composite payload inside an enum",
            src: "struct Inner { h: Handle, a: Word }\n\
                  enum Held { Wrapped(Inner), Empty }\n\
                  fn main() -> Word { let e = Held::Wrapped(Inner { h: make_handle(), a: 8 }); \
                   match e { Held::Wrapped(i) => i.a, Held::Empty => 0 } }",
            expect: 8,
        },
        Shape {
            // A const parameter ERASED to a literal at monomorphization, where
            // the erased value then feeds an array SIZE. The width analyses see
            // no symbolic constant, so this checks the erased size against the
            // descriptor rather than against the source.
            name: "const-generic array length",
            src: "fn first<const n: Word>(a: [Word; n]) -> Word { a[0] }\n\
                  fn main() -> Word { first::<2>([4, 5]) }",
            expect: 4,
        },
        Shape {
            // A MULTI-LIMB value over the word: the only shape whose
            // representation is a count of words rather than a single one.
            name: "multiword limb index",
            src: "fn low<const n: Word>(m: Multiword<n>) -> Word { m[0] }\n\
                  fn main() -> Word { low::<2>((7, 0) as Multiword<2>) }",
            expect: 7,
        },
        Shape {
            // Stride NESTED rather than composed: the outer index multiplies an
            // element size that is itself an array's total size.
            name: "array of arrays",
            src: "fn main() -> Word { let a = [[1, 2], [3, 4]]; a[0][1] * 10 + a[1][0] }",
            expect: 23,
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
    let mut not_run: Vec<String> = Vec::new();
    let (mut ran, mut compile_refused, mut load_refused, mut other) = (0, 0, 0, 0);

    for (label, target) in targets() {
        for shape in corpus() {
            match run_cell(&target, &shape) {
                Cell::Ran => ran += 1,
                Cell::CompileRefused => {
                    compile_refused += 1;
                    // **NAME THE SHAPE, NOT JUST THE COUNT.** A shape refused
                    // under every descriptor contributes no evidence, and a bare
                    // total cannot distinguish that from one refused nowhere.
                    not_run.push(format!("{label} / {} (compile)", shape.name));
                }
                Cell::LoadRefused => {
                    load_refused += 1;
                    not_run.push(format!("{label} / {} (load)", shape.name));
                }
                Cell::OtherFault(m) => {
                    other += 1;
                    not_run.push(format!("{label} / {}: {m}", shape.name));
                }
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
         unsafe, but the sweep's shape has changed and the document describing it is now stale. \
         The cells that did not run:\n{}",
        targets().len() * corpus().len(),
        not_run.join("\n")
    );
}

// ---------------------------------------------------------------------------
// THE SECOND AUTHORITY
// ---------------------------------------------------------------------------
//
// Every width is carried TWICE: by the module header that `compile_with_target`
// writes, and by the runtime type parameters of `GenericVm<W, A, F>`. The load
// check refuses a module WIDER than the runtime and admits one NARROWER, and
// that asymmetry is where the second authority becomes visible. It is where the
// original opaque-width defect lived.
//
// The sweep above varies the module and holds the runtime at the build's
// default `Vm`. So it exercises the narrower-module direction only at whichever
// runtime the build happens to provide.
//
// `Word` is implemented for `i8`, `i16`, `i32` and `i64`, and `Address` for
// `u8`, `u16`, `u32` and `u64`, all unconditionally. **Sixteen runtime pairs are
// therefore constructible in the DEFAULT build**, with no `narrow-*` feature.
// `tests/composite_width_skew.rs` already relies on that for two hand-picked
// runtimes; this generalises it from two points to the whole grid.

/// One cell, on an explicitly chosen runtime rather than the build's default.
///
/// The natives are built through the width traits rather than from literals,
/// because a literal `100` would infer to the runtime's word only by accident
/// of it fitting, and would stop compiling the moment a narrower word joined
/// the grid.
fn run_cell_on<
    W: keleusma::word::Word,
    A: keleusma::address::Address,
    F: keleusma::float::Float,
>(
    target: &Target,
    shape: &Shape,
) -> Cell {
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
    let Ok(mut vm) = keleusma::vm::GenericVm::<'_, '_, W, A, F>::new(module, &arena) else {
        return Cell::LoadRefused;
    };
    vm.register_native("make_handle", |_args| {
        Ok(GenericValue::Opaque(host_arc(Handle)))
    });
    vm.register_native("handle_val", |_args| {
        Ok(GenericValue::Int(W::from_i64_wrap(100)))
    });
    match vm.call(&[]) {
        Ok(keleusma::vm::GenericVmState::Finished(GenericValue::Int(n))) => {
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

/// Tally for one runtime across the whole descriptor and shape space.
#[derive(Default)]
struct Tally {
    ran: usize,
    /// How many cells SHOULD load, computed from the documented rule rather
    /// than from the loader. See the closed-form check at the end of the grid.
    predicted: usize,
    compile_refused: usize,
    load_refused: usize,
    other: usize,
    findings: Vec<String>,
    wrong: Vec<String>,
}

fn sweep_runtime<
    W: keleusma::word::Word,
    A: keleusma::address::Address,
    F: keleusma::float::Float,
>(
    rt: &str,
    t: &mut Tally,
) {
    for (label, target) in targets() {
        // **THE INDEPENDENT PATH.** The loader's rule is that a module is
        // admitted when no declared width exceeds the runtime's. Evaluating
        // that here, from the descriptor and the runtime's own trait constants,
        // gives a prediction the loader never sees. A harness that silently
        // skipped a runtime, or a refusal arriving from some check other than
        // the width one, would break the agreement.
        if target.word_bits_log2 <= W::BITS_LOG2
            && target.addr_bits_log2 <= A::BITS_LOG2
            && (!target.has_floats || target.float_bits_log2 <= F::BITS_LOG2)
        {
            t.predicted += corpus().len();
        }
        for shape in corpus() {
            match run_cell_on::<W, A, F>(&target, &shape) {
                Cell::Ran => t.ran += 1,
                Cell::CompileRefused => t.compile_refused += 1,
                Cell::LoadRefused => t.load_refused += 1,
                Cell::OtherFault(_) => t.other += 1,
                Cell::WrongAnswer(got) => t.wrong.push(format!(
                    "runtime {rt} / {label} / {}: expected {}, got {got}",
                    shape.name, shape.expect
                )),
                Cell::InvalidBytecodeAtRuntime(m) => t
                    .findings
                    .push(format!("runtime {rt} / {label} / {}: {m}", shape.name)),
            }
        }
    }
}

/// **THE GRID.** Every implemented word paired with every implemented address.
///
/// The pairs are concrete Rust types, so the types themselves must be written
/// out; what is NOT written out is any width number beside them. Each runtime's
/// widths come from the trait, so a change to either family cannot leave a stale
/// number here.
macro_rules! grid {
    ($t:expr, $( ($w:ty, $a:ty) ),+ $(,)?) => {
        $(
            sweep_runtime::<$w, $a, f64>(
                &format!(
                    "w{}/a{}",
                    <$w as keleusma::word::Word>::BITS_LOG2,
                    <$a as keleusma::address::Address>::BITS_LOG2
                ),
                $t,
            );
        )+
    };
}

/// The same question as the sweep above, asked of the OTHER authority.
///
/// A load refusal here is the guarantee WORKING: a module declaring a width
/// wider than the runtime must be refused, and most of this grid is exactly
/// that. Only a module that compiled, loaded, and then faulted is a finding.
#[test]
fn no_runtime_and_module_width_pair_reaches_invalid_bytecode() {
    let mut t = Tally::default();
    grid!(
        &mut t,
        (i8, u8),
        (i8, u16),
        (i8, u32),
        (i8, u64),
        (i16, u8),
        (i16, u16),
        (i16, u32),
        (i16, u64),
        (i32, u8),
        (i32, u16),
        (i32, u32),
        (i32, u64),
        (i64, u8),
        (i64, u16),
        (i64, u32),
        (i64, u64),
    );

    // **BOTH DIRECTIONS OF NON-VACUITY.** Cells must actually run, and cells
    // must actually be refused -- unlike the single-runtime sweep, where
    // requiring a refusal was an unmeasured assumption, here a refusal is
    // GUARANTEED by the load check for any module wider than its runtime, and
    // its absence would mean the grid had collapsed to one runtime.
    assert!(
        t.ran > 0,
        "no cell ran on any runtime, so this grid measured nothing"
    );
    assert!(
        t.load_refused > 0,
        "no module was refused at load anywhere in a grid that pairs narrow runtimes with \
         wide modules; the load-time width check is not being exercised"
    );
    assert!(
        t.findings.is_empty(),
        "a module that COMPILED and LOADED raised InvalidBytecode on some runtime:\n{}",
        t.findings.join("\n")
    );
    assert!(
        t.wrong.is_empty(),
        "a module that compiled and loaded returned the WRONG ANSWER on some runtime, which \
         nothing else reports:\n{}",
        t.wrong.join("\n")
    );

    // **THE RAN COUNT IS PREDICTED BY AN INDEPENDENT PATH.**
    //
    // The first version of this check used a closed form: the product of two
    // triangular numbers, on the assumption that the runtime grid and the
    // descriptor set span the same widths. **They do not.** The grid is over
    // CONCRETE Rust types and is the same in every build, while the descriptor
    // set shrinks with the build's maxima. The form was right at the default
    // build and wrong at all four narrow selectors, which is where it failed --
    // 2730 ran against 1170 predicted under `narrow-word-16`.
    //
    // The prediction now comes from evaluating the loader's documented rule per
    // cell, which holds in every build because it makes no assumption about how
    // the two sets relate. Agreement tests the HARNESS, not the runtime.
    assert_eq!(
        t.ran, t.predicted,
        "the number of cells that LOADED disagrees with the documented \
         module-no-wider-than-runtime rule: {} ran, {} predicted, {} refused at load. Either the \
         harness is not pairing what it claims to pair, or a refusal came from a check other \
         than the width one",
        t.ran, t.predicted, t.load_refused
    );
}

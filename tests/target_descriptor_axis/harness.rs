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
//!
//! # Why this is a shared harness rather than one test file
//!
//! Until 2026-10-30 the sweep and the grid lived in one file,
//! `tests/target_descriptor_axis.rs`. The grid instantiates `GenericVm` once for
//! each of thirty-two runtimes, and that binary took between 594 and 659 seconds
//! to compile, which made it the last to finish in every test build. The
//! operator adopted option (a) of decision K4a in
//! `docs/process/kaizen/2026-10-30-open-decisions.md`, which splits the grid into
//! four binaries, one per word type, each covering eight runtimes. This module
//! holds everything those four binaries share, and each binary keeps the
//! original `cfg` gate, so the feature coverage is unchanged.
//!
//! The four binaries are named in [`GRID_FILES`], and
//! [`assert_every_grid_file_is_present`] fails when one of them is removed, so the
//! grid cannot lose a word type silently.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use keleusma::bytecode::GenericValue;
use keleusma::compiler::compile_with_target;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::target::Target;
use keleusma::vm::VmError;
use keleusma::{Arena, HostOpaque, host_arc};

pub struct Handle;
impl HostOpaque for Handle {
    fn type_name(&self) -> &'static str {
        "Handle"
    }
}

/// The narrowest widths any runtime implements, from the trait impls.
pub const WORD_FLOOR: u8 = <i8 as keleusma::word::Word>::BITS_LOG2;
pub const ADDR_FLOOR: u8 = <u8 as keleusma::address::Address>::BITS_LOG2;

pub const PRELUDE: &str = "use make_handle() -> Handle\nuse handle_val(Handle) -> Word\n";

/// A program and the value it must produce wherever it runs at all.
///
/// **Every expected value is small enough to exist in an EIGHT-BIT word.** A
/// corpus whose answers overflow at the narrow end would report a legitimate
/// arithmetic trap as though it were an artefact defect, which is the failure
/// mode that makes a sweep like this useless.
pub struct Shape {
    pub name: &'static str,
    pub src: &'static str,
    pub expect: i64,
    /// Whether the program cannot compile for a target that declares no floats.
    ///
    /// **This exists because the prediction below must model what each shape
    /// NEEDS, not only what each descriptor offers.** Before a float-using
    /// shape joined the corpus, every shape ran under every admissible
    /// descriptor and the prediction could ignore the distinction. It cannot
    /// now, and a prediction that ignored it would report a correct refusal as
    /// a harness fault.
    pub needs_floats: bool,
}

pub fn corpus() -> Vec<Shape> {
    #[allow(unused_mut)]
    let mut v = alloc::vec![
        Shape {
            name: "scalar arithmetic",
            src: "fn main() -> Word { 1 + 2 }",
            expect: 3,
            needs_floats: false,
        },
        Shape {
            name: "word field after an opaque",
            src: "struct P { h: Handle, n: Word }\n\
                  fn main() -> Word { let p = P { h: make_handle(), n: 7 }; p.n }",
            expect: 7,
            needs_floats: false,
        },
        Shape {
            name: "nested composite child",
            src: "struct Inner { h: Handle, a: Word }\n\
                  struct Outer { i: Inner, b: Word }\n\
                  fn main() -> Word { let o = Outer { i: Inner { h: make_handle(), a: 3 }, b: 5 }; \
                   o.i.a * 10 + o.b }",
            expect: 35,
            needs_floats: false,
        },
        Shape {
            name: "array stride over opaque-bearing elements",
            src: "struct P { h: Handle, n: Word }\n\
                  fn main() -> Word { let a = [P { h: make_handle(), n: 1 }, \
                   P { h: make_handle(), n: 2 }]; a[1].n * 10 + a[0].n }",
            expect: 21,
            needs_floats: false,
        },
        Shape {
            name: "enum payload after a discriminant",
            src: "enum Held { Wrapped(Handle, Word), Empty }\n\
                  fn main() -> Word { let e = Held::Wrapped(make_handle(), 9); \
                   match e { Held::Wrapped(h, n) => n, Held::Empty => 0 } }",
            expect: 9,
            needs_floats: false,
        },
        Shape {
            name: "tuple with fields after an opaque",
            src: "fn main() -> Word { let t = (make_handle(), 3, 4); t.1 * 2 + t.2 }",
            expect: 10,
            needs_floats: false,
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
            needs_floats: false,
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
            needs_floats: false,
        },
        Shape {
            // Stride COMPOSED with a field offset: the array's element size is
            // multiplied inside a body that already has an opaque ahead of it.
            name: "array inside a struct after an opaque",
            src: "struct S { h: Handle, a: [Word; 2], n: Word }\n\
                  fn main() -> Word { let s = S { h: make_handle(), a: [4, 5], n: 6 }; \
                   s.a[1] * 10 + s.n }",
            expect: 56,
            needs_floats: false,
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
            needs_floats: false,
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
            needs_floats: false,
        },
        Shape {
            // A MULTI-LIMB value over the word: the only shape whose
            // representation is a count of words rather than a single one.
            name: "multiword limb index",
            src: "fn low<const n: Word>(m: Multiword<n>) -> Word { m[0] }\n\
                  fn main() -> Word { low::<2>((7, 0) as Multiword<2>) }",
            expect: 7,
            needs_floats: false,
        },
        Shape {
            // Stride NESTED rather than composed: the outer index multiplies an
            // element size that is itself an array's total size.
            name: "array of arrays",
            src: "fn main() -> Word { let a = [[1, 2], [3, 4]]; a[0][1] * 10 + a[1][0] }",
            expect: 23,
            needs_floats: false,
        },
    ];
    // THE THIRD WIDTH, USED RATHER THAN ONLY DECLARED.
    //
    // Without this shape the float width affects only ADMISSIBILITY: a module
    // declaring a float the runtime cannot host is refused, and nothing else in
    // the corpus ever places a float in a body. Here one sits between the
    // opaque and the word that follows it, so the float width enters a computed
    // OFFSET exactly as the address width does through the opaque.
    //
    // **The value read back is the integer field, never the float.** A shape
    // whose expected value were a computed float would report a rounding
    // difference between an `f32` and an `f64` runtime as a wrong answer, which
    // is a width difference the sweep is not entitled to call a defect.
    #[cfg(feature = "floats")]
    v.push(Shape {
        name: "word field after an opaque and a Float",
        src: "struct P { h: Handle, f: Float, n: Word }\n\
              fn main() -> Word { let p = P { h: make_handle(), f: 2Float, n: 7 }; p.n }",
        expect: 7,
        needs_floats: true,
    });
    v
}

/// What one (target, program) cell did.
#[derive(Debug, PartialEq, Eq)]
pub enum Cell {
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

/// Every descriptor the compiler could accept, derived from the runtime's own
/// maxima. `has_floats` is swept too, because the float floor is enforced in a
/// different function from the other two and the flag gates it.
pub fn targets() -> Vec<(String, Target)> {
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
// The default-runtime sweep, which lives in `tests/target_descriptor_axis_i64.rs`
// since the split, varies the module and holds the runtime at the build's
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
pub fn run_cell_on<
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
pub struct Tally {
    pub ran: usize,
    /// How many cells SHOULD load, computed from the documented rule rather
    /// than from the loader. See the closed-form check at the end of the grid.
    pub predicted: usize,
    /// How many cells the same rule says the loader must REFUSE.
    ///
    /// Added when the grid was split across four binaries, one per word type, so that each
    /// binary can state its own refusal expectation rather than inherit the whole grid's.
    pub predicted_refused: usize,
    pub compile_refused: usize,
    pub load_refused: usize,
    pub other: usize,
    pub findings: Vec<String>,
    pub wrong: Vec<String>,
}

pub fn sweep_runtime<
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
            // A shape that needs floats does not run where the descriptor
            // declares none, and that refusal comes from the COMPILER rather
            // than the loader. The prediction has to model the shape's own
            // requirement or it would count a correct refusal as a fault.
            t.predicted += corpus()
                .iter()
                .filter(|s| !s.needs_floats || target.has_floats)
                .count();
        } else {
            // The complement of the same rule. Every shape the compiler admits for this
            // descriptor yields a module the loader must refuse on this runtime.
            t.predicted_refused += corpus()
                .iter()
                .filter(|s| !s.needs_floats || target.has_floats)
                .count();
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

/// The label for one runtime, built from the width traits so no number is
/// written beside a type.
pub fn rt_label<
    W: keleusma::word::Word,
    A: keleusma::address::Address,
    F: keleusma::float::Float,
>() -> String {
    format!(
        "w{}/a{}/f{}",
        W::BITS_LOG2,
        A::BITS_LOG2,
        <F as keleusma::float::Float>::BITS_LOG2
    )
}

/// Both float runtimes for one (word, address) pair.
///
/// **The float is the third authority and the grid held it fixed until
/// 2026-09-10.** A module declaring a sixty-four-bit float against an `f32`
/// runtime must be refused at load, the same asymmetry already swept on the
/// word and the address.
fn sweep_pair<W: keleusma::word::Word, A: keleusma::address::Address>(t: &mut Tally) {
    sweep_runtime::<W, A, f64>(&rt_label::<W, A, f64>(), t);
    sweep_runtime::<W, A, f32>(&rt_label::<W, A, f32>(), t);
}

/// **ONE ROW OF THE GRID.** One implemented word paired with every implemented
/// address, each on both float runtimes, which is eight runtimes.
///
/// `Address` is implemented for `u8`, `u16`, `u32` and `u64`, all
/// unconditionally. The address types are written out here once, for every
/// word, so the four binaries cannot disagree about which addresses a row
/// covers. Each runtime's widths come from the trait, so a change to either
/// family cannot leave a stale number here.
pub fn sweep_word<W: keleusma::word::Word>(t: &mut Tally) {
    sweep_pair::<W, u8>(t);
    sweep_pair::<W, u16>(t);
    sweep_pair::<W, u32>(t);
    sweep_pair::<W, u64>(t);
}

/// The four grid binaries, one per implemented word type, as paths relative to
/// the package root, each with the call that sweeps its row.
///
/// `Word` is implemented for `i8`, `i16`, `i32` and `i64`, all unconditionally,
/// and each binary sweeps one of them through [`sweep_word`]. Together they
/// cover the sixteen (word, address) pairs on both float runtimes, which is the
/// thirty-two runtimes the single file covered before the split.
pub const GRID_FILES: [(&str, &str); 4] = [
    ("tests/target_descriptor_axis_i8.rs", "sweep_word::<i8>"),
    ("tests/target_descriptor_axis_i16.rs", "sweep_word::<i16>"),
    ("tests/target_descriptor_axis_i32.rs", "sweep_word::<i32>"),
    ("tests/target_descriptor_axis_i64.rs", "sweep_word::<i64>"),
];

/// **THE GRID CANNOT SHRINK SILENTLY.**
///
/// Each binary sees only its own row, so no single binary can count the whole
/// grid. This check reads every sibling source and requires each to exist and
/// to call [`sweep_word`] for its word type, so removing a binary, or leaving
/// one that no longer sweeps its row, fails in each of the others.
pub fn assert_every_grid_file_is_present() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut missing = Vec::new();
    for (file, call) in GRID_FILES {
        match std::fs::read_to_string(root.join(file)) {
            Ok(text) if text.contains(call) => {}
            Ok(_) => missing.push(format!("{file} does not call {call}")),
            Err(e) => missing.push(format!("{file} is unreadable: {e}")),
        }
    }
    assert!(
        missing.is_empty(),
        "the runtime grid has lost a row, so it no longer pairs every implemented word \
         with every implemented address:\n{}",
        missing.join("\n")
    );
}

/// The same question as the default-runtime sweep, asked of the OTHER authority,
/// for one row of the grid.
///
/// A load refusal here is the guarantee WORKING. A module declaring a width
/// wider than the runtime must be refused, and most of this grid is exactly
/// that. Only a module that compiled, loaded, and then faulted is a finding.
///
/// `require_refusal` carries the unconditional refusal claim the whole grid made
/// before the split. The narrowest row refuses every module that any row
/// refuses, because the width rule is monotone in each runtime width, so the
/// whole grid saw a refusal exactly when the `i8` row does. That row passes
/// `true`. Every row also checks its refusal count against the independent
/// prediction in both directions.
pub fn assert_grid_row(t: &Tally, word: &str, require_refusal: bool) {
    // The reach of this row, printed so a reader can sum the four rows and compare
    // the total with the grid's figure before the split.
    eprintln!(
        "{word} row: {} ran, {} predicted, {} refused at load, {} predicted refused, \
         {} compile-refused, {} other",
        t.ran, t.predicted, t.load_refused, t.predicted_refused, t.compile_refused, t.other
    );
    // **BOTH DIRECTIONS OF NON-VACUITY.** Cells must actually run, and cells
    // must actually be refused -- unlike the single-runtime sweep, where
    // requiring a refusal was an unmeasured assumption, here a refusal is
    // GUARANTEED by the load check for any module wider than its runtime, and
    // its absence would mean the grid had collapsed to one runtime.
    assert!(
        t.ran > 0,
        "no cell ran on any {word} runtime, so this row measured nothing"
    );
    if require_refusal {
        assert!(
            t.load_refused > 0,
            "no module was refused at load anywhere in a grid that pairs narrow runtimes \
             with wide modules; the load-time width check is not being exercised"
        );
    }
    assert_eq!(
        t.load_refused > 0,
        t.predicted_refused > 0,
        "the {word} row's load refusals disagree with the documented \
         module-no-wider-than-runtime rule: {} refused at load, {} predicted",
        t.load_refused,
        t.predicted_refused
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
        "the number of cells that LOADED on the {word} row disagrees with the documented \
         module-no-wider-than-runtime rule: {} ran, {} predicted, {} refused at load. Either the \
         harness is not pairing what it claims to pair, or a refusal came from a check other \
         than the width one",
        t.ran, t.predicted, t.load_refused
    );
}

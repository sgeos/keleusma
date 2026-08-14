//! Differential tests for `src/selfhost/kel/verify_types.kel`, the self-hosted
//! type-REJECTION stage.
//!
//! # Slice 0: the harness, before any rule
//!
//! The stage accepts everything, and that is the point rather than a
//! placeholder. The corpus must then show **every well-typed case agreeing and
//! every ill-typed case DISAGREEING**. A harness that reports success here is
//! broken, and finding that out costs nothing before a rule exists and a great
//! deal afterwards.
//!
//! # The oracle
//!
//! **Verdict agreement. Accept versus reject.** Not message agreement, which
//! would commit the stage to reproducing English the reference is free to
//! reword. This is what the `verify_*.kel` family already uses.
//!
//! # The direction that is not symmetric
//!
//! `verify_structural` and friends may over-approximate and defer to a runtime
//! guard. A type checker may not: **rejecting a valid program is a language
//! change**, not a conservative choice. The well-typed side of this corpus
//! therefore grows with every slice rather than staying at the five controls the
//! sizing spike used, because the "must accept" obligation is unbounded by any
//! corpus while the "must reject" obligation is enumerable.
//!
//! # Why a corpus of rejections alone would be useless
//!
//! It cannot detect a checker that rejects everything, which would score
//! perfectly. The sizing spike recorded the converse mistake, made while
//! building it: a case labelled ill-typed that was in fact well-typed, reported
//! as "accepted but should not be". It did not mislead **only because explicit
//! well-typed controls existed to check it against**.
#![cfg(all(
    feature = "compile",
    feature = "verify",
    not(feature = "narrow-word-8"),
    not(feature = "narrow-word-16"),
    not(feature = "narrow-word-32")
))]

use keleusma::Arena;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, required_persistent_capacity_for};

const TYPES_KEL: &str = include_str!("../src/selfhost/kel/verify_types.kel");

/// The fifteen rejection shapes, measured by execution rather than counted from
/// `TypeError` sites.
///
/// Reading gives 163 `TypeError::new` sites in `src/typecheck.rs`, about twenty
/// of which mention traits or bounds and are outside Order-1 scope. **The number
/// that matters is how many an ill-typed program in the SUBSET can reach**, and
/// that needed execution: eighteen ill-typed programs, seventeen rejected.
const ILL_TYPED: &[(&str, &str)] = &[
    ("add-word-and-bool", "fn main() -> Word { 1 + true }"),
    ("body-versus-return", "fn main() -> Word { true }"),
    (
        "wrong-argument-type",
        "fn f(a: Word) -> Word { a }\nfn main() -> Word { f(true) }",
    ),
    (
        "too-few-arguments",
        "fn f(a: Word, b: Word) -> Word { a + b }\nfn main() -> Word { f(1) }",
    ),
    (
        "too-many-arguments",
        "fn f(a: Word) -> Word { a }\nfn main() -> Word { f(1, 2) }",
    ),
    ("undefined-function", "fn main() -> Word { nope(1) }"),
    ("undefined-identifier", "fn main() -> Word { nope }"),
    (
        "if-branches-differ",
        "fn main() -> Word { if true { 1 } else { false } }",
    ),
    (
        "non-bool-condition",
        "fn main() -> Word { if 1 { 1 } else { 2 } }",
    ),
    (
        "unknown-field",
        "struct P { a: Word }\nfn main() -> Word { let p = P { a: 1 }; p.b }",
    ),
    (
        "wrong-field-count",
        "struct P { a: Word, b: Word }\nfn main() -> Word { let p = P { a: 1 }; p.a }",
    ),
    (
        "index-a-scalar",
        "fn main() -> Word { let x: Word = 1; x[0] }",
    ),
    (
        "field-access-on-a-scalar",
        "fn main() -> Word { let x: Word = 1; x.a }",
    ),
    (
        "byte-against-word-argument",
        "fn f(a: Byte) -> Byte { a }\nfn main() -> Word { f(1) as Word }",
    ),
    (
        "array-elements-differ",
        "fn main() -> Word { let a = [1, true]; 0 }",
    ),
    // THE ODD ONE OUT. A V0.2.0 surface restriction rather than a type error,
    // and the one rejection of the fifteen that carries no `type error:`
    // prefix. A stage locating rejections by that prefix would miss it, and a
    // stage reproducing the reference's routing would be reproducing English.
    (
        "calling-a-local",
        "fn g() -> Word { 1 }\nfn main() -> Word { let f = g; f() }",
    ),
];

/// The controls. **Without these the corpus cannot detect a checker that rejects
/// everything**, which would score perfectly against the table above.
///
/// They are deliberately varied rather than minimal: each reaches a construct
/// some ill-typed case is a near-miss of, so a rule written too broadly fails
/// here rather than passing quietly.
const WELL_TYPED: &[(&str, &str)] = &[
    ("scalar-arith", "fn main() -> Word { 1 + 2 * 3 }"),
    (
        "bool-condition",
        "fn main() -> Word { if true { 1 } else { 2 } }",
    ),
    (
        "matching-argument",
        "fn f(a: Word) -> Word { a }\nfn main() -> Word { f(1) }",
    ),
    (
        "struct-field-read",
        "struct P { a: Word, b: Word }\nfn main() -> Word { let p = P { a: 1, b: 2 }; p.a }",
    ),
    (
        "byte-to-byte",
        "fn f(a: Byte) -> Byte { a }\nfn main() -> Word { f(1 as Byte) as Word }",
    ),
    (
        "array-of-one-type",
        "fn main() -> Word { let a = [1, 2, 3]; a[0] }",
    ),
    (
        "shared-data-word-field",
        "shared data s { n: Word }\nfn main() -> Word { s.n }",
    ),
];

/// The reference verdict: does `compile` accept this source?
///
/// **This is the oracle, and it is deliberately the WHOLE pipeline** rather than
/// `typecheck::check` alone. "Calling a local" is rejected by the compiler as a
/// surface restriction rather than by the type-check pass, so an oracle narrowed
/// to the type checker would report that case as accepted and the corpus would
/// be wrong about its own contents.
fn reference_accepts(src: &str) -> bool {
    match tokenize(src) {
        Err(_) => false,
        Ok(toks) => match parse(&toks) {
            Err(_) => false,
            Ok(ast) => compile(&ast).is_ok(),
        },
    }
}

/// The stage's verdict. Slice 0 has no input surface, so the program is not
/// passed: the stage cannot see it and says so by accepting.
fn stage_accepts() -> bool {
    let module = compile(&parse(&tokenize(TYPES_KEL).expect("lex")).expect("parse"))
        .expect("verify_types.kel compiles");
    let need = required_persistent_capacity_for(&module);
    let arena = Box::leak(Box::new(Arena::with_capacity(
        DEFAULT_ARENA_CAPACITY + need,
    )));
    arena
        .resize_persistent(need)
        .expect("arena persistent region");
    let mut vm = Vm::new(module, arena).expect("verify");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    let out = vm
        .call_with_shared(&mut shared, &[keleusma::bytecode::Value::Int(0)])
        .expect("run");
    match out {
        keleusma::vm::VmState::Finished(keleusma::bytecode::Value::Int(v)) => v == 0,
        other => panic!("verify_types.kel returned {other:?}, not a finished Int verdict"),
    }
}

/// THE CORPUS IS CHECKED AGAINST THE REFERENCE BEFORE ANYTHING ELSE IS.
///
/// A case labelled ill-typed that the reference accepts is a badly-constructed
/// test, not a compiler defect, and the sizing spike made exactly that mistake
/// once. Catching it here means a later slice's failure is about the stage.
#[test]
fn the_corpus_labels_agree_with_the_reference() {
    for (label, src) in ILL_TYPED {
        assert!(
            !reference_accepts(src),
            "{label}: labelled ill-typed and the reference ACCEPTS it, so the corpus is wrong \
             about its own contents"
        );
    }
    for (label, src) in WELL_TYPED {
        assert!(
            reference_accepts(src),
            "{label}: labelled well-typed and the reference REJECTS it, so the corpus is wrong \
             about its own contents"
        );
    }

    // MUST-FIRE on the corpus being non-empty in both directions. A table
    // emptied by a later edit would leave every loop above vacuous.
    assert!(ILL_TYPED.len() >= 15, "the rejection corpus shrank");
    assert!(!WELL_TYPED.is_empty(), "the control corpus is empty");
}

/// SLICE 0: the harness discriminates, and the stage does not yet.
///
/// The stage accepts everything, so agreement must be **exactly** the well-typed
/// set. If this test ever reports full agreement, the harness is comparing
/// something other than what it claims.
#[test]
fn the_accepting_stage_agrees_on_every_control_and_on_no_rejection() {
    let stage = stage_accepts();
    assert!(
        stage,
        "the slice-0 stage rejected, and it is incapable of rejecting, so the harness is reading \
         something other than the stage's verdict"
    );

    let mut agreed = 0;
    let mut disagreed = 0;
    for (_, src) in WELL_TYPED {
        if reference_accepts(src) == stage {
            agreed += 1;
        }
    }
    for (_, src) in ILL_TYPED {
        if reference_accepts(src) != stage {
            disagreed += 1;
        }
    }

    assert_eq!(
        agreed,
        WELL_TYPED.len(),
        "an accepting stage must agree with the reference on every well-typed case"
    );
    assert_eq!(
        disagreed,
        ILL_TYPED.len(),
        "an accepting stage must DISAGREE on every ill-typed case. Full agreement here would mean \
         the harness is not comparing verdicts at all, which is the failure this slice exists to \
         rule out before any rule is written."
    );
}

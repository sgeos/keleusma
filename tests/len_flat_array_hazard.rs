//! The compiler must never emit `Op::Len`, and this file is the ratchet on that.
//!
//! # What this file used to pin, and what changed
//!
//! It recorded a LATENT trap. `static_for_in_length` had no `Expr::If` arm, so
//! `for x in if c { a } else { b }` fell through to a dynamic path that emitted
//! `Op::Len`. The virtual machine refuses that opcode on a flat array body,
//! `verify()` accepted the module, and what held the trap shut was the
//! resource-bound extractor refusing a loop with no statically extractable
//! bound -- a refusal in the LIFTABLE category of this project's
//! conservative-verification taxonomy. Lifting it, a desirable improvement
//! someone would make with no reason to look at `Op::Len`, would have turned a
//! rejected program into one that loads and traps.
//!
//! **Both emission sites are now gone.** The for-in bound and the
//! checked-index bounds check each fold the length to a constant or fail with a
//! compile error; neither emits the opcode. The sequence this file pins
//! therefore CHANGED rather than disappeared, which is what its previous
//! revision instructed a reader to record.
//!
//! # The second hazard, which was not latent
//!
//! Measured 2026-09-04 while building the floor: the checked-index construct
//! over a `Multiword` compiled, passed `verify()`, LOADED, and trapped
//! `InvalidBytecode` at run time. The bounds check folded its length through a
//! helper that answers only for array types, and a `Multiword` is not one, so
//! it fell back to `Op::Len` over a flat multi-word body.
//!
//! **That one was reachable with no lifted refusal and no future change.** The
//! array trap needed someone to improve the bound extractor first; this one
//! needed nothing. It is pinned below in the direction of working, because the
//! repair was to fold the multi-word length rather than to refuse.
//!
//! # What is deliberately NOT claimed
//!
//! Not that `Op::Len` is unreachable. The virtual machine keeps its refusals,
//! because a corrupt or hand-built module can still carry the opcode, and this
//! file makes no claim about that path. What is claimed is narrower and
//! checkable: **the compiler has no emission site**, and no source form tried
//! here produces one.

#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::Arena;
use keleusma::bytecode::Value;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm};

/// The historical witness: an `if` expression in iterable position.
const IF_ITERABLE: &str = "data s { n: Word }\n\
     fn main(k: Word) -> Word { let a = [1, 2, 3]; let b = [4, 5, 6]; \
     for x in if true { a } else { b } { s.n = x; } s.n }";

/// The control: an ordinary array iterable.
const PLAIN_ITERABLE: &str = "data s { n: Word }\n\
     fn main(k: Word) -> Word { let a = [1, 2, 3]; for x in a { s.n = x; } s.n }";

/// Compile, then report whether `Op::Len` appears anywhere in the module.
fn emits_len(src: &str) -> bool {
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    format!(
        "{:?}",
        module.chunks.iter().map(|c| &c.ops).collect::<Vec<_>>()
    )
    .contains("Len")
}

/// Compile, verify, load and run, returning the returned word.
///
/// Every step is checked rather than unwrapped blindly, because the defect
/// class this file guards produced a module that passed the first three and
/// died at the fourth.
fn run_to_word(src: &str) -> i64 {
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    keleusma::verify::verify(&module).expect("verify");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("load");
    let mut shared = alloc::vec![0u8; vm.shared_data_bytes()];
    match vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("run")
    {
        keleusma::vm::VmState::Finished(Value::Int(n)) => n,
        other => panic!("expected a finished word, got {other:?}"),
    }
}

extern crate alloc;

/// **The ratchet.** The historical witness compiles, and it emits no `Op::Len`.
///
/// If this fails, an emission site has returned. The virtual machine refuses
/// the opcode on a flat body, so a module carrying it is one that can load and
/// then trap -- the class `verify()` exists to exclude.
#[test]
fn the_if_expression_iterable_emits_no_len() {
    assert!(
        !emits_len(IF_ITERABLE),
        "the if-expression iterable emits Op::Len again. The virtual machine refuses \
         that opcode on a flat array body, so this module can load and then trap. \
         Fold the length at the emission site rather than deferring to run time."
    );
}

/// The control, which keeps the test above from being satisfied by a compiler
/// that emits nothing at all.
#[test]
fn a_plain_array_iterable_emits_no_len_either() {
    assert!(
        !emits_len(PLAIN_ITERABLE),
        "a plain array iterable emits Op::Len"
    );
}

/// **The bound is not merely present, it is RIGHT.**
///
/// A folded length that is too small iterates too few times and reports a
/// smaller last element; one that is too large indexes past the array and
/// traps. Either failure is visible here, which is what separates this from
/// asserting that the opcode is absent.
///
/// A wrong bound would be worse than the trap this file used to pin: a trap is
/// loud, and a wrong iteration count is silent and is consumed by the
/// worst-case execution time analysis.
#[test]
fn the_folded_bound_equals_the_true_element_count() {
    // `[1, 2, 3]`: the last element written is 3 exactly when three iterations run.
    assert_eq!(
        run_to_word(IF_ITERABLE),
        3,
        "the then-branch array is [1, 2, 3]"
    );
    assert_eq!(
        run_to_word(PLAIN_ITERABLE),
        3,
        "the plain array is [1, 2, 3]"
    );

    // The else branch, so the fold is not accidentally reading only one side.
    let else_side = "data s { n: Word }\n\
         fn main(k: Word) -> Word { let a = [1, 2, 3]; let b = [4, 5, 6]; \
         for x in if false { a } else { b } { s.n = x; } s.n }";
    assert_eq!(
        run_to_word(else_side),
        6,
        "the else-branch array is [4, 5, 6]"
    );
}

/// **Why taking the length from either branch is sound.**
///
/// The fold reads the `if` expression's own type, and the type checker refuses
/// an `if` whose branches disagree. So there is no program in which the two
/// branches have different lengths and the fold must choose. This is measured
/// rather than assumed, because "the type checker surely forbids that" is the
/// shape of assumption that produced a false comment in the virtual machine.
#[test]
fn branches_of_differing_length_are_refused_by_the_type_checker() {
    let src = "data s { n: Word }\n\
         fn main(k: Word) -> Word { let a = [1, 2, 3]; let b = [4, 5, 6, 7, 8]; \
         for x in if true { a } else { b } { s.n = x; } s.n }";
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let err = compile(&program).expect_err("branches of differing array length must not compile");
    assert!(
        err.message.contains("differing types"),
        "the refusal changed identity. The fold's soundness rests on branches agreeing \
         on type; whatever refuses now must be at least as strong. Got: {}",
        err.message
    );
}

/// **The second hazard, pinned in the direction of working.**
///
/// This program compiled, verified, loaded, and trapped `InvalidBytecode`
/// before the multi-word length was folded. It now returns the low word.
#[test]
fn a_checked_index_on_a_multiword_runs_instead_of_trapping() {
    let src = "fn main(k: Word) -> Word { let m = (7, 0) as Multiword<2>; \
         m[0] { ok(v) => v, invalid_index(i) => 0 - 1 } }";
    assert!(
        !emits_len(src),
        "the multi-word bounds check emits Op::Len again"
    );
    assert_eq!(
        run_to_word(src),
        7,
        "the low word of Multiword<2>(7, 0) is 7; a trap here is the InvalidBytecode \
         regression this test exists for"
    );
}

/// The ordinary checked index, in range and out, so the test above is not
/// satisfied by a bounds check that no longer works.
#[test]
fn the_ordinary_checked_index_still_decides_both_ways() {
    let ok = "fn main(k: Word) -> Word { let a = [1, 2, 3]; \
         a[1] { ok(v) => v, invalid_index(i) => 0 - 1 } }";
    let bad = "fn main(k: Word) -> Word { let a = [1, 2, 3]; \
         a[7] { ok(v) => v, invalid_index(i) => 0 - 1 } }";
    assert_eq!(run_to_word(ok), 2, "a[1] of [1, 2, 3] is 2");
    assert_eq!(
        run_to_word(bad),
        -1,
        "a[7] of [1, 2, 3] takes the invalid_index arm"
    );
}

/// **Every iterable form that can hold an array type folds, and runs.**
///
/// # Why this is a corpus rather than one case
///
/// `parse_iterable` calls the full expression parser, so every expression form
/// is syntactically admissible after `in`. The analysis in
/// `docs/decisions/OP_LEN_ROOT_REPAIR.md` predicted that a generic
/// type-inference fallback would close ONE of the seven forms that can
/// realistically carry an array type. **Measured, it closes six**, because
/// `infer_expr_type` consults the authoritative per-span type table recorded by
/// the post-monomorphization type-check pass before its own structural half --
/// which the prediction was made without.
///
/// The seventh, `classify`, is refused earlier and for an unrelated reason: a
/// labelled array is not an array to for-in. That row is struck rather than
/// defended, as the analysis instructed.
///
/// Each case writes its iteration variable to a data field and returns it, so
/// the assertion is on the ITERATION COUNT and not merely on compilation.
#[test]
fn every_array_typed_iterable_form_folds_and_runs() {
    let cases: &[(&str, &str)] = &[
        (
            "array literal",
            "data s { n: Word }\nfn main(k: Word) -> Word { for x in [1, 2, 3] { s.n = x; } s.n }",
        ),
        (
            "identifier",
            "data s { n: Word }\nfn main(k: Word) -> Word { let a = [1, 2, 3]; \
             for x in a { s.n = x; } s.n }",
        ),
        (
            "call",
            "data s { n: Word }\nfn mk() -> [Word; 3] { [1, 2, 3] }\n\
             fn main(k: Word) -> Word { for x in mk() { s.n = x; } s.n }",
        ),
        (
            "field access",
            "data s { n: Word }\nstruct B { xs: [Word; 3] }\n\
             fn main(k: Word) -> Word { let b = B { xs: [1, 2, 3] }; \
             for x in b.xs { s.n = x; } s.n }",
        ),
        (
            "array index",
            "data s { n: Word }\nfn main(k: Word) -> Word { let m = [[1, 2, 3], [4, 5, 6]]; \
             for x in m[0] { s.n = x; } s.n }",
        ),
        (
            "match",
            "data s { n: Word }\nfn main(k: Word) -> Word { let a = [1, 2, 3]; \
             for x in match k { 0 => a, _ => a } { s.n = x; } s.n }",
        ),
        (
            "if expression",
            "data s { n: Word }\nfn main(k: Word) -> Word { let a = [1, 2, 3]; let b = [4, 5, 6]; \
             for x in if true { a } else { b } { s.n = x; } s.n }",
        ),
        (
            "tuple index",
            "data s { n: Word }\nfn main(k: Word) -> Word { let t = ([1, 2, 3], 9); \
             for x in t.0 { s.n = x; } s.n }",
        ),
        (
            "method call",
            "data s { n: Word }\nstruct B { xs: [Word; 3] }\n\
             trait G { fn get(self) -> [Word; 3]; }\n\
             impl G for B { fn get(b: B) -> [Word; 3] { b.xs } }\n\
             fn main(k: Word) -> Word { let b = B { xs: [1, 2, 3] }; \
             for x in b.get() { s.n = x; } s.n }",
        ),
        (
            "pipeline",
            "data s { n: Word }\nfn id(a: [Word; 3]) -> [Word; 3] { a }\n\
             fn main(k: Word) -> Word { let a = [1, 2, 3]; \
             for x in a |> id() { s.n = x; } s.n }",
        ),
        (
            "declassify",
            "data s { n: Word }\nfn main(k: Word) -> Word { let a = [1, 2, 3]; \
             for x in declassify a @Secret { s.n = x; } s.n }",
        ),
    ];

    // NON-VACUOUS. A corpus that shrank to nothing, or whose cases stopped
    // reaching the compiler, would otherwise satisfy an all-pass loop while
    // establishing nothing. This repository has had two derivations pass that
    // way.
    assert!(
        cases.len() >= 10,
        "the corpus shrank to {} cases; it is meant to span every expression form that \
         can carry an array type",
        cases.len()
    );

    for (name, src) in cases {
        assert!(!emits_len(src), "the `{name}` iterable emits Op::Len");
        assert_eq!(
            run_to_word(src),
            3,
            "the `{name}` iterable did not run three times over [1, 2, 3]"
        );
    }
}

/// **Is the flat-TUPLE refusal reachable?** Unchanged in purpose from the
/// previous revision: answered with programs rather than by reading, because
/// the array case's own comment asserted unreachability and was wrong.
///
/// The virtual machine has two `Op::Len` refusals: a flat array and a flat
/// tuple. One of the four attempts compiles, and it is the one iterating an
/// ARRAY of tuples, whose length folds. The three that try to iterate a tuple
/// directly do not compile, because a tuple is not an iterable here.
///
/// If any of these compiles AND emits `Op::Len`, the tuple refusal is a live
/// hazard of the same shape as the array one. That is a finding, not a broken
/// test.
#[test]
fn no_tuple_shaped_iterable_reaches_op_len() {
    let attempts: &[(&str, &str)] = &[
        (
            "a tuple literal directly",
            "fn main() -> Word { for x in (1, 2, 3) { } 0 }",
        ),
        (
            "a tuple-typed local",
            "fn main() -> Word { let t: (Word, Word) = (1, 2); for x in t { } 0 }",
        ),
        (
            "an if-expression yielding tuples",
            "fn main() -> Word { let a: (Word, Word) = (1, 2); let b: (Word, Word) = (3, 4); \
             for x in if true { a } else { b } { } 0 }",
        ),
        (
            "a tuple inside an array, iterated",
            "fn main() -> Word { let a: [(Word, Word); 2] = [(1, 2), (3, 4)]; for x in a { } 0 }",
        ),
    ];

    let mut compiled_and_emitted: alloc::vec::Vec<&str> = alloc::vec::Vec::new();
    let mut compiled_at_all: alloc::vec::Vec<&str> = alloc::vec::Vec::new();

    for (name, src) in attempts {
        let Ok(tokens) = tokenize(src) else { continue };
        let Ok(program) = parse(&tokens) else {
            continue;
        };
        let Ok(module) = compile(&program) else {
            continue;
        };
        compiled_at_all.push(name);
        let ops = format!(
            "{:?}",
            module
                .chunks
                .iter()
                .map(|c| &c.ops)
                .collect::<alloc::vec::Vec<_>>()
        );
        if ops.contains("Len") {
            compiled_and_emitted.push(name);
        }
    }

    assert!(
        compiled_and_emitted.is_empty(),
        "these tuple-shaped iterables compile AND emit Op::Len, so the flat-tuple \
         refusal is reachable and unpinned: {compiled_and_emitted:?}"
    );
    // NON-VACUOUS. If nothing compiled, the loop above proved nothing, and a
    // later change making every attempt fail to parse would leave this test
    // green while blind.
    assert!(
        !compiled_at_all.is_empty(),
        "not one attempt compiled, so this test established nothing about the tuple \
         refusal. The attempts are stale and need rewriting against the current grammar."
    );
}

/// **The floor: the compiler has no `Op::Len` emission site at all.**
///
/// # Why this is a source guard and not a program
///
/// No source form found reaches the floor. Every expression form that can carry
/// an array type folds, so the compile error behind the floor has **no known
/// witness**. That is recorded as "not found" rather than as "unreachable",
/// which is the distinction this repository has twice paid for confusing.
///
/// A defence with no witness cannot be pinned by running a program, and the
/// property that matters is nevertheless checkable: the emission sites are
/// gone, and a future one would be a new call. So the guard reads the compiler
/// source for the emission form.
///
/// # Its reach, stated rather than assumed
///
/// It sees `fc.emit(Op::Len)` written in that shape in `src/compiler.rs`. It
/// would NOT see an emission written through a differently named binding, an
/// emission from another module, or one built by pushing to the op vector
/// directly. It is a tripwire on the form the two removed sites used, not a
/// proof of absence -- and saying so is the point, because a guard whose reach
/// is unstated gets read as a guarantee.
#[test]
fn the_compiler_carries_no_len_emission_site() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/compiler.rs");
    let text = std::fs::read_to_string(&path).expect("read src/compiler.rs");

    // NON-VACUOUS, in two independent ways. The file must be the one intended,
    // and the pattern shape must be one that actually occurs -- otherwise a
    // count of zero would mean the guard is looking for something the compiler
    // never writes, and it would report clean forever.
    assert!(
        text.len() > 100_000,
        "src/compiler.rs read as {} bytes, which is not the compiler; this guard is \
         reading the wrong file and would report clean regardless",
        text.len()
    );
    let total_emissions = text.matches("fc.emit(Op::").count();
    assert!(
        total_emissions > 50,
        "only {total_emissions} emissions of the form this guard scans for were found, \
         so the emission style has changed and a `Op::Len` written in the new style \
         would be invisible here"
    );

    let len_emissions = text.matches("fc.emit(Op::Len)").count();
    assert_eq!(
        len_emissions, 0,
        "the compiler emits Op::Len at {len_emissions} site(s). The virtual machine \
         refuses that opcode on a flat array or tuple body, so such a module passes \
         verify(), loads, and traps InvalidBytecode at run time. Fold the length from \
         the operand's type, or fail with a compile error naming the unfoldable length."
    );
}

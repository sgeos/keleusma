//! Which opcodes a SOURCE PROGRAM can actually produce.
//!
//! # Why this file exists
//!
//! The `v0.3.0` line maintains an opcode census: for each of the 66 opcodes, a
//! source program that emits it. Two resisted after eight construct attempts, and
//! they framed the question better than "which construct" — **the question is
//! whether one exists at all.**
//!
//! That distinction is the point. For an instruction set whose opcode count is a
//! stated rad-hard design constraint, an opcode no program can produce is a more
//! valuable finding than one more witnessed opcode. So this file records a
//! verdict for each, and is careful never to report "I could not find one" as
//! "none exists".
//!
//! # "REACHABLE" NEEDS QUALIFYING ON THIS PROJECT, AND THE FIRST VERDICT HERE
//! SHOWS WHY
//!
//! `Op::Len` is reachable in BYTECODE. The program that witnesses it cannot be
//! given a memory bound: `verify()` accepts it and `module_wcmu` refuses it,
//! because the same missing `Expr::If` case defeats both the static-length lookup
//! and the loop-bound extractor.
//!
//! On a language whose value proposition is definitive worst-case execution time
//! and memory usage, an opcode reachable only in a program the resource analysis
//! rejects is closer to unwitnessed than a bare "reachable" suggests. **Neither
//! framing alone is honest**, so both are asserted.
//!
//! # The technique, which generalises
//!
//! Both opcodes are emitted only as a FALLBACK when a static type is unknown. So
//! the target is not an unusual shape — it is **making inference fail**. Reading
//! the guard's own match arms for the kinds it does NOT handle is what cracked the
//! first one; guessing at constructs is what failed eight times before that.

#![cfg(feature = "compile")]

use keleusma::bytecode::Op;
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

fn module_of(src: &str) -> keleusma::bytecode::Module {
    let ast = parse(&tokenize(src).expect("lex")).expect("parse");
    compile(&ast).expect("the reference must accept the probe")
}

fn ops_of(src: &str) -> Vec<Op> {
    module_of(src)
        .chunks
        .iter()
        .flat_map(|c| c.ops.clone())
        .collect()
}

/// **`Op::Len` IS REACHABLE IN BYTECODE, AND THE WITNESSING PROGRAM CANNOT BE
/// GIVEN A MEMORY BOUND.** Both halves matter on this project.
///
/// The construct is an `if` EXPRESSION as a `for`-in source.
///
/// `Op::Len` is emitted only when `static_for_in_length` returns `None`. That
/// function matches on `ArrayLiteral`, `Call`, `FieldAccess`, `Ident`,
/// `ArrayIndex` and `Match`, then falls through to `_ => None`. **`Expr::If` is
/// not among them**, so an `if` expression as the iteration source takes the
/// fallback.
///
/// Found by reading the guard's match arms for what it omits, after six probes
/// that varied the SHAPE of the source rather than its expression KIND.
///
/// The controls matter as much as the case: every other source kind is checked to
/// take the static path, so a change that made `static_for_in_length` return
/// `None` for everything would fail here rather than look like a win.
#[test]
fn a_for_in_over_an_if_expression_reaches_op_len() {
    const WITNESS: &str = "fn f(c: bool) -> Word { let a = [1, 2]; let b = [3, 4]; \
                           for x in if c { a } else { b } { let _d = x; } 0 }\n\
                           fn main() -> Word { f(true) }";
    let reached = ops_of(
        "fn f(c: bool) -> Word { let a = [1, 2]; let b = [3, 4]; \
         for x in if c { a } else { b } { let _d = x; } 0 }\n\
         fn main() -> Word { f(true) }",
    );
    assert!(
        reached.iter().any(|o| matches!(o, Op::Len)),
        "an `if`-expression `for`-in source no longer reaches `Op::Len`; if \
         `static_for_in_length` gained an `Expr::If` arm, this opcode may have no \
         remaining producer and that is worth knowing"
    );

    // **AND THE QUALIFICATION THAT CHANGES WHAT "REACHABLE" MEANS HERE.**
    //
    // Raised by the `v0.3.0` line and verified here rather than taken on report.
    // `verify()` ACCEPTS this program; `module_wcmu` REFUSES it, with "loop at
    // instruction 20 has no statically extractable iteration bound".
    //
    // **THE TWO FACTS ARE ONE FACT.** `Op::Len` is emitted exactly when the
    // `for`-in source has no statically known length, and a loop whose trip count
    // is not statically known is exactly what the bound extractor refuses. The
    // property that makes the opcode reachable IS the property that makes the loop
    // unbounded. They are not two limitations that might be lifted separately.
    //
    // **THE OBVIOUS OBJECTION IS RULED OUT.** "The arms differ in length, so of
    // course the bound is unknown" — no: both arms here are length TWO, the trip
    // count is two on every path, and it is still refused. Neither
    // `static_for_in_length` nor the bound extractor looks THROUGH an `Expr::If`.
    // It is the same omission twice, which makes this the SECOND category of
    // conservative rejection in `LANGUAGE_DESIGN.md` — provable in principle,
    // analysis not implemented — rather than the first.
    //
    // **NOT A DEFECT.** A verifier refusing a bound it cannot prove is the
    // documented stance. It is recorded because it qualifies the reachability
    // claim: on a language whose value proposition is definitive WCET and WCMU,
    // an opcode reachable only in a program the resource analysis rejects is
    // closer to unwitnessed than the headline suggests.
    assert!(
        keleusma::verify::verify(&module_of(WITNESS)).is_ok(),
        "the structural verifier now rejects the witness, so the split between \
         `verify` accepting and the bound analysis refusing no longer holds"
    );
    assert!(
        keleusma::verify::module_wcmu(&module_of(WITNESS), &[]).is_err(),
        "the resource analysis now BOUNDS the `Op::Len` witness. That closes the \
         qualification above: the opcode is reachable in an admissible program, \
         and this comment should be rewritten rather than deleted"
    );
    // The must-not-fire half: a statically-sized `for`-in IS boundable, so the
    // refusal above is attributable to the unknown length rather than to `for`-in.
    assert!(
        keleusma::verify::module_wcmu(
            &module_of("fn main() -> Word { let xs = [10, 20]; for x in xs { let _d = x; } 0 }"),
            &[]
        )
        .is_ok(),
        "a statically-sized `for`-in is no longer boundable either, so the refusal \
         above says nothing about the unknown length specifically"
    );

    // THE CONTROLS. Each of these IS handled by `static_for_in_length`, so each
    // must take the static path. Without them a blanket regression would pass.
    const STATIC_SOURCES: &[(&str, &str)] = &[
        (
            "ident",
            "fn main() -> Word { let xs = [10, 20]; for x in xs { let _d = x; } 0 }",
        ),
        (
            "literal",
            "fn main() -> Word { for x in [10, 20] { let _d = x; } 0 }",
        ),
        (
            "index",
            "fn main() -> Word { let m = [[1, 2], [3, 4]]; for x in m[0] { let _d = x; } 0 }",
        ),
        (
            "call",
            "fn g() -> [Word; 2] { [1, 2] }\nfn main() -> Word { for x in g() { let _d = x; } 0 }",
        ),
        (
            "field",
            "struct S { xs: [Word; 2] }\nfn main() -> Word { let s = S { xs: [1, 2] }; \
             for x in s.xs { let _d = x; } 0 }",
        ),
        (
            "match",
            "fn main() -> Word { let a = [1, 2]; for x in match 1 { _ => a } { let _d = x; } 0 }",
        ),
    ];
    let mut checked = 0;
    for (label, src) in STATIC_SOURCES {
        assert!(
            !ops_of(src).iter().any(|o| matches!(o, Op::Len)),
            "{label}: a statically-sized source now emits `Op::Len`, so the \
             fallback is being taken where the static path should apply"
        );
        checked += 1;
    }
    assert_eq!(checked, STATIC_SOURCES.len(), "not every control ran");
}

/// **THE MATCH-SCRUTINEE ROUTE TO `Op::IsStruct` IS A DEAD END, AND KNOWING WHY
/// IS WHAT FOUND THE REAL ONE.**
///
/// This test asserted that `Op::IsStruct` had no witness at all. It has one — see
/// `op_is_struct_is_reachable_and_its_witness_traps_at_run_time` — so what it now
/// pins is the narrower, still-true fact: **no `match` scrutinee reaches the
/// opcode**, across seventeen attempts spanning two sessions and two lines.
///
/// Kept rather than deleted because the dead end is the informative half. The
/// guard is `named_type_name(ty) != Some(type_name)`, and every attempt tried to
/// make the two DIFFER. **The type checker forbids that outright** — "struct
/// pattern `P` does not match scrutinee type" — so the inequality is satisfiable
/// only when `ty` is `None`.
///
/// And a match scrutinee's `ty` comes from `infer_expr_type`, which genuinely does
/// omit `If`, `MethodCall`, `Pipeline` and nine other variants. Reading those
/// omissions is what cracked `Op::Len`; here it is a dead end, because **every
/// expression that survives type checking at this site also survives inference**.
///
/// The real route was never an expression at all. It was a DECLARATION site with
/// no type to lose: a parameter written without an annotation.
///
/// # What was tried on this route, so it is not repeated
///
/// Struct-pattern matches whose scrutinee is: a plain local, an `if` expression
/// (constant and runtime condition, bare and parenthesised), a call result, a
/// method call, an array index, a nested `match`, a struct field, a tuple element,
/// an enum payload, and an `Option` payload. All compile; none emits the opcode.
#[test]
fn no_match_scrutinee_reaches_op_is_struct() {
    let probes = [
        "struct P { a: Word, b: Word }\nfn f(x: P) -> Word { match x { P { a, b } => a + b, _ => 0 } }",
        "struct P { a: Word, b: Word }\nfn f(c: bool, x: P, y: P) -> Word { match (if c { x } else { y }) { P { a, b } => a + b, _ => 0 } }",
        "struct P { a: Word, b: Word }\nfn f(n: Word, x: P, y: P) -> Word { match match n { 0 => x, _ => y } { P { a, b } => a + b, _ => 0 } }",
        "struct Q { p: P, n: Word }\nstruct P { a: Word, b: Word }\nfn f(q: Q) -> Word { match q.p { P { a, b } => a + b, _ => 0 } }",
        "struct P { a: Word, b: Word }\nfn f(xs: [P; 2]) -> Word { match xs[0] { P { a, b } => a + b, _ => 0 } }",
        "struct P { a: Word, b: Word }\nenum E { V(P), W }\nfn f(e: E) -> Word { match e { E::V(P { a, b }) => a + b, _ => 0 } }",
        "struct P { a: Word, b: Word }\nfn f(o: Option<P>) -> Word { match o { Option::Some(P { a, b }) => a + b, _ => 0 } }",
    ];
    for src in probes {
        assert!(
            !ops_of(src).iter().any(|o| matches!(o, Op::IsStruct(_))),
            "a MATCH scrutinee now reaches `Op::IsStruct` ({src:?}). That is a second, \
             independent route and it should be qualified the way the parameter route is: \
             does its witness verify, receive a bound, and run?"
        );
    }

    // Non-vacuity: these probes must actually compile, or the loop above passes by
    // measuring nothing. Every one is asserted to reach the reference compiler.
    for src in probes {
        assert!(
            !ops_of(src).is_empty(),
            "a probe compiled to no ops at all, so it establishes nothing"
        );
    }
}

/// **`Op::IsStruct` IS REACHABLE, AND ITS WITNESS COMPILES, VERIFIES, AND THEN
/// TRAPS AT RUN TIME.** The second half is the finding.
///
/// The construct is a **struct pattern on a parameter with no type annotation**:
///
/// ```text
///   struct P { a: Word, b: Word }
///   fn g(P { a, b }) -> Word { a + b }
/// ```
///
/// # Why seventeen earlier attempts missed it
///
/// `Op::IsStruct` is emitted only when `named_type_name(ty) != Some(pattern_type)`.
/// Every earlier attempt, including eight of mine, tried to make the two DIFFER —
/// and **the type checker forbids that outright**, with "struct pattern `P` does
/// not match scrutinee type". The inequality is therefore only satisfiable when
/// `ty` is `None`.
///
/// The `match` path takes its type from `infer_expr_type`, which does omit `If`,
/// `MethodCall` and nine other variants — the same reading that cracked `Op::Len`.
/// It is a dead end here, and the reason is worth keeping: **a match scrutinee is
/// an expression, and every expression that survives type checking here also
/// survives inference.**
///
/// The other call site is the FUNCTION-PARAMETER path, which takes the declared
/// `param.type_expr` — and a parameter written without an annotation has none.
/// **The route was never an expression whose type is hard to infer; it was a
/// declaration site with no type to lose.**
///
/// # What the witness does, which is the part that matters
///
/// | stage | result |
/// |---|---|
/// | compile | emits `IsStruct(0)` |
/// | `verify()` | **accepts** |
/// | `module_wcmu()` | **succeeds**, `[(224, 0), (224, 16)]` |
/// | execution | **traps `InvalidBytecode`** |
///
/// The virtual machine refuses the op it was handed: *"Op::IsStruct on a flat
/// struct; the type test is a compile-time constant."* The compiler's own comment
/// says the fold exists to keep a flat struct away from this op — and the fold is
/// conditional on a type that an un-annotated parameter does not have, so a flat
/// struct reaches it anyway.
///
/// **`InvalidBytecode` is the class `verify()` exists to exclude at load time.** A
/// legal program reaching it at run time is a hole in the load-time check, not a
/// bad program. Pinned rather than repaired: `src/verify.rs` is held read-only by
/// the `v0.3.0` line pending an announcement, and the compiler-side alternative —
/// folding the test out when the pattern's own type is known regardless of the
/// scrutinee's — is a judgment call about which side owns the invariant.
#[test]
fn op_is_struct_is_reachable_and_its_witness_traps_at_run_time() {
    const SRC: &str = "struct P { a: Word, b: Word }\n\
                       fn g(P { a, b }) -> Word { a + b }\n\
                       fn main() -> Word { g(P { a: 1, b: 2 }) }";

    let module = module_of(SRC);
    assert!(
        module
            .chunks
            .iter()
            .any(|c| c.ops.iter().any(|o| matches!(o, Op::IsStruct(_)))),
        "no `Op::IsStruct` emitted. If the compiler now folds the test out for an \
         un-annotated parameter, that is a REPAIR of the trap recorded below: say so \
         here and re-verify the opcode census, which counted this opcode unwitnessed"
    );

    // **THE CONTROL, AND IT IS THE WHOLE ARGUMENT.** The same program with the
    // parameter annotated folds the test out. Without this, the assertion above
    // would be satisfied by a compiler that emitted `IsStruct` for every struct
    // pattern, which would say nothing about the missing annotation being the cause.
    const ANNOTATED: &str = "struct P { a: Word, b: Word }\n\
                             fn g(p: P) -> Word { match p { P { a, b } => a + b, _ => 0 } }\n\
                             fn main() -> Word { g(P { a: 1, b: 2 }) }";
    assert!(
        !ops_of(ANNOTATED)
            .iter()
            .any(|o| matches!(o, Op::IsStruct(_))),
        "the annotated control ALSO emits `Op::IsStruct`, so the witness above does \
         not isolate the missing type annotation and this test attributes it wrongly"
    );

    // `verify()` accepts it, which is why the trap is a load-time hole rather than
    // a rejected program.
    keleusma::verify::verify(&module).expect(
        "`verify()` now rejects the witness. If that is deliberate, this trap became a \
         load-time refusal, which is the sound direction -- record it here",
    );

    // And it is given a memory bound, so the resource analysis does not exclude it
    // either. Contrast `Op::Len`, whose witness `module_wcmu` refuses.
    keleusma::verify::module_wcmu(&module, &[])
        .expect("the witness no longer receives a WCMU bound; the contrast with `Op::Len` moved");
}

/// **THE WITNESS TRAPS, AND THE MESSAGE NAMES THE CAUSE.**
///
/// Separated from the reachability assertion because the two can come apart: a
/// repair on either side changes exactly one of them, and a single test would not
/// say which. Pinned in the FIRING direction, so the day this executes cleanly the
/// failure is the notice.
#[test]
fn the_is_struct_witness_is_refused_by_the_virtual_machine() {
    const SRC: &str = "struct P { a: Word, b: Word }\n\
                       fn g(P { a, b }) -> Word { a + b }\n\
                       fn main() -> Word { g(P { a: 1, b: 2 }) }";

    let arena = keleusma::Arena::with_capacity(keleusma::vm::DEFAULT_ARENA_CAPACITY);
    let mut vm = keleusma::vm::Vm::new(module_of(SRC), &arena)
        .expect("the module loads; the refusal is at CALL time, not construction");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    let err = vm.call_with_shared(&mut shared, &[]).expect_err(
        "the witness now RUNS. That is a repair: the compiler stopped emitting \
                     `Op::IsStruct` for a flat struct, or the VM learned to execute it. Either \
                     way the load-time hole recorded here is closed -- say which, and update \
                     the sibling test",
    );
    let text = alloc_msg(&err);
    assert!(
        text.contains("IsStruct"),
        "the witness fails for some other reason now ({text}), so this test no longer \
         measures the flat-struct type test"
    );
}

fn alloc_msg(e: &keleusma::vm::VmError) -> String {
    format!("{e:?}")
}

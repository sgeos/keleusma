//! Which opcodes a SOURCE PROGRAM can actually produce.
//!
//! # Why this file exists
//!
//! The `v0.3.0` line maintains an opcode census: for each of the 66 opcodes, a
//! source program that emits it. Two resisted after eight construct attempts, and
//! they framed the question better than "which construct" — **the question is
//! whether one exists at all.** Both now have witnesses; see below for why that is
//! a weaker statement than it sounds.
//!
//! That distinction is the point. For an instruction set whose opcode count is a
//! stated rad-hard design constraint, an opcode no program can produce is a more
//! valuable finding than one more witnessed opcode. So this file records a
//! verdict for each, and is careful never to report "I could not find one" as
//! "none exists".
//!
//! # BOTH ARE NOW WITNESSED, AND "REACHABLE" NEEDED QUALIFYING FOR EACH
//!
//! `Op::Len` is reachable via an `if` expression as a `for`-in source. `Op::IsStruct`
//! is reachable via a struct pattern on a parameter with **no type annotation** —
//! found after seventeen attempts across two sessions and two lines, every one of
//! which tried to make a scrutinee's type DIFFER from the pattern's. The type
//! checker forbids that, so the route was never an expression whose inference
//! fails; it was a declaration site with no type to lose.
//!
//! **Neither witness is an ordinary working program, and they fail differently:**
//!
//! | witness | `verify()` | `module_wcmu` | load | run |
//! |---|---|---|---|---|
//! | `Op::Len` | accepts | refuses | **`Vm::new` REFUSES** | never runs |
//! | `Op::IsStruct` | accepts | accepts | loads | **traps** |
//!
//! `Op::Len`'s witness is refused at LOAD, which is the conservative-verification
//! stance working as designed. `Op::IsStruct`'s satisfies every load-time check and
//! dies at call time on `InvalidBytecode` — the class `verify()` exists to exclude —
//! so it is a load-time hole rather than another instance of the same pattern.
//!
//! On a language whose value proposition is definitive worst-case execution time
//! and memory usage, **an opcode reachable only in a program the resource analysis
//! rejects, or only in one that cannot execute, is closer to unwitnessed than a bare
//! "reachable" suggests.** Both framings are asserted for both opcodes.
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

/// **THE LOAD-TIME HOLE IS CLOSED, AND CLOSING IT LEFT `Op::IsStruct` WITH NO PRODUCER.**
///
/// This test asserted, hours earlier, that a struct pattern on an un-annotated parameter reached
/// `Op::IsStruct` and that its witness **verified, received a memory bound, loaded, and then
/// trapped `InvalidBytecode`**. That was a hole in the load-time check, since `InvalidBytecode` is
/// exactly the class `verify()` exists to exclude.
///
/// # What was repaired, and why in the compiler rather than the verifier
///
/// Two repairs were available. Rejecting the module in `verify()` would have made a legal program
/// fail EARLIER. Folding the irrefutable type test at compile time makes it **work**. The second is
/// strictly better for a program the type checker accepts.
///
/// The fold already existed; it was conditional on the SCRUTINEE's type matching the pattern's, and
/// an un-annotated parameter has no scrutinee type at all. **An absent type is not an unconfirmed
/// one.** The type checker has already established the match — it refuses a mismatch outright —
/// so when the scrutinee's type is merely absent, the pattern's own type is the answer.
///
/// # The consequence, which is an ISA finding rather than a tidy ending
///
/// With the fold widened, **no construct known to this tree produces `Op::IsStruct`.** The only
/// witness ever found was a compiler defect, and repairing it removed the witness.
///
/// On an instruction set whose opcode count is a stated rad-hard design constraint, an opcode with
/// no producer is worth more as a finding than as a curiosity. **It is recorded as "no producer
/// found", never as "unreachable"** — the fallback and the virtual machine's refusal both remain,
/// and would matter if inference ever reached that site with a real disagreement.
#[test]
fn the_is_struct_witness_now_compiles_verifies_and_runs() {
    const SRC: &str = "struct P { a: Word, b: Word }\n\
                       fn g(P { a, b }) -> Word { a + b }\n\
                       fn main() -> Word { g(P { a: 1, b: 2 }) }";

    let module = module_of(SRC);
    assert!(
        !module
            .chunks
            .iter()
            .any(|c| c.ops.iter().any(|o| matches!(o, Op::IsStruct(_)))),
        "`Op::IsStruct` is emitted again for an un-annotated parameter. If that is deliberate, the \
         load-time hole this test records is open again: the witness verifies and then traps"
    );

    keleusma::verify::verify(&module).expect("the witness must verify");
    keleusma::verify::module_wcmu(&module, &[]).expect("the witness must receive a memory bound");

    // **THE HALF THAT WAS FAILING BEFORE.** It loaded and then died at call time; now it runs and
    // returns the right answer, which is what distinguishes a repair from a relocation of the
    // failure to a different stage.
    let arena = keleusma::Arena::with_capacity(keleusma::vm::DEFAULT_ARENA_CAPACITY);
    let mut vm = keleusma::vm::Vm::new(module, &arena).expect("the witness must load");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    let state = vm
        .call_with_shared(&mut shared, &[])
        .expect("the witness must RUN; a trap here means the load-time hole is open again");
    assert!(
        matches!(
            state,
            keleusma::vm::VmState::Finished(keleusma::bytecode::Value::Int(3))
        ),
        "the witness ran but computed {state:?} rather than 1 + 2; folding the type test changed \
         the program's meaning, which is worse than the trap it replaced"
    );

    // The CONTROL: the annotated form was always folded and must still be.
    const ANNOTATED: &str = "struct P { a: Word, b: Word }\n\
                             fn g(p: P) -> Word { match p { P { a, b } => a + b, _ => 0 } }\n\
                             fn main() -> Word { g(P { a: 1, b: 2 }) }";
    assert!(
        !ops_of(ANNOTATED)
            .iter()
            .any(|o| matches!(o, Op::IsStruct(_))),
        "the annotated control now emits `Op::IsStruct`, so the fold regressed for the case it \
         always covered"
    );
}

/// **THE LOAD-TIME HOLE IS CLOSED, AND THIS TEST HAS BEEN WRONG ONCE ABOUT EXACTLY THAT.**
///
/// # Read the history before trusting the claim
///
/// 1. `Op::IsStruct` was reached by a struct pattern on an un-annotated parameter. Its witness
///    verified, took a memory bound, loaded, and trapped `InvalidBytecode` at call time.
/// 2. A compile-time fold closed that construct. **This file then claimed the opcode had no
///    producer**, from three guessed constructs. The `v0.3.0` line disproved it within the hour
///    with four counterexamples, found by reading the emission condition rather than guessing.
/// 3. Two root causes were then found, both SYMMETRY GAPS rather than novel defects, and each
///    masking the other.
///
/// # The two gaps
///
/// **`rewrite_pattern_enum_name` rewrites ENUM names in patterns on specialization; nothing did
/// the same for structs.** Its `Pattern::Struct` arm recurses into a struct pattern's fields while
/// ignoring the struct's own name. So `fn g(P { a, b }: P<Word>)` had its TYPE rewritten to
/// `P__Word` and its PATTERN left naming `P`.
///
/// **`check_pattern_against_type` holds the correct nominal rule and was called only for match
/// arms.** Function parameters were `bind_pattern`-ed, never checked. So the disagreement above
/// never failed type checking, and the lowering fell back to a runtime test.
///
/// Each gap hid the other: without the missing check, the un-rewritten pattern was silent.
///
/// # What each repair does
///
/// | construct | before | after |
/// |---|---|---|
/// | generic struct destructured in a parameter | verified, loaded, **trapped** | **runs** |
/// | pattern `P` against annotation `Q` | verified, loaded, **trapped** | **rejected at compile time** |
/// | tuple-typed annotation | verified, loaded, trapped | **rejected at compile time** |
/// | array-typed annotation | verified, loaded, trapped | **rejected at compile time** |
/// | un-annotated parameter | trapped | runs |
///
/// # WHAT THIS TEST DOES NOT CLAIM
///
/// **It does not claim the opcode has no producer.** That claim was made here once, from three
/// constructs, and was false. Twelve shapes are now tried and none produces it — but sample size
/// was never the problem. What falsified the earlier claim was reading the emission condition and
/// enumerating which type shapes satisfy it, and a reader who can construct a survivor should
/// treat this list as incomplete rather than as a boundary.
#[test]
fn no_shape_tried_reaches_the_is_struct_trap() {
    // The four that used to trap, and the shapes around them.
    let shapes = [
        (
            "generic struct destructured in a parameter",
            "struct P<T> { a: T, b: T }\nfn g(P { a, b }: P<Word>) -> Word { a + b }\nfn main() -> Word { g(P { a: 1, b: 2 }) }",
        ),
        (
            "un-annotated parameter",
            "struct P { a: Word, b: Word }\nfn g(P { a, b }) -> Word { a + b }\nfn main() -> Word { g(P { a: 1, b: 2 }) }",
        ),
        (
            "annotated matching struct",
            "struct P { a: Word, b: Word }\nfn g(P { a, b }: P) -> Word { a + b }\nfn main() -> Word { g(P { a: 1, b: 2 }) }",
        ),
        (
            "struct pattern inside a tuple parameter",
            "struct P { a: Word, b: Word }\nfn g((P { a, b }, n): (P, Word)) -> Word { a + b + n }\nfn main() -> Word { g((P { a: 1, b: 2 }, 3)) }",
        ),
        (
            "multihead, generic",
            "struct P<T> { a: T, b: T }\nfn g(P { a, b }: P<Word>) -> Word { a + b }\nfn g(x) -> Word { 0 }\nfn main() -> Word { g(P { a: 1, b: 2 }) }",
        ),
        (
            "match on a generic struct",
            "struct P<T> { a: T, b: T }\nfn f(x: P<Word>) -> Word { match x { P { a, b } => a + b, _ => 0 } }",
        ),
        (
            "match on an if-expression",
            "struct P { a: Word, b: Word }\nfn f(c: bool, x: P, y: P) -> Word { match (if c { x } else { y }) { P { a, b } => a + b, _ => 0 } }",
        ),
        (
            "struct in an enum payload",
            "struct P { a: Word, b: Word }\nenum E { V(P), W }\nfn f(e: E) -> Word { match e { E::V(P { a, b }) => a + b, _ => 0 } }",
        ),
    ];
    for (label, src) in shapes {
        assert!(
            !ops_of(src).iter().any(|o| matches!(o, Op::IsStruct(_))),
            "{label} produces `Op::IsStruct`. Qualify it before recording the opcode as reachable: \
             does it verify, take a bound, load, and RUN? If it loads and traps, the load-time \
             hole is open again"
        );
    }

    // **THE GENERIC CASE MUST ACTUALLY RUN**, not merely avoid the opcode. Asserting the VALUE
    // because a repair that changed the program's meaning would be worse than the trap it replaced.
    const GENERIC: &str = "struct P<T> { a: T, b: T }\n\
                           fn g(P { a, b }: P<Word>) -> Word { a + b }\n\
                           fn main() -> Word { g(P { a: 1, b: 2 }) }";
    let arena = keleusma::Arena::with_capacity(keleusma::vm::DEFAULT_ARENA_CAPACITY);
    let mut vm = keleusma::vm::Vm::new(module_of(GENERIC), &arena).expect("loads");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    assert!(
        matches!(
            vm.call_with_shared(&mut shared, &[])
                .expect("must RUN, not trap"),
            keleusma::vm::VmState::Finished(keleusma::bytecode::Value::Int(3))
        ),
        "the generic witness ran but did not compute 1 + 2"
    );
}

/// **THE THREE ILL-TYPED PATTERNS ARE REJECTED AT COMPILE TIME, NAMING THE CAUSE.**
///
/// Separate from the census above because these are a NARROWING: each compiled before and now does
/// not. That is the direction that needs pinning from both sides — the sibling test pins what still
/// compiles, and this pins what no longer does.
///
/// All three were previously accepted, lowered to a runtime type test, and trapped. Rejecting them
/// at the source removes the emission rather than folding it.
#[test]
fn a_struct_pattern_against_a_foreign_type_is_refused_by_the_type_checker() {
    for (label, src) in [
        (
            "a different struct",
            "struct P { a: Word, b: Word }\nstruct Q { a: Word, b: Word }\nfn g(P { a, b }: Q) -> Word { a + b }\nfn main() -> Word { g(Q { a: 1, b: 2 }) }",
        ),
        (
            "a tuple",
            "struct P { a: Word, b: Word }\nfn g(P { a, b }: (Word, Word)) -> Word { a + b }\nfn main() -> Word { g((1, 2)) }",
        ),
        (
            "an array",
            "struct P { a: Word, b: Word }\nfn g(P { a, b }: [Word; 2]) -> Word { a + b }\nfn main() -> Word { g([1, 2]) }",
        ),
    ] {
        let ast = parse(&tokenize(src).expect("lex")).expect("parse");
        let err = compile(&ast).expect_err(&format!(
            "{label}: accepted again. If that is deliberate, it lowers to a \
                                  runtime type test that the virtual machine refuses on a flat \
                                  struct"
        ));
        assert!(
            err.message.contains("does not match scrutinee type"),
            "{label}: refused for another reason ({}), so this no longer measures the nominal \
             pattern rule",
            err.message
        );
    }
}

/// **THE THREE "SHOULD NEVER HAVE BEEN EMITTED" REFUSALS, AND WHAT IS LEFT OF THEM.**
///
/// The virtual machine carries exactly three refusals of that shape, naming only two opcodes:
///
/// ```text
///   Op::Len      on a flat array;  length is a compile-time constant
///   Op::Len      on a flat tuple;  arity  is a compile-time constant
///   Op::IsStruct on a flat struct; the type test is a compile-time constant
/// ```
///
/// Those are precisely the two the opcode census could not witness, which is not a coincidence:
/// both are emitted only as a dynamic fallback when a static type is unknown, and both are refused
/// when the value turns out to be statically known after all.
///
/// # The history, because the shape of it is the lesson
///
/// | | `Op::Len` | `Op::IsStruct` |
/// |---|---|---|
/// | witness found | an `if` expression as a `for`-in source | a struct pattern on an un-annotated parameter |
/// | `verify()` | accepts | accepted |
/// | resource analysis | **refuses** | accepted |
/// | load | **`Vm::new` REFUSES** | loaded |
/// | run | never runs | **trapped** |
/// | now | unchanged | **repaired; no producer remains** |
///
/// `Op::Len`'s witness cannot be admitted at all — refused at LOAD by the strict iteration-bound
/// check, which is the conservative-verification stance working as designed. It was never a hole.
///
/// `Op::IsStruct`'s witness satisfied every load-time check and died at call time, which WAS a
/// hole. Folding the irrefutable type test closed **that construct** — and, for about an hour, this
/// file claimed it had closed the opcode. **It had not.** Four producers survive and two still trap;
/// see `op_is_struct_still_has_producers_and_two_still_trap`.
///
/// **The asymmetry this test asserts is therefore narrower than it first read**: `Op::Len` has a
/// witness that cannot be ADMITTED, and `Op::IsStruct` has witnesses that are admitted and then
/// fail. Both matter on an instruction set whose opcode count is a design constraint, and neither
/// is a claim that an opcode is unreachable.
#[test]
fn op_len_has_an_inadmissible_witness_and_op_is_struct_a_narrowed_one() {
    const LEN_WITNESS: &str = "fn f(c: bool) -> Word { let a = [1, 2]; let b = [3, 4]; \
                               for x in if c { a } else { b } { let _d = x; } 0 }\n\
                               fn main() -> Word { f(true) }";

    // `Op::Len` is still produced, and its witness is still refused at LOAD rather than trapping.
    let len_mod = module_of(LEN_WITNESS);
    assert!(
        len_mod
            .chunks
            .iter()
            .any(|c| c.ops.iter().any(|o| matches!(o, Op::Len))),
        "the `Op::Len` witness stopped producing the opcode; if it has no producer either, both \
         fallbacks are now unwitnessed and that is a larger ISA finding"
    );
    keleusma::verify::verify(&len_mod).expect("the `Op::Len` witness must still verify");
    let arena = keleusma::Arena::with_capacity(keleusma::vm::DEFAULT_ARENA_CAPACITY);
    assert!(
        keleusma::vm::Vm::new(len_mod, &arena).is_err(),
        "the `Op::Len` witness now LOADS. If it also runs, the load-time bound check stopped \
         catching it; if it loads and then traps, the refusal merely moved later, which is worse"
    );

    // **`Op::IsStruct` STILL HAS PRODUCERS.** This asserts only the narrower true thing: the
    // construct the fold closed no longer emits it. The surviving producers, two of which still
    // trap, are enumerated in `op_is_struct_still_has_producers_and_two_still_trap`.
    //
    // An earlier revision asserted here that the opcode had NO producer, which was wrong and was
    // disproved by the other line within the hour.
    const FOLDED: &str = "struct P { a: Word, b: Word }\n\
                          fn g(P { a, b }) -> Word { a + b }\n\
                          fn main() -> Word { g(P { a: 1, b: 2 }) }";
    assert!(
        !ops_of(FOLDED).iter().any(|o| matches!(o, Op::IsStruct(_))),
        "the unannotated-parameter case emits `Op::IsStruct` again; the fold regressed"
    );
}

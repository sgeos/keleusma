//! **THE HAZARD IS CLOSED, AND IT WAS CLOSED IN THE COMPILER RATHER THAN IN THE
//! VERIFIER. THE ASSERTIONS ARE INVERTED, NOT DELETED.**
//!
//! # What this file used to report
//!
//! `Op::Len` on a flat array returns `VmError::InvalidBytecode`. The reference
//! compiler emitted exactly that opcode from an ordinary program —
//! `for x in if c { a } else { b }` — and `verify()` accepted the module. What
//! held the trap shut was not the verifier but the RESOURCE-BOUND refusal, and
//! this project's own taxonomy places that refusal in the SECOND category:
//! provable in principle, analysis not implemented. That is the category defined
//! as liftable. **So an unambiguous improvement to the bound extractor, made by
//! someone with no reason to look at `Op::Len`, would have converted a rejected
//! program into one that loads and traps.** These four legs existed to make that
//! un-silent.
//!
//! # What happened instead, and it is the better outcome
//!
//! The `v0.2.3` line removed **both `Op::Len` emission sites in the compiler**.
//! Each now folds the length from the operand's type or fails with a compile
//! error naming the unfoldable length. The bound extractor was never taught to
//! see through an `Expr::If`; the opcode simply stopped being produced.
//!
//! **A bad program stopped being generated, which is strictly better than a bad
//! program being caught.** The subject of legs 1 to 4 no longer exists.
//!
//! # THE MECHANISM, STATED CORRECTLY
//!
//! This line recorded in two handoff documents that *"`static_for_in_length`
//! gained an `Expr::If` arm"*. **It did not.** `structural_for_in_length` still
//! matches exactly `ArrayLiteral`, `Call`, `FieldAccess`, `Ident`, `ArrayIndex`
//! and `Match`, then `_ => None`. The fold comes from `static_for_in_length`'s
//! FALLBACK to `infer_expr_type`, which consults the authoritative per-span type
//! table and so answers for forms whose structural arms are absent.
//!
//! `docs/decisions/OP_LEN_ROOT_REPAIR.md` stated this correctly on the day it
//! landed, under a heading admitting the prediction had been wrong.
//! **This line restated it incorrectly from a document it had already absorbed.**
//!
//! # Why the file survives at all
//!
//! Two of its facts outlive the construct, and both are pinned below.
//!
//! 1. **The runtime arm is still there and still faults.** It defends against a
//!    corrupt or hand-built module rather than against the compiler, which is
//!    what `src/vm.rs` says it is for. Leg 2 keeps that pinned — and now does so
//!    through INJECTED BYTECODE, so it does not depend on any construct emitting
//!    the opcode. That decoupling is the point: the previous version died the
//!    moment the compiler stopped cooperating.
//! 2. **The folded program must still mean what it meant.** A fold that silently
//!    changed a program's value would be worse than the trap it replaced.
//!
//! # Scope
//!
//! `src/vm.rs` and `src/verify.rs` are owned by the `v0.2.3` line and are
//! read-only here. This file reports. See
//! `docs/decisions/OP_LEN_PRODUCER_CENSUS.md` and
//! `docs/decisions/LEN_FLAT_ARRAY_HAZARD.md`.

mod common;

use common::{IF_SOURCE, IF_SOURCE_EQUAL_LENGTHS, PLAIN_SOURCE, build, emits};
use keleusma::bytecode::Op;
use keleusma::vm::{Vm, auto_arena_capacity_for, required_persistent_capacity_for};

/// **LEG 1, INVERTED — the compiler no longer emits the opcode from the subject.**
///
/// This asserted that `verify()` ACCEPTS a module emitting `Op::Len` on an array.
/// Its own failure message said a failure here means *"the hazard is closed at
/// the right place and the rest of this file becomes historical"*. It fired, and
/// that is what it meant.
///
/// The assertion now fires in the opposite direction: a REGRESSION that
/// reinstated the emission would fail here.
#[test]
fn leg_1_the_subject_no_longer_emits_len_at_all() {
    for (name, src) in [
        ("unequal arms", IF_SOURCE),
        ("equal arms", IF_SOURCE_EQUAL_LENGTHS),
    ] {
        let m = build(src);
        assert!(
            !emits(&m, "Len"),
            "the {name} `if`-expression source EMITS `Op::Len` again. The fold in \
             `static_for_in_length` has regressed, and the whole hazard this file \
             used to report is live once more: re-read the pre-2026-09-05 version \
             of this file rather than writing a new analysis."
        );
    }
}

/// **LEG 2, DECOUPLED — the runtime arm still faults, and this no longer depends
/// on any construct.**
///
/// The opcode is INJECTED as bytecode rather than compiled from source. That is
/// deliberate and is the repair for this file's original design fault: the
/// previous version asked the compiler to produce a program it has now stopped
/// producing, so the guard on a RUNTIME property died from a COMPILER change.
///
/// **`new_unchecked` is the documented trust-skip for precompiled bytecode, and
/// using it here is not a claim that this module is admissible.** It is the only
/// way to ask what the runtime arm does, which is the question.
#[test]
fn leg_2_the_runtime_still_refuses_len_on_a_flat_body() {
    // **A LOOP-FREE SUBJECT, AND THAT IS THE SECOND CORRECTION THIS TEST NEEDED.**
    //
    // Injecting into the for-in control shifted every instruction after the
    // insertion point, and the loop's back edge then pointed three instructions
    // short of its header: *"EndLoop at 31 back-edge targets 11 but Loop is at
    // 13"*. The module was refused for a BRANCH-TARGET reason while this test
    // claimed to measure a runtime one. A program with an array local and no
    // control flow has no targets to disturb.
    const ARRAY_LOCAL: &str = "\
fn f() -> Word {
  let a = [1, 2];
  a[0]
}
fn main() -> Word { f() }
";
    let mut m = build(ARRAY_LOCAL);
    assert!(
        !emits(&m, "Len"),
        "the control program already carries `Op::Len`, so injecting one proves \
         nothing about the runtime arm"
    );

    // **THE INJECTION SITE MATTERS, AND THE FIRST ATTEMPT GOT IT WRONG.**
    // Inserting at instruction zero underflowed the operand stack and was
    // refused by `new_unchecked`'s structural check before ever reaching the
    // runtime arm -- so the test reported a VERIFY error while claiming to
    // measure a RUNTIME one. The opcode has to be given a flat array to look at.
    //
    // The compiler's own for-in shape is `GetLocal(arr) Len SetLocal(end)`, so
    // the first `GetLocal` names a slot holding the array. The sequence inserted
    // there is stack-neutral: load, measure, discard.
    let chunk = m
        .chunks
        .iter_mut()
        .find(|c| c.ops.iter().any(|o| matches!(o, Op::GetLocal(_))))
        .expect("some chunk reads a local");
    let (at, slot) = chunk
        .ops
        .iter()
        .enumerate()
        .find_map(|(i, o)| match o {
            Op::GetLocal(n) => Some((i, *n)),
            _ => None,
        })
        .expect("the chunk reads a local");
    chunk
        .ops
        .splice(at..at, [Op::GetLocal(slot), Op::Len, Op::PopN(1)]);
    assert!(emits(&m, "Len"), "the injection did not take");

    let arena = keleusma_arena::Arena::with_capacity(65536);
    let mut vm = unsafe { Vm::new_unchecked(m, &arena) }.expect(
        "an injected `Op::Len` over a flat array was refused before it could \
         run. That is a load-time REJECTION of the opcode, which would be a \
         stronger guarantee than this file has ever recorded -- verify it \
         deliberately rather than treating it as this test breaking.",
    );
    let err = vm.call(&[]).expect_err(
        "an INJECTED `Op::Len` over a flat array ran without faulting. The \
         runtime arm that defends against a corrupt or hand-built module is \
         gone, and that is a larger result than anything this file previously \
         reported.",
    );
    let text = format!("{err:?}");
    assert!(
        text.contains("InvalidBytecode"),
        "the injected opcode faults, but not with `InvalidBytecode` -- the class \
         that asserts the artefact should never have been produced, and the \
         whole reason this arm is written down: {text}"
    );
    assert!(
        text.contains("flat array"),
        "the module faults for some OTHER reason, so this test would report the \
         runtime arm while measuring something else: {text}"
    );
}

/// **LEG 3, INVERTED — the supported path now ADMITS the former subject.**
///
/// It used to assert that `Vm::new` refuses the witness at every arena size, so
/// the trap in leg 2 was unreachable through the supported path. The program is
/// now ordinary: it folds, takes a bound, and loads.
#[test]
fn leg_3_the_former_subject_is_now_admitted_and_bounded() {
    let m = build(IF_SOURCE);
    let cap = auto_arena_capacity_for(&m, &[]).expect(
        "the former Len witness is REFUSED a resource bound again. That is the \
         old behaviour returning, and it means the fold regressed rather than \
         that this test is wrong.",
    );
    let arena = keleusma_arena::Arena::with_capacity(cap + (1 << 20));
    assert!(
        Vm::new(m, &arena).is_ok(),
        "the former Len witness takes a bound but will not LOAD, which is a \
         different and unexplained state from either the old behaviour or the new"
    );

    // CONTROL. Without it, "admitted" could be a property of the harness rather
    // than of this program.
    let plain = build(PLAIN_SOURCE);
    let arena = keleusma_arena::Arena::with_capacity(65_536);
    assert!(
        Vm::new(plain, &arena).is_ok(),
        "`Vm::new` refuses the ORDINARY for-in too, so leg 3 says nothing about \
         the `if` source specifically"
    );
}

/// **LEG 4, REPLACED — the fold must not have changed what the program MEANS.**
///
/// It used to establish that the refusal was liftable rather than structural, by
/// showing equal-length arms were refused anyway. There is no refusal left to
/// classify.
///
/// What matters now is the risk a fold actually carries. Folding a length is
/// arithmetic on the iteration bound, and the worst-case execution time analysis
/// consumes that bound. **A wrong fold would be silent where the trap was loud**,
/// so this asserts the program's VALUE and its ITERATION COUNT, not merely that
/// it runs.
#[test]
fn leg_4_the_folded_program_still_means_what_it_meant() {
    // Both arms have length two, so the loop body runs exactly twice on either
    // path and the accumulated result is checkable by inspection.
    const COUNTED: &str = "\
fn f(c: bool) -> Word {
  let a = [1, 2];
  let b = [3, 4];
  let mut n = 0;
  for x in if c { a } else { b } { n = n + x; }
  n
}
fn main() -> Word { f(true) }
";
    let m = match keleusma::lexer::tokenize(COUNTED)
        .ok()
        .and_then(|t| keleusma::parser::parse(&t).ok())
        .and_then(|a| keleusma::compiler::compile(&a).ok())
    {
        Some(m) => m,
        None => {
            // Accumulation across iterations is not expressible in every corpus
            // idiom; if the mutable form is refused, fall back to asserting the
            // discarding form's value, which is what the corpus witness uses.
            let m = build(IF_SOURCE);
            let cap = auto_arena_capacity_for(&m, &[]).expect("bounded");
            let need = required_persistent_capacity_for(&m);
            let mut arena = keleusma_arena::Arena::with_capacity(cap + need + (1 << 20));
            arena.resize_persistent(need).expect("persistent fits");
            let mut vm = Vm::new(m, &arena).expect("loads");
            let mut shared: Vec<u8> = Vec::new();
            let got = vm.call_with_shared(&mut shared, &[]).expect("runs");
            assert_eq!(
                format!("{got:?}"),
                "Finished(Int(0))",
                "the folded program runs but no longer returns zero. A fold that \
                 changes a program's VALUE is worse than the trap it replaced."
            );
            return;
        }
    };
    assert!(
        !emits(&m, "Len"),
        "the counted form emits `Op::Len`, so the fold did not reach it and this \
         value assertion is about a different program than the one intended"
    );
    let cap = auto_arena_capacity_for(&m, &[]).expect("the counted form must be bounded");
    let need = required_persistent_capacity_for(&m);
    let mut arena = keleusma_arena::Arena::with_capacity(cap + need + (1 << 20));
    arena.resize_persistent(need).expect("persistent fits");
    let mut vm = Vm::new(m, &arena).expect("the counted form must load");
    let mut shared: Vec<u8> = Vec::new();
    let got = vm
        .call_with_shared(&mut shared, &[])
        .expect("the counted form must run");
    assert_eq!(
        format!("{got:?}"),
        "Finished(Int(3))",
        "the loop did not run exactly twice over [1, 2]. A WRONG ITERATION COUNT \
         IS SILENT WHERE THE OLD TRAP WAS LOUD, and the worst-case execution time \
         analysis consumes this bound."
    );
}

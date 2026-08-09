//! The correctness signal for native lowering: identical observable results
//! from native code and from the VM, over the same bytecode.
//!
//! # Why this file is the oracle and not a smoke test
//!
//! Per `docs/roadmap/V0_3_X_ROADMAP.md`, the VM stays as a differential oracle
//! and the bytecode remains the verification artefact. Native lowering must be
//! shown to *preserve* semantics, not to re-establish them.
//!
//! # Read this before adding a case
//!
//! The first version of the lowering carried a defect that `maxi(2, 3)` passed
//! straight through, because that input takes the else path, which happened to
//! be correct. Only `maxi(9, 4)`, which takes the then path, exposed it. **A
//! case that exercises one side of a branch proves nothing about the other.**
//! When adding an opcode, add inputs that distinguish its paths, and satisfy
//! yourself that each new case can actually fail.
//!
//! # Vocabulary
//!
//! "Negative control" is used in two opposite senses in ordinary speech and both
//! appeared in this file, so it is avoided here in favour of an unambiguous pair:
//!
//! | Name | Input | Catches a check that is |
//! |---|---|---|
//! | **must-fire case** | defect known PRESENT | too STRICT (never fires) |
//! | **must-not-fire case** | defect known ABSENT | too LOOSE (fires spuriously) |
//!
//! A structural check needs BOTH. A must-fire case shows the check *can* fire;
//! it cannot show the check fires *only* when it should.

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::{Op, Value};
use keleusma::vm::{Vm, auto_arena_capacity_for};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerOptions, OverflowPolicy, check_word_width, lower_chunk};
use std::collections::BTreeMap;

/// Run `src` on the VM with `args`, returning the finished integer result.
fn vm_result(src: &str, args: &[i64]) -> i64 {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena capacity");
    let arena = keleusma_arena::Arena::with_capacity(cap);
    let mut vm = Vm::new(m, &arena).expect("vm");
    let vals: Vec<Value> = args.iter().map(|&x| Value::Int(x)).collect();
    match vm.call(&vals).expect("vm run") {
        keleusma::vm::VmState::Finished(Value::Int(v)) => v,
        other => panic!("unexpected VM outcome: {other:?}"),
    }
}

/// Compile `src`, lower chunk 0 to native, JIT it, and call it with `args`.
fn native_result(src: &str, args: &[i64]) -> i64 {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    check_word_width(m.word_bits_log2).expect("word width");

    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_chunk(
        &ctx,
        &lm,
        &m.chunks[0],
        "kel_entry",
        LowerOptions::default(),
    )
    .expect("lower");
    lm.verify().expect("LLVM module verification");

    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");
    match args.len() {
        1 => {
            let f = unsafe { ee.get_function::<unsafe extern "C" fn(i64) -> i64>("kel_entry") }
                .expect("symbol");
            unsafe { f.call(args[0]) }
        }
        2 => {
            let f =
                unsafe { ee.get_function::<unsafe extern "C" fn(i64, i64) -> i64>("kel_entry") }
                    .expect("symbol");
            unsafe { f.call(args[0], args[1]) }
        }
        n => panic!("test harness does not yet drive {n}-argument entry points"),
    }
}

/// Lower `src` and return the emitted LLVM IR as text, for assertions about
/// structure that runtime behaviour cannot demonstrate.
fn lowered_ir(src: &str) -> String {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_chunk(
        &ctx,
        &lm,
        &m.chunks[0],
        "kel_entry",
        LowerOptions::default(),
    )
    .expect("lower");
    lm.print_to_string().to_string()
}

fn assert_agrees(src: &str, args: &[i64]) {
    let native = native_result(src, args);
    let vm = vm_result(src, args);
    assert_eq!(
        native, vm,
        "native and VM disagree for {src:?} with {args:?}: native={native}, vm={vm}"
    );
}

#[test]
fn wrapping_addition_agrees_with_the_vm() {
    // Bare `a + b` compiles to `CheckedAdd; PopN(2)`, which discards the flag
    // and the high word. That is wrapping addition, and the i64::MAX case is
    // the one that pins the wrap: without it, any lowering that computed only a
    // 64-bit sum would pass.
    let src = "fn main(a: Word, b: Word) -> Word { a + b }";
    for args in [[2, 3], [-7, 4], [0, 0], [i64::MAX, 1], [i64::MIN, -1]] {
        assert_agrees(src, &args);
    }
}

#[test]
fn structured_control_flow_agrees_with_the_vm_on_both_paths() {
    // BOTH branches, deliberately. `[2, 3]` alone passed while the lowering was
    // broken; `[9, 4]` is what exposed it. The equal case pins the boundary of
    // a strict comparison.
    let src = "fn main(a: Word, b: Word) -> Word { if a > b { a } else { b } }";
    for args in [[2, 3], [9, 4], [-1, -1], [i64::MIN, i64::MAX]] {
        assert_agrees(src, &args);
    }
}

#[test]
fn an_unsupported_opcode_is_refused_rather_than_mislowered() {
    // The subset boundary must fail loudly. A lowering that silently emitted
    // something plausible for an unhandled opcode is the failure mode this
    // whole file exists to prevent, and it would not be caught by any test that
    // only checks supported programs.
    //
    // This case was `a * b` until multiplication entered the subset, at which
    // point the test kept passing for the wrong reason for exactly as long as it
    // took to run it. Division is the current boundary: `Op::Div` needs a
    // zero-divisor guard and an `i64::MIN / -1` guard, both of which are
    // undefined behaviour in LLVM, so it is refused until those exist.
    let src = "fn main(a: Word, b: Word) -> Word { a / b }";
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    let err = lower_chunk(
        &ctx,
        &lm,
        &m.chunks[0],
        "kel_entry",
        LowerOptions::default(),
    );
    assert!(
        err.is_err(),
        "division is outside the supported subset and must be refused, not lowered"
    );
}

/// Every source below returns an *operand* rather than a literal, deliberately.
/// `PushImmediate` is not in the supported subset, so `{ 1 } else { 0 }` would
/// make each of these fail for a reason unrelated to what it is testing.
/// The else branch is `b + b`, not `b`, and that asymmetry is load-bearing.
///
/// An earlier version used `{ a } else { b }`. It passed a must-fire case
/// that swapped `SLT` for `SLE`, because the two predicates differ only when
/// `a == b`, and at `a == b` both branches return the same value. The test was
/// vacuous with respect to exactly the distinction it claimed to draw. With
/// asymmetric branches the equal case separates every strict predicate from its
/// non-strict partner.
const CMP_SOURCES: &[&str] = &[
    "fn main(a: Word, b: Word) -> Word { if a == b { a } else { b + b } }",
    "fn main(a: Word, b: Word) -> Word { if a != b { a } else { b + b } }",
    "fn main(a: Word, b: Word) -> Word { if a < b { a } else { b + b } }",
    "fn main(a: Word, b: Word) -> Word { if a > b { a } else { b + b } }",
    "fn main(a: Word, b: Word) -> Word { if a <= b { a } else { b + b } }",
    "fn main(a: Word, b: Word) -> Word { if a >= b { a } else { b + b } }",
];

#[test]
fn every_comparison_agrees_with_the_vm() {
    // The equal cases are the discriminating ones, and they only discriminate
    // because the branches differ. Confirmed by a must-fire case: swapping SLT for SLE
    // now fails here, and did not before.
    for src in CMP_SOURCES {
        for args in [[2, 3], [9, 4], [5, 5], [i64::MIN, i64::MAX], [-3, -3]] {
            assert_agrees(src, &args);
        }
    }
}

#[test]
fn logical_not_agrees_with_the_vm() {
    let src = "fn main(a: Word, b: Word) -> Word { if not (a > b) { a } else { b } }";
    for args in [[2, 3], [9, 4], [5, 5]] {
        assert_agrees(src, &args);
    }
}

#[test]
fn bitwise_operators_agree_with_the_vm() {
    for src in [
        "fn main(a: Word, b: Word) -> Word { a band b }",
        "fn main(a: Word, b: Word) -> Word { a bor b }",
        "fn main(a: Word, b: Word) -> Word { a bxor b }",
    ] {
        for args in [[0b1100, 0b1010], [0, -1], [-1, -1], [i64::MIN, i64::MAX]] {
            assert_agrees(src, &args);
        }
    }
}

#[test]
fn shift_counts_are_masked_exactly_as_the_vm_masks_them() {
    // THIS IS THE CASE THAT MATTERS. The VM masks the count to the word width,
    // so every count is defined; an unmasked LLVM shift by >= 64 is poison.
    // A test using only counts in 0..64 passes with the mask omitted and would
    // leave undefined behaviour in the lowering for precisely the inputs the VM
    // gives an answer for.
    //
    // NOTE: this test alone does NOT prove the mask is present. See
    // `the_shift_lowering_masks_the_count_structurally` below, and the control
    // result recorded there. It still pins that the lowering AGREES with the VM
    // across the whole count range, which is worth having independently.
    //
    // 64 and 65 wrap to 0 and 1. -1 exercises the VM's `as u32` truncation
    // followed by the mask, which agrees with a plain `& 63` because both look
    // only at the low six bits. 4294967296 is 2^32, which truncates to zero in
    // the VM's u32 cast and masks to zero here.
    for src in [
        "fn main(a: Word, b: Word) -> Word { a asl b }",
        "fn main(a: Word, b: Word) -> Word { a asr b }",
    ] {
        for args in [
            [1, 0],
            [1, 1],
            [1, 63],
            [1, 64],
            [1, 65],
            [-1, 1],
            [i64::MIN, 63],
            [1, -1],
            [1, 4294967296],
            [1, 4294967301],
        ] {
            assert_agrees(src, &args);
        }
    }
}

#[test]
fn the_shift_lowering_masks_the_count_structurally() {
    // RUNTIME BEHAVIOUR CANNOT DEMONSTRATE THIS, which is why the assertion is
    // structural rather than differential.
    //
    // Established by a must-fire case on 2026-08-08: deleting the mask from the
    // lowering left every behavioural shift case passing. AArch64's
    // shift-by-register instruction masks the count to its low six bits in
    // hardware, so an unmasked LLVM shift produces identical results on this
    // target at this optimisation level.
    //
    // The mask is still required. An LLVM shift by at least the bit width is
    // poison, which is a compile-time licence for the optimiser rather than a
    // promise about the hardware, and a different target or optimisation level
    // may exploit it. So the presence of the mask is asserted directly.
    for src in [
        "fn main(a: Word, b: Word) -> Word { a asl b }",
        "fn main(a: Word, b: Word) -> Word { a asr b }",
    ] {
        let ir = lowered_ir(src);

        // MUST-NOT-FIRE CASE: defect known absent. The real lowering masks, so
        // the check must stay silent.
        assert_eq!(
            shift_takes_masked_count(&ir),
            Some(true),
            "the shift must take the MASKED count as its operand, not the raw \
             one. Behavioural tests cannot catch this on AArch64, whose \
             shift-by-register masks in hardware. IR was:\n{ir}"
        );

        // MUST-FIRE CASE: defect known present. Mutate the emitted IR so the
        // shift consumes the raw count and require the check to notice.
        //
        // This encodes permanently a control that was previously an ad-hoc
        // shell edit, run once by hand and preserved nowhere. Without it the
        // predicate could become too strict -- always reporting the mask
        // present -- and the must-not-fire case above would pass forever
        // without noticing, which is exactly how the earlier
        // `ir.contains("and i64")` version survived.
        let mutated: Vec<String> = ir
            .lines()
            .map(|l| {
                if l.contains(" shl i64 ") || l.contains(" ashr i64 ") {
                    l.replace("%shmask", "%rawcount")
                } else {
                    l.to_string()
                }
            })
            .collect();
        assert_eq!(
            shift_takes_masked_count(&mutated.join("\n")),
            Some(false),
            "the mask check does not discriminate: it reported a masked shift \
             for IR whose shift takes the raw count."
        );
    }
}

/// Does the shift instruction take the MASKED count as its operand?
///
/// `None` means no shift was found at all. Returning `Some(false)` is the check
/// FIRING: it has detected the defect.
///
/// Asserting that a mask exists somewhere in the function is not enough. An
/// earlier version checked `ir.contains("and i64")` and passed its must-fire
/// case, because removing the mask from the shift OPERAND leaves the `and`
/// computed and merely unused. The presence of a value says nothing about
/// whether anything reads it.
fn shift_takes_masked_count(ir: &str) -> Option<bool> {
    ir.lines()
        .find(|l| l.contains(" shl i64 ") || l.contains(" ashr i64 "))
        .map(|l| l.contains("%shmask"))
}

/// Loops, and the two structural cases the design predicted.
///
/// `loop { break; }` shows both: the compiler emits the arm's Unit push
/// immediately AFTER the unconditional `Break`, where no edge reaches it, and a
/// `loop` whose exit is reached only by `break` never falls through to it.
/// A data-dependent `break` is REJECTED BY THE VERIFIER, not by the lowering:
/// "no statically extractable iteration bound; strict mode requires loops with
/// fall-through bodies to match the canonical for-range pattern". So the
/// admitted forms are the range `for`, which is that canonical pattern, and a
/// loop whose break is unconditional and therefore trivially bounded.
const LOOP_SOURCES: &[&str] = &[
    "fn main(a: Word, b: Word) -> Word { loop { break; } a }",
    "fn main(a: Word, b: Word) -> Word { for i in 0..3 { } a }",
];

/// Returns true if `ir` contains a branch to a block defined earlier in the
/// text, i.e. a back edge. Written because a loop's ITERATION COUNT is not
/// observable with the current opcode subset: Keleusma locals are immutable
/// ("assignment is only supported for data block fields"), so accumulating
/// across iterations needs a data block, which is a later increment. Without
/// this, a lowering that dropped the loop entirely would pass every
/// differential case.
fn has_back_edge(ir: &str) -> bool {
    // Builds the control-flow graph from LLVM's `preds` annotations and looks
    // for a CYCLE. A block reachable from itself is a loop. That is a graph
    // property, and unlike every earlier attempt it does not depend on the order
    // blocks happen to be printed in.
    //
    // THREE EARLIER VERSIONS USED TEXT ORDER AND ALL THREE WERE WRONG.
    // (1) `strip_suffix(':')` never matched, because LLVM prints
    //     `op5:  ; preds = ...`; it reported no back edge for a real loop.
    // (2) "branches to an earlier-defined block" reported true for any branch to
    //     the `trap` block, which is emitted near the top.
    // (3) "a pred defined later in the text" failed the same way, for the same
    //     block, in the other direction.
    // Each was caught only by running a must-fire case. The lesson is that loop
    // structure is not recoverable from text position; build the graph.
    let mut edges: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut nodes: Vec<&str> = Vec::new();
    for line in ir.lines() {
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        let Some((label, rest)) = line.split_once(':') else {
            continue;
        };
        if label.is_empty() || label.contains(' ') {
            continue;
        }
        nodes.push(label);
        if let Some(preds) = rest.split("preds =").nth(1) {
            for p in preds.split(',') {
                let name = p.trim().trim_start_matches('%');
                if !name.is_empty() {
                    edges.entry(name).or_default().push(label);
                }
            }
        }
    }

    fn reaches<'a>(
        from: &'a str,
        target: &'a str,
        edges: &BTreeMap<&'a str, Vec<&'a str>>,
        seen: &mut Vec<&'a str>,
    ) -> bool {
        if seen.contains(&from) {
            return false;
        }
        seen.push(from);
        for n in edges.get(from).map(|v| v.as_slice()).unwrap_or(&[]) {
            if *n == target || reaches(n, target, edges, seen) {
                return true;
            }
        }
        false
    }

    nodes.iter().any(|n| reaches(n, n, &edges, &mut Vec::new()))
}

#[test]
fn loops_agree_with_the_vm() {
    for src in LOOP_SOURCES {
        for args in [[9, 4], [1, 0], [i64::MAX, 3], [-2, 5]] {
            assert_agrees(src, &args);
        }
    }
}

#[test]
fn the_range_for_lowering_actually_emits_a_back_edge() {
    // The differential cases above pin stack discipline THROUGH a loop, but
    // cannot pin that the loop iterates, for the reason given on
    // `has_back_edge`. Assert the back edge directly.
    let ir = lowered_ir("fn main(a: Word, b: Word) -> Word { for i in 0..3 { } a }");
    assert!(
        has_back_edge(&ir),
        "the range `for` lowering must emit a back edge; a lowering that dropped \
         the loop would pass every behavioural case in this file. IR was:\n{ir}"
    );

    // MUST-NOT-FIRE CASE: defect known absent. Straight-line code has no cycle,
    // so the check must stay silent. Without this the predicate could be
    // trivially true and prove nothing.
    let straight = lowered_ir("fn main(a: Word, b: Word) -> Word { a + b }");
    assert!(
        !has_back_edge(&straight),
        "straight-line code reported a back edge, so the predicate does not \
         discriminate. IR was:\n{straight}"
    );
}

#[test]
fn small_integer_literals_agree_with_the_vm() {
    // PushImmediate encodes Int(0..=15) inline. Both ends of that range, since
    // an off-by-one in the `operand - 4` decode would still pass at one end.
    // These route through `Const` and the chunk constant pool rather than
    // through `PushImmediate`, which was the assumption before probing.
    for src in [
        "fn main(a: Word, b: Word) -> Word { if a > b { 0 } else { 15 } }",
        "fn main(a: Word, b: Word) -> Word { if a > b { 15 } else { 1 } }",
        "fn main(a: Word, b: Word) -> Word { a + 7 }",
    ] {
        for args in [[9, 4], [1, 2], [-5, -5]] {
            assert_agrees(src, &args);
        }
    }
}

// ---------------------------------------------------------------------------
// Integer arithmetic
//
// `Op::Add`, `Op::Sub`, `Op::Mul` and `Op::Neg` are NOT what these exercise,
// and the distinction is not pedantic. Consolidation B narrowed those four
// opcodes away from `Int` operands: the compiler emits `Checked*; PopN(2)` for
// every `Word` expression, and the VM raises a type error if an `Int` ever
// reaches `Op::Add`. Verified by dumping the opcode stream, not inferred from
// the opcode names. So the whole `Word` arithmetic surface is the checked
// family, and the four unchecked opcodes are reachable only for `Byte`,
// `Fixed` and `Float`.
// ---------------------------------------------------------------------------

#[test]
fn wrapping_subtraction_agrees_with_the_vm() {
    // `a - b` is `CheckedSub; PopN(2)`. The underflow cases are what pin the
    // wrap: a lowering that computed a plain 64-bit difference passes every
    // in-range case and differs from nothing until an operand pair leaves the
    // range.
    let src = "fn main(a: Word, b: Word) -> Word { a - b }";
    for args in [
        [7, 3],
        [3, 7],
        [0, 0],
        [i64::MIN, 1],
        [i64::MAX, -1],
        [i64::MIN, i64::MAX],
        [-1, i64::MIN],
    ] {
        assert_agrees(src, &args);
    }
}

#[test]
fn wrapping_multiplication_agrees_with_the_vm() {
    // `a * b` is `CheckedMul(0); PopN(2)`. The `0` is the Q-format fraction-bit
    // count, so zero fraction bits is exactly integer multiply; a non-zero count
    // is fixed-point and is refused by the lowering.
    let src = "fn main(a: Word, b: Word) -> Word { a * b }";
    for args in [
        [6, 7],
        [-6, 7],
        [-6, -7],
        [0, i64::MAX],
        [i64::MAX, 2],
        [i64::MIN, 2],
        [i64::MIN, -1],
        [4611686018427387904, 8],
    ] {
        assert_agrees(src, &args);
    }
}

#[test]
fn wrapping_negation_agrees_with_the_vm() {
    // `-a` is `CheckedNeg; PopN(2)`. `i64::MIN` is the entire point: it is the
    // one input whose negation is not representable, and a lowering that
    // negated in 64 bits rather than 128 loses the overflow outcome silently
    // while still returning the right low word.
    let src = "fn main(a: Word) -> Word { -a }";
    for args in [[5], [-5], [0], [i64::MIN], [i64::MAX]] {
        assert_agrees(src, &args);
    }
}

// ---------------------------------------------------------------------------
// The high word and the outcome flag
//
// Until this increment NOTHING in this file observed either. Every arithmetic
// case went through `Checked*; PopN(2)`, which discards the flag and the high
// word, so a lowering that computed both incorrectly -- or pushed them in the
// wrong order -- passed the entire suite. The handled form is what makes them
// observable, and probing showed it needs no opcode outside the current subset:
// it lowers as a dispatch on the flag built from `Loop`, `CmpEq`, `If` and
// `Break`, all of which already work.
// ---------------------------------------------------------------------------

// Must-fire results for the tests below, run 2026-08-09 by mutating the
// lowering and re-running the suite.
//
// **These are MEASUREMENTS, NOT CONTROLS.** Each was executed once by hand and
// its apparatus discarded, which by this project's own rule does not make it a
// control: nothing re-runs them, so they decay silently as the lowering
// changes. They are recorded because a recorded measurement beats an
// unexecuted claim, not because they close the question.
//
// | Mutation | Result |
// |---|---|
// | push order of `low` and `high` swapped | 8 tests failed, and `loops_agree_with_the_vm` **hung** |
// | high word shifted out by 63 instead of 64 | 2 tests failed |
// | overflow and underflow flag codes swapped | 2 tests failed |
// | multiply performed in 64 bits then widened | 1 test failed |
// | negation performed in 64 bits then widened | 1 test failed |
// | subtract operands reversed | 2 tests failed |
// | arithmetic shift changed to logical | **NOTHING FAILED** |
//
// The last row is not a coverage gap. The mutation is semantically null: the
// truncate to `i64` that follows the shift keeps only bits 0 to 63 of the
// shifted value, and both shift kinds agree there. A mutation that changes no
// behaviour cannot be a must-fire case, and reading it as a gap would have led
// to writing a test that could never fail.
//
// The hang is worth carrying separately. A wrong lowering can emit
// NON-TERMINATING native code -- there, the range `for` counter took the high
// word instead of the low one, so the induction variable never advanced. Any
// future mutation run needs a hard timeout, and on macOS that is `gtimeout`,
// since `timeout` does not exist.

/// The handled form binds `overflow(h, l)` with `h` the high word and `l` the
/// low word. Each pair below returns a DIFFERENT half from each arm, so a
/// lowering that swapped high for low is caught rather than hidden by symmetry.
const HANDLED_SOURCES: &[&str] = &[
    "fn main(a: Word, b: Word) -> Word { a + b { ok(v) => v, overflow(h, l) => h, underflow(h, l) => l } }",
    "fn main(a: Word, b: Word) -> Word { a + b { ok(v) => v, overflow(h, l) => l, underflow(h, l) => h } }",
    "fn main(a: Word, b: Word) -> Word { a * b { ok(v) => v, overflow(h, l) => h, underflow(h, l) => l } }",
    "fn main(a: Word, b: Word) -> Word { a * b { ok(v) => v, overflow(h, l) => l, underflow(h, l) => h } }",
    "fn main(a: Word, b: Word) -> Word { a - b { ok(v) => v, overflow(h, l) => h, underflow(h, l) => l } }",
    "fn main(a: Word, b: Word) -> Word { a - b { ok(v) => v, overflow(h, l) => l, underflow(h, l) => h } }",
];

#[test]
fn the_high_word_and_the_outcome_flag_agree_with_the_vm() {
    // Inputs chosen so that all three flag values arise for each operator, and
    // so that the high word is sometimes 0, sometimes -1, and sometimes neither.
    //
    // A high word of neither 0 nor -1 needs a product: addition and subtraction
    // overflow by at most one bit, so their high word only ever distinguishes
    // sign. `[2^62, 8]` gives a true product of 2^65, whose high word is 2 --
    // the case that pins the shift amount rather than merely its sign.
    for src in HANDLED_SOURCES {
        for args in [
            [3, 4],
            [-3, 4],
            [0, 0],
            [i64::MAX, 1],
            [i64::MIN, -1],
            [i64::MAX, 2],
            [i64::MIN, 2],
            [i64::MIN, 3],
            [4611686018427387904, 8],
            [-4611686018427387904, 8],
        ] {
            assert_agrees(src, &args);
        }
    }
}

#[test]
fn negation_overflow_reaches_the_overflow_arm_and_agrees() {
    // `-i64::MIN` is the only negation that overflows, so this is a
    // one-input test by construction. The in-range values are what show the
    // dispatch does not fire spuriously.
    for src in [
        "fn main(a: Word) -> Word { -a { ok(v) => v, overflow(h, l) => h } }",
        "fn main(a: Word) -> Word { -a { ok(v) => v, overflow(h, l) => l } }",
    ] {
        for args in [[i64::MIN], [i64::MAX], [0], [7], [-7]] {
            assert_agrees(src, &args);
        }
    }
}

// ---------------------------------------------------------------------------
// The trap overflow policy
// ---------------------------------------------------------------------------

/// Lower `src` under explicit options and return the IR, for the trap policy,
/// which by construction has no differential case: it DIVERGES from the VM.
fn lowered_ir_with(src: &str, opts: LowerOptions) -> String {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_chunk(&ctx, &lm, &m.chunks[0], "kel_entry", opts).expect("lower");
    lm.print_to_string().to_string()
}

/// Does the IR branch CONDITIONALLY to the trap block?
///
/// `Op::Trap` emits an unconditional branch to the same block, so the
/// conditional form is what distinguishes the overflow policy from an ordinary
/// trap site. Matching on `br i1` rather than on the block name alone is what
/// makes that distinction.
fn traps_conditionally_on_overflow(ir: &str) -> bool {
    ir.lines()
        .any(|l| l.trim_start().starts_with("br i1 ") && l.contains("label %trap"))
}

#[test]
fn the_trap_overflow_policy_emits_a_guard_on_every_checked_opcode() {
    // The trap policy has NO differential case available, because it is defined
    // as diverging from the VM: `add(i64::MAX, 1)` aborts here and wraps there.
    // The oracle cannot speak to it, so the assertion is structural, and it
    // needs the must-fire / must-not-fire pair for the reason the shift mask
    // needed one.
    //
    // All four checked opcodes are covered because they now share one helper.
    // That sharing is why the guard cannot be present on one and absent on
    // another, and this test is what makes the claim checkable rather than
    // merely stated.
    for src in [
        "fn main(a: Word, b: Word) -> Word { a + b }",
        "fn main(a: Word, b: Word) -> Word { a - b }",
        "fn main(a: Word, b: Word) -> Word { a * b }",
        "fn main(a: Word) -> Word { -a }",
    ] {
        // MUST-FIRE CASE: the policy is on, so the guard must be present.
        let trapping = lowered_ir_with(
            src,
            LowerOptions {
                overflow: OverflowPolicy::Trap,
            },
        );
        assert!(
            traps_conditionally_on_overflow(&trapping),
            "the trap policy must emit a conditional branch to the trap block \
             for {src:?}. IR was:\n{trapping}"
        );

        // MUST-NOT-FIRE CASE: the default policy wraps, matching the VM, so
        // there must be no guard. Without this the predicate could be trivially
        // true and the must-fire case above would prove nothing.
        let wrapping = lowered_ir_with(
            src,
            LowerOptions {
                overflow: OverflowPolicy::Wrap,
            },
        );
        assert!(
            !traps_conditionally_on_overflow(&wrapping),
            "the default wrapping policy must NOT emit an overflow trap for \
             {src:?}; it diverges from the VM. IR was:\n{wrapping}"
        );
    }
}

#[test]
fn a_fixed_point_multiply_is_refused_rather_than_lowered_as_an_integer_one() {
    // `Op::CheckedMul` carries the Q-format fraction-bit count. The lowering
    // matches `CheckedMul(0)` specifically, so a non-zero count falls through to
    // the refusal arm. Lowering it as an integer multiply would be wrong by a
    // factor of 2^n and would produce no error at all, which is the failure mode
    // the subset boundary exists to prevent.
    //
    // If `Fixed` arithmetic ever enters the subset, this test must be changed
    // deliberately -- as the division case above had to be when multiplication
    // arrived -- rather than deleted.
    // A bare `a * b` on `Fixed` emits `Op::FixedMul`, a different opcode
    // entirely; only the handled form reaches `Op::CheckedMul` with a non-zero
    // count. A `Fixed` overflow arm binds one value rather than two halves,
    // because its high word is not meaningful.
    let src = "fn main(a: Fixed<16>, b: Fixed<16>) -> Fixed<16> { a * b { ok(v) => v, overflow(w) => w } }";
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    assert!(
        m.chunks[0]
            .ops
            .iter()
            .any(|op| matches!(op, Op::CheckedMul(n) if *n != 0)),
        "this test is vacuous unless the source actually emits a non-zero \
         fraction-bit CheckedMul; opcodes were {:?}",
        m.chunks[0].ops
    );

    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    let err = lower_chunk(
        &ctx,
        &lm,
        &m.chunks[0],
        "kel_entry",
        LowerOptions::default(),
    );
    assert!(
        err.is_err(),
        "a fixed-point multiply carries a non-zero fraction-bit count and must \
         be refused, not lowered as if the operand were absent"
    );
}

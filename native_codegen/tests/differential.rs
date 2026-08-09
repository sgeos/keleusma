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
use keleusma::bytecode::Value;
use keleusma::vm::{Vm, auto_arena_capacity_for};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerOptions, check_word_width, lower_chunk};
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
    let src = "fn main(a: Word, b: Word) -> Word { a * b }";
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
        "multiplication is outside the supported subset and must be refused, not lowered"
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

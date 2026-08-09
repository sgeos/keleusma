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
    // This case was `a * b`, then `a / b`, and each time the opcode entered the
    // subset the test went on passing for the wrong reason until someone
    // noticed. **It is now self-checking**: it asserts the chosen source really
    // does emit the opcode it names, so when `Op::Call` is implemented this test
    // FAILS and has to be repointed deliberately rather than rotting quietly.
    //
    // `Op::Call` is Group 3: it needs multi-chunk lowering and the symbol
    // mangling scheme, neither of which exists.
    let src = "fn helper(x: Word) -> Word { x }
               fn main(a: Word, b: Word) -> Word { helper(a) }";
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");

    let entry = m
        .chunks
        .iter()
        .find(|c| c.ops.iter().any(|op| matches!(op, Op::Call(_, _))))
        .expect("this test is vacuous unless some chunk emits Op::Call");

    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    let err = lower_chunk(&ctx, &lm, entry, "kel_entry", LowerOptions::default());
    assert!(
        err.is_err(),
        "a call is outside the supported subset and must be refused, not lowered"
    );
}

/// Every source below returns an *operand* rather than a literal, deliberately.
/// The else branch is `b + b`, not `b`, and that asymmetry is load-bearing.
///
/// This comment used to justify the operands by claiming `PushImmediate` was
/// outside the supported subset. **That was false when written and is still
/// false**: the lowering has handled it since the first commit. The operands are
/// worth keeping for the asymmetry reason below, which is the real one, but the
/// stale justification is removed rather than left to be believed. A comment
/// asserting a subset boundary is a claim about code and decays exactly like a
/// test does, with nothing to fail when it stops being true.
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

// ---------------------------------------------------------------------------
// `PushImmediate`, whose integer decode the compiler cannot reach
// ---------------------------------------------------------------------------

/// Run an already-built module on the VM, returning its finished integer.
fn vm_result_of(m: keleusma::bytecode::Module, args: &[i64]) -> i64 {
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena capacity");
    let arena = keleusma_arena::Arena::with_capacity(cap);
    let mut vm = Vm::new(m, &arena).expect("vm");
    let vals: Vec<Value> = args.iter().map(|&x| Value::Int(x)).collect();
    match vm.call(&vals).expect("vm run") {
        keleusma::vm::VmState::Finished(Value::Int(v)) => v,
        other => panic!("unexpected VM outcome: {other:?}"),
    }
}

/// Lower and JIT an already-built module's first chunk.
fn native_result_of(m: &keleusma::bytecode::Module, args: &[i64]) -> i64 {
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
    let f = unsafe { ee.get_function::<unsafe extern "C" fn(i64, i64) -> i64>("kel_entry") }
        .expect("symbol");
    unsafe { f.call(args[0], args[1]) }
}

#[test]
fn the_inline_integer_encoding_agrees_with_the_vm() {
    // **THE REFERENCE COMPILER NEVER EMITS THIS.** Probed across 16 source
    // shapes covering literals, tuples, arrays, structs, enums, matches, calls,
    // shifts, bounded loops and handled arithmetic: the only `PushImmediate`
    // operands emitted are 0 (Unit) and 1/2 (the boolean literals). Every
    // integer literal, including 0 through 15, routes through `Const` and the
    // constant pool instead.
    //
    // So the `operand - 4` decode in the lowering had no reachable caller and no
    // test. An off-by-one there would have been invisible: `small_integer_
    // literals_agree_with_the_vm` looks like it covers this and does not, which
    // is recorded in its own comment.
    //
    // The opcode is still part of the instruction set and a hand-written or
    // future-compiler module may use it, so it is tested here by REWRITING real
    // bytecode -- the same technique the typed-verifier conformance corpus uses.
    // The VM accepts the rewritten module through the ordinary verified path,
    // not through `new_unchecked`, so this is a genuine oracle rather than a
    // trust-skip.
    let src = "fn main(a: Word, b: Word) -> Word { 7 }";

    // Both ends of the range and the boundaries, since an off-by-one in the
    // decode still agrees somewhere in the middle if the error is a shift.
    for imm in [4u8, 5, 6, 11, 18, 19] {
        let base = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
        let mut m = base.clone();
        let mut rewrote = 0;
        for op in m.chunks[0].ops.iter_mut() {
            if matches!(op, Op::Const(_)) {
                *op = Op::PushImmediate(imm);
                rewrote += 1;
            }
        }
        assert_eq!(
            rewrote, 1,
            "this test is vacuous unless exactly one Const load was rewritten; \
             the source's opcodes were {:?}",
            base.chunks[0].ops
        );

        let native = native_result_of(&m, &[1, 2]);
        let vm = vm_result_of(m, &[1, 2]);
        assert_eq!(
            native, vm,
            "native and VM disagree on PushImmediate({imm}): native={native}, vm={vm}"
        );
        // Pin the absolute value too. Agreement alone would be satisfied by two
        // implementations that shared the same wrong offset, and the VM is the
        // oracle precisely because it is independent -- but the encoding is
        // documented, so the documented value is checkable directly.
        assert_eq!(
            vm,
            i64::from(imm) - 4,
            "PushImmediate({imm}) must decode to Int({})",
            imm - 4
        );
    }
}

#[test]
fn the_reserved_and_option_immediates_are_refused() {
    // MUST-NOT-FIRE for the acceptance above: operands outside the integer
    // range must NOT be lowered. `3` is `None`, which needs an Option
    // representation this backend has not settled, and 20 and above are
    // reserved. Inventing a value for either would be the exact failure the
    // subset boundary exists to prevent.
    for imm in [3u8, 20, 255] {
        let src = "fn main(a: Word, b: Word) -> Word { 7 }";
        let mut m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
        for op in m.chunks[0].ops.iter_mut() {
            if matches!(op, Op::Const(_)) {
                *op = Op::PushImmediate(imm);
            }
        }
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
            "PushImmediate({imm}) is outside the integer encoding and must be \
             refused, not given an invented value"
        );
    }
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
//
// A second round covered the division family, and produced one of each of the
// three ways a mutation can fail to fire:
//
// | Mutation | Result |
// |---|---|
// | zero-divisor guard branches past the trap | structural test fired |
// | checked zero divisor puts the quotient in the low slot | fired |
// | unsigned division instead of signed | 2 tests fired |
// | `i64::MIN / -1` guard removed entirely | **nothing fired — REAL GAP** |
// | checked zero divisor reports flag 0 instead of 3 | **nothing fired — VACUOUS TEST** |
// | checked zero divisor leaves a stale high word | **nothing fired — UNOBSERVABLE** |
//
// The three are genuinely different and were nearly conflated:
//
//   * **Real gap.** Removing the `i64::MIN / -1` guard left all 23 behavioural
//     tests passing, because AArch64's SDIV defines that input in hardware to
//     the same value the VM returns. The undefined behaviour is real and the
//     guard is required; no runtime test on this target can see it. Fixed by
//     `the_division_lowering_guards_the_unrepresentable_quotient_structurally`,
//     which now fires on this mutation.
//   * **Vacuous test.** Reporting flag 0 instead of 3 changed nothing, because
//     `ok(v) => v` and `zero_divisor(n) => n` both bind the low slot and on a
//     zero divisor that slot holds the numerator either way. The test could not
//     distinguish the arms it was written to distinguish. Fixed by returning
//     `n + n` from one arm; it now fires.
//   * **Unobservable.** A stale high word on a zero divisor cannot be reached by
//     any Keleusma program: `zero_divisor(n)` binds one value, and the arm that
//     binds a high word is guarded by a different flag. The lowering still
//     matches the VM there, deliberately, so a future language change that
//     exposes the slot does not silently inherit a divergence. **It is not
//     tested and cannot be**, which is stated here rather than left for someone
//     to discover by trusting the count of passing tests.

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

// ---------------------------------------------------------------------------
// The division family
//
// Two things are undefined behaviour in LLVM and neither is undefined in the
// VM: a zero divisor, and `i64::MIN` divided by `-1`. The VM's treatment
// DIFFERS between them and between the checked and unchecked forms, which the
// inventory got wrong until it was executed:
//
// | | zero divisor | `i64::MIN` by `-1` |
// |---|---|---|
// | `Div` | faults | `i64::MIN`, NO fault |
// | `Mod` | faults | `0`, NO fault |
// | `CheckedDiv(0)` | flag 3, numerator in low | flag 1, low `i64::MIN` |
// | `CheckedMod` | flag 3, numerator in low | flag 0, low `0` |
// ---------------------------------------------------------------------------

/// Run `src` on the VM and return the error it raises, or `None` if it finishes.
///
/// Needed because a zero divisor is the one input where the differential oracle
/// cannot be used at all: the VM raises `DivisionByZero` and native aborts
/// through `llvm.trap`, and there is no value to compare. What CAN be checked is
/// that the VM faults, which is what makes trapping the right lowering rather
/// than a guess.
fn vm_error(src: &str, args: &[i64]) -> Option<String> {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena capacity");
    let arena = keleusma_arena::Arena::with_capacity(cap);
    let mut vm = Vm::new(m, &arena).expect("vm");
    let vals: Vec<Value> = args.iter().map(|&x| Value::Int(x)).collect();
    vm.call(&vals).err().map(|e| format!("{e:?}"))
}

#[test]
fn division_and_modulo_agree_with_the_vm_including_the_unrepresentable_quotient() {
    // `i64::MIN / -1` is THE case. Its true quotient is 2^63, which is not
    // representable, so LLVM's `sdiv` is undefined there -- but the VM wraps and
    // returns `i64::MIN` rather than faulting. A lowering that emitted a bare
    // `sdiv` has undefined behaviour on exactly the input the VM answers.
    //
    // The negative operands are not decoration: they pin that both languages
    // truncate toward zero rather than flooring. `-7 / 2` is -3, not -4, and
    // `-7 % 2` is -1, not 1.
    for src in [
        "fn main(a: Word, b: Word) -> Word { a / b }",
        "fn main(a: Word, b: Word) -> Word { a % b }",
    ] {
        for args in [
            [7, 2],
            [-7, 2],
            [7, -2],
            [-7, -2],
            [i64::MIN, -1],
            [i64::MIN, 1],
            [i64::MAX, -1],
            [0, 5],
            [5, 1],
            [i64::MIN, 2],
        ] {
            assert_agrees(src, &args);
        }
    }
}

#[test]
fn a_zero_divisor_faults_in_the_vm_which_is_why_native_traps() {
    // The premise check for the test below. If the VM ever stopped faulting
    // here, trapping natively would become a divergence rather than a match, and
    // the structural assertion would be enforcing the wrong thing while still
    // passing.
    for src in [
        "fn main(a: Word, b: Word) -> Word { a / b }",
        "fn main(a: Word, b: Word) -> Word { a % b }",
    ] {
        assert_eq!(
            vm_error(src, &[7, 0]).as_deref(),
            Some("DivisionByZero"),
            "the VM must fault on a zero divisor for {src:?}"
        );
        // MUST-NOT-FIRE CASE: a non-zero divisor must not fault, or the check
        // above would be satisfied by a VM that faulted on everything.
        assert_eq!(
            vm_error(src, &[7, 2]),
            None,
            "a non-zero divisor must not fault for {src:?}"
        );
    }
}

/// Does the IR guard the divisor against zero before dividing?
///
/// Returns whether a conditional branch to the trap block appears BEFORE the
/// first `sdiv`/`srem` in the text. Textual order is sound here, unlike for the
/// loop back edge, because both instructions are emitted into the same straight
/// line by the same opcode arm with no branch between them -- there is no graph
/// to reconstruct. That is a claim about this lowering, not a general licence.
fn divisor_is_guarded_before_dividing(ir: &str) -> bool {
    let lines: Vec<&str> = ir.lines().collect();
    let div = lines
        .iter()
        .position(|l| l.contains(" sdiv i64 ") || l.contains(" srem i64 "));
    let guard = lines
        .iter()
        .position(|l| l.trim_start().starts_with("br i1 ") && l.contains("label %trap"));
    match (div, guard) {
        (Some(d), Some(g)) => g < d,
        _ => false,
    }
}

#[test]
fn the_zero_divisor_guard_precedes_the_division_structurally() {
    // RUNTIME BEHAVIOUR CANNOT DEMONSTRATE THIS. The differential oracle is
    // unusable for a zero divisor -- the VM faults and native aborts, so there
    // is nothing to compare -- and an UNGUARDED `sdiv` by zero is undefined
    // behaviour, which means it may appear to work. A test that divided by zero
    // and observed a crash would be testing the platform, not the lowering.
    for src in [
        "fn main(a: Word, b: Word) -> Word { a / b }",
        "fn main(a: Word, b: Word) -> Word { a % b }",
    ] {
        // MUST-FIRE CASE: mutate the IR so the guard branches somewhere other
        // than the trap block, and require the check to notice. Encoded rather
        // than run once by hand, because an unencoded control decays.
        let ir = lowered_ir(src);
        let mutated: Vec<String> = ir
            .lines()
            .map(|l| {
                if l.trim_start().starts_with("br i1 ") && l.contains("label %trap") {
                    l.replace("label %trap", "label %nonzerodivisor")
                } else {
                    l.to_string()
                }
            })
            .collect();
        assert!(
            !divisor_is_guarded_before_dividing(&mutated.join("\n")),
            "the guard check does not discriminate: it reported a guarded \
             divisor for IR whose branch does not reach the trap block."
        );

        // MUST-NOT-FIRE CASE: the real lowering guards, so the check must be
        // satisfied.
        assert!(
            divisor_is_guarded_before_dividing(&ir),
            "the divisor must be tested against zero and branch to the trap \
             block BEFORE the division for {src:?}. IR was:\n{ir}"
        );
    }
}

/// Does the division instruction take the GUARDED divisor as its operand?
///
/// `None` means no division was found. `Some(false)` is the check FIRING.
///
/// Same shape as [`shift_takes_masked_count`], and for the same reason: a
/// must-fire case that removed the `i64::MIN / -1` guard entirely left all 23
/// behavioural tests passing. AArch64's `SDIV` is architecturally defined to
/// return `i64::MIN` for that input, which is the answer the VM gives, so the
/// undefined behaviour does not manifest on this target at this optimisation
/// level. The guard is still required — undefined behaviour is a licence for the
/// optimiser rather than a promise about the hardware — so its presence is
/// asserted directly.
fn division_takes_guarded_divisor(ir: &str) -> Option<bool> {
    ir.lines()
        .find(|l| l.contains(" sdiv i64 ") || l.contains(" srem i64 "))
        .map(|l| l.contains("%safediv"))
}

#[test]
fn the_division_lowering_guards_the_unrepresentable_quotient_structurally() {
    for src in [
        "fn main(a: Word, b: Word) -> Word { a / b }",
        "fn main(a: Word, b: Word) -> Word { a % b }",
    ] {
        let ir = lowered_ir(src);

        // MUST-NOT-FIRE CASE: the real lowering guards, so the check is silent.
        assert_eq!(
            division_takes_guarded_divisor(&ir),
            Some(true),
            "the division must take the GUARDED divisor, not the raw one. \
             Behavioural tests cannot catch this on AArch64, whose SDIV defines \
             i64::MIN / -1 in hardware. IR was:\n{ir}"
        );

        // MUST-FIRE CASE: mutate the IR so the division consumes the raw
        // divisor, and require the check to notice. Without this the predicate
        // could be too strict and always report the guard present.
        let mutated: Vec<String> = ir
            .lines()
            .map(|l| {
                if l.contains(" sdiv i64 ") || l.contains(" srem i64 ") {
                    l.replace("%safediv", "%pop1")
                } else {
                    l.to_string()
                }
            })
            .collect();
        assert_eq!(
            division_takes_guarded_divisor(&mutated.join("\n")),
            Some(false),
            "the guard check does not discriminate: it reported a guarded \
             divisor for IR whose division takes the raw one."
        );
    }
}

/// The checked division forms, where a zero divisor is DATA rather than a fault.
///
/// This is the one place the differential oracle can cover a zero divisor at
/// all, because the handled form binds it instead of trapping.
/// The `zero_divisor` arm returns `n + n`, not `n`, and that asymmetry is
/// load-bearing for the same reason `CMP_SOURCES` returns `b + b`.
///
/// With `=> n` the test was VACUOUS with respect to the flag. A must-fire case
/// that reported flag 0 instead of 3 changed nothing observable, because the
/// `ok(v)` arm binds the low slot, the `zero_divisor(n)` arm binds the low slot,
/// and on a zero divisor the low slot holds the numerator either way. Both arms
/// returned the same value, so taking the wrong one was undetectable. Doubling
/// in one arm separates them for every non-zero numerator.
const CHECKED_DIV_SOURCES: &[&str] = &[
    "fn main(a: Word, b: Word) -> Word { a / b { ok(v) => v, overflow(h, l) => h, zero_divisor(n) => n + n } }",
    "fn main(a: Word, b: Word) -> Word { a / b { ok(v) => v, overflow(h, l) => l, zero_divisor(n) => n + n } }",
    "fn main(a: Word, b: Word) -> Word { a % b { ok(v) => v, zero_divisor(n) => n + n } }",
];

#[test]
fn the_checked_division_forms_agree_with_the_vm_including_a_zero_divisor() {
    // `[7, 0]` and `[i64::MIN, 0]` reach the `zero_divisor(n)` arm, which binds
    // the NUMERATOR from the low slot. A lowering that put anything else there
    // -- zero, or the quotient -- passes every non-zero case and fails only
    // here.
    //
    // `[i64::MIN, -1]` is the other corner, and the two forms differ on it:
    // checked division reports overflow with the wrapped quotient, while checked
    // modulo reports success with 0. Both are covered because the sources above
    // include each operator.
    for src in CHECKED_DIV_SOURCES {
        for args in [
            [7, 2],
            [-7, 2],
            [7, 0],
            [0, 0],
            [i64::MIN, 0],
            [i64::MAX, 0],
            [i64::MIN, -1],
            [i64::MIN, 1],
            [-1, 0],
        ] {
            assert_agrees(src, &args);
        }
    }
}

#[test]
fn a_fixed_point_divide_is_refused_rather_than_lowered_as_an_integer_one() {
    // The same boundary as the fixed-point multiply: `CheckedDiv` carries the
    // Q-format fraction-bit count and only zero is integer division.
    let src = "fn main(a: Fixed<16>, b: Fixed<16>) -> Fixed<16> { a / b { ok(v) => v, overflow(w) => w, zero_divisor(n) => n } }";
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    assert!(
        m.chunks[0]
            .ops
            .iter()
            .any(|op| matches!(op, Op::CheckedDiv(n) if *n != 0)),
        "this test is vacuous unless the source actually emits a non-zero \
         fraction-bit CheckedDiv; opcodes were {:?}",
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
        "a fixed-point divide carries a non-zero fraction-bit count and must be \
         refused, not lowered as if the operand were absent"
    );
}

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

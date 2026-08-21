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
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetMachine};
use keleusma::bytecode::{Op, Value};
use keleusma::vm::{Vm, auto_arena_capacity_for};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{
    LowerError, LowerOptions, MAX_STACK, OverflowPolicy, check_word_width, lower_chunk,
    lower_module,
};
use std::collections::BTreeMap;

mod common;

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

    // **THE PRECONDITION THIS HARNESS ALWAYS HAD AND NEVER CHECKED.**
    //
    // `vm_result` calls the module's ENTRY POINT. This function lowers
    // `chunks[0]`. For a single-function program those are the same chunk, which
    // is why every test here has been valid. **For a multi-function program they
    // are DIFFERENT FUNCTIONS**, and `assert_agrees` then compares one function
    // against another and reports agreement or disagreement about nothing.
    //
    // Measured, by writing one: a two-function fixed-point test compared
    // `add_fx` natively against `main` in the virtual machine and PASSED --
    // because scaling is linear, so `((a<<16) + (b<<16)) >> 16` equals `a + b`
    // and the two happened to coincide. A false pass by mathematical accident is
    // exactly the shape this line keeps finding, and it was self-inflicted.
    //
    // The precondition is now enforced rather than assumed. A multi-function
    // program fails here with an explanation instead of producing a number.
    assert_eq!(
        m.entry_point,
        Some(0),
        "this harness lowers chunks[0] natively and calls the ENTRY POINT in the \
         virtual machine. They coincide only for a single-function program. This \
         source has its entry at {:?}, so the two sides would run DIFFERENT \
         FUNCTIONS and any agreement would be about nothing. Use the corpus \
         differential, which drives whole modules, for anything multi-function.",
        m.entry_point
    );

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

    common::maybe_optimize(&lm);
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
    // This case was `a * b`, then `a / b`, then `Op::Call`, and each time the
    // opcode entered the subset the test went on passing for the wrong reason.
    // The self-checking assertion below was added to stop that, and **it did not
    // stop it** for `Op::Call`: calls became supported through `lower_module`
    // while `lower_chunk` still refused them, so the test kept passing on a
    // refusal that no longer meant "outside the subset". A self-check on the
    // OPCODE is not a self-check on the REASON.
    //
    // **This paragraph is rewritten whole each time the subject moves, not
    // appended to.** Four successive appends left a running commentary in which
    // each sentence contradicted the one before it, and a reader could not tell
    // which was current.
    //
    // **BOUNDARY MOVED 2026-08-13, and only after the oracle agreed.** A static
    // string constant was the subject. `lower_module` now lowers one, and the
    // five string cases in `native_calls.rs` are the evidence — including an
    // interior NUL, which is what proves the length is carried rather than a C
    // string's terminator. Moving a must-not-fire boundary because an admission
    // merely looks safe is exactly what this file exists to prevent.
    //
    // Subjects so far, each retired the moment a differential agreed with the
    // virtual machine: composite construction, array indexing, nested composite
    // reads, tuple fields, static string constants. **The subject is now a
    // `Float` constant.**
    //
    // It was chosen by running `probe_unsupported`, as every subject since the
    // second has been, after four consecutive guesses cost four compile-and-run
    // cycles. That run mattered here: when native calls and static strings both
    // entered the subset, EVERY case the probe carried began reporting LOWERS,
    // so the probe was extended with five candidates first. Three of them —
    // all three stream shapes — turned out to be REJECTED BY THE REFERENCE
    // COMPILER rather than refused by this backend, which is not a subset
    // boundary at all and would have made this test assert nothing about the
    // lowering. The probe distinguishes those two outcomes; a guess would not
    // have.
    let src = "fn main(a: Word, b: Word) -> Word { let f = 1.5; a + b }";
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");

    // **The vacuity guard is on the REFUSAL, not on a chunk search.** Three
    // attempts to locate the offending op by pattern or by debug rendering
    // reported "no such op" for a module `lower_module` demonstrably refuses by
    // name — a test defect that reads exactly like a corpus fact. Asserting that
    // the refusal names the construct is stronger anyway: it pins WHICH opcode is
    // unsupported, which a chunk search never did.
    let ctx = Context::create();
    let lm2 = ctx.create_module("kel2");
    let err = lower_module(&ctx, &lm2, &m, LowerOptions::default()).expect_err(
        "lower_module must refuse a Float constant; a refusal that only \
             lower_chunk makes is not evidence the opcode is unsupported, which is \
             how the Op::Call version of this test rotted",
    );
    let rendered = format!("{err:?}");
    assert!(
        rendered.contains("Float"),
        "refused for the wrong reason: {rendered}"
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
    common::maybe_optimize(&lm);
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

// ---------------------------------------------------------------------------
// Calls, and therefore multi-function programs
// ---------------------------------------------------------------------------

/// Compile `src`, lower EVERY chunk, and JIT the chunk named `entry`.
///
/// Selecting by name rather than by index is deliberate. Chunk order follows
/// declaration order, so indexing would silently test the wrong function the
/// moment a source declares its helpers after its entry point.
fn native_result_multi(src: &str, entry: &str, args: &[i64]) -> i64 {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let idx = m
        .chunks
        .iter()
        .position(|c| c.name == entry)
        .unwrap_or_else(|| {
            panic!(
                "no chunk named {entry:?}; chunks are {:?}",
                m.chunks.iter().map(|c| &c.name).collect::<Vec<_>>()
            )
        });

    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("lower module");
    lm.verify().expect("LLVM module verification");

    common::maybe_optimize(&lm);
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");
    let sym = format!("kel_chunk_{idx}");
    match args.len() {
        1 => {
            let f = unsafe { ee.get_function::<unsafe extern "C" fn(i64) -> i64>(&sym) }
                .expect("symbol");
            unsafe { f.call(args[0]) }
        }
        2 => {
            let f = unsafe { ee.get_function::<unsafe extern "C" fn(i64, i64) -> i64>(&sym) }
                .expect("symbol");
            unsafe { f.call(args[0], args[1]) }
        }
        n => panic!("test harness does not yet drive {n}-argument entry points"),
    }
}

fn assert_multi_agrees(src: &str, args: &[i64]) {
    let native = native_result_multi(src, "main", args);
    let vm = vm_result(src, args);
    assert_eq!(
        native, vm,
        "native and VM disagree for {src:?} with {args:?}: native={native}, vm={vm}"
    );
}

#[test]
fn calls_agree_with_the_vm() {
    // **ARGUMENT ORDER IS THE WHOLE RISK HERE.** The VM sets the callee's frame
    // base to `len - arg_count`, so arguments sit in declaration order with the
    // last on top, and popping yields them reversed. A lowering that forgot the
    // reversal is invisible for a one-argument callee, and invisible for any
    // call whose arguments happen to be equal.
    //
    // `sub2(a, b)` is therefore the load-bearing case: it is asymmetric in its
    // parameters, so swapping them changes the answer for every input where
    // `a != b`.
    for src in [
        "fn id(x: Word) -> Word { x }
         fn main(a: Word, b: Word) -> Word { id(a) + b }",
        "fn sub2(x: Word, y: Word) -> Word { x - y }
         fn main(a: Word, b: Word) -> Word { sub2(a, b) }",
        // Argument EXPRESSIONS, not just operands, so the operand stack is
        // non-trivial at the call site.
        "fn sub2(x: Word, y: Word) -> Word { x - y }
         fn main(a: Word, b: Word) -> Word { sub2(a + 1, b * 2) }",
        // A call in one arm of a branch: the call must not disturb the
        // per-block operand depth bookkeeping.
        "fn dbl(x: Word) -> Word { x + x }
         fn main(a: Word, b: Word) -> Word { if a > b { dbl(a) } else { b } }",
        // Depth two through the acyclic call graph, and a callee declared AFTER
        // its caller so the forward declaration is exercised.
        "fn outer(x: Word) -> Word { inner(x) + 1 }
         fn inner(x: Word) -> Word { x * 3 }
         fn main(a: Word, b: Word) -> Word { outer(a) - b }",
    ] {
        for args in [
            [7, 3],
            [3, 7],
            [-5, 2],
            [0, 0],
            [i64::MAX, 1],
            [i64::MIN, -1],
        ] {
            assert_multi_agrees(src, &args);
        }
    }
}

#[test]
fn a_call_is_still_refused_when_only_one_chunk_is_lowered() {
    // `lower_chunk` cannot resolve a call: the target is an index into the
    // module's chunk table, which a single chunk does not carry. Refusing is the
    // only correct answer, and this pins that adding `lower_module` did not
    // quietly make the single-chunk path emit a call to nothing.
    let src = "fn helper(x: Word) -> Word { x }
               fn main(a: Word, b: Word) -> Word { helper(a) }";
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let caller = m
        .chunks
        .iter()
        .find(|c| c.ops.iter().any(|op| matches!(op, Op::Call(_, _))))
        .expect("this test is vacuous unless some chunk emits Op::Call");

    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    assert!(
        lower_chunk(&ctx, &lm, caller, "kel_entry", LowerOptions::default()).is_err(),
        "lower_chunk must refuse a call it cannot resolve"
    );

    // MUST-NOT-FIRE CASE: the same program through `lower_module` must succeed,
    // or the refusal above would be indistinguishable from calls being broken.
    let lm2 = ctx.create_module("kel2");
    assert!(
        lower_module(&ctx, &lm2, &m, LowerOptions::default()).is_ok(),
        "lower_module must resolve the same call"
    );
}

// ---------------------------------------------------------------------------
// Byte conversions and the bounds check
// ---------------------------------------------------------------------------

#[test]
fn byte_conversions_agree_with_the_vm() {
    // A `Byte` occupies a full i64 slot holding 0..=255, which is what makes
    // `ByteToWord` a no-op. That representation is not arbitrary: the `v0.2.3`
    // session measured that `Byte as Word` ZERO-extends, so `0xFF` reads as
    // 255 rather than -1. A sign-extending lowering would agree on every input
    // below 128 and differ on exactly half the range.
    //
    // `-1` and `256` are the cases that pin the mask. `i64::MAX` pins it at the
    // other end, and `255`/`128` straddle the sign bit of the byte, which is
    // where a sign-extending implementation first diverges.
    for src in [
        "fn main(a: Word, b: Word) -> Word { let x = a as Byte; x as Word }",
        "fn main(a: Word, b: Word) -> Word { let x = (a + b) as Byte; x as Word }",
    ] {
        for args in [
            [0, 0],
            [1, 0],
            [127, 0],
            [128, 0],
            [255, 0],
            [256, 0],
            [-1, 0],
            [-128, 0],
            [i64::MAX, 0],
            [i64::MIN, 0],
            [300, 0],
        ] {
            assert_agrees(src, &args);
        }
    }
}

/// Build a module whose entry chunk is `GetLocal(0); BoundsCheck(bound); Return`.
///
/// `Op::BoundsCheck` is emitted by the reference compiler ONLY for multi-level
/// data-segment indexing, and the data-segment opcodes are Workstream D and
/// unsupported here, so no compilable source reaches it. Rewriting real
/// bytecode is the same technique used for `PushImmediate`'s integer encoding,
/// and for the same reason: the opcode is part of the instruction set whether or
/// not today's compiler emits it.
fn bounds_check_module(bound: u16) -> keleusma::bytecode::Module {
    let src = "fn main(a: Word, b: Word) -> Word { a }";
    let mut m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    m.chunks[0].ops = vec![Op::GetLocal(0), Op::BoundsCheck(bound), Op::Return];
    m
}

#[test]
fn an_in_range_bounds_check_passes_the_operand_through_unchanged() {
    // **`BoundsCheck` PEEKS, it does not pop.** The VM reads `stack.last()` and
    // leaves the operand for the indexing opcode that follows. A lowering that
    // consumed it would leave `Return` reading the wrong slot, and the failure
    // would surface as a wrong VALUE rather than as an obvious stack error.
    //
    // That is exactly what this checks: the operand must survive the check and
    // still be what `Return` yields.
    for (bound, idx) in [(10u16, 0i64), (10, 9), (1, 0), (65535, 65534)] {
        let m = bounds_check_module(bound);
        let native = native_result_of(&m, &[idx, 0]);
        let vm = vm_result_of(m, &[idx, 0]);
        assert_eq!(
            (native, vm),
            (idx, idx),
            "an in-range index must pass through unchanged for bound={bound}, idx={idx}"
        );
    }
}

#[test]
fn an_out_of_range_bounds_check_faults_in_the_vm_which_is_why_native_traps() {
    // The differential oracle cannot cover the failing case: the VM raises
    // `IndexOutOfBounds` and native aborts through `llvm.trap`, so there is no
    // value to compare. What is checkable is that the VM really does fault,
    // which is what makes trapping the right lowering rather than a guess.
    //
    // **The negative index is the case that matters.** The lowering folds both
    // failure directions into ONE unsigned compare, which is only correct
    // because a negative i64 reinterpreted as unsigned is enormous. A signed
    // compare against the bound would accept every negative index silently.
    for (bound, idx) in [(10u16, 10i64), (10, 11), (1, 1), (10, -1), (10, i64::MIN)] {
        let m = bounds_check_module(bound);
        let cap = auto_arena_capacity_for(&m, &[]).expect("arena capacity");
        let arena = keleusma_arena::Arena::with_capacity(cap);
        let mut vm = Vm::new(m, &arena).expect("vm");
        let err = vm.call(&[Value::Int(idx), Value::Int(0)]).err();
        assert!(
            matches!(err, Some(keleusma::vm::VmError::IndexOutOfBounds(_, _))),
            "the VM must fault for bound={bound}, idx={idx}; got {err:?}"
        );
    }

    // MUST-NOT-FIRE CASE: an in-range index must NOT fault, or the assertion
    // above is satisfied by a check that rejects everything.
    let m = bounds_check_module(10);
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena capacity");
    let arena = keleusma_arena::Arena::with_capacity(cap);
    let mut vm = Vm::new(m, &arena).expect("vm");
    assert!(
        vm.call(&[Value::Int(5), Value::Int(0)]).is_ok(),
        "an in-range index must not fault"
    );
}

#[test]
fn the_bounds_check_guards_with_an_unsigned_compare() {
    // Structural, because the failing path cannot be executed differentially.
    // The predicate must be UNSIGNED: `icmp uge`. With `sge` every negative
    // index passes the guard and reaches an out-of-bounds access, and no
    // in-range differential case would ever notice.
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    let m = bounds_check_module(10);
    lower_chunk(
        &ctx,
        &lm,
        &m.chunks[0],
        "kel_entry",
        LowerOptions::default(),
    )
    .expect("lower");
    let ir = lm.print_to_string().to_string();

    let guard = ir
        .lines()
        .find(|l| l.contains("icmp") && l.contains("oob"))
        .unwrap_or_else(|| panic!("no bounds-check compare found. IR was:\n{ir}"));
    assert!(
        guard.contains("icmp uge"),
        "the bounds check must use an UNSIGNED compare so a negative index is \
         caught; a signed compare accepts every negative value. Found: {guard}"
    );
    assert!(
        ir.lines()
            .any(|l| l.trim_start().starts_with("br i1 ") && l.contains("label %trap")),
        "the bounds check must branch to the trap block. IR was:\n{ir}"
    );
}

// ---------------------------------------------------------------------------
// The middle end is load-bearing, and this encodes that rather than recording it
// ---------------------------------------------------------------------------

/// Count the operand-stack allocas in a module's IR.
fn alloca_count(ir: &str) -> usize {
    ir.lines().filter(|l| l.contains("alloca i64")).count()
}

#[test]
fn mem2reg_removes_every_operand_slot_alloca() {
    // **WHY THIS IS A TEST AND NOT A NOTE.** The lowering models the operand
    // stack as allocas and relies on `mem2reg` to build SSA form. Measured on
    // 2026-08-09 for `thumbv7em-none-eabihf`, the difference that pass makes to
    // the stack frame is 536 bytes against 0 for `a + b`, and 616 against 20 for
    // a handled multiply. 512 of those bytes are `MAX_STACK` slots the program
    // never touches.
    //
    // The trap that measurement exposed is that the frame is decided by WHICH
    // TOOL RUNS, not by the optimisation level: `llc` at `-O0`, `-O1`, `-O2` and
    // `-Os` all give 536, because `mem2reg` is a middle-end pass and `llc` does
    // not run it. The lowering's own documentation had asserted the opposite.
    //
    // So the reliance is asserted here. If a future change makes an alloca
    // survive `mem2reg` -- a slot address escaping, or an alloca emitted outside
    // the entry block where it is no longer promotable -- this fails, instead of
    // the frame quietly growing by half a kilobyte per function on a part that
    // may have four kilobytes in total.
    Target::initialize_native(&InitializationConfig::default()).expect("init native target");
    let triple = TargetMachine::get_default_triple();
    let machine = Target::from_triple(&triple)
        .expect("target")
        .create_target_machine(
            &triple,
            "generic",
            "",
            OptimizationLevel::Default,
            RelocMode::Default,
            CodeModel::Default,
        )
        .expect("target machine");

    for src in [
        "fn main(a: Word, b: Word) -> Word { a + b }",
        "fn main(a: Word, b: Word) -> Word { if a > b { a * b } else { b - a } }",
        "fn main(a: Word, b: Word) -> Word { for i in 0..10 { } a / b }",
        "fn main(a: Word, b: Word) -> Word { a * b { ok(v) => v, overflow(h, l) => h, underflow(h, l) => l } }",
    ] {
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

        // MUST-NOT-FIRE for the assertion below: the allocas have to be there
        // BEFORE the pass, or "none afterwards" is satisfied by a lowering that
        // never emitted any and the test proves nothing about `mem2reg`.
        //
        // **The bound moved on 2026-08-12 and this is the deliberate record.**
        // It read `before > MAX_STACK`, because the lowering provisioned all 64
        // operand slots unconditionally. The comment at the top of this test
        // already named that as waste: "512 of those bytes are `MAX_STACK` slots
        // the program never touches". Slots are now allocated on demand, so the
        // count is proportional to what the chunk uses and the old assertion
        // would fail for the right reason.
        //
        // The must-not-fire property is unchanged and is what is asserted: SOME
        // alloca must exist before the pass.
        let before = alloca_count(&lm.print_to_string().to_string());
        assert!(
            before > 0,
            "the unoptimised lowering must emit operand-slot allocas for \
             `mem2reg` to promote; found none for {src:?}"
        );
        // And the improvement itself, pinned so it cannot silently regress to
        // fixed provisioning. These programs use a handful of slots; the
        // ceiling is 64.
        assert!(
            before < MAX_STACK,
            "operand slots are allocated on demand, so a small program must not \
             pay for all {MAX_STACK} of them; got {before} for {src:?}"
        );

        lm.run_passes("mem2reg", &machine, PassBuilderOptions::create())
            .expect("mem2reg");

        let after = alloca_count(&lm.print_to_string().to_string());
        assert_eq!(
            after, 0,
            "mem2reg must promote every operand-slot alloca for {src:?}; \
             {after} of {before} survived. Each survivor is 8 bytes of stack \
             frame in every compiled function."
        );
    }
}

#[test]
fn an_operand_stack_deeper_than_the_provisioning_is_refused_not_panicked() {
    // `MAX_STACK` is a provisioning decision made by this backend, not a limit
    // the language imposes. Its doc comment used to assert that exceeding it
    // "is a lowering bug, not a program error" -- an assumption with nothing
    // enforcing it. What actually happened was a `Vec` index panic inside a
    // library, which is the worst of the three possible outcomes.
    //
    // The verifier already computes the true figure per module as
    // `RuntimeFootprint::max_operand_slots`, so a caller can raise the
    // provisioning deliberately. Refusing is what makes that possible.
    let src = "fn main(a: Word, b: Word) -> Word { 7 }";
    let base = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let konst = base.chunks[0]
        .ops
        .iter()
        .find(|op| matches!(op, Op::Const(_)))
        .copied()
        .expect("the source must contain a Const load for this rewrite");

    let mut m = base.clone();
    let deep = MAX_STACK + 8;
    m.chunks[0].ops = core::iter::repeat_n(konst, deep)
        .chain(core::iter::once(Op::Return))
        .collect();

    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    match lower_chunk(
        &ctx,
        &lm,
        &m.chunks[0],
        "kel_entry",
        LowerOptions::default(),
    ) {
        Err(LowerError::OperandStackTooDeep {
            needed,
            provisioned,
        }) => {
            assert_eq!(provisioned, MAX_STACK);
            assert!(
                needed > MAX_STACK,
                "the reported requirement {needed} must exceed the provisioning"
            );
        }
        Err(other) => panic!("expected OperandStackTooDeep, got {other:?}"),
        Ok(_) => panic!("a chunk needing {deep} slots must be refused, not lowered"),
    }

    // MUST-NOT-FIRE CASE: a chunk that fits must still lower. Without this the
    // refusal could be unconditional and the test would pass while the backend
    // rejected everything.
    let mut shallow = base.clone();
    shallow.chunks[0].ops = core::iter::repeat_n(konst, 4)
        .chain(core::iter::once(Op::Return))
        .collect();
    let lm2 = ctx.create_module("kel2");
    assert!(
        lower_chunk(
            &ctx,
            &lm2,
            &shallow.chunks[0],
            "kel_entry",
            LowerOptions::default(),
        )
        .is_ok(),
        "a chunk within the provisioning must lower"
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
                ..LowerOptions::default()
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
                ..LowerOptions::default()
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
// ---------------------------------------------------------------------------
// PREPARED WHILE THE GATE RAN — append to native_codegen/tests/differential.rs
// after applying apply_queued_fixes.py. Not yet compiled.
//
// Both controls MUST-FIRE against the unfixed lowering:
//   - without Fix 1, `undef` reaches the return and the value is arbitrary;
//   - without Fix 2, the final block has no terminator and `lm.verify()` fails.
//
// Both mutate REAL compiled bytecode rather than hand-building a Module, which
// is the technique the typed-verifier conformance corpus already uses. Hand
// construction would need every Chunk and Module field right, and a field I got
// wrong would make the test measure my construction rather than the lowering.
//
// The expected values come from the VM, not from me. That is deliberate: these
// were written alongside the fixes rather than after them, and an assertion of
// my own expectation written at the same moment as the code would encode the
// same mistake twice. A differential oracle cannot.
//
// KNOWN COMPILE RISKS, written down rather than discovered:
//
//  1. `mutate` is passed to BOTH helpers, but each takes `impl FnOnce` by
//     value. This compiles only because both closures capture nothing and
//     non-capturing closures are `Copy`. If either ever captures, change the
//     bound to `impl Fn(..) + Copy` or pass it twice explicitly.
//  2. `differential.rs` imports `keleusma::bytecode::{Op, Value}` but not
//     `Module`; the full path is used below to avoid touching the import list.
//  3. The mutated module keeps `signatures[0]` from BEFORE the mutation, so the
//     recorded return shape no longer matches what the chunk returns. The typed
//     pass validates offsets rather than return-type agreement, so this should
//     be accepted — but it is an assumption, not a certainty.
//
// AND THE POINT ON WHICH THIS IS FALSIFIABLE: if `Vm::new` REJECTS either
// mutated module, then `verify()` does not admit these chunks and the inventory
// section claiming it does is WRONG. That would be a useful result, not a broken
// test. The `expect` message says so so the failure reads correctly.
// ---------------------------------------------------------------------------

/// Compile `src`, hand the module to `mutate`, then run the mutated module on
/// the VM. Returns the finished value.
///
/// `Vm::new` runs `verify()`, so a module that reaches execution here has been
/// ADMITTED by the verifier. Both controls below depend on that.
fn vm_result_mutated(src: &str, mutate: impl FnOnce(&mut keleusma::bytecode::Module)) -> Value {
    let mut m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    mutate(&mut m);
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena capacity");
    let arena = keleusma_arena::Arena::with_capacity(cap);
    let mut vm = Vm::new(m, &arena).expect("verify() ADMITTED the mutated module");
    match vm.call(&[]).expect("vm run") {
        keleusma::vm::VmState::Finished(v) => v,
        other => panic!("unexpected VM outcome: {other:?}"),
    }
}

/// Same mutation, lowered and JITed. Returns the native result.
fn native_result_mutated(src: &str, mutate: impl FnOnce(&mut keleusma::bytecode::Module)) -> i64 {
    let mut m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    mutate(&mut m);
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("lower");
    // Fix 3 makes `lower_module` verify internally; this stays as the explicit
    // statement of what the control is about.
    lm.verify().expect("LLVM module verification");
    common::maybe_optimize(&lm);
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");
    let f =
        unsafe { ee.get_function::<unsafe extern "C" fn() -> i64>("kel_chunk_0") }.expect("symbol");
    unsafe { f.call() }
}

/// MUST-FIRE for Fix 1: a `GetLocal` of a slot nothing ever wrote.
///
/// `Op::Call` fills a callee frame's non-parameter slots with `Unit`, so the VM
/// has a DEFINED value here. An uninitialised `alloca` loads as `undef`, so
/// before Fix 1 the native side returned an arbitrary value.
///
/// The encoding claim this pins is explicit rather than incidental: **zero is
/// this backend's `Unit`.** The operand stack is uniformly `i64` and `Unit`
/// occupies zero bytes, so it has no natural width; choosing zero is a decision
/// and the control is where that decision is stated.
#[test]
fn control_unwritten_local_reads_as_unit_not_undef() {
    // A program with no locals at all, then given a local slot nothing writes.
    let src = "fn main() -> Word { 7 }";
    let mutate = |m: &mut keleusma::bytecode::Module| {
        let c = &mut m.chunks[0];
        assert_eq!(c.param_count, 0, "fixture assumption: main takes no params");
        // Give the chunk a local slot, and read it instead of the constant.
        c.local_count = 1;
        let ret = c.ops.pop().expect("non-empty chunk");
        assert!(
            matches!(ret, Op::Return),
            "fixture assumption: main ends in Return, found {ret:?}"
        );
        c.ops.clear();
        c.ops.push(Op::GetLocal(0));
        c.ops.push(Op::Return);
    };

    let vm = vm_result_mutated(src, mutate);
    assert_eq!(
        vm,
        Value::Unit,
        "the VM fills non-parameter locals with Unit"
    );

    // STRUCTURAL, and this is the part that is actually a control.
    //
    // The behavioural comparison below does NOT fire without the fix, and that
    // was verified rather than assumed: with the initialising store removed, the
    // test still passed. An uninitialised `alloca` loaded immediately reads
    // whatever occupies the slot, and a fresh frame slot is usually zero, so
    // `undef` materialised as 0 and matched the expected value BY ACCIDENT.
    //
    // That is a vacuous test, the exact class this project keeps catching, and
    // it would have sat here looking like protection. `lowered_ir` exists for
    // "assertions about structure that runtime behaviour cannot demonstrate",
    // which is precisely this. The store either appears in the IR or it does
    // not, and no amount of lucky stack contents can fake it.
    let ir = lowered_ir_mutated(src, mutate);
    assert!(
        ir.contains("store i64 0, ptr %l0"),
        "the non-parameter local must be initialised to this backend's Unit; \
         without that store a GetLocal of an unwritten slot loads undef.\nIR:\n{ir}"
    );

    // Retained as a regression check, NOT as a control: it agrees with the VM
    // but cannot fail when the fix is absent.
    let native = native_result_mutated(src, mutate);
    assert_eq!(
        native, 0,
        "zero is this backend's Unit; agrees with the VM's Unit"
    );
}

/// Lower a mutated module and return its IR text.
fn lowered_ir_mutated(src: &str, mutate: impl FnOnce(&mut keleusma::bytecode::Module)) -> String {
    let mut m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    mutate(&mut m);
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("lower");
    lm.print_to_string().to_string()
}

/// Does `Vm::new` REJECT this mutated module, and with what message?
///
/// `vm_result_mutated` cannot express a rejection: it `expect`s admission and
/// panics otherwise. This returns the verifier's verdict instead of asserting it.
fn vm_new_rejection(
    src: &str,
    mutate: impl FnOnce(&mut keleusma::bytecode::Module),
) -> Option<String> {
    let mut m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    mutate(&mut m);
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena capacity");
    let arena = keleusma_arena::Arena::with_capacity(cap);
    Vm::new(m, &arena).err().map(|e| format!("{e:?}"))
}

/// MUST-FIRE for Fix 2, and a CROSS-SESSION REGRESSION GUARD.
///
/// A chunk whose ops end without `Op::Return`. This test used to demonstrate
/// that `verify()` ADMITTED such a chunk, which was the proof of concept for the
/// worst-case-memory hole this branch reported to the `v0.2.3` session. **They
/// fixed it**, so the demonstration is now a guard that the fix holds, checked
/// from the consumer side rather than from inside the verifier's own suite.
///
/// # The falsification clause that fired with a false conclusion
///
/// The previous version of this comment said: *if `Vm::new` rejects, then
/// `verify()` does not admit these chunks and the inventory claiming it does is
/// WRONG.* That clause fired, and its conclusion is false. The reading was
/// correct when it was taken; the verifier changed afterwards **because of it**.
/// A clause that assumes its subject is fixed cannot express "the subject moved",
/// and any test either session writes to pin a defect the other may remove has
/// this shape. The assertions below say which of the two happened.
///
/// # Why Fix 2 is still load-bearing after the verifier tightened
///
/// **The backend never calls `verify()`.** It consumes a `Module` straight from
/// `compile()`, so a chunk the verifier now rejects can still reach the lowering,
/// and this test's own mutation is the demonstration that the path exists. A
/// chunk with no terminator would emit a final basic block with no terminator,
/// which is malformed IR rather than a wrong answer.
///
/// # Where the expected value comes from, since the VM will no longer produce it
///
/// This file's rule is that expected values come from the VM rather than from the
/// author, because an assertion written at the same moment as the code encodes
/// the same mistake twice. Inverting the VM half appears to break that rule.
///
/// It is preserved by taking the oracle from the **unmutated** program, which the
/// VM still runs, plus one documented semantic step: the mutation removes only
/// the trailing `Return`, leaving the same value on the stack, and falling off
/// the end returns the top of stack. **`Vm::new_unchecked` is deliberately NOT
/// used.** It would give a direct oracle, but `CLAUDE.md` calls it intentional
/// misuse for admitting programs that would fail verification, and this program
/// would fail verification. The structural assertion below carries the weight
/// that the behavioural one no longer can, which is the same resolution the
/// sibling control above reached when its behavioural check proved vacuous.
///
/// The original text follows, for the claim that is still true:
///
/// 2. The VM defines the case as returning the top of stack (or `Unit` when
///    empty), and after Fix 2 the lowering agrees.
///
/// Before Fix 2 the lowering emitted a final block with no terminator, so
/// `lm.verify()` failed — malformed IR rather than a wrong answer, which is why
/// Fix 3 matters as much as this one.
///
/// NOT asserted here: the operand-stack leak. The VM does not truncate to
/// `frame.base` on this path and the lowering deliberately does not reproduce
/// that. The asymmetry is recorded in the inventory as a decision, and pinning
/// it as expected behaviour would entrench a defect.
#[test]
fn control_chunk_without_trailing_return_falls_off_the_end() {
    let src = "fn main() -> Word { 7 }";
    let mutate = |m: &mut keleusma::bytecode::Module| {
        let c = &mut m.chunks[0];
        let ret = c.ops.pop().expect("non-empty chunk");
        assert!(
            matches!(ret, Op::Return),
            "fixture assumption: main ends in Return, found {ret:?}"
        );
        // Everything before the Return is left intact, so the value 7 is on the
        // stack when the chunk runs out of ops.
    };

    // REGRESSION GUARD for the `v0.2.3` fix, from the consumer side.
    let rejection = vm_new_rejection(src, mutate);
    let message = rejection.expect(
        "verify() ADMITTED a chunk that can run off its own end. That hole was \
         closed on v0.2.3; if this passes again the fix has regressed, and the \
         worst-case-memory bound is unsound for every such chunk.",
    );
    assert!(
        message.contains("run off the end"),
        "the module was rejected, but not for running off the end, so this test \
         is no longer guarding what it claims.\nverdict: {message}"
    );

    // ORACLE, taken from the UNMUTATED program because the VM will no longer run
    // the mutated one. The mutation removes only the trailing `Return`, so the
    // same value is on the stack when the chunk runs out of ops.
    assert_eq!(
        vm_result(src, &[]),
        7,
        "fixture assumption: the unmutated program returns 7"
    );

    // STRUCTURAL MUST-FIRE for Fix 2, and the part that is actually a control.
    //
    // Without Fix 2 the final block carries no terminator, and `lower_module`
    // now verifies internally (Fix 3), so the failure would surface as a refusal
    // rather than a wrong answer. Asserting the `ret` is present states the
    // property directly instead of inferring it from a value that happens to
    // agree. This mirrors the sibling control above, whose behavioural check was
    // shown to pass against unfixed code.
    let ir = lowered_ir_mutated(src, mutate);
    assert!(
        ir.contains("ret i64"),
        "Fix 2 must emit an implicit `ret` for a chunk with no trailing Return; \
         without it the final block has no terminator and the IR is malformed.\nIR:\n{ir}"
    );

    // Retained as a regression check on the VALUE, not as the oracle for it.
    let native = native_result_mutated(src, mutate);
    assert_eq!(
        native, 7,
        "Fix 2 returns the top of stack, matching the unmutated program's value"
    );
}
// ---------------------------------------------------------------------------
// PREPARED WHILE ANOTHER SESSION'S GATE HELD THE MACHINE. Not yet compiled.
// Append to native_codegen/tests/differential.rs.
//
// WHY THIS EXISTS, and it is not tidiness.
//
// `V0_4_0_NATIVE_CODEGEN.md`'s Out of scope list says "JIT compilation. V0.4.0
// is AOT only." Every case in this file runs the JIT at `OptimizationLevel::None`
// — the configuration the architecture EXCLUDES from the deliverable — while the
// shipped shape is AOT at `default<O2>`, covered end to end by exactly one test
// in `aot_linkage.rs`.
//
// That gap is not hypothetical. The unwritten-local control in this file PASSED
// against the unfixed lowering: an uninitialised `alloca`, loaded immediately at
// O0, read zero and matched the expected value by accident. **At O2 LLVM does
// not leave `undef` alone — it propagates it and deletes branches on the
// assumption it may take any convenient value.** The same defect that was
// invisible at O0 can produce actively wrong control flow at O2. The coverage
// gap already concealed a real defect from the control written to catch it.
//
// TWO DIMENSIONS, DELIBERATELY SEPARATED. An earlier note in the inventory
// recommended "an AOT-and-O2 arm", which conflated them:
//
//   1. OPTIMISATION LEVEL — where undef/poison exploitation lives. Closed here,
//      cheaply, by running the middle end before executing. No linker per case.
//   2. DELIVERY SHAPE — platform calling convention, external symbol emission,
//      real linkage. Already covered by `aot_linkage.rs` for representative
//      programs, and expensive per case (link plus subprocess).
//
// Dimension 1 carries the soundness risk and costs almost nothing. Dimension 2
// carries integration risk and is adequately sampled. Closing 1 across the whole
// corpus and sampling 2 is the right split; running every case through a linker
// would buy little for a large cost.
// ---------------------------------------------------------------------------

/// Lower `src`, run the REAL optimisation pipeline over it, then JIT and call.
///
/// The distinction from [`native_result`] is one line — `run_passes` — and it is
/// the line that matters. `default<O2>` is the same pipeline `aot_linkage.rs`
/// uses to emit shipped objects, so a disagreement here is a disagreement the
/// deliverable would exhibit.
fn native_result_o2(src: &str, args: &[i64]) -> i64 {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    check_word_width(m.word_bits_log2).expect("word width");

    // `lower_module`, NOT `lower_chunk`. The first draft copied
    // `native_result`'s `&m.chunks[0]`, which is only valid for a single-chunk
    // program: with a helper present, chunk 0 is the HELPER, so the harness
    // lowered `helper`, called it with main's arguments and compared the result
    // against main's. It reported 10 against 40 and looked like an O2
    // miscompilation. It was the test being wrong, and the cross-function cases
    // below are exactly the ones worth keeping, because inlining is what O2 does.
    let ctx = Context::create();
    let lm = ctx.create_module("kel_o2");
    lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("lower");
    let idx = m
        .chunks
        .iter()
        .position(|c| c.name == "main")
        .expect("entry chunk named main");
    let entry = format!("kel_chunk_{idx}");
    lm.verify().expect("LLVM module verification");

    // The middle end. Without this the test is just `native_result` again.
    Target::initialize_native(&InitializationConfig::default()).expect("init native target");
    let triple = TargetMachine::get_default_triple();
    let target = Target::from_triple(&triple).expect("target from triple");
    let machine = target
        .create_target_machine(
            &triple,
            // "generic"/"" matches `aot_linkage.rs` exactly. The host-CPU
            // accessors return `LLVMString` rather than `&str` and would not
            // compile as first drafted; more importantly, matching the shipped
            // emitter's settings is the point of this arm.
            "generic",
            "",
            OptimizationLevel::Default,
            RelocMode::PIC,
            CodeModel::Default,
        )
        .expect("target machine");
    lm.run_passes("default<O2>", &machine, PassBuilderOptions::create())
        .expect("O2 pipeline");

    // Re-verify AFTER optimisation. A pass that miscompiles malformed-but-
    // accepted IR shows up here rather than as a wrong answer, and this is the
    // only place in the suite that checks the post-optimisation module.
    lm.verify().expect("LLVM module verification after O2");

    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::Default)
        .expect("jit");
    match args.len() {
        1 => {
            let f = unsafe { ee.get_function::<unsafe extern "C" fn(i64) -> i64>(&entry) }
                .expect("symbol");
            unsafe { f.call(args[0]) }
        }
        2 => {
            let f = unsafe { ee.get_function::<unsafe extern "C" fn(i64, i64) -> i64>(&entry) }
                .expect("symbol");
            unsafe { f.call(args[0], args[1]) }
        }
        n => panic!("harness does not drive {n}-argument entry points"),
    }
}

/// The O2 arm of the differential oracle.
///
/// Deliberately reuses cases that already pass at O0. The question is not
/// whether the lowering is right — O0 answers that — but whether it SURVIVES the
/// pipeline that ships. A case passing at O0 and failing here is exactly the
/// class the architecture's AOT-only scope makes load-bearing.
///
/// Inputs distinguish branch paths, per this file's own rule: `maxi(2, 3)` takes
/// the else path and proved nothing about the then path, which is how the first
/// lowering defect survived.
#[test]
fn the_optimised_pipeline_agrees_with_the_vm() {
    let cases: &[(&str, &[i64])] = &[
        // Branch, both directions.
        (
            "fn main(a: Word, b: Word) -> Word { if a > b { a } else { b } }",
            &[9, 4],
        ),
        (
            "fn main(a: Word, b: Word) -> Word { if a > b { a } else { b } }",
            &[2, 3],
        ),
        // Checked multiply, whose triple O2 will fold hard.
        ("fn main(a: Word, b: Word) -> Word { a * b }", &[7, 6]),
        // Cross-function call, which O2 will inline.
        (
            "fn helper(x: Word) -> Word { x + 1 }\n\
          fn main(a: Word, b: Word) -> Word { helper(a) + b }",
            &[41, 1],
        ),
        // Wrapping corner. Verified against `wrapping_addition_agrees_with_the_vm`
        // that `a + b` WRAPS rather than trapping here, so this case tests the
        // triple's low word rather than failing for an unrelated reason.
        (
            "fn main(a: Word, b: Word) -> Word { a + b }",
            &[i64::MAX, 1],
        ),
        // Division, where the divisor substitution guard must not be optimised
        // away as unreachable.
        (
            "fn main(a: Word, b: Word) -> Word { a / b }",
            &[i64::MIN, -1],
        ),
    ];

    for (src, args) in cases {
        let vm = vm_result(src, args);
        let o2 = native_result_o2(src, args);
        // The O0 pre-check that stood here is REMOVED, not disabled. It called
        // `native_result`, which lowers `chunks[0]` and is therefore valid only
        // for single-chunk programs, while two cases here are deliberately
        // cross-function so that O2 has something to inline. It reported 42
        // against 43 — the helper's result compared against main's — and read as
        // an O0 regression. A pre-check that cannot express the cases it guards
        // is worse than no pre-check, because its failure points away from the
        // real cause. The O0 path is covered by its own tests in this file.
        assert_eq!(
            vm, o2,
            "THE OPTIMISED PIPELINE DISAGREES WITH THE VM. This is the shipped \
             configuration; O0 agreeing is not sufficient.\nsrc: {src}\nargs: {args:?}"
        );
    }
}

/// MUST-FIRE evidence for the arm itself.
///
/// An arm that merely re-runs passing cases at a second optimisation level can
/// look like coverage while being unable to fail differently from the O0 arm.
/// This pins the one property that is genuinely O2-only: the module must still
/// verify AFTER the middle end has run.
///
/// It fires if a pass ever produces IR that LLVM's own verifier rejects, which
/// no O0 test can observe because no O0 test runs a pass.
#[test]
fn the_module_still_verifies_after_the_optimisation_pipeline() {
    // Exercises locals, a branch, a call and the checked triple together, so the
    // post-pipeline module is not trivially small.
    let src = "fn helper(x: Word) -> Word { x + 1 }\n\
               fn main(a: Word, b: Word) -> Word { \
                 if a > b { helper(a) * b } else { helper(b) + a } }";
    // Reaching the assertion at all means both verifies passed inside the
    // helper; the value check is a bonus rather than the point.
    let vm = vm_result(src, &[9, 4]);
    let o2 = native_result_o2(src, &[9, 4]);
    assert_eq!(vm, o2);
}

/// **The first composite that actually EXECUTES natively, checked against the VM.**
///
/// Everything before this asserted structure. This is the only thing that can
/// settle whether the store run, the packing offsets and the address
/// representation are right, and it is the evidence required before the refusal
/// boundary above may be moved.
///
/// The harness supplies the region exactly as a host does, which is what the
/// calling convention promises: at least `keleusma_native::region::plan_chunk_region(chunk).bytes`
/// writable bytes, taken in production from the arena's bottom section.
fn composite_native_result(src: &str, args: &[i64]) -> i64 {
    use inkwell::context::Context;
    use keleusma_native::lower_module;

    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let idx = m
        .chunks
        .iter()
        .position(|c| c.name == "main")
        .expect("entry chunk named main");

    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("lower module");
    lm.verify().expect("LLVM module verification");
    common::maybe_optimize(&lm);
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");

    // Sized from the same pass the emitter placed against, so a disagreement
    // between them shows up here as a fault rather than as silent corruption.
    let bytes = keleusma_native::region::plan_chunk_region(&m.chunks[idx]).bytes as usize;
    let mut region = vec![0u8; bytes.max(8)];
    // Word-aligned backing for the two data pointers. This module declares no
    // slots so neither is read, but a misaligned pointer where the ABI promises
    // alignment is undefined behaviour rather than an unused argument.
    let mut shared = vec![0i64; 8];
    let mut private = vec![0i64; 8];

    let sym = format!("kel_chunk_{idx}");
    unsafe {
        let f = ee
            .get_function::<unsafe extern "C" fn(i64, i64, *mut i64, *mut i64, *mut u8) -> i64>(
                &sym,
            )
            .expect("symbol");
        f.call(
            args[0],
            args[1],
            shared.as_mut_ptr(),
            private.as_mut_ptr(),
            region.as_mut_ptr(),
        )
    }
}

#[test]
fn a_flat_struct_agrees_with_the_vm() {
    let src = "struct P { x: Word, y: Word }
               fn main(a: Word, b: Word) -> Word { let p = P { x: a, y: b }; p.x }";
    for args in [[2, 3], [-7, 4], [0, 0], [i64::MIN, i64::MAX]] {
        let native = composite_native_result(src, &args);
        let vm = vm_result(src, &args);
        assert_eq!(
            native, vm,
            "flat struct disagrees for {args:?}: native={native}, vm={vm}"
        );
    }
}

/// **Reading the SECOND field, which the first case cannot distinguish.**
///
/// `p.x` is at offset zero, so a lowering that ignored the field offset entirely
/// would pass the case above. Only a non-zero offset separates them.
#[test]
fn the_second_field_of_a_flat_struct_agrees_with_the_vm() {
    let src = "struct P { x: Word, y: Word }
               fn main(a: Word, b: Word) -> Word { let p = P { x: a, y: b }; p.y }";
    for args in [[2, 3], [-7, 4], [i64::MIN, i64::MAX]] {
        let native = composite_native_result(src, &args);
        let vm = vm_result(src, &args);
        assert_eq!(
            native, vm,
            "second field disagrees for {args:?}: native={native}, vm={vm}"
        );
    }
}

/// Array element reads, with the index VARIED at run time.
///
/// A constant index would let a lowering that ignored the stride entirely pass,
/// which is the same defect the second-field struct case guards against one level
/// down. Element zero and element one must both be right, and the index arrives
/// as a parameter so it cannot be folded away.
#[test]
fn a_flat_array_element_agrees_with_the_vm() {
    let src = "fn main(a: Word, i: Word) -> Word { let xs = [a, a + 1]; xs[i] }";
    for args in [[10, 0], [10, 1], [-3, 0], [-3, 1], [i64::MAX, 1]] {
        let native = composite_native_result(src, &args);
        let vm = vm_result(src, &args);
        assert_eq!(
            native, vm,
            "array element disagrees for {args:?}: native={native}, vm={vm}"
        );
    }
}

/// Native result AND the region bytes the run left behind.
///
/// The test owns the region, so it can read back what the emitter wrote. That is
/// what makes a nested-body copy observable without `FlatNested` reads — the
/// point missed when this path was briefly deleted as "unverifiable".
fn composite_native_with_region(src: &str, args: &[i64]) -> (i64, Vec<u8>) {
    use inkwell::context::Context;
    use keleusma_native::lower_module;

    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let idx = m
        .chunks
        .iter()
        .position(|c| c.name == "main")
        .expect("entry chunk named main");
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("lower module");
    lm.verify().expect("LLVM module verification");
    common::maybe_optimize(&lm);
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");

    // **A GUARD BAND, because the region is the only thing standing between a
    // mis-sized copy and heap corruption in this process.** A mutation that
    // doubled a copy length passed every assertion here while writing past the
    // end of the buffer: the damage was outside everything anyone looked at. The
    // guard is checked below, so any write past the planned region is a test
    // failure rather than undefined behaviour.
    const GUARD: usize = 64;
    const GUARD_BYTE: u8 = 0xAA;
    let bytes = keleusma_native::region::plan_chunk_region(&m.chunks[idx]).bytes as usize;
    let planned = bytes.max(8);
    let mut region = vec![0u8; planned + GUARD];
    region[planned..].fill(GUARD_BYTE);
    let mut shared = vec![0i64; 8];
    let mut private = vec![0i64; 8];
    let sym = format!("kel_chunk_{idx}");
    let out = unsafe {
        let f = ee
            .get_function::<unsafe extern "C" fn(i64, i64, *mut i64, *mut i64, *mut u8) -> i64>(
                &sym,
            )
            .expect("symbol");
        f.call(
            args[0],
            args[1],
            shared.as_mut_ptr(),
            private.as_mut_ptr(),
            region.as_mut_ptr(),
        )
    };
    assert!(
        region[planned..].iter().all(|b| *b == GUARD_BYTE),
        "the lowering wrote past the {planned}-byte region it was given; a copy or \
         store ran outside the body it was placing"
    );
    region.truncate(planned);
    (out, region)
}

/// ROUTE 1: the NEIGHBOUR of a nested body, which costs nothing and was
/// available all along.
///
/// `b` sits immediately after the nested `i` in the parent body, at a flat offset
/// the backend already reads. A copy of the wrong LENGTH, or to the wrong offset,
/// runs over `b` — so a supported read detects an unsupported one's mistake. This
/// is the case whose absence made the copy look unobservable.
#[test]
fn a_nested_body_copy_does_not_clobber_its_neighbour() {
    let src = "struct I { a: Word }
               struct O { i: I, b: Word }
               fn main(a: Word, b: Word) -> Word { let o = O { i: I { a: a }, b: b }; o.b }";
    for args in [[7, 11], [-1, 5], [i64::MIN, i64::MAX], [0, -9]] {
        let (native, _) = composite_native_with_region(src, &args);
        let vm = vm_result(src, &args);
        assert_eq!(
            native, vm,
            "the neighbour of a nested body disagrees for {args:?}: native={native}, vm={vm}"
        );
    }
}

/// ROUTE 2: read the copied bytes back out of the region directly.
///
/// The neighbour case proves the copy did not overrun. This proves it wrote the
/// right CONTENT at the right place, which no amount of neighbour-checking can:
/// a copy of the correct length that copied the wrong bytes passes route 1 and
/// fails here.
#[test]
fn a_nested_body_copy_writes_the_right_bytes() {
    let src = "struct I { a: Word }
               struct O { i: I, b: Word }
               fn main(a: Word, b: Word) -> Word { let o = O { i: I { a: a }, b: b }; o.b }";
    let (a, b) = (0x1122_3344_5566_7788_i64, -424_242_i64);
    let (_, region) = composite_native_with_region(src, &[a, b]);

    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let idx = m
        .chunks
        .iter()
        .position(|c| c.name == "main")
        .expect("main");
    let plan = keleusma_native::region::plan_chunk_region(&m.chunks[idx]);
    // Two sites: the inner `I` then the outer `O`. The outer is the last placed
    // and is the one whose body must contain a COPY of the inner, not a pointer.
    let outer = plan.sites.last().expect("a placed site");
    assert_eq!(outer.size, 16, "the outer body is two words");

    let at = |off: usize| -> i64 {
        i64::from_le_bytes(region[off..off + 8].try_into().expect("eight bytes"))
    };
    let base = outer.offset as usize;
    assert_eq!(
        at(base),
        a,
        "the nested body's word was not COPIED into the parent; a pointer here \
         would make every downstream offset still look correct"
    );
    assert_eq!(at(base + 8), b, "the neighbouring field was not written");
}

/// **The case a mutation exposed: an OVER-COPY, with the nested field LAST.**
///
/// `a_nested_body_copy_does_not_clobber_its_neighbour` was expected to catch a
/// copy of the wrong length. It does not, and a mutant proved it: doubling the
/// copy length passes both earlier tests, because the neighbouring field is
/// stored AFTER the copy and simply overwrites the damage.
///
/// Putting the nested field last removes that mask — nothing is written after the
/// copy, so an over-copy runs past the end of the body and stays there to be
/// seen. The assertion is on the bytes BEYOND the composite, which is the only
/// place the evidence survives.
#[test]
fn a_nested_body_copy_does_not_run_past_the_body() {
    let src = "struct I { a: Word }
               struct O { b: Word, i: I }
               fn main(a: Word, b: Word) -> Word { let o = O { b: b, i: I { a: a } }; o.b }";
    let (a, b) = (0x0F0F_0F0F_0F0F_0F0F_i64, 12345_i64);
    let (_, region) = composite_native_with_region(src, &[a, b]);

    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let idx = m
        .chunks
        .iter()
        .position(|c| c.name == "main")
        .expect("main");
    let plan = keleusma_native::region::plan_chunk_region(&m.chunks[idx]);
    let outer = plan.sites.last().expect("a placed site");
    let end = (outer.offset + outer.size) as usize;

    assert!(
        end <= region.len(),
        "the plan places a body past the region it sized"
    );
    // Everything after the outer body must be untouched. The buffer starts
    // zeroed, so a non-zero byte here is something the emitter wrote outside the
    // body it was placing.
    for (k, byte) in region.iter().enumerate().skip(end) {
        assert_eq!(
            *byte, 0,
            "byte {k} past the end of the body at {end} was written; a copy ran \
             past the body it was placing"
        );
    }
}

/// **The full VM differential for a nested body, which was impossible until the
/// nested read landed.**
///
/// This reads a scalar field OUT of a copied nested body and compares against the
/// virtual machine. It is the case whose absence made the copy look unobservable,
/// and it subsumes the region-inspection tests as evidence of CONTENT while they
/// remain as evidence of PLACEMENT.
#[test]
fn a_field_read_out_of_a_nested_body_agrees_with_the_vm() {
    let src = "struct I { a: Word }
               struct O { i: I, b: Word }
               fn main(a: Word, b: Word) -> Word { let o = O { i: I { a: a }, b: b }; o.i.a }";
    for args in [[7, 11], [-1, 5], [i64::MIN, i64::MAX], [0, -9]] {
        let (native, _) = composite_native_with_region(src, &args);
        let vm = vm_result(src, &args);
        assert_eq!(
            native, vm,
            "a field read out of a nested body disagrees for {args:?}: native={native}, vm={vm}"
        );
    }
}

/// An element of an array of arrays, indexed at RUN TIME on the outer array.
///
/// The nested element read is `index * size` with a composite stride, which a
/// constant index cannot distinguish from a fixed offset.
#[test]
fn a_nested_array_element_agrees_with_the_vm() {
    let src =
        "fn main(a: Word, i: Word) -> Word { let xs = [a, a + 1]; let ys = [xs, xs]; ys[i][1] }";
    for args in [[5, 0], [5, 1], [-2, 0], [-2, 1]] {
        let (native, _) = composite_native_with_region(src, &args);
        let vm = vm_result(src, &args);
        assert_eq!(
            native, vm,
            "a nested array element disagrees for {args:?}: native={native}, vm={vm}"
        );
    }
}

/// **A MIXED-WIDTH composite, which is where the packing rule actually bites.**
///
/// `struct M { a: Byte, b: Word }` is nine bytes with the word at offset ONE.
/// Every uniform-word composite is blind to that: cumulative packing and an
/// eight-byte stride agree on all of them, which is five of the six shapes the
/// corpus contains. This is the case that separates them, executed rather than
/// inspected.
#[test]
fn a_mixed_width_composite_agrees_with_the_vm() {
    let src = "struct M { a: Byte, b: Word }
               fn main(a: Word, b: Word) -> Word { let m = M { a: 1 as Byte, b: b }; m.b }";
    for args in [[0, 7], [0, -1], [0, i64::MIN], [0, i64::MAX]] {
        let (native, _) = composite_native_with_region(src, &args);
        let vm = vm_result(src, &args);
        assert_eq!(
            native, vm,
            "a word at offset one disagrees for {args:?}: native={native}, vm={vm}"
        );
    }
}

/// Reading the BYTE of a mixed-width composite, which the word case cannot check.
///
/// A `Byte` occupies a full operand slot holding `0..=255`, so a sign-extending
/// load would read `0xFF` as `-1`. The byte here is deliberately above 127.
#[test]
fn a_byte_field_zero_extends_like_the_vm() {
    let src = "struct M { a: Byte, b: Word }
               fn main(a: Word, b: Word) -> Word { let m = M { a: 200 as Byte, b: b }; m.a as Word }";
    for args in [[0, 1], [0, -5]] {
        let (native, _) = composite_native_with_region(src, &args);
        let vm = vm_result(src, &args);
        assert_eq!(
            native, vm,
            "a byte field disagrees for {args:?}: native={native}, vm={vm}"
        );
    }
}

/// Tuple fields, which reach the emitter through the `GetTupleField`
/// normalisation rather than through arms of their own.
///
/// Element ONE, deliberately: element zero sits at offset zero and would pass
/// even if the normalisation dropped the offset entirely.
#[test]
fn a_tuple_field_agrees_with_the_vm() {
    let src = "fn main(a: Word, b: Word) -> Word { let t = (a, b); t.1 }";
    for args in [[2, 3], [-7, 4], [i64::MIN, i64::MAX]] {
        let (native, _) = composite_native_with_region(src, &args);
        let vm = vm_result(src, &args);
        assert_eq!(
            native, vm,
            "a tuple field disagrees for {args:?}: native={native}, vm={vm}"
        );
    }
}

/// A tuple of mixed widths, where the normalisation has to carry the offset the
/// packing rule produced rather than a stride.
#[test]
fn a_mixed_width_tuple_agrees_with_the_vm() {
    let src = "fn main(a: Word, b: Word) -> Word { let t = (1 as Byte, b); t.1 }";
    for args in [[0, 9], [0, -3], [0, i64::MAX]] {
        let (native, _) = composite_native_with_region(src, &args);
        let vm = vm_result(src, &args);
        assert_eq!(
            native, vm,
            "a mixed-width tuple disagrees for {args:?}: native={native}, vm={vm}"
        );
    }
}

/// An enum match: the discriminant test and the payload read together.
///
/// **BOTH variants are exercised.** A lowering whose discriminant comparison was
/// inverted, or that ignored the tested variant entirely, would still return the
/// right answer for whichever arm the source happens to construct — which is the
/// `maxi(2, 3)` defect this file was written about, one level up.
#[test]
fn an_enum_match_agrees_with_the_vm() {
    let a_src = "enum E { A(Word), B(Word) }
                 fn main(a: Word, b: Word) -> Word { let e = E::A(a); match e { E::A(x) => x, E::B(y) => y + b } }";
    let b_src = "enum E { A(Word), B(Word) }
                 fn main(a: Word, b: Word) -> Word { let e = E::B(a); match e { E::A(x) => x, E::B(y) => y + b } }";
    for src in [a_src, b_src] {
        for args in [[3, 4], [-11, 6], [i64::MIN, 1]] {
            let (native, _) = composite_native_with_region(src, &args);
            let vm = vm_result(src, &args);
            assert_eq!(
                native, vm,
                "an enum match disagrees for {args:?}: native={native}, vm={vm}"
            );
        }
    }
}

/// **BYTE ARITHMETIC AGREES WITH THE VIRTUAL MACHINE, INCLUDING AT THE WRAP.**
///
/// `Op::Add`, `Op::Sub`, `Op::Mul` and `Op::Neg` were recorded for a long time
/// as blocked on the operator's float representation. They were not: the opcode
/// is emitted for `Byte`, `Fixed` AND `Float`, and only the last needs a
/// representation this backend lacks. With the module-level float guard closing
/// every route a float can take, a matched `Byte` pair is unambiguous.
///
/// **The corpus does NOT exercise this**, which is why these tests exist rather
/// than a corpus figure. `opcode_witness.kel` still refuses on `FixedDiv`, `Len`
/// and `IsStruct`, so the differential never drives its `byte_mix`. Four opcodes
/// lowering with nothing executing them is precisely the shape this line keeps
/// finding, so the check is hand-written and the boundary values are the point.
///
/// **The wrap is the whole content.** The virtual machine computes a `Byte` in
/// `i64` and masks with `& 0xFF`; a lowering that omitted the mask agrees on
/// every small case and diverges only past 255. `200 + 100` and `3 - 5` are the
/// two cases that catch it.
#[test]
fn byte_addition_agrees_with_the_vm_including_the_wrap() {
    let src = "fn main(a: Word, b: Word) -> Word {
        let x = a as Byte;
        let y = b as Byte;
        (x + y) as Word
    }";
    // 200+100 = 300 -> 44 after the mask. Without the mask the sides differ.
    for args in [[2, 3], [200, 100], [255, 1], [255, 255], [0, 0]] {
        assert_agrees(src, &args);
    }
}

/// Subtraction below zero, which wraps upward on a `u8`.
#[test]
fn byte_subtraction_agrees_with_the_vm_including_the_borrow() {
    let src = "fn main(a: Word, b: Word) -> Word {
        let x = a as Byte;
        let y = b as Byte;
        (x - y) as Word
    }";
    // 3-5 = -2 -> 254. An unmasked lowering yields -2 and disagrees.
    for args in [[5, 3], [3, 5], [0, 1], [255, 255], [0, 255]] {
        assert_agrees(src, &args);
    }
}

/// Multiplication, where the product leaves eight bits almost immediately.
#[test]
fn byte_multiplication_agrees_with_the_vm_including_the_overflow() {
    let src = "fn main(a: Word, b: Word) -> Word {
        let x = a as Byte;
        let y = b as Byte;
        (x * y) as Word
    }";
    // 16*16 = 256 -> 0, the smallest product that vanishes under the mask.
    for args in [[3, 4], [16, 16], [255, 255], [17, 15], [0, 200]] {
        assert_agrees(src, &args);
    }
}

/// Negation, which the virtual machine performs as `u8::wrapping_neg`.
#[test]
fn byte_negation_agrees_with_the_vm() {
    let src = "fn main(a: Word) -> Word {
        let x = a as Byte;
        (-x) as Word
    }";
    // -0 is 0; -1 is 255. An unmasked lowering yields -1 and disagrees.
    for args in [[0], [1], [128], [255], [100]] {
        assert_agrees(src, &args);
    }
}

/// **FIXED-POINT ARITHMETIC IS VERIFIED IN THE CORPUS, NOT HERE, AND THE REASON
/// IS THIS HARNESS'S OWN LIMIT.**
///
/// A `Fixed` operand's width is trusted only where a signature states it and the
/// chunk never writes the local — so exercising `Op::Add` on a `Fixed` pair
/// needs a function TAKING `Fixed` parameters. The entry point cannot: this
/// harness and the virtual machine both pass `Value::Int`. So any fixed-point
/// probe here is necessarily MULTI-FUNCTION, which the precondition in
/// `native_result` now refuses.
///
/// **Four tests lived here and one of them passed falsely** before that
/// precondition existed, comparing `add_fx` natively against `main` in the
/// virtual machine. See `examples/scripts/fixed_arithmetic.kel`, which the
/// corpus differential drives as a whole module.
#[test]
fn fixed_arithmetic_is_covered_by_the_corpus_and_not_by_this_harness() {
    let src = "fn add_fx(x: Fixed<16>, y: Fixed<16>) -> Fixed<16> { x + y }\n\
               fn main(a: Word, b: Word) -> Word { add_fx(a as Fixed<16>, b as Fixed<16>) as Word }";
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    assert_ne!(
        m.entry_point,
        Some(0),
        "a fixed-point probe became SINGLE-FUNCTION, so this harness could drive \
         it after all and the corpus program is no longer the only route"
    );
}

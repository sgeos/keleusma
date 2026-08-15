//! Suspension, compared as a SEQUENCE rather than as a single value.
//!
//! # Why this harness exists
//!
//! Every other differential test in this package compares one returned value.
//! That oracle is blind to a whole class of defect in suspension: a lowering
//! that produced the right yielded values IN THE WRONG ORDER would pass all of
//! them. The inventory records that gap as the precondition for attempting the
//! stream-rotation transformation, since a rotation is exactly a reordering and
//! an oracle that cannot see order cannot police it.
//!
//! So this harness collects the whole interaction: every value the program
//! yields, in order, together with the value it finally returns, and compares
//! the sequences.
//!
//! # The control inversion, and why it is sound to compare across it
//!
//! The runtime suspends outward: `call` returns `Yielded(v)` and the host calls
//! `resume(r)`. The native lowering suspends inward: it calls `kel_yield(v)`
//! and receives `r` as the return value. These are different control shapes and
//! the same OBSERVABLE SEQUENCE, which is the thing being compared. The
//! inversion is why this fits a reentrant `yield fn`, which suspends a bounded
//! number of times and returns, and does not fit a divergent `loop fn`.

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::Value;
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerOptions, lower_module};
use std::sync::Mutex;

mod common;

/// Values the native program yielded, in order, and the replies handed back.
///
/// **These are process-global because the yield ABI is a plain `extern "C"`
/// function with no context parameter**, so the callback has nowhere else to put
/// them. That makes concurrent native runs mutually destructive, and `cargo
/// test` runs tests in parallel by default: the first version of this harness
/// passed every test in isolation and failed two of four together, because one
/// test's yields landed in another's collection. `HARNESS` serialises the native
/// runs. A context pointer in the ABI would remove the need, and is one reason
/// the callback shape is provisional.
static YIELDED: Mutex<Vec<i64>> = Mutex::new(Vec::new());
static REPLIES: Mutex<Vec<i64>> = Mutex::new(Vec::new());
static HARNESS: Mutex<()> = Mutex::new(());

/// The host side of the provisional yield ABI.
extern "C" fn kel_yield(v: i64) -> i64 {
    let mut y = YIELDED.lock().unwrap();
    y.push(v);
    let replies = REPLIES.lock().unwrap();
    // Replies are supplied in order; past the end, resume with zero.
    replies.get(y.len() - 1).copied().unwrap_or(0)
}

fn arena_for(m: &keleusma::bytecode::Module) -> keleusma_arena::Arena {
    let need = required_persistent_capacity_for(m);
    // **Plus a host-side margin**, which the runtime's own exhaustion message
    // asks for: `auto_arena_capacity_for` sizes the operand stack for the
    // worst case the verifier proves for a single entry, and a suspended chunk
    // resumed repeatedly grows past it. The runtime says so in the error rather
    // than leaving it to be discovered, and the margin is generous here because
    // this is a test harness and not a deployment budget.
    const HOST_MARGIN: usize = 64 * 1024;
    let cap = auto_arena_capacity_for(m, &[]).expect("arena capacity") + need + HOST_MARGIN;
    let mut a = keleusma_arena::Arena::with_capacity(cap);
    a.resize_persistent(need).expect("persistent region");
    a
}

/// Drive the VM, collecting every yielded value and the final result.
fn vm_sequence(src: &str, args: &[i64], replies: &[i64]) -> (Vec<i64>, i64) {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let arena = arena_for(&m);
    let mut vm = Vm::new(m, &arena).expect("vm");
    let vals: Vec<Value> = args.iter().map(|&x| Value::Int(x)).collect();
    let mut yielded = Vec::new();
    let mut st = vm.call(&vals).expect("vm run");
    loop {
        match st {
            VmState::Yielded(Value::Int(v)) => {
                yielded.push(v);
                let r = replies.get(yielded.len() - 1).copied().unwrap_or(0);
                st = vm.resume(Value::Int(r)).expect("vm resume");
            }
            VmState::Finished(Value::Int(v)) => return (yielded, v),
            // A STREAM chunk costs TWO host round-trips per iteration: the body
            // runs to its `Yield`, and the following `resume` walks the
            // `PopN(1); Reset` tail and hands back `Reset` before the next
            // iteration starts. The reply given here is discarded by that
            // `PopN(1)`, so the SAME reply is offered again; it is the one that
            // lands in slot 0 and drives the next iteration.
            //
            // Feeding the same value twice is what makes the two forms line up
            // ONE-TO-ONE: `step(r)` natively equals the VM's
            // `resume(r) -> Reset; resume(r) -> Yielded(v)` pair. A fresh reply
            // on the Reset leg would be silently discarded and the sequences
            // would diverge for a reason that has nothing to do with the
            // lowering.
            VmState::Reset => {
                let r = replies
                    .get(yielded.len().saturating_sub(1))
                    .copied()
                    .unwrap_or(0);
                st = vm.resume(Value::Int(r)).expect("vm resume after reset");
            }
            other => panic!("unexpected VM state {other:?}"),
        }
    }
}

/// Drive the native lowering, collecting the same two things.
fn native_sequence(src: &str, args: &[i64], replies: &[i64]) -> (Vec<i64>, i64) {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let idx = m
        .chunks
        .iter()
        .position(|c| c.name == "main")
        .expect("entry chunk named main");
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    let fns = lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("lower module");
    lm.verify().expect("LLVM module verification");
    common::maybe_optimize(&lm);
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");
    if let Some(hook) = lm.get_function("kel_yield") {
        ee.add_global_mapping(&hook, kel_yield as *const () as usize);
    }
    let _ = fns;

    let _serialise = HARNESS.lock().unwrap_or_else(|e| e.into_inner());
    YIELDED.lock().unwrap().clear();
    *REPLIES.lock().unwrap() = replies.to_vec();

    let sym = format!("kel_chunk_{idx}");
    let out = match args.len() {
        1 => {
            let f = unsafe { ee.get_function::<unsafe extern "C" fn(i64) -> i64>(&sym) }
                .expect("symbol");
            unsafe { f.call(args[0]) }
        }
        k => panic!("harness does not drive {k}-argument entry points"),
    };
    let y = YIELDED.lock().unwrap().clone();
    (y, out)
}

fn assert_sequences_agree(src: &str, args: &[i64], replies: &[i64]) {
    let (ny, nr) = native_sequence(src, args, replies);
    let (vy, vr) = vm_sequence(src, args, replies);
    assert_eq!(
        ny, vy,
        "YIELD SEQUENCE differs for {src:?} args {args:?} replies {replies:?}\n  native={ny:?}\n  vm    ={vy:?}"
    );
    assert_eq!(nr, vr, "final result differs: native={nr} vm={vr}");
}

#[test]
fn a_single_suspension_agrees_in_sequence_and_result() {
    // `yield a` suspends with `a` and evaluates to the resume value.
    let src = "yield main(a: Word) -> Word { yield a }";
    for (args, replies) in [
        ([7i64], vec![100i64]),
        ([0], vec![-1]),
        ([i64::MAX], vec![1]),
        ([-5], vec![0]),
    ] {
        assert_sequences_agree(src, &args, &replies);
    }
}

#[test]
fn the_resume_value_is_used_not_discarded() {
    // **THE CASE THAT PINS POP-ONE-PUSH-ONE.** The resume value is combined with
    // the argument, so a lowering that pushed the yielded value back instead of
    // the reply returns `a` where the VM returns `a + r`. Replies differ from
    // arguments in every case below, because equal ones would agree under both.
    let src = "yield main(a: Word) -> Word { let r = yield a; r + a }";
    for (args, replies) in [
        ([7i64], vec![100i64]),
        ([3], vec![-9]),
        ([0], vec![42]),
        ([i64::MIN], vec![-1]),
    ] {
        assert_sequences_agree(src, &args, &replies);
    }
}

#[test]
fn two_suspensions_agree_in_order() {
    // **THE CASE AN ORDER-BLIND ORACLE MISSES.** Two suspensions with distinct
    // yielded values: a lowering that emitted them in the wrong order returns
    // the same final result and differs only in the sequence.
    let src = "yield main(a: Word) -> Word { let p = yield a; let q = yield (a + 1); p + q }";
    for (args, replies) in [
        ([7i64], vec![10i64, 20]),
        ([0], vec![-1, 1]),
        ([100], vec![5, 5]),
    ] {
        assert_sequences_agree(src, &args, &replies);
    }
}

#[test]
fn a_divergent_loop_function_is_refused() {
    // `Stream` and `Reset` are refused DELIBERATELY, not by omission. The
    // callback ABI inverts control, so a divergent `loop fn` would spin inside
    // native code with no way for the host to stop it. Supporting it needs a
    // host-driven shape, which is the coroutine path.
    // WAS `let x = yield a; x` until 2026-08-11. That shape is now ADMITTED, and
    // its equivalence is asserted by `an_effect_free_tail_after_the_yield_...`:
    // the block's value is discarded by the `PopN(1)` before `Reset`, so binding
    // the resume value and returning it is dead code. The oracle was asked
    // BEFORE the boundary was moved, not after, because "obviously equivalent"
    // is what the previous rule's author thought too.
    //
    // This case still refuses, for a reason the comment above gives: the tail
    // can TRAP, which is observable, and the virtual machine would take the trap
    // after the suspension where native code has already returned.
    let src = "loop main(a: Word) -> Word { yield a; a * a }";
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    assert!(
        lower_module(&ctx, &lm, &m, LowerOptions::default()).is_err(),
        "a divergent loop function must be refused under a callback yield ABI"
    );
}

// ---------------------------------------------------------------------------
// STREAM drivers. A `loop` chunk is DIVERGENT: it never returns `Finished`, so
// `vm_sequence` and `native_sequence`, which both wait for a final result,
// cannot drive one. They were written for `yield fn` chunks, which terminate.
//
// Equivalence for a stream is therefore over the YIELDED SEQUENCE ALONE, bounded
// by the caller. There is no final result to compare because there is never a
// final result — that is what productive divergence means, not a gap in the test.
// ---------------------------------------------------------------------------

/// Drive a STREAM chunk on the virtual machine for `replies.len()` iterations.
///
/// Two host round-trips per iteration: the body runs to its `Yield`, then the
/// following `resume` walks the `PopN(1); Reset` tail and hands back `Reset`. The
/// reply on the Reset leg is discarded by that `PopN(1)`, so the SAME value is
/// offered twice. That is what makes the two forms line up one-to-one, since
/// natively `step(r)` is the whole iteration.
fn vm_stream_sequence(src: &str, args: &[i64], replies: &[i64]) -> Vec<i64> {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let arena = arena_for(&m);
    let mut vm = Vm::new(m, &arena).expect("vm");
    let vals: Vec<Value> = args.iter().map(|&x| Value::Int(x)).collect();
    let mut yielded: Vec<i64> = Vec::new();
    let mut st = vm.call(&vals).expect("vm run");
    // Bounded by construction: a divergent loop would otherwise spin forever, and
    // a hang is a far worse failure than a wrong answer because it reports
    // nothing. The bound is the reply count, so the caller sets it.
    while yielded.len() < replies.len() {
        match st {
            VmState::Yielded(Value::Int(v)) => {
                yielded.push(v);
                let r = replies[yielded.len() - 1];
                st = vm.resume(Value::Int(r)).expect("resume");
            }
            VmState::Reset => {
                let r = replies[yielded.len().saturating_sub(1)];
                st = vm.resume(Value::Int(r)).expect("resume after reset");
            }
            other => panic!("a stream chunk produced {other:?}"),
        }
    }
    yielded
}

/// Drive the SAME chunk as native code: one call per iteration, no callback.
///
/// This is the whole claim in executable form. If the degenerate lowering is
/// right, calling `kel_chunk_N(r)` repeatedly reproduces the virtual machine's
/// yielded sequence, with the previous return value feeding nothing and the
/// reply feeding slot 0.
fn native_stream_sequence(src: &str, args: &[i64], replies: &[i64]) -> Vec<i64> {
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

    let sym = format!("kel_chunk_{idx}");
    let f = unsafe { ee.get_function::<unsafe extern "C" fn(i64) -> i64>(&sym) }.expect("symbol");

    let mut out = Vec::new();
    let mut input = args[0];
    for &r in replies {
        out.push(unsafe { f.call(input) });
        input = r;
    }
    out
}

/// The degenerate form's observational equivalence, which is the ONE claim the
/// whole design rests on and the only thing that can settle it.
fn assert_stream_sequences_agree(src: &str, args: &[i64], replies: &[i64]) {
    let vy = vm_stream_sequence(src, args, replies);
    let ny = native_stream_sequence(src, args, replies);
    assert_eq!(
        ny, vy,
        "YIELD SEQUENCE differs for {src:?} args {args:?} replies {replies:?}\n  \
         native={ny:?}\n  vm    ={vy:?}"
    );
}

/// Observational equivalence for the degenerate form, which is the ONLY thing
/// that settles it.
///
/// `assert_sequences_agree` compares the whole yielded sequence and the final
/// result between the VM and native code, so a transformation that produced the
/// right values in the wrong order fails here and nowhere else. The inventory has
/// carried "equivalence is unproven" as the load-bearing gap since the rotation
/// was first written; these cases are what close it, or fail to.
///
/// The replies differ from each other and from the arguments on purpose. Equal
/// values would let a form that returns the argument instead of the resumed value
/// pass, which is exactly the confusion the two delivery paths invite.
#[test]
fn the_degenerate_stream_agrees_in_sequence_and_result() {
    // The bare shape: the yield IS the body.
    assert_stream_sequences_agree(
        "loop main(a: Word) -> Word { yield a }",
        &[10],
        &[21, 32, 43],
    );
    // A body that computes before yielding, so the yielded value is not simply
    // the parameter and a form that confuses the two is visible.
    assert_stream_sequences_agree(
        "loop main(a: Word) -> Word { yield a * 2 + 1 }",
        &[10],
        &[21, 32, 43],
    );
    // A branch before the yield, so the body is not straight-line and the
    // depth-zero rule is exercised against a chunk that really has an `If`.
    assert_stream_sequences_agree(
        "loop main(a: Word) -> Word { yield if a > 20 { a - 20 } else { a } }",
        &[10],
        &[21, 32, 43],
    );
    // A call, since eight of the ten self-hosted stages are `yield run()` and a
    // call is the shape that actually ships.
    assert_stream_sequences_agree(
        "fn double(x: Word) -> Word { x * 2 }\n\
         loop main(a: Word) -> Word { yield double(a) }",
        &[10],
        &[21, 32, 43],
    );
}

/// MUST-NOT-FIRE for the predicate: shapes it has to REFUSE.
///
/// A predicate verified only in the admitting direction is the vacuous-control
/// failure this project keeps catching. Each case below is refused for a
/// different one of the six conditions, so a predicate that lost any single
/// condition still fails this test.
///
/// Refusal is observed through `lower_module`, not by calling the predicate
/// directly, because that is the boundary a consumer meets. A predicate that
/// returns `None` while the emitter lowers the chunk anyway would pass a direct
/// test and ship a wrong module.
#[test]
fn shapes_outside_the_degenerate_class_are_still_refused() {
    // The resumed value is CONSUMED, so the tail is not `[PopN(1)]`. This is the
    // case `a_divergent_loop_function_is_refused` already pins; asserted here
    // too because it is the condition most likely to be relaxed by someone who
    // reads `PopN(1)` as bookkeeping.
    //
    // A tail that writes the DATA SEGMENT. That write survives `Reset` and is
    // therefore observable, unlike a local, so the yield is not in tail position
    // however balanced the operand stack is. This replaced a case that the
    // 2026-08-11 generalisation legitimately admits.
    assert_refused(
        "data st { n: Word }\n\
         loop main(a: Word) -> Word { yield a; st.n = a; 0 }",
    );

    // TWO top-level yields: a real partition, which the degenerate form does not
    // have. This is the multi-segment case that still needs the rotation.
    assert_refused("loop main(a: Word) -> Word { yield a; yield a + 1 }");

    // A NESTED yield was refused here until 2026-08-11, on the depth-zero rule.
    // **The rule deliberately widened** to tail position, and this shape is now
    // ADMITTED — both yields end their path, so it is a control-flow join rather
    // than a suspension. Its equivalence is asserted by
    // `nested_yields_in_tail_position_agree_in_sequence`, which drives both arms.
    //
    // The case is moved rather than deleted. A must-not-fire case that stops
    // firing because the rule changed is a decision, and deleting it silently
    // would leave no record that the boundary moved on purpose.

    // A DELEGATED suspension, which is the case no chunk-local reading catches.
    //
    // The op vector of `main` here looks degenerate: one top-level `Yield`, tail
    // exactly `PopN(1)`, `Stream` first and `Reset` last. It is NOT degenerate,
    // because `helper` suspends too, and on that suspension the VM overwrites
    // `main`'s resume parameter while native code does not.
    //
    // If this case is ever DROPPED because it looks redundant next to the others,
    // the predicate silently starts miscompiling a shape the corpus contains:
    // `codegen.kel` delegates its entire body this way.
    assert_refused(
        "yield helper(x: Word) -> Word { let r = yield x; r }\n\
         loop main(a: Word) -> Word { yield helper(a) }",
    );
}

/// Lower `src` and assert the module is REFUSED.
///
/// Deliberately does not match on the refusal text. The reason a chunk is
/// outside the degenerate class is not a stable interface, and asserting on it
/// would make this test fail when a message improves rather than when behaviour
/// regresses.
fn assert_refused(src: &str) {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    assert!(
        lower_module(&ctx, &lm, &m, LowerOptions::default()).is_err(),
        "this shape is outside the degenerate class and must be refused, not \
         lowered as though the resumed value were discarded:\n{src}"
    );
}

/// Observational equivalence for the NESTED case, which the tail-position rule
/// admits and the earlier depth-zero rule refused.
///
/// `lexer.kel` is this shape: nineteen yields, every one inside an `If` and none
/// under a `Loop`, nested up to depth eleven. Lowering it is not evidence that
/// lowering it is CORRECT — the transformation turns each of those yields into a
/// separate `ret`, so the claim is that every path yields once and ends, and only
/// a whole-sequence comparison can check that.
///
/// The replies deliberately drive DIFFERENT branches on successive iterations,
/// because a case that takes one path every time would exercise one `ret` and
/// prove nothing about the join.
#[test]
fn nested_yields_in_tail_position_agree_in_sequence() {
    // Both arms yield. Alternating replies cross the branch each iteration.
    assert_stream_sequences_agree(
        "loop main(a: Word) -> Word { if a > 10 { yield a * 2 } else { yield a + 100 } }",
        &[5],
        &[20, 3, 40, 7],
    );
    // Deeper nesting, three levels, with a yield at each leaf — the shape
    // `lexer.kel` actually has, in miniature.
    assert_stream_sequences_agree(
        "loop main(a: Word) -> Word { \
           if a > 100 { yield 1 } else { \
           if a > 50 { yield 2 } else { \
           if a > 10 { yield 3 } else { yield 4 } } } }",
        &[5],
        &[200, 75, 25, 1, 200],
    );
    // A yield in one arm and a call in the other, so the two paths are not
    // symmetric and a lowering that collapsed them would show up.
    assert_stream_sequences_agree(
        "fn triple(x: Word) -> Word { x * 3 }\n\
         loop main(a: Word) -> Word { if a > 10 { yield triple(a) } else { yield a } }",
        &[5],
        &[20, 3, 40],
    );
}

/// MUST-NOT-FIRE for the tail-position rule.
///
/// The rule admits a yield only when nothing but block delimiters and one
/// `PopN(1)` runs between it and `Reset`. These do work after the suspension, so
/// each must still be refused. Without them the rule could be relaxed to "any
/// nested yield" and nothing would catch it.
#[test]
fn yields_not_in_tail_position_are_still_refused() {
    // Work after the yield inside the branch: the `+ 1` runs post-suspension.
    assert_refused("loop main(a: Word) -> Word { if a > 0 { (yield a) + 1 } else { yield 0 } }");
}

/// The shape the tail-position rule refused until the allowlist was replaced by
/// the property it stood for.
///
/// Ten corpus chunks end `yield x; 0`, giving a tail of
/// `PopN(1), Const(0), PopN(1)`: the resume value is discarded, a constant is
/// pushed as the block's value, and that is discarded too. The sequence is
/// effect-free and reaches the same operand depth as the bare `PopN(1)` the old
/// rule admitted, so it is exactly as safe. Equivalence is asserted rather than
/// argued, because "obviously equivalent" is what the old rule's author thought
/// about the allowlist.
#[test]
fn an_effect_free_tail_after_the_yield_agrees_in_sequence() {
    assert_stream_sequences_agree(
        "loop main(a: Word) -> Word { yield a; 0 }",
        &[7],
        &[11, 22, 33],
    );
    // The same, with the yield nested, so the generalisation composes with the
    // tail-position walk rather than only working at the top level.
    assert_stream_sequences_agree(
        "loop main(a: Word) -> Word { if a > 10 { yield a * 2 } else { yield a }; 0 }",
        &[7],
        &[20, 3, 40],
    );
}

/// MUST-NOT-FIRE for the generalisation.
///
/// The rule now admits any tail that only touches the operand stack and this
/// frame's locals. It must still refuse a tail that TRAPS, because a trap is
/// observable and the virtual machine would take it after the suspension where
/// native code, having already returned, would not.
#[test]
fn a_tail_that_can_trap_is_still_refused() {
    // Checked arithmetic after the suspension. Under the trap policy this can
    // fault, so it is not effect-free and the yield is not in tail position.
    assert_refused("loop main(a: Word) -> Word { yield a; a * a }");
}

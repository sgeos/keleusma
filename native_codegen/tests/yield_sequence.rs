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
    let src = "loop main(a: Word) -> Word { let x = yield a; x }";
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    assert!(
        lower_module(&ctx, &lm, &m, LowerOptions::default()).is_err(),
        "a divergent loop function must be refused under a callback yield ABI"
    );
}

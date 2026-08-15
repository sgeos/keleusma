//! The three rogue AI streams, EXECUTED against the virtual machine.
//!
//! These lower only because `degenerate_stream_yield`'s tail walk was widened to
//! admit `NewComposite(Flat)`. Moving a soundness predicate on the reasoning
//! that a case looks fine is what the must-not-fire boundary exists to prevent,
//! so the widening does not stand on `module_refusals(...).is_empty()`. This is
//! the evidence.
//!
//! # These modules take AND return a composite
//!
//! ```text
//! loop main(input: (Word, Word, Word, Word, Word)) -> (Word, Word, Word)
//! ```
//!
//! Both directions are just an `i64` address, because that is how the emitter
//! represents every composite operand. The caller owns the argument body; the
//! returned body lives in the region buffer the caller passed in, which outlives
//! the call. **No `sret` slot is needed for THESE modules** — each is a single
//! chunk, so there is no caller/callee region collision to resolve. The `sret`
//! convention recorded in `NATIVE_COMPOSITE_RETURN_ABI.md` is still the answer
//! for the general case, where a callee's sites would overlap the caller's.
//!
//! # The oracle
//!
//! Per-tick **returned triple**, decoded on both sides, plus the shared data
//! segment byte for byte. These are state machines over `data state`, so the
//! segment is where their work lands; a lowering that computed the right triple
//! from the wrong slot would pass on the triple alone.
use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::{Module, SlotVisibility, TupleBody, Value};
use keleusma::flat_value::read_i64;
use keleusma::vm::{
    Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for, shared_data_bytes_for,
};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerOptions, lower_module};

mod common;

/// Enough ticks to leave the init branch and drive the state machine.
const TICKS: i64 = 400;

/// The five-word input for tick `t`. Deliberately asymmetric in every position,
/// so a swapped or dropped element changes the answer.
fn input_at(t: i64) -> [i64; 5] {
    [t % 17, (t * 3) % 11, 9 - (t % 5), 4 + (t % 7), t % 3]
}

fn module_of(name: &str) -> Module {
    let p = std::path::Path::new("../examples/scripts/rogue").join(name);
    let src = std::fs::read_to_string(&p).expect("read source");
    compile(&parse(&tokenize(&src).expect("lex")).expect("parse")).expect("compile")
}

/// Plus a host margin: `auto_arena_capacity_for` sizes the nominal stack, and a
/// long tick loop exhausts it. A property of this harness, not of the lowering —
/// the native side has a fixed frame and never touches this arena.
fn arena_for(m: &Module) -> keleusma_arena::Arena {
    const HOST_MARGIN: usize = 1 << 20;
    let need = required_persistent_capacity_for(m);
    let cap = auto_arena_capacity_for(m, &[]).expect("arena capacity") + need + HOST_MARGIN;
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent fits");
    arena
}

fn boxed_input(t: i64) -> Value {
    Value::Tuple(TupleBody::boxed(
        input_at(t).iter().map(|&x| Value::Int(x)).collect(),
    ))
}

/// Decode a yielded `(Word, Word, Word)` into three integers.
fn triple(st: &VmState, arena: &keleusma_arena::Arena) -> [i64; 3] {
    let v = match st {
        VmState::Yielded(v) | VmState::Finished(v) => v,
        other => panic!("unexpected VM outcome: {other:?}"),
    };
    match v {
        Value::Tuple(TupleBody::Flat(fc)) => {
            let b = fc.resolve(arena).expect("live flat body");
            [read_i64(b, 0), read_i64(b, 8), read_i64(b, 16)]
        }
        Value::Tuple(TupleBody::Boxed(els)) => {
            let g = |i: usize| match els.get(i) {
                Some(Value::Int(x)) => *x,
                other => panic!("non-integer tuple element: {other:?}"),
            };
            [g(0), g(1), g(2)]
        }
        other => panic!("expected a 3-tuple, got {other:?}"),
    }
}

fn run_vm(m: &Module) -> (Vec<[i64; 3]>, Vec<u8>) {
    let n_shared = shared_data_bytes_for(m);
    let arena = arena_for(m);
    let mut vm = Vm::new(m.clone(), &arena).expect("vm");
    let mut shared = vec![0u8; n_shared];
    let mut out = Vec::new();

    let first = vm
        .call_with_shared(&mut shared, &[boxed_input(0)])
        .expect("vm call");
    out.push(triple(&first, &arena));

    for t in 1..TICKS {
        // One tick is a `Reset` leg then a `Yielded` leg, and the SAME reply
        // goes to both. A fresh reply on the Reset leg is silently discarded and
        // desynchronises the sides. The native side has no counterpart leg — the
        // degenerate lowering is one call per tick, and that asymmetry is the
        // transformation under test.
        let mut st = vm
            .resume_with_shared(&mut shared, boxed_input(t))
            .expect("vm resume");
        if matches!(st, VmState::Reset) {
            st = vm
                .resume_with_shared(&mut shared, boxed_input(t))
                .expect("vm resume after reset");
        }
        out.push(triple(&st, &arena));
    }
    (out, shared)
}

fn run_native(m: &Module) -> (Vec<[i64; 3]>, Vec<u8>) {
    let entry = m.entry_point.expect("entry point");
    let n_shared = shared_data_bytes_for(m);
    let n_priv = m
        .data_layout
        .as_ref()
        .map(|dl| {
            dl.slots
                .iter()
                .filter(|s| s.visibility == SlotVisibility::Private)
                .count()
        })
        .unwrap_or(0);
    // **Transitive**, not the per-chunk sum. Each call site now receives a
    // disjoint block of the caller's region, so the entry needs everything it
    // can reach. The per-chunk sum under-counts and the canary would catch it.
    let n_region: usize = keleusma_native::region::region_total_bytes(m, entry, 0) as usize;

    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_module(&ctx, &lm, m, LowerOptions::default()).expect("lower module");
    lm.verify().expect("LLVM module verification");
    common::maybe_optimize(&lm);
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");

    const CANARY: u64 = 0xDEAD_BEEF_FEED_FACE;
    let mut shared = vec![0u8; n_shared + 8];
    shared[n_shared..].copy_from_slice(&CANARY.to_le_bytes());
    let mut privs = vec![0u64; n_priv + 1];
    privs[n_priv] = CANARY;
    let mut region = vec![0u64; n_region.div_ceil(8) + 1];
    let region_canary = n_region.div_ceil(8);
    region[region_canary] = CANARY;
    // The argument body is CALLER-OWNED storage, exactly as the `sret` reasoning
    // says it should be. It is deliberately a separate allocation from the
    // region, so a lowering that wrote through the argument pointer would
    // corrupt this buffer rather than silently land inside the region.
    let mut arg = vec![0i64; 5];

    let sym = format!("kel_chunk_{entry}");
    let declared = lm.get_function(&sym).expect("entry fn").count_params();
    assert_eq!(
        declared, 4,
        "entry `{sym}` takes {declared} parameters; this harness passes the \
         argument body address plus the three trailing pointers"
    );
    let f = unsafe {
        ee.get_function::<unsafe extern "C" fn(i64, *mut u8, *mut u8, *mut u8) -> i64>(&sym)
    }
    .expect("entry symbol");

    let mut out = Vec::new();
    for t in 0..TICKS {
        arg.copy_from_slice(&input_at(t));
        let ret = unsafe {
            f.call(
                arg.as_mut_ptr() as i64,
                shared.as_mut_ptr(),
                privs.as_mut_ptr() as *mut u8,
                region.as_mut_ptr() as *mut u8,
            )
        };
        // The return is the ADDRESS of the returned body, which lives in the
        // region buffer this caller provided and therefore outlives the call.
        assert!(ret != 0, "tick {t}: the entry returned a null body address");
        let p = ret as *const u8;
        let b = unsafe { core::slice::from_raw_parts(p, 24) };
        out.push([read_i64(b, 0), read_i64(b, 8), read_i64(b, 16)]);
    }

    assert_eq!(
        &shared[n_shared..],
        &CANARY.to_le_bytes(),
        "wrote past the {n_shared}-byte shared segment"
    );
    assert_eq!(privs[n_priv], CANARY, "wrote past the private region");
    assert_eq!(
        region[region_canary], CANARY,
        "wrote past the {n_region}-byte composite region"
    );

    shared.truncate(n_shared);
    (out, shared)
}

fn assert_agrees(name: &str) {
    let m = module_of(name);
    let (nres, nshared) = run_native(&m);
    let (vres, vshared) = run_vm(&m);

    if vres != nres {
        let at = vres
            .iter()
            .zip(nres.iter())
            .position(|(a, b)| a != b)
            .unwrap_or(0);
        panic!(
            "{name}: returned triples diverge at tick {at}\n  input  = {:?}\n  vm     = {:?}\n  native = {:?}",
            input_at(at as i64),
            vres.get(at),
            nres.get(at)
        );
    }
    assert_eq!(
        vshared, nshared,
        "{name}: the shared data segment disagrees after {TICKS} ticks"
    );
    assert!(
        vres.iter().any(|t| t != &[0, 0, 0]),
        "{name}: every returned triple is zero, so comparing them asserts \
         nothing about what the module computes"
    );
}

#[test]
fn rogue_ai_boss_agrees_with_the_vm() {
    assert_agrees("rogue_ai_boss.kel");
}

#[test]
fn rogue_ai_hunter_agrees_with_the_vm() {
    assert_agrees("rogue_ai_hunter.kel");
}

#[test]
fn rogue_ai_tracker_agrees_with_the_vm() {
    assert_agrees("rogue_ai_tracker.kel");
}

/// **Must not fire.** `codegen.kel` refuses on `Stream` for a SOUNDNESS reason,
/// not a gap: it has no `Yield` of its own and a `Reentrant` callee, which is
/// the delegated-suspension case. `resume_after_enter` writes slot 0 of the
/// ENTRY chunk whenever that entry is a `Stream`, regardless of which frame
/// suspended, so natively a callee's `kel_yield` return would reach only the
/// callee and the next iteration would read a stale resume value.
///
/// The tail-walk widening must not have admitted it. Counting it alongside the
/// three modules above as "four `Stream` refusals" was a planning error: they
/// share an opcode and nothing else.
#[test]
fn codegen_kel_is_still_refused_for_delegated_suspension() {
    let src = std::fs::read_to_string("../src/selfhost/kel/codegen.kel").expect("read codegen.kel");
    let m = compile(&parse(&tokenize(&src).expect("lex")).expect("parse")).expect("compile");
    let refusals = keleusma_native::module_refusals(&m, LowerOptions::default());
    assert!(
        refusals
            .iter()
            .any(|(_, e)| format!("{e}").contains("Stream")),
        "codegen.kel must still refuse on Stream; the widening was meant to \
         admit a discarded-composite tail, not a delegated suspension. \
         Refusals: {refusals:?}"
    );
}

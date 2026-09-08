//! **DOES A RESUMABLE STREAM AGREE WITH THE VIRTUAL MACHINE?**
//!
//! `yield_sequence.rs` settles the DEGENERATE shape, where the single tail
//! `yield` becomes the return and no state crosses the suspension. This file
//! asks the same question of the general shape, where a `yield` is followed by
//! more of the body and the chunk must genuinely resume.
//!
//! # The claim under test
//!
//! A general stream lowers to a function that RETURNS at each `yield` and
//! re-enters at the same point, with its locals and its resume state in the
//! arena. So calling it repeatedly must reproduce exactly the sequence the
//! virtual machine yields.
//!
//! **The arena is the instance.** Nothing here is global, so two arenas would be
//! two independent streams — that property is what the design rests on, and it
//! is why the frame is addressed from the pointers the host supplies rather than
//! from anything static.
//!
//! # Why the signature is asserted before the call
//!
//! Extending the arena pointers to stream chunks once changed the signature of
//! the DEGENERATE ones too, and `yield_sequence.rs` calls those through a
//! hand-written `extern "C" fn(i64) -> i64`. **Ten tests passed on the calling
//! convention's good manners** rather than on being right, and would have kept
//! passing until a chunk first dereferenced one of the garbage pointers. A
//! harness that names a signature cannot see it change, so this one checks
//! `count_params` first.

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::Value;
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerOptions, lower_module, module_refusals};

mod common;

fn arena_for(m: &keleusma::bytecode::Module) -> keleusma_arena::Arena {
    let need = required_persistent_capacity_for(m);
    let cap = auto_arena_capacity_for(m, &[]).expect("arena capacity") + need + (64 << 10);
    let mut a = keleusma_arena::Arena::with_capacity(cap);
    a.resize_persistent(need).expect("persistent region");
    a
}

/// Every value the virtual machine yields, bounded by the reply count.
fn vm_sequence(src: &str, first: i64, replies: &[i64]) -> Vec<i64> {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let arena = arena_for(&m);
    let mut vm = Vm::new(m, &arena).expect("vm");
    let mut out = Vec::new();
    let mut st = vm.call(&[Value::Int(first)]).expect("vm run");
    while out.len() < replies.len() {
        match st {
            VmState::Yielded(Value::Int(v)) => {
                out.push(v);
                let r = replies[out.len() - 1];
                st = vm.resume(Value::Int(r)).expect("resume");
            }
            // The runtime reports the rewind as a leg of its own; the native
            // driver collapses it, so it contributes no yielded value here.
            VmState::Reset => {
                let r = replies[out.len().saturating_sub(1)];
                st = vm.resume(Value::Int(r)).expect("resume after reset");
            }
            other => panic!("a stream produced {other:?}"),
        }
    }
    out
}

/// The same chunk as native code: one call per suspension, arena-resident state.
fn native_sequence(src: &str, first: i64, replies: &[i64]) -> Vec<i64> {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let entry = m.entry_point.expect("entry point");
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("lower module");
    lm.verify().expect("LLVM module verification");
    common::maybe_optimize(&lm);
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");

    let sym = format!("kel_chunk_{entry}");
    let f = lm.get_function(&sym).expect("entry function");
    assert_eq!(
        f.count_params(),
        u32::from(m.chunks[entry].param_count) + 3,
        "a resumable stream must carry the three trailing pointers; the call below \
         names that signature by hand and cannot detect a change to it"
    );
    let callable = unsafe {
        ee.get_function::<unsafe extern "C" fn(i64, *mut u8, *mut u8, *mut u8) -> i64>(&sym)
    }
    .expect("entry symbol");

    // **THE ARENA IS THE INSTANCE**, so every buffer here belongs to this stream
    // and to no other. Persistent carries the resume state, which is why it is
    // sized with the supplement rather than with the runtime's figure alone.
    let persistent = required_persistent_capacity_for(&m)
        + keleusma_native::region::persistent_supplement_bytes(&m) as usize;
    let mut privs = vec![0u8; persistent + 64];
    let mut shared = vec![0u8; 4096];
    let mut region =
        vec![0u8; keleusma_native::region::host_arena_supplement_bytes(&m) as usize + 4096];

    let mut out = Vec::new();
    let mut input = first;
    for &r in replies {
        out.push(unsafe {
            callable.call(
                input,
                shared.as_mut_ptr(),
                privs.as_mut_ptr(),
                region.as_mut_ptr(),
            )
        });
        input = r;
    }
    out
}

fn assert_agrees(src: &str, first: i64, replies: &[i64]) {
    assert!(
        module_refusals(
            &compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile"),
            LowerOptions::default()
        )
        .is_empty(),
        "the backend refuses {src:?}, so this file would be asserting agreement about \
         a program it never ran"
    );
    let vm = vm_sequence(src, first, replies);
    let nat = native_sequence(src, first, replies);
    assert_eq!(
        nat, vm,
        "YIELD SEQUENCE differs for {src:?}\n  native={nat:?}\n  vm    ={vm:?}"
    );
}

/// **A yield followed by more body** — the shape the degenerate path cannot take.
#[test]
fn a_yield_with_a_trailing_expression_agrees() {
    assert_agrees(
        "loop main(t: Word) -> Word { let a = yield t; yield a + 1 }\n",
        7,
        &[10, 20, 30, 40],
    );
}

/// **Two suspensions in one iteration**, so the resume state must distinguish
/// them rather than always rewinding to the top.
#[test]
fn two_yields_in_one_iteration_agree() {
    assert_agrees(
        "loop main(t: Word) -> Word { yield t; yield t + 1 }\n",
        3,
        &[5, 6, 7, 8, 9],
    );
}

/// **A yield inside a branch WITH CODE AFTER IT — REFUSED, and that is the
/// designed answer rather than a gap.**
///
/// Its resume point lands on the `Op::Else`, so the resume edge (empty stack) and
/// the fall-through (carrying the branch's value) disagree about the operand
/// stack. Reconciling them is a spill question, and this backend refuses rather
/// than inventing a layout the differential could not vouch for.
///
/// # A first version of this test asserted the wrong thing
///
/// It used `if t > 0 { yield t } else { yield 0 }` and expected agreement. Yields
/// in tail position of both arms are DEGENERATE, already handled by the existing
/// path, and that chunk carries no arena pointers at all — **the signature
/// assertion caught it** rather than letting this file claim credit for a shape it
/// never exercised.
#[test]
fn a_yield_inside_an_if_with_a_tail_is_refused() {
    let src =
        "loop main(t: Word) -> Word { if t > 0 { let a = yield t; yield a } else { yield 0 } }\n";
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let r = module_refusals(&m, LowerOptions::default());
    assert!(
        !r.is_empty(),
        "a suspension inside an expression now lowers. If a spill layout was added, \
         this must become an AGREEMENT test rather than being deleted."
    );
    assert!(
        r.iter().any(|(_, e)| e.to_string().contains("operand")),
        "refused, but not for the operand-stack reason this file is about: {r:?}"
    );
}

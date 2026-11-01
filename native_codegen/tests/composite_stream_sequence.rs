//! **DOES A COMPOSITE-YIELDING STREAM AGREE WITH THE VIRTUAL MACHINE, YIELD BY
//! YIELD?**
//!
//! # The gap this closes, and why it was open
//!
//! `stream_frontier.rs` has carried "the suspension differential drives no
//! composite-yielding subject" as a recorded gap. Its stated cause changed twice,
//! and both statements were true when written:
//!
//! 1. Until 2026-09-08, a composite-yielding stream needed a yield that is not in
//!    tail position, and every non-tail yield was refused for `Op::Stream`.
//! 2. After general `Stream` lowering landed, that blocker was gone and a new one
//!    was visible: **a composite built from a RESUMED VALUE was refused for an
//!    operand of unknown width.** The resume value was pushed at `Width::Unknown`
//!    while the identical value reached local slot 0 at its DECLARED width.
//!
//! With the declared width reaching the operand stack, the shape lowers. **That
//! is not the claim under test.** Lowering says an arm ran. This file compares
//! the BODY BYTES of every yielded composite against the reference runtime, for
//! a subject that suspends and resumes repeatedly.
//!
//! # Why bytes rather than a handle
//!
//! `composite_yield_witness.rs` recorded the mistake this file inherits: a first
//! attempt there compared the reference's `Debug` text, which shows the arena
//! HANDLE and not the body. The bytes have to be read through the arena on the
//! reference side and out of the region on the native side.
//!
//! # Why the region offset is read fresh on every call
//!
//! Every construction site has a FIXED offset, so successive iterations write the
//! same bytes. The host must therefore read a yielded body BEFORE making the next
//! call — which is exactly the discipline the yield-escape refusal exists to
//! enforce for the cases where it cannot be relied upon. Here the subject builds
//! outside any user loop, so the read-then-call order is sufficient and the
//! refusal correctly does not fire.

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::{StructBody, Value};
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
use keleusma_native::{LowerOptions, lower_module, module_refusals, region};

mod common;

/// A composite yielded from a resume point: the second `yield` builds `P` out of
/// the value delivered by the first, which is the operand whose width was
/// unknown.
const SUBJECT: &str = "struct P { a: Word, b: Word }\n\
                       loop main(t: Word) -> P { \
                         let r = yield P { a: t, b: t + 1 }; \
                         yield P { a: r, b: r + 2 } }";

/// Every yielded composite's BODY BYTES, from the reference runtime.
fn vm_bodies(src: &str, first: i64, replies: &[i64]) -> Vec<Vec<u8>> {
    let m = common::build(src);
    let need = required_persistent_capacity_for(&m);
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena capacity") + need + (64 << 10);
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent region");

    let mut vm = Vm::new(m, &arena).expect("vm");
    let mut out: Vec<Vec<u8>> = Vec::new();
    let mut st = vm.call(&[Value::Int(first)]).expect("vm run");
    while out.len() < replies.len() {
        match st {
            VmState::Yielded(Value::Struct(StructBody::Flat(ref fc))) => {
                out.push(fc.resolve(&arena).expect("a live flat body").to_vec());
                let r = replies[out.len() - 1];
                st = vm.resume(Value::Int(r)).expect("resume");
            }
            // The runtime reports the rewind as a leg of its own; the native
            // driver collapses it, so it yields no body and takes the same reply.
            VmState::Reset => {
                let r = replies[out.len().saturating_sub(1)];
                st = vm.resume(Value::Int(r)).expect("resume after reset");
            }
            ref other => panic!("expected a yielded flat struct, got {other:?}"),
        }
    }
    out
}

/// The same, from native code: one call per suspension, body read out of the
/// region at the offset the call returns.
fn native_bodies(src: &str, first: i64, replies: &[i64], body_len: usize) -> Vec<Vec<u8>> {
    let m = common::build(src);
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
        "the call below names this signature by hand and cannot detect a change \
         to it; ten tests once passed on the calling convention's good manners"
    );
    let callable = unsafe {
        ee.get_function::<unsafe extern "C" fn(i64, *mut u8, *mut u8, *mut u8) -> i64>(&sym)
    }
    .expect("entry symbol");

    let persistent =
        required_persistent_capacity_for(&m) + region::persistent_supplement_bytes(&m) as usize;
    let mut privs = vec![0u8; persistent + 64];
    let mut shared = vec![0u8; 4096];
    let n_region = region::host_arena_supplement_bytes(&m) as usize + 4096;
    let mut region_buf = vec![0u8; n_region];

    let base = region_buf.as_ptr() as usize;
    let mut out: Vec<Vec<u8>> = Vec::new();
    let mut input = first;
    for &r in replies {
        let ret = unsafe {
            callable.call(
                input,
                shared.as_mut_ptr(),
                privs.as_mut_ptr(),
                region_buf.as_mut_ptr(),
            )
        } as usize;
        assert!(
            ret >= base && ret + body_len <= base + n_region,
            "the yielded handle {ret:#x} is not a body inside the region the host \
             provided ({base:#x}, {n_region} bytes), so the host cannot account \
             for what it was handed"
        );
        let off = ret - base;
        // **READ BEFORE THE NEXT CALL.** The site's offset is fixed, so the next
        // iteration overwrites these bytes.
        out.push(region_buf[off..off + body_len].to_vec());
        input = r;
    }
    out
}

/// **THE CLAIM.** A composite-yielding stream reproduces the reference's yielded
/// bodies, byte for byte, across repeated suspension and resumption.
#[test]
fn a_composite_yielding_stream_agrees_body_for_body() {
    assert!(
        module_refusals(&common::build(SUBJECT), LowerOptions::default()).is_empty(),
        "the backend refuses the subject, so this file would be asserting \
         agreement about a program it never ran"
    );

    // The replies differ from each other and from the argument, and the second
    // yield derives BOTH fields from the resumed value, so a lowering that
    // delivered the argument instead of the reply would diverge immediately.
    let first = 7;
    let replies = [11i64, 20, 31, 40, 55];

    let vm = vm_bodies(SUBJECT, first, &replies);
    let body_len = vm.first().expect("the reference yielded nothing").len();
    let nat = native_bodies(SUBJECT, first, &replies, body_len);

    println!("\n================ COMPOSITE YIELD SEQUENCE");
    for (i, (v, n)) in vm.iter().zip(nat.iter()).enumerate() {
        println!("  {i}: reference {v:?}\n     native    {n:?}");
    }
    println!("================\n");

    assert_eq!(
        nat, vm,
        "the yielded composite bodies differ.\n  native={nat:?}\n  vm    ={vm:?}"
    );
}

/// **NON-VACUITY: the bodies must actually vary across the sequence.**
///
/// Every construction site has a fixed offset, so a lowering that never wrote
/// the region again would return the same bytes forever — and if the reference
/// happened to be compared against a constant sequence, the test above would
/// pass while establishing nothing about resumption.
#[test]
fn the_yielded_bodies_are_not_all_identical() {
    let vm = vm_bodies(SUBJECT, 7, &[11, 20, 31, 40, 55]);
    let distinct: std::collections::BTreeSet<&Vec<u8>> = vm.iter().collect();
    println!(
        "\n  distinct reference bodies: {} of {}",
        distinct.len(),
        vm.len()
    );
    assert!(
        distinct.len() > 1,
        "the reference yields the same body every time, so the comparison above \
         would hold for a lowering that never resumed at all: {vm:?}"
    );
}

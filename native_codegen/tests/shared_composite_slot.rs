//! **A COMPOSITE IN A SHARED DATA SLOT: COPY IN, ADDRESS OUT.**
//!
//! # What was refused, and why the refusal was right
//!
//! A data slot access is one word. For a flat composite the operand is the
//! address of a body, so writing it into a destination that outlives the region
//! stores a pointer where the reference stores bytes. The shared segment is the
//! host's buffer and outlives everything, so the general refusal caught it.
//!
//! # Why this is easier than the private pool was
//!
//! `SharedSlotLayout` **states** the body length in a field. The private pool
//! needed a size derived from neighbouring offsets and therefore a validated
//! partition; this needs neither. The length is read, not computed.
//!
//! # The semantic difference from a private composite slot
//!
//! A private composite slot's initial value is `Unit`, which is not a body, so
//! reading an unwritten one FAULTS — that is what the per-slot initialisation
//! word is for. **The shared segment has no such state.** The host owns the
//! buffer and initialises it by contract, and the reference copies out whatever
//! bytes are there and re-wraps them as the declared kind.
//!
//! **So no initialisation word belongs here**, and inventing a fault would be
//! inventing one the reference does not have. `an_unwritten_shared_slot_reads_the_hosts_bytes`
//! is the subject that would fail if one were ever added.
//!
//! # Why this file carries its own harness
//!
//! The shared differential helper drives `Vm::call`, which refuses a module with
//! shared data. And the observable that matters here is **the host buffer's bytes
//! after the call**, which no helper returns: a test that only compared the
//! returned scalar would pass against a lowering that wrote the body somewhere
//! else and read it back from there.
//!
//! # The indexed form
//!
//! Also lowered, by the resolver the scalar shared arrays already used, extended
//! to compare each element's STATED body length — **a matching composite kind is
//! not a matching size.** A range that is not contiguous at that stride, or not
//! uniform in kind or length, is refused with the property that failed named.
//! No compiler can currently produce one, so those refusals are reasoned rather
//! than driven.

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::Value;
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
use keleusma_native::{LowerOptions, lower_module, module_refusals, region};

mod common;

const SUBJECT: &str = "struct F { a: Word, b: Word }\n\
                       shared data io { latest: F, n: Word }\n\
                       fn main(t: Word, u: Word) -> Word {\n\
                           io.latest = F { a: t, b: t + 1 };\n\
                           io.latest.a + io.latest.b + io.n + u\n\
                       }\n";

/// Run on the reference with a host buffer, returning the value and the buffer.
fn vm_run(src: &str, a: i64, b: i64, seed: &[u8]) -> (i64, Vec<u8>) {
    let m = common::build(src);
    let n_shared = keleusma::vm::shared_data_bytes_for(&m);
    let mut shared = vec![0u8; n_shared];
    let n = seed.len().min(n_shared);
    shared[..n].copy_from_slice(&seed[..n]);
    let need = required_persistent_capacity_for(&m);
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena") + need + (1 << 20);
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent");
    let mut vm = Vm::new(m, &arena).expect("vm");
    let v = match vm
        .call_with_shared(&mut shared, &[Value::Int(a), Value::Int(b)])
        .expect("vm run")
    {
        VmState::Finished(Value::Int(v)) | VmState::Yielded(Value::Int(v)) => v,
        other => panic!("unexpected VM outcome: {other:?}"),
    };
    (v, shared)
}

/// The same natively, over a buffer sized from the published contract.
fn native_run(src: &str, a: i64, b: i64, seed: &[u8]) -> (i64, Vec<u8>) {
    let m = common::build(src);
    let entry = m.entry_point.expect("entry");
    let ctx = Context::create();
    let lm = ctx.create_module("k");
    lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("lower");
    common::maybe_optimize(&lm);
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");

    const CANARY: u64 = 0xDEAD_BEEF_FEED_FACE;
    let n_shared = keleusma::vm::shared_data_bytes_for(&m);
    let mut shared = vec![0u8; n_shared + 8];
    let n = seed.len().min(n_shared);
    shared[..n].copy_from_slice(&seed[..n]);
    shared[n_shared..].copy_from_slice(&CANARY.to_le_bytes());
    let n_priv = (required_persistent_capacity_for(&m)
        + region::persistent_supplement_bytes(&m) as usize)
        .div_ceil(8)
        .max(1);
    let mut privs = vec![0u64; n_priv + 1];
    privs[n_priv] = CANARY;
    common::install_private_init(&m, &mut privs[..n_priv]);
    let n_region = region::host_arena_supplement_bytes(&m) as usize;
    let mut reg = vec![0u64; n_region.div_ceil(8) + 1];
    let reg_canary = reg.len() - 1;
    reg[reg_canary] = CANARY;

    let f = unsafe {
        ee.get_function::<unsafe extern "C" fn(i64, i64, *mut u8, *mut u8, *mut u8) -> i64>(
            &format!("kel_chunk_{entry}"),
        )
    }
    .expect("entry symbol");
    let v = unsafe {
        f.call(
            a,
            b,
            shared.as_mut_ptr(),
            privs.as_mut_ptr() as *mut u8,
            reg.as_mut_ptr() as *mut u8,
        )
    };
    assert_eq!(
        u64::from_le_bytes(shared[n_shared..].try_into().unwrap()),
        CANARY,
        "the lowering wrote past the shared segment"
    );
    assert_eq!(
        privs[n_priv], CANARY,
        "the lowering wrote past the private region"
    );
    assert_eq!(
        reg[reg_canary], CANARY,
        "the lowering wrote past the composite region"
    );
    (v, shared[..n_shared].to_vec())
}

/// **The value AND the bytes.** A test comparing only the returned scalar would
/// pass against a lowering that put the body somewhere else and read it back
/// from there.
#[test]
fn a_shared_composite_slot_agrees_in_the_value_and_in_the_hosts_bytes() {
    let refusals = module_refusals(&common::build(SUBJECT), LowerOptions::default());
    assert!(refusals.is_empty(), "the subject must lower: {refusals:?}");

    let seed = [0u8; 0];
    let (vm_v, vm_buf) = vm_run(SUBJECT, 5, 1, &seed);
    let (nat_v, nat_buf) = native_run(SUBJECT, 5, 1, &seed);

    assert_eq!(
        vm_v, 12,
        "5 + 6 + 0 + 1; if this moves the subject has changed"
    );
    assert_eq!(nat_v, vm_v, "value: native={nat_v} vm={vm_v}");
    assert_eq!(
        nat_buf, vm_buf,
        "the host's bytes differ after the call:\n  native={nat_buf:?}\n  vm    ={vm_buf:?}"
    );

    // The body must actually be in the buffer at the stated offset, or both
    // sides could agree by both being wrong.
    let m = common::build(SUBJECT);
    let e = m.data_layout.as_ref().expect("layout").shared_layout[0];
    let off = e.offset as usize;
    let a = i64::from_le_bytes(nat_buf[off..off + 8].try_into().unwrap());
    assert_eq!(
        a, 5,
        "the composite's first field must be at its stated offset"
    );
}

/// **No fault is invented for a slot the program has not written.**
///
/// The host's bytes are the value. A lowering that borrowed the private path's
/// initialisation word would trap here, where the reference simply reads.
#[test]
fn an_unwritten_shared_slot_reads_the_hosts_bytes() {
    const READ_ONLY: &str = "struct F { a: Word, b: Word }\n\
                             shared data io { latest: F, n: Word }\n\
                             fn main(t: Word, u: Word) -> Word {\n\
                                 io.latest.a + io.latest.b + t + u\n\
                             }\n";
    let refusals = module_refusals(&common::build(READ_ONLY), LowerOptions::default());
    assert!(
        refusals.is_empty(),
        "the read-only subject must lower: {refusals:?}"
    );

    // Host-provided bytes: a = 7, b = 9.
    let mut seed = vec![0u8; 24];
    seed[0..8].copy_from_slice(&7i64.to_le_bytes());
    seed[8..16].copy_from_slice(&9i64.to_le_bytes());

    let (vm_v, _) = vm_run(READ_ONLY, 1, 2, &seed);
    let (nat_v, _) = native_run(READ_ONLY, 1, 2, &seed);
    assert_eq!(
        vm_v, 19,
        "7 + 9 + 1 + 2; the reference reads what the host put there"
    );
    assert_eq!(nat_v, vm_v, "native={nat_v} vm={vm_v}");
}

/// **THE INDEXED FORM, AT CONSTANT AND RUNTIME INDICES.**
///
/// It was refused when the direct case landed, because the layout entries for the
/// range were not proven contiguous and uniform. They are now, by the same
/// resolver the scalar shared arrays already used — extended to compare each
/// element's STATED body length, since a matching composite kind is not a
/// matching size.
#[test]
fn an_indexed_shared_composite_agrees_and_lands_at_the_stated_offsets() {
    const ARRAY: &str = "struct F { a: Word, b: Word }\n\
                         shared data io { items: [F; 3], n: Word }\n\
                         fn main(t: Word, u: Word) -> Word {\n\
                             io.items[0] = F { a: t, b: t + 1 };\n\
                             io.items[2] = F { a: t * 10, b: 0 };\n\
                             io.items[u].a\n\
                         }\n";
    // **NO EARLY RETURN.** An earlier draft skipped when the reference declined
    // the shape, and `skippable_tests.rs` refused it: a test that can pass
    // without testing joins a pass count as evidence it did not earn. The
    // reference's acceptance of a `.kel` shape is a fact of THIS TREE, not an
    // environment capability like having a C compiler, so it is asserted — if the
    // reference ever stops compiling this, a loud failure is the honest outcome.
    let m = common::try_build(ARRAY)
        .expect("the reference compiles an array-of-composite shared slot; if it no longer does,                  that is the finding rather than a reason to skip");
    let refusals = module_refusals(&m, LowerOptions::default());
    assert!(
        refusals.is_empty(),
        "the indexed shared form must lower: {refusals:?}"
    );

    let seed = [0u8; 0];
    for u in [0i64, 2] {
        let (vm_v, vm_buf) = vm_run(ARRAY, 5, u, &seed);
        let (nat_v, nat_buf) = native_run(ARRAY, 5, u, &seed);
        assert_eq!(nat_v, vm_v, "index {u}: native={nat_v} vm={vm_v}");
        assert_eq!(
            nat_buf, vm_buf,
            "index {u}: the host's bytes differ after the call"
        );
    }

    // **The bodies must be at the offsets the layout STATES**, or both sides
    // could agree by both writing somewhere else.
    let (_, buf) = native_run(ARRAY, 5, 0, &seed);
    let dl = m.data_layout.as_ref().expect("layout");
    for (i, expect) in [(0usize, 5i64), (2, 50)] {
        let off = dl.shared_layout[i].offset as usize;
        let a = i64::from_le_bytes(buf[off..off + 8].try_into().unwrap());
        assert_eq!(a, expect, "element {i}'s first field at its stated offset");
    }
}

/// **A scalar shared array is unaffected**, since the same resolver serves both
/// and a composite branch that changed its scalar answer would break arms with no
/// relation to this work.
#[test]
fn a_scalar_shared_array_still_agrees() {
    const SCALARS: &str = "shared data io { xs: [Word; 3], n: Word }\n\
                           fn main(t: Word, u: Word) -> Word {\n\
                               io.xs[0] = t;\n\
                               io.xs[2] = t * 10;\n\
                               io.xs[u] + io.n\n\
                           }\n";
    let refusals = module_refusals(&common::build(SCALARS), LowerOptions::default());
    assert!(
        refusals.is_empty(),
        "the scalar control must lower: {refusals:?}"
    );
    let seed = [0u8; 0];
    for u in [0i64, 2] {
        let (vm_v, vm_buf) = vm_run(SCALARS, 7, u, &seed);
        let (nat_v, nat_buf) = native_run(SCALARS, 7, u, &seed);
        assert_eq!(nat_v, vm_v, "index {u}: native={nat_v} vm={vm_v}");
        assert_eq!(nat_buf, vm_buf, "index {u}: host bytes differ");
    }
}

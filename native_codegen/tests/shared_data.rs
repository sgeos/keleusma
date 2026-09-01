//! Shared data-segment slots, differential against the VM.
//!
//! # Why this file is separate from `differential.rs`
//!
//! A module with shared slots has a different calling convention. The host
//! buffer arrives as a trailing pointer parameter, so the entry point is not the
//! `fn(i64, i64) -> i64` the other harness drives, and the VM side must be
//! entered through `call_with_shared` rather than `call`.
//!
//! # The oracle here is stronger than elsewhere
//!
//! Every other differential test in this package compares a returned value. This
//! one compares **the host buffer as well**, byte for byte, after both runs.
//! That matters because a data-segment store is an effect rather than a result,
//! and a lowering that wrote the right value to the wrong offset, or the right
//! offset at the wrong width, would return an identical answer while corrupting
//! an adjacent slot. Comparing only the return value would not see it.

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::Value;
use keleusma::vm::{
    Vm, auto_arena_capacity_for, required_persistent_capacity_for, shared_data_bytes_for,
};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerOptions, lower_module};

mod common;

/// Build an arena sized for BOTH the operand stack and the persistent region.
///
/// `auto_arena_capacity_for` covers the stack and call frames and **not** the
/// persistent region, so a module with private data slots gets an arena whose
/// persistent capacity is zero. The runtime then computes a private slot
/// address from a zero-length region and the process dies with SIGBUS, which
/// looks like a code-generation fault and is not one. The `v0.2.3` session
/// recorded exactly this trap after six constructs appeared to be rejected by
/// the language when the arena was the cause.
fn arena_for(m: &keleusma::bytecode::Module) -> keleusma_arena::Arena {
    let need = required_persistent_capacity_for(m);
    let cap = auto_arena_capacity_for(m, &[]).expect("arena capacity") + need;
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena
        .resize_persistent(need)
        .expect("persistent region fits the capacity just reserved for it");
    arena
}

/// Run `src` on the VM with an exactly-sized shared buffer, returning the
/// result and the buffer's final contents.
fn vm_shared(src: &str, args: &[i64]) -> (i64, Vec<u8>) {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let n = shared_data_bytes_for(&m);
    let arena = arena_for(&m);
    let mut vm = Vm::new(m, &arena).expect("vm");
    let mut buf = vec![0u8; n];
    let vals: Vec<Value> = args.iter().map(|&x| Value::Int(x)).collect();
    let st = vm.call_with_shared(&mut buf, &vals).expect("vm run");
    match st {
        keleusma::vm::VmState::Finished(Value::Int(v)) => (v, buf),
        other => panic!("unexpected VM outcome: {other:?}"),
    }
}

/// Lower every chunk, JIT the entry, and call it with the same buffer shape.
fn native_shared(src: &str, args: &[i64]) -> (i64, Vec<u8>) {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let n = shared_data_bytes_for(&m);
    let idx = m
        .chunks
        .iter()
        .position(|c| c.name == "main")
        .expect("entry chunk");

    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("lower module");
    lm.verify().expect("LLVM module verification");
    common::maybe_optimize(&lm);
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");

    let mut buf = vec![0u8; n];
    // The private region is the caller's to provide, exactly as the shared
    // buffer is, and its layout is the backend's own. **Allocated as words, not
    // bytes**: the region must be aligned to `PRIVATE_SLOT_BYTES`, which is part
    // of the ABI, and a `Vec<u8>` carries an alignment contract of one byte.
    let n_priv = m
        .data_layout
        .as_ref()
        .map(|dl| {
            dl.slots
                .iter()
                .filter(|s| s.visibility == keleusma::bytecode::SlotVisibility::Private)
                .count()
        })
        .unwrap_or(0);
    // Exactly `n_priv` words, plus ONE CANARY word the lowering must never
    // touch. A uniform offset error in private addressing is value-invariant --
    // reads and writes shift together, so every round-trip returns the same
    // answer -- and is therefore invisible to a differential comparison. It is
    // observable only as a write outside the region, which the canary detects
    // deterministically. Found by a mutation that deleted the shared-boundary
    // subtraction and left every value test passing.
    const CANARY: u64 = 0xDEAD_BEEF_FEED_FACE;
    let mut priv_region = vec![0u64; n_priv + 1];
    priv_region[n_priv] = CANARY;
    let sym = format!("kel_chunk_{idx}");
    let out = match args.len() {
        2 => {
            let f = unsafe {
                ee.get_function::<unsafe extern "C" fn(i64, i64, *mut u8, *mut u8) -> i64>(&sym)
            }
            .expect("symbol");
            unsafe {
                f.call(
                    args[0],
                    args[1],
                    buf.as_mut_ptr(),
                    priv_region.as_mut_ptr() as *mut u8,
                )
            }
        }
        k => panic!("harness does not drive {k}-argument entry points"),
    };
    assert_eq!(
        priv_region[n_priv], CANARY,
        "the lowering wrote past the private region: a private slot index \
         exceeded the {n_priv} slots the module declares"
    );
    (out, buf)
}

fn assert_shared_agrees(src: &str, args: &[i64]) {
    let (nv, nb) = native_shared(src, args);
    let (vv, vb) = vm_shared(src, args);
    assert_eq!(
        nv, vv,
        "return value differs for {src:?} args {args:?}: native={nv} vm={vv}"
    );
    assert_eq!(
        nb, vb,
        "SHARED BUFFER differs for {src:?} args {args:?}\n  native={nb:?}\n  vm    ={vb:?}"
    );
}

#[test]
fn shared_word_slots_agree_with_the_vm_in_value_and_in_buffer() {
    // A store followed by a load, so both directions are exercised, and the
    // wrapping corner because a data slot is a plain word with no checked form.
    let src = "shared data state { counter: Word, flag: Byte }
               fn main(a: Word, b: Word) -> Word { state.counter = a + b; state.counter }";
    for args in [[7, 5], [0, 0], [-3, 1], [i64::MAX, 1], [i64::MIN, -1]] {
        assert_shared_agrees(src, &args);
    }
}

#[test]
fn a_narrow_slot_does_not_disturb_its_neighbour() {
    // **THE CASE THAT JUSTIFIES COMPARING BUFFERS.** `flag` is one byte at
    // offset 8 and `counter` is eight bytes at offset 0. A lowering that stored
    // the byte at the wrong width would overwrite bytes 9 through 15, which no
    // return value would reveal, because the function returns `counter` and the
    // damage is past it.
    let src = "shared data state { counter: Word, flag: Byte, tail: Word }
               fn main(a: Word, b: Word) -> Word {
                   state.counter = a; state.tail = b; state.flag = 255 as Byte; state.counter
               }";
    for args in [[1, 2], [-1, -1], [i64::MAX, i64::MIN]] {
        assert_shared_agrees(src, &args);
    }
}

#[test]
fn a_narrow_slot_reads_back_zero_extended() {
    // **ADDED BECAUSE A MUTATION FOUND THE GAP.** Changing the narrow load from
    // zero-extension to sign-extension left every other test in this file
    // passing, because they all WRITE a byte slot and none READS one back. The
    // sign of a byte-slot read is unobservable unless a test returns it.
    //
    // `200` is the discriminating value: zero-extended it is 200, sign-extended
    // it is -56. Values below 128 agree under both and would prove nothing,
    // which is the same symmetry trap that produced three vacuous tests earlier
    // in this work.
    let src = "shared data state { flag: Byte, counter: Word }
               fn main(a: Word, b: Word) -> Word {
                   state.flag = a as Byte; (state.flag) as Word + b
               }";
    for args in [[200, 0], [255, 0], [128, 0], [127, 0], [0, 0], [1, 5]] {
        assert_shared_agrees(src, &args);
    }
}

#[test]
fn private_slots_agree_with_the_vm() {
    // Private storage uses a flat word array of this backend's own choosing,
    // which is legitimate because nothing outside a running program can observe
    // it. The VM keeps its own tagged representation and the two never meet;
    // the oracle compares RESULTS, which is exactly the boundary the argument
    // relies on.
    for src in [
        "private data h { n: Word }
         fn main(a: Word, b: Word) -> Word { h.n = a + b; h.n }",
        // Two slots, so an offset error moves a value between them.
        "private data h { p: Word, q: Word }
         fn main(a: Word, b: Word) -> Word { h.p = a; h.q = b; h.p - h.q }",
    ] {
        for args in [[7, 5], [0, 0], [-3, 9], [i64::MAX, 1], [i64::MIN, -1]] {
            assert_shared_agrees(src, &args);
        }
    }
}

#[test]
fn a_shared_array_indexes_contiguously() {
    // Shared indexed access was previously REFUSED on the ground that the
    // layout table does not state a slot range is contiguous. Measuring found
    // all 556,496 adjacent shared scalar pairs in the corpus contiguous with no
    // exceptions, which is a property of today's compiler rather than a wire
    // guarantee, so the lowering now PROVES contiguity per module instead of
    // assuming it or refusing outright.
    //
    // The buffer is compared as well as the value, so an element written at the
    // wrong stride is caught even when the returned answer happens to match.
    let src = "shared data s { xs: [Word; 4], tail: Word }
               fn main(i: Word, v: Word) -> Word { s.xs[i] = v; s.xs[0] + s.tail }";
    for args in [[0, 11], [1, 22], [2, 33], [3, 44]] {
        assert_shared_agrees(src, &args);
    }
}

#[test]
fn a_non_contiguous_shared_array_is_refused() {
    // **THE CONTIGUITY GUARD HAD NO POSITIVE CASE.** A mutation that disabled
    // it entirely left every test passing, because every layout the compiler
    // emits today is contiguous, so the guard defends against a layout that
    // does not occur. That makes it exactly the kind of check that is believed
    // rather than tested.
    //
    // Rewriting the layout table supplies the missing case, the same technique
    // used for `PushImmediate`'s unreachable integer encoding. If the compiler
    // ever emits a padded or reordered shared array, this is the behaviour that
    // must hold: refusal, not a read at a fabricated stride.
    let src = "shared data s { xs: [Word; 4] }
               fn main(i: Word, v: Word) -> Word { s.xs[i] = v; s.xs[0] }";
    let mut m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let dl = m.data_layout.as_mut().expect("shared layout");
    assert!(
        dl.shared_layout.len() >= 4,
        "this test is vacuous unless the array really occupies four shared slots"
    );
    // Push element 2 one byte past where a uniform stride predicts.
    dl.shared_layout[2].offset += 1;

    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    let err = lower_module(&ctx, &lm, &m, LowerOptions::default());
    assert!(
        err.is_err(),
        "a shared array whose elements are not contiguous must be refused; \
         lowering it would read the host buffer at a stride the layout denies"
    );

    // MUST-NOT-FIRE: the same module with its layout untouched must lower, or
    // the refusal above would be indistinguishable from indexed access being
    // broken outright.
    let clean = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let lm2 = ctx.create_module("kel2");
    assert!(
        lower_module(&ctx, &lm2, &clean, LowerOptions::default()).is_ok(),
        "an untouched contiguous layout must still lower"
    );
}

#[test]
fn a_module_with_both_kinds_keeps_them_apart() {
    // **ADDED BECAUSE A MUTATION FOUND THE GAP.** Deleting the shared-boundary
    // subtraction from the private index left every test passing, because every
    // other private test uses a module with NO shared slots, where the boundary
    // is zero and `slot - 0 == slot`. The subtraction was untested.
    //
    // Here the private slot is index 1 in the unified space and index 0 in the
    // private region, so an unsubtracted index addresses one word past the
    // intended slot. The shared buffer is compared as well, so a private write
    // that strayed into shared territory would also be caught.
    let src = "shared data s { visible: Word }
               private data h { hidden: Word }
               fn main(a: Word, b: Word) -> Word {
                   s.visible = a; h.hidden = b; s.visible + h.hidden
               }";
    for args in [[7, 5], [0, 0], [-1, 1], [i64::MAX, 0], [3, -9]] {
        assert_shared_agrees(src, &args);
    }
}

#[test]
fn a_private_array_indexes_by_consecutive_slots() {
    // A private array is NOT a composite. The compiler expands `[Word; 4]` into
    // four consecutive scalar slots, so an indexed access is `base + index` in
    // slot space. Reading back a different element than was written is what an
    // index or stride error looks like, so the test writes one element and
    // reads another.
    let src = "private data h { a: [Word; 4] }
               fn main(i: Word, v: Word) -> Word { h.a[i] = v; h.a[0] + h.a[3] }";
    for args in [[0, 11], [3, 22], [1, 33], [2, 44]] {
        assert_shared_agrees(src, &args);
    }
}

#[test]
fn an_out_of_range_private_index_faults_in_the_vm() {
    // The differential oracle cannot cover the failing case, since the VM
    // raises and native traps, so what is checkable is that the VM really does
    // fault and therefore that trapping is the right lowering.
    let src = "private data h { a: [Word; 4] }
               fn main(i: Word, v: Word) -> Word { h.a[i] = v; h.a[0] }";
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let n = shared_data_bytes_for(&m);
    let arena = arena_for(&m);
    let mut vm = Vm::new(m, &arena).expect("vm");
    let mut buf = vec![0u8; n];
    let err = vm
        .call_with_shared(&mut buf, &[Value::Int(4), Value::Int(1)])
        .err();
    assert!(err.is_some(), "index 4 into a 4-element array must fault");

    // MUST-NOT-FIRE: an in-range index must not fault.
    let m2 = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let arena2 = arena_for(&m2);
    let mut vm2 = Vm::new(m2, &arena2).expect("vm");
    let mut buf2 = vec![0u8; n];
    assert!(
        vm2.call_with_shared(&mut buf2, &[Value::Int(3), Value::Int(1)])
            .is_ok(),
        "index 3 into a 4-element array must not fault"
    );
}

// ---------------------------------------------------------------------------
// Float slots. The operator's Option A ruling settles the representation as
// IEEE-754 bytes at the stated offset, which is what the reference's
// `read_scalar_le` already does, so a slot access is an eight-byte load or store
// plus a `Float` tag on the read. See `docs/decisions/FLOAT_SHARED_SLOT_BRIEF.md`.
//
// **THE BUFFER COMPARISON IS WHY THESE TESTS ARE HERE RATHER THAN ELSEWHERE.**
// It pins the exact bit pattern, so the inputs chosen below are the ones a
// value comparison would not discriminate: the infinities, a negative zero and
// a NaN. A test over small positive values would prove almost nothing, which is
// the symmetry trap this package has fallen into before.
// ---------------------------------------------------------------------------

#[test]
fn a_float_shared_slot_agrees_in_value_and_in_buffer() {
    // Division supplies the interesting bit patterns from RUNTIME arguments, so
    // nothing is constant-folded away before it reaches the target's
    // instructions -- the trap that hid a poison `FloatToInt` for an increment.
    let src = "shared data s { x: Float, tail: Word }
               fn main(a: Word, b: Word) -> Word {
                   s.tail = b; s.x = (a as Float) / (b as Float); (s.x) as Word
               }";
    for args in [
        [7, 2],
        [-7, 2],
        [1, 0],  // +inf
        [-1, 0], // -inf
        [0, 0],  // NaN -- the buffer comparison pins its payload
        [0, -1], // negative zero
        [i64::MAX, 3],
        [i64::MIN, 1],
    ] {
        assert_shared_agrees(src, &args);
    }
}

#[test]
fn a_float_slot_does_not_disturb_its_neighbours() {
    // **THE WIDTH AND OFFSET CASE.** `flag` is one byte, `x` is eight, `tail` is
    // eight. `x` is written LAST and `tail` is returned, so a float store of the
    // wrong width or at the wrong offset overwrites a value that has already
    // been written and is about to be read back. Writing `x` first would let the
    // later `tail` store repair the damage and the test would pass on a broken
    // lowering.
    let src = "shared data s { flag: Byte, x: Float, tail: Word }
               fn main(a: Word, b: Word) -> Word {
                   s.flag = 255 as Byte; s.tail = b; s.x = a as Float; s.tail
               }";
    for args in [[1, 2], [-1, -1], [0, 0], [i64::MAX, i64::MIN]] {
        assert_shared_agrees(src, &args);
    }
}

#[test]
fn a_shared_float_array_indexes_contiguously() {
    // The contiguity proof in `resolve_shared_array` reads the element width
    // from the kind, so a float array is the case that would break if the float
    // element were sized as anything but eight bytes: the stride and the
    // declared offsets would disagree and the module would be REFUSED rather
    // than mispacked. That it lowers at all is therefore part of the evidence.
    let src = "shared data s { xs: [Float; 4], tail: Word }
               fn main(i: Word, v: Word) -> Word {
                   s.tail = v; s.xs[i] = v as Float; (s.xs[0]) as Word + s.tail
               }";
    for args in [[0, 11], [1, 22], [2, 33], [3, 44], [0, -5]] {
        assert_shared_agrees(src, &args);
    }
}

#[test]
fn an_untagged_float_from_a_private_slot_stores_correctly_into_a_shared_float_slot() {
    // **THE CASE THAT DECIDED AGAINST A WRITE-SIDE KIND CHECK.** A private
    // slot's read is not kind-tracked, so `h.f` reaches the store tagged `Int`
    // while carrying correct float bits. A guard comparing the operand's kind
    // against the slot's would refuse this program; nothing converts at the
    // store, so refusing it would prevent no wrong byte and lose a program the
    // reference runs. The buffer comparison is what makes "stores correctly" a
    // measurement rather than an assertion.
    let src = "private data h { f: Float }
               shared data s { x: Float }
               fn main(a: Word, b: Word) -> Word {
                   h.f = (a as Float) / (b as Float); s.x = h.f; b
               }";
    for args in [[7, 2], [1, 0], [0, 0], [-3, 4]] {
        assert_shared_agrees(src, &args);
    }
}

#[test]
fn a_bool_shared_slot_agrees_in_value_and_in_buffer() {
    // **THE LAST REACHABLE SHARED-SLOT KIND.** `Byte` slots are reached by the
    // corpus in their hundreds of thousands; `Bool` by nothing. The slot is one
    // byte, so the neighbouring word is what a wrong width would damage — hence
    // `n` is written first and read back, and the buffer is compared as well.
    //
    // Both branches are exercised, because a boolean that is always true passes
    // through a lowering that ignores the stored value entirely.
    let src = "shared data s { f: bool, n: Word }
               fn main(a: Word, b: Word) -> Word {
                   s.n = b; s.f = a > 0; if s.f { s.n } else { 0 - s.n }
               }";
    for args in [[1, 7], [-1, 7], [0, 5], [3, -3], [0, i64::MIN]] {
        assert_shared_agrees(src, &args);
    }
}

#[test]
fn a_fixed_shared_slot_agrees_in_value_and_in_buffer() {
    // **THE OPERATOR RULED THE SCALE OUT OF BAND** on 2026-08-31: the host is
    // expected to know the interpretation of the bits, as a C header lays out
    // the contract for a separately compiled procedure. So a `Fixed` slot is a
    // word of Q-format bits at the stated offset, and this pins that the backend
    // writes the same bytes the reference does.
    //
    // **THE BITS ARE THE VALUE.** A lowering that rescaled, masked or
    // sign-adjusted would return a plausible number, so the values below are
    // chosen to differ from their integer reading: `as Fixed<8>` shifts left by
    // eight, so 3 is stored as 768.
    let src = "shared data s { q: Fixed<8>, n: Word }
               fn main(a: Word, b: Word) -> Word {
                   s.n = b; s.q = a as Fixed<8>; ((s.q) as Word) + s.n
               }";
    for args in [
        [3, 5],
        [-3, 5],
        [0, 0],
        [1, -1],
        [12345, 7],
        [i64::MIN / 512, 1],
    ] {
        assert_shared_agrees(src, &args);
    }
}

#[test]
fn a_fixed_shared_array_indexes_contiguously() {
    // The stride case, and the one the example policy actually uses: a fixed
    // array of per-zone values written under a bounded loop.
    let src = "shared data s { qs: [Fixed<8>; 3], n: Word }
               fn main(a: Word, b: Word) -> Word {
                   s.n = b; s.qs[a] = b as Fixed<8>; ((s.qs[0]) as Word) + s.n
               }";
    for args in [[0, 11], [1, 22], [2, 33], [0, -7]] {
        assert_shared_agrees(src, &args);
    }
}

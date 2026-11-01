//! **CHECKED ARITHMETIC ON `Byte` RETURNED AN UNTRUNCATED RESULT.**
//!
//! ```text
//! fn main(a: Byte, b: Byte) -> Byte { a * b { ok(v) => v, overflow(w) => w } }
//!
//!   200 * 100 : reference Byte(32),  this backend 20000
//!   255 * 255 : reference Byte(1),   this backend 65025
//!   200 + 100 : reference Byte(44),  this backend 300
//! ```
//!
//! # The runtime's `Byte` arm is not its integer arm at a narrower type
//!
//! Read from `src/vm.rs`: the result is computed wide, the **low eight bits**
//! become the low slot, the middle slot is **unused and zero** where the integer
//! arm puts a high half, and the flag is `1` above `0xFF` and **never 2** —
//! unsigned byte addition and multiplication cannot underflow.
//!
//! **The flag is the half that is easy to miss.** Truncating the value while
//! classifying against the 64-bit range would leave every byte overflow reporting
//! flag 0: a wrong value replaced by a wrong value with a wrong arm.
//!
//! # How it went unseen
//!
//! `backend_support_census.rs` reported **0 refusals** while probing
//! `CheckedMul` with `Word` operands. **It is keyed by opcode NAME, and both
//! support and semantics are decided by the OPERAND.** The checked fixed-point
//! multiply and divide sat outside it for the same reason; this was found by
//! asking what else that blind spot hides.
//!
//! # Scope, measured rather than inferred from the two that were found
//!
//! | operation on `Byte` | status |
//! |---|---|
//! | `*` | diverged; fixed |
//! | `+` | diverged; fixed |
//! | `-` | **no subject** — the reference rejects an `overflow` arm as an outcome that cannot arise |
//! | `/`, `%` | agree already: a byte quotient or remainder cannot leave the byte range |
//!
//! # The residual, stated rather than implied away
//!
//! The byte arm is taken when **both operand widths are known to be one byte**.
//! An operand whose width the backend could not reconstruct keeps the integer
//! path — which is what every previously lowering program relies on, and which
//! would still be wrong for a byte. No subject here produces that shape; it is
//! recorded as a known limit rather than claimed closed.

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::Value;
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
use keleusma_native::{LowerOptions, lower_module, module_refusals};

mod common;

fn vm_byte(src: &str, a: u8, b: u8) -> i64 {
    let m = common::build(src);
    let need = required_persistent_capacity_for(&m);
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena") + need + (1 << 20);
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent");
    let mut vm = Vm::new(m, &arena).expect("vm");
    match vm.call(&[Value::Byte(a), Value::Byte(b)]).expect("vm run") {
        VmState::Finished(Value::Byte(v)) => i64::from(v),
        VmState::Finished(Value::Int(v)) => v,
        other => panic!("unexpected outcome {other:?}"),
    }
}

fn native_byte(src: &str, a: u8, b: u8) -> i64 {
    let m = common::build(src);
    let entry = m.entry_point.expect("entry point");
    let ctx = Context::create();
    let lm = ctx.create_module("k");
    lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("lower");
    common::maybe_optimize(&lm);
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");
    let f = unsafe {
        ee.get_function::<unsafe extern "C" fn(i64, i64) -> i64>(&format!("kel_chunk_{entry}"))
    }
    .expect("entry symbol");
    unsafe { f.call(i64::from(a), i64::from(b)) }
}

const MUL: &str = "fn main(a: Byte, b: Byte) -> Byte { a * b { ok(v) => v, overflow(w) => w } }";
const ADD: &str = "fn main(a: Byte, b: Byte) -> Byte { a + b { ok(v) => v, overflow(w) => w } }";

#[test]
fn checked_byte_multiply_and_add_agree_inside_and_outside_the_byte_range() {
    for (src, name) in [(MUL, "multiply"), (ADD, "add")] {
        assert!(
            module_refusals(&common::build(src), LowerOptions::default()).is_empty(),
            "{name} must lower"
        );
        for (a, b) in [(3u8, 4u8), (200, 100), (255, 255), (16, 16)] {
            let vm = vm_byte(src, a, b);
            let nat = native_byte(src, a, b);
            assert_eq!(nat, vm, "{name} {a},{b}: native={nat} vm={vm}");
            assert!(
                (0..=0xFF).contains(&vm),
                "{name} {a},{b}: the reference produced {vm}, outside a byte — this \
                 subject is no longer about byte arithmetic"
            );
        }
    }
}

/// **THE ARM IS SELECTED, WHICH VALUE AGREEMENT CANNOT SHOW.**
///
/// Both arms of the subjects above return the wrapped value, so they agree even
/// if the flag is always zero. These return DIFFERENT constants, so the arm taken
/// is the observable.
#[test]
fn the_byte_overflow_arm_is_actually_selected() {
    const MUL_ARMS: &str =
        "fn main(a: Byte, b: Byte) -> Word { a * b { ok(v) => 1, overflow(w) => 2 } }";
    const ADD_ARMS: &str =
        "fn main(a: Byte, b: Byte) -> Word { a + b { ok(v) => 1, overflow(w) => 2 } }";

    for (src, name, inside, outside) in [
        (MUL_ARMS, "multiply", (3u8, 4u8), (200u8, 100u8)),
        (ADD_ARMS, "add", (3, 4), (200, 100)),
    ] {
        assert_eq!(
            vm_byte(src, inside.0, inside.1),
            1,
            "{name}: in range is the ok arm"
        );
        assert_eq!(
            vm_byte(src, outside.0, outside.1),
            2,
            "{name}: out of range is the overflow arm — if the reference stops \
             flagging, this subject proves nothing"
        );
        assert_eq!(
            native_byte(src, inside.0, inside.1),
            1,
            "{name}: native took the wrong arm in range"
        );
        assert_eq!(
            native_byte(src, outside.0, outside.1),
            2,
            "{name}: native took the OK arm for an out-of-range byte result. The \
             integer classifier asks whether the result left the 64-bit range, \
             which a byte product never does."
        );
    }
}

/// `Byte` division and modulo were measured as ALREADY agreeing, and are pinned
/// so the scope statement above is checkable rather than recalled.
#[test]
fn byte_divide_and_modulo_were_already_correct() {
    for (src, name) in [
        (
            "fn main(a: Byte, b: Byte) -> Byte { a / b { ok(v) => v, zero_divisor(n) => n } }",
            "divide",
        ),
        (
            "fn main(a: Byte, b: Byte) -> Byte { a % b { ok(v) => v, zero_divisor(n) => n } }",
            "modulo",
        ),
    ] {
        for (a, b) in [(200u8, 3u8), (200, 0), (7, 2)] {
            let vm = vm_byte(src, a, b);
            assert_eq!(native_byte(src, a, b), vm, "{name} {a},{b}");
        }
    }
}

/// **`Byte` subtraction has no overflow arm**, which is why it is absent above.
/// Recorded as a property of the reference rather than as an untested gap.
#[test]
fn byte_subtraction_admits_no_overflow_arm_on_the_reference() {
    let rejected = common::try_build(
        "fn main(a: Byte, b: Byte) -> Byte { a - b { ok(v) => v, overflow(w) => w } }",
    );
    assert!(
        rejected.is_none(),
        "the reference now admits an overflow arm for Byte subtraction. That is a \
         new shape this backend has never been driven on — add it to the scope \
         table above rather than assuming the byte arm covers it."
    );
}

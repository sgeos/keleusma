//! **DOES EACH OPERATION AGREE FOR EVERY OPERAND TYPE IT ACCEPTS?**
//!
//! # Why, and the evidence that this shape of question matters
//!
//! Four defects in two increments hid behind one instrument's shape.
//! `backend_support_census.rs` is keyed by opcode NAME while support and
//! semantics are decided by the OPERAND, and every `Checked*` row probed `Word`.
//! It reported **0 refused** while the `Fixed` variants of `CheckedMul` and
//! `CheckedDiv` were refused outright and the `Byte` variants of `CheckedMul` and
//! `CheckedAdd` returned untruncated values.
//!
//! That census also says what it does not do: *"a pass here does NOT mean the
//! emitted code is correct."* **Support and agreement are different questions**,
//! and nothing asked the second one per operand variant.
//!
//! # What this does differently
//!
//! It EXECUTES both sides. A lowering that merely succeeds is not a pass here —
//! that is precisely the reading which let two silently wrong lowerings sit in a
//! supported column.
//!
//! # Values are chosen to leave the operand's natural range
//!
//! `3 * 4` agrees under every wrong lowering that was just repaired. Each case
//! carries at least one input that overflows a byte, or exceeds a word after
//! scaling, so a case cannot pass on arithmetic too small to discriminate.
//!
//! # ⚠ WHAT THIS CANNOT DO
//!
//! **It covers the variants it was GIVEN, not all that exist.** The matrix is a
//! hand-written list, which is the same limitation the support census now states
//! about itself. A clean run here is evidence about these combinations and not a
//! proof that no variant diverges.
//!
//! Shapes the reference rejects are recorded as rejected rather than skipped, so
//! a gap in the matrix is visible instead of silent.

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::Value;
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
use keleusma_native::{LowerOptions, lower_module, module_refusals};

mod common;

/// One driven comparison: source, the argument values, and a label.
struct Case {
    label: &'static str,
    src: &'static str,
    args: Vec<Value>,
}

fn raw(v: &Value) -> i64 {
    match v {
        Value::Byte(x) => i64::from(*x),
        Value::Int(x) | Value::Fixed(x) => *x,
        other => panic!("this sweep drives scalars only, got {other:?}"),
    }
}

fn scalar(v: &Value) -> i64 {
    raw(v)
}

fn run_both(c: &Case) -> (i64, i64) {
    let m = common::build(c.src);
    let need = required_persistent_capacity_for(&m);
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena") + need + (1 << 20);
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent");
    let mut vm = Vm::new(m.clone(), &arena).expect("vm");
    let vm_out = match vm.call(&c.args).expect("vm run") {
        VmState::Finished(v) => scalar(&v),
        other => panic!("{}: unexpected outcome {other:?}", c.label),
    };

    let entry = m.entry_point.expect("entry point");
    let ctx = Context::create();
    let lm = ctx.create_module("k");
    lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("lower");
    common::maybe_optimize(&lm);
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");
    let sym = format!("kel_chunk_{entry}");
    let nat = match c.args.len() {
        1 => {
            let f =
                unsafe { ee.get_function::<unsafe extern "C" fn(i64) -> i64>(&sym) }.expect("s");
            unsafe { f.call(raw(&c.args[0])) }
        }
        2 => {
            let f = unsafe { ee.get_function::<unsafe extern "C" fn(i64, i64) -> i64>(&sym) }
                .expect("s");
            unsafe { f.call(raw(&c.args[0]), raw(&c.args[1])) }
        }
        n => panic!(
            "{}: this sweep drives one or two arguments, not {n}",
            c.label
        ),
    };
    (vm_out, nat)
}

fn cases() -> Vec<Case> {
    let b = Value::Byte;
    let w = Value::Int;
    let fx = Value::Fixed;
    vec![
        // `Byte`, unchecked. Each pair overflows or saturates the byte.
        Case {
            label: "byte add",
            src: "fn main(a: Byte, b: Byte) -> Byte { a + b }",
            args: vec![b(200), b(100)],
        },
        Case {
            label: "byte sub",
            src: "fn main(a: Byte, b: Byte) -> Byte { a - b }",
            args: vec![b(3), b(200)],
        },
        Case {
            label: "byte mul",
            src: "fn main(a: Byte, b: Byte) -> Byte { a * b }",
            args: vec![b(200), b(100)],
        },
        Case {
            label: "byte band",
            src: "fn main(a: Byte, b: Byte) -> Byte { a band b }",
            args: vec![b(200), b(100)],
        },
        Case {
            label: "byte bor",
            src: "fn main(a: Byte, b: Byte) -> Byte { a bor b }",
            args: vec![b(200), b(100)],
        },
        Case {
            label: "byte bxor",
            src: "fn main(a: Byte, b: Byte) -> Byte { a bxor b }",
            args: vec![b(200), b(100)],
        },
        Case {
            label: "byte bnot",
            src: "fn main(a: Byte) -> Byte { bnot a }",
            args: vec![b(200)],
        },
        // `Byte` shifts, whose amount is a `Word`. A left shift of a large byte
        // is the case that needs the promote-operate-truncate masking.
        Case {
            label: "byte lsl",
            src: "fn main(a: Byte, n: Word) -> Byte { a lsl n }",
            args: vec![b(200), w(2)],
        },
        Case {
            label: "byte lsr",
            src: "fn main(a: Byte, n: Word) -> Byte { a lsr n }",
            args: vec![b(200), w(2)],
        },
        Case {
            label: "byte asr",
            src: "fn main(a: Byte, n: Word) -> Byte { a asr n }",
            args: vec![b(200), w(2)],
        },
        // `Byte`, checked. These are the two that were wrong.
        Case {
            label: "byte checked add",
            src: "fn main(a: Byte, b: Byte) -> Byte { a + b { ok(v) => v, overflow(w) => w } }",
            args: vec![b(200), b(100)],
        },
        Case {
            label: "byte checked mul",
            src: "fn main(a: Byte, b: Byte) -> Byte { a * b { ok(v) => v, overflow(w) => w } }",
            args: vec![b(200), b(100)],
        },
        Case {
            label: "byte checked div",
            src: "fn main(a: Byte, b: Byte) -> Byte { a / b { ok(v) => v, zero_divisor(n) => n } }",
            args: vec![b(200), b(0)],
        },
        Case {
            label: "byte checked mod",
            src: "fn main(a: Byte, b: Byte) -> Byte { a % b { ok(v) => v, zero_divisor(n) => n } }",
            args: vec![b(200), b(0)],
        },
        // `Fixed`. The bare forms saturate; the checked ones wrap.
        Case {
            label: "fixed add",
            src: "fn main(a: Fixed<16>, b: Fixed<16>) -> Fixed<16> { a + b }",
            args: vec![fx(1 << 40), fx(1 << 40)],
        },
        Case {
            label: "fixed neg",
            src: "fn main(a: Fixed<16>) -> Fixed<16> { 0 - a }",
            args: vec![fx(65536)],
        },
        Case {
            label: "fixed mul",
            src: "fn main(a: Fixed<16>, b: Fixed<16>) -> Fixed<16> { a * b }",
            args: vec![fx(1 << 40), fx(1 << 40)],
        },
        Case {
            label: "fixed div",
            src: "fn main(a: Fixed<16>, b: Fixed<16>) -> Fixed<16> { a / b }",
            args: vec![fx(1 << 40), fx(1 << 8)],
        },
        Case {
            label: "fixed checked mul",
            src: "fn main(a: Fixed<16>, b: Fixed<16>) -> Fixed<16> { a * b { ok(v) => v, overflow(w) => w } }",
            args: vec![fx(1 << 40), fx(1 << 40)],
        },
        Case {
            label: "fixed checked div",
            src: "fn main(a: Fixed<16>, b: Fixed<16>) -> Fixed<16> { a / b { ok(v) => v, overflow(w) => w, zero_divisor(n) => n } }",
            args: vec![fx(1 << 40), fx(0)],
        },
        // `Word`, as the control: the variant every `Checked*` row already probed.
        //
        // ⚠ **THE ARM'S ARITY VARIES BY OPERAND TYPE TOO.** A `Word` overflow arm
        // binds the high AND low halves — `overflow(h, l)` — where `Byte` and
        // `Fixed` bind one value, because their middle slot is unused. The
        // reference says so in a type error, and it is one more way the OPERAND
        // decides the shape rather than the opcode.
        Case {
            label: "word checked mul",
            src: "fn main(a: Word, b: Word) -> Word { a * b { ok(v) => v, overflow(h, l) => l } }",
            args: vec![w(i64::MAX), w(2)],
        },
    ]
}

/// Shapes the reference rejects, recorded so their absence from the matrix is
/// visible rather than silent.
const REFERENCE_REJECTS: &[(&str, &str)] = &[
    (
        "byte checked sub",
        "fn main(a: Byte, b: Byte) -> Byte { a - b { ok(v) => v, overflow(w) => w } }",
    ),
    (
        "byte comparison to Bool",
        "fn main(a: Byte, b: Byte) -> Bool { a < b }",
    ),
];

/// Cases at the stamp.
const RECORDED_CASES: usize = 21;

#[test]
fn every_operand_variant_agrees_with_the_reference() {
    let cases = cases();
    assert_eq!(
        cases.len(),
        RECORDED_CASES,
        "the variant matrix changed. Place the new variant in the table and give it \
         an input that leaves the operand's natural range, rather than editing this \
         number: four defects hid behind a census that probed one operand type per \
         opcode."
    );

    println!("\n================ OPERAND-VARIANT AGREEMENT");
    for c in &cases {
        let (vm, nat) = run_both(c);
        println!("  {:22} vm={vm:<22} native={nat}", c.label);
        assert_eq!(
            nat, vm,
            "{}: native={nat} vm={vm}. The operand type decides the runtime's arm; \
             the backend must take the same one.",
            c.label
        );
    }
    println!("  ------------------------------------------------");
    println!("  cases driven: {}", cases.len());
    println!(
        "\n  BOTH SIDES EXECUTED. A lowering that merely succeeds is not a pass\n  \
         here — that reading is what let two silently wrong lowerings sit in a\n  \
         supported column.\n================\n"
    );
}

/// **The rejected shapes are rejected**, so a gap in the matrix above is a fact
/// about the reference rather than an untested combination.
#[test]
fn the_shapes_absent_from_the_matrix_are_rejected_by_the_reference() {
    for (label, src) in REFERENCE_REJECTS {
        assert!(
            common::try_build(src).is_none(),
            "{label} now COMPILES on the reference. It is absent from the variant \
             matrix because it did not; add it there rather than leaving a shape \
             this backend has never been driven on."
        );
    }
}

/// **Non-vacuity**: every case must actually lower, or the sweep is comparing
/// nothing and reporting agreement.
#[test]
fn every_case_lowers_before_it_is_compared() {
    for c in cases() {
        let refusals = module_refusals(&common::build(c.src), LowerOptions::default());
        assert!(
            refusals.is_empty(),
            "{}: refused, so the agreement assertion would never run: {refusals:?}",
            c.label
        );
    }
}

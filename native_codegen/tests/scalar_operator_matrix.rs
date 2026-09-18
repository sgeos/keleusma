//! **THE OPERAND-TYPE AXIS, WITH A DENOMINATOR — AND THE CELL WHERE THE
//! REFERENCE TRAPS AND THIS BACKEND USED TO ANSWER.**
//!
//! # Two variant axes, and this is the second
//!
//! `opcode_denominator.rs` closed the first: an opcode's own operand FIELD, where
//! `NewComposite` carries eight variants under one name. **This is the other
//! axis: the runtime TYPE of the operands, which has no field in the bytecode at
//! all.** `Op::Add`, `Op::Mod` and `Op::Shl` carry nothing; the virtual machine
//! dispatches on the values it pops. That axis produced the three defects this
//! line has already fixed.
//!
//! `operand_variant_sweep.rs` covers it with 27 hand-written cases and no
//! denominator — the shape the opcode census had before it was given one. This
//! file enumerates the cells from the type list and the operator list instead.
//!
//! # ⚠ THE TYPE LIST WAS THREE OF FOUR SCALAR TYPES UNTIL 2026-09-18
//!
//! `Float` was absent while this file described itself as enumerating *"every
//! cell"*. **That omission cost a real defect**: `Op::Neg` on a `Float` emitted
//! invalid intermediate representation under `narrow-float-32`, so float negation
//! was broken outright in a configuration the gate runs, and it was found by a
//! composite-field probe rather than here. All four scalar types are enumerated
//! now, and the driver derives its call signature from the lowered function
//! instead of assuming an `i64` one.
//!
//! # What the denominator found: `Fixed % Fixed`
//!
//! - The reference **compiler accepts it** and emits a plain `Op::Mod`.
//! - The reference **virtual machine traps**:
//!   `TypeError("cannot modulo Fixed by Fixed")`. Its `Op::Mod` arm handles
//!   `Int`/`Int`, `Byte`/`Byte` and `Float`/`Float`; `Fixed` falls to the
//!   catch-all.
//! - This backend **lowered it and returned a value** — `4.0` for `200.0 % 7.0`,
//!   arithmetically right and **not what the reference does**.
//!
//! Now refused in the emitter. `Op::Div` is refused on a `Fixed` operand for the
//! same reason, its virtual-machine arm being equally Fixed-less.
//!
//! # ⚠ WHY THE OLD SWEEP COULD NOT HAVE FOUND THIS
//!
//! `run_both` panics on any virtual-machine outcome that is not `Finished`. **A
//! trapping cell cannot be represented in that matrix at all** — adding one would
//! panic the harness rather than record a result.
//!
//! The previous defect hid behind an instrument's KEYING. This one hid behind an
//! instrument's **outcome type**: the harness could say *agree* and *disagree*,
//! but had no way to say *the reference refuses*. **An instrument can only find
//! defects it has a vocabulary for**, so [`Cell`] below carries the outcomes
//! this backend can actually produce, including the two that are defects.
//!
//! # The shift rows say which type was refused
//!
//! A `Byte` value shifts by a `Word` amount or a literal; a `Byte` AMOUNT is
//! refused. Recording that as "byte shifts are refused" would be the same
//! conflation that once filed an unknown type name as a fact about comparisons,
//! so each shift is driven in all three amount forms and they are recorded
//! separately. `Fixed` is refused by the shift operators in every form.

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::execution_engine::ExecutionEngine;
use inkwell::values::FunctionValue;
use keleusma::bytecode::Value;
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
use keleusma_native::{LowerOptions, lower_module, module_refusals};

mod common;

/// What a cell's two implementations did.
///
/// **The last three exist because the old sweep could not express them.**
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
enum Cell {
    /// Both produced the same value.
    Agree,
    /// The reference compiler refused the program. Nothing is claimed about the
    /// backend, which never saw a module.
    RefRejects,
    /// The reference traps at run time and the backend refuses to lower. Sound:
    /// neither produces a value.
    VmTrapsNativeRefuses,
    /// The reference traps at run time and the backend **returns a value**. A
    /// DEFECT: native code answers where the reference errors.
    VmTrapsNativeComputes,
    /// Both produced a value and the values differ. A DEFECT.
    Disagree,
    /// The backend refused a program the reference runs to completion. Not a
    /// correctness defect, but a coverage loss worth seeing.
    NativeRefusesOnly,
}

/// The float width THIS BACKEND lowers to. `f64` by default, `f32` under
/// `narrow-float-32`.
#[cfg(feature = "narrow-float-32")]
type Flt = f32;
#[cfg(not(feature = "narrow-float-32"))]
type Flt = f64;

/// Widen this configuration's float to `f64`.
///
/// **Split by configuration rather than written once**, because `f64::from` is
/// required at four bytes and is a `useless_conversion` lint error at eight.
#[cfg(feature = "narrow-float-32")]
fn wide(x: Flt) -> f64 {
    f64::from(x)
}
#[cfg(not(feature = "narrow-float-32"))]
fn wide(x: Flt) -> f64 {
    x
}

/// Narrow a host `f64` to the width this backend lowers to.
#[cfg(feature = "narrow-float-32")]
fn narrow(x: f64) -> Flt {
    x as f32
}
#[cfg(not(feature = "narrow-float-32"))]
fn narrow(x: f64) -> Flt {
    x
}

/// Does this value survive a round trip through four bytes unchanged?
///
/// # ⚠ WHY EVERY FLOAT HERE MUST SATISFY THIS
///
/// `keleusma::vm::Vm` is `GenericVm<.., f64>` — **the reference is instantiated at
/// EIGHT bytes in BOTH float configurations**, while this backend lowers `Float`
/// at the configured width. Under `narrow-float-32` the comparison below is
/// therefore f64-against-f32, and any value needing more than a four-byte mantissa
/// would differ **legitimately**, reporting ordinary rounding as a divergence.
///
/// That would be a false finding about the emitter, and this line has already
/// recorded the shape: a report that is really a statement about the harness.
///
/// **The check is unconditional, not gated on the narrow configuration**, so a
/// subject that violates it fails in either configuration rather than only in the
/// one that happens to expose it. A tolerance would be the wrong fix: the
/// correctness signal on this line is an exact differential, and an epsilon here
/// would weaken the property the package rests on to paper over a harness
/// artefact.
fn is_exact_in_four_bytes(x: f64) -> bool {
    f64::from(x as f32) == x
}

/// A driven result, kept in the shape the implementation produced.
///
/// **Not flattened to `i64`.** Reinterpreting a float's bits as an integer is how
/// a probe in this package has already produced a wrong number, and `raw` below
/// still refuses to do it.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Out {
    Int(i64),
    Flt(f64),
}

impl Out {
    /// Fail loudly on a float this matrix must not be comparing.
    fn checked(self, side: &str, src: &str) -> Self {
        if let Out::Flt(v) = self {
            assert!(
                is_exact_in_four_bytes(v),
                "the {side} produced {v} for `{src}`, which is not exact in four \
                 bytes. The reference runs at f64 in both configurations and this \
                 backend lowers at the configured width, so comparing this value \
                 would report rounding as divergence. Choose operands whose every \
                 result is four-byte exact; do not add a tolerance."
            );
        }
        self
    }
}

fn out_of(v: &Value) -> Out {
    match v {
        Value::Float(x) => Out::Flt(*x),
        other => Out::Int(raw(other)),
    }
}

/// This configuration's float, from a `Value::Float`.
fn flt(v: &Value) -> Flt {
    match v {
        Value::Float(x) => narrow(*x),
        other => panic!(
            "the lowered signature says this parameter is a float, but the driver \
             was handed {other:?}. Calling through a mismatched signature is \
             undefined behaviour."
        ),
    }
}

/// Call the lowered entry point through a signature read from the IR.
///
/// # ⚠ THE SIGNATURE IS DERIVED, NEVER GUESSED
///
/// A float-in, float-out chunk lowers with a FLOATING-POINT parameter and return,
/// not an `i64`. Calling through the wrong shape is undefined behaviour that
/// surfaces as a SIGBUS inside JIT code with no usable stack, **which this package
/// has already paid for once**. The parameter and return kinds are read off the
/// emitted function; a shape not anticipated here panics rather than reaching a
/// call.
fn call_native(ee: &ExecutionEngine, f: FunctionValue, sym: &str, args: &[Value]) -> Out {
    let float_params: Vec<bool> = (0..f.count_params())
        .map(|i| f.get_nth_param(i).expect("a parameter").is_float_value())
        .collect();
    let float_ret = f
        .get_type()
        .get_return_type()
        .is_some_and(|t| t.is_float_type());
    assert_eq!(
        float_params.len(),
        args.len(),
        "the lowered `{sym}` takes {} parameters and the driver has {} arguments",
        float_params.len(),
        args.len()
    );

    macro_rules! jit {
        ($t:ty) => {
            unsafe { ee.get_function::<$t>(sym) }.expect("entry symbol")
        };
    }

    match (float_params.as_slice(), float_ret) {
        ([false], false) => {
            let g = jit!(unsafe extern "C" fn(i64) -> i64);
            Out::Int(unsafe { g.call(raw(&args[0])) })
        }
        ([false, false], false) => {
            let g = jit!(unsafe extern "C" fn(i64, i64) -> i64);
            Out::Int(unsafe { g.call(raw(&args[0]), raw(&args[1])) })
        }
        ([true], true) => {
            let g = jit!(unsafe extern "C" fn(Flt) -> Flt);
            Out::Flt(wide(unsafe { g.call(flt(&args[0])) }))
        }
        ([true], false) => {
            let g = jit!(unsafe extern "C" fn(Flt) -> i64);
            Out::Int(unsafe { g.call(flt(&args[0])) })
        }
        ([true, true], true) => {
            let g = jit!(unsafe extern "C" fn(Flt, Flt) -> Flt);
            Out::Flt(wide(unsafe { g.call(flt(&args[0]), flt(&args[1])) }))
        }
        ([true, true], false) => {
            let g = jit!(unsafe extern "C" fn(Flt, Flt) -> i64);
            Out::Int(unsafe { g.call(flt(&args[0]), flt(&args[1])) })
        }
        ([true, false], true) => {
            let g = jit!(unsafe extern "C" fn(Flt, i64) -> Flt);
            Out::Flt(wide(unsafe { g.call(flt(&args[0]), raw(&args[1])) }))
        }
        (shape, ret) => panic!(
            "`{sym}` lowers with float-parameter mask {shape:?} and float return \
             {ret}, a shape this driver does not name. Add it deliberately -- \
             calling through a guessed signature is undefined behaviour."
        ),
    }
}

fn raw(v: &Value) -> i64 {
    match v {
        Value::Byte(x) => i64::from(*x),
        Value::Int(x) | Value::Fixed(x) => *x,
        Value::Bool(x) => i64::from(*x),
        // **`Value::Float` DELIBERATELY FALLS HERE.** A float reaches the
        // driver through `flt`, never through this function. Reinterpreting a
        // double's bit pattern as an integer is how a probe in this package has
        // already produced a wrong number, so the integer path refuses floats
        // rather than transmuting them.
        other => panic!(
            "the integer path was handed {other:?}; a float must reach the \
             lowered function through `flt`, not as a bit pattern"
        ),
    }
}

fn drive(src: &str, args: &[Value]) -> Cell {
    // **THE OPERANDS ARE CHECKED, NOT ONLY THE RESULTS — AND A PERTURBATION IS WHY.**
    //
    // `Out::checked` watches what each implementation PRODUCES. That is not
    // enough, and the first version of this file shipped with only that. Driving
    // `0.1` as an operand under `narrow-float-32` produced a phantom
    // `float %: Disagree` **before** any result check fired: `narrow(0.1) != 0.1`,
    // so the backend was computing a different problem from the reference, and
    // for `%` both sides happened to land on four-byte-exact results that
    // nevertheless differ.
    //
    // **A guard watching outputs cannot see a violated input.** An inexact operand
    // is the actual precondition, so it is asserted here, before either
    // implementation runs.
    for a in args {
        if let Value::Float(x) = a {
            assert!(
                is_exact_in_four_bytes(*x),
                "`{src}` is driven with the operand {x}, which is not exact in \
                 four bytes. Under `narrow-float-32` the backend would receive a \
                 different value from the one the reference receives, and any \
                 difference in the results would say nothing about the emitter."
            );
        }
    }
    let Some(m) = common::try_build(src) else {
        return Cell::RefRejects;
    };
    let need = required_persistent_capacity_for(&m);
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena") + need + (1 << 20);
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent");
    let mut vm = Vm::new(m.clone(), &arena).expect("vm");
    // **A non-`Finished` outcome is recorded, not panicked on.** This is the
    // whole reason the file exists.
    let vm_out: Option<Out> = match vm.call(args) {
        Ok(VmState::Finished(v)) => Some(out_of(&v).checked("reference", src)),
        _ => None,
    };
    if !module_refusals(&m, LowerOptions::default()).is_empty() {
        return if vm_out.is_none() {
            Cell::VmTrapsNativeRefuses
        } else {
            Cell::NativeRefusesOnly
        };
    }
    let entry = m.entry_point.expect("entry point");
    let ctx = Context::create();
    let lm = ctx.create_module("k");
    // **INVALID IR IS THE CLASS THAT BROKE FLOAT NEGATION, AND THIS LINE IS WHERE
    // IT SURFACES.** `lower_module` verifies the module as a postcondition and
    // returns `LowerError::InvalidIr`, so the `expect` below is the assertion; a
    // separate `lm.verify()` here was written and then REMOVED, because it sits
    // after a call that cannot return `Ok` with invalid IR and so could never
    // fire. Measured, not assumed: reinstating the pre-fix bitcast in `Op::Neg`
    // fails this line under `narrow-float-32` with
    // `InvalidIr("Invalid bitcast .. float .. to i64")`, and correctly does NOT
    // fail under the default configuration, where that bitcast is well formed.
    lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("lower");
    common::maybe_optimize(&lm);
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");
    let sym = format!("kel_chunk_{entry}");
    let f = lm.get_function(&sym).expect("entry function");
    let nat = call_native(&ee, f, &sym, args).checked("backend", src);
    match vm_out {
        None => Cell::VmTrapsNativeComputes,
        Some(v) if v == nat => Cell::Agree,
        Some(_) => Cell::Disagree,
    }
}

/// Binary operators that take a same-typed pair.
const PAIR_OPS: &[&str] = &["+", "-", "*", "/", "%", "band", "bor", "bxor"];
/// Shift operators, driven in three amount forms.
const SHIFT_OPS: &[&str] = &["lsl", "asl", "lsr", "asr"];
/// Comparisons, which return `bool` rather than the operand type.
const CMP_OPS: &[&str] = &["<", ">", "<=", ">=", "==", "!="];

/// Every cell, enumerated from the types and the operators.
fn observe() -> Vec<(String, Cell)> {
    let b = Value::Byte;
    let w = Value::Int;
    let fx = Value::Fixed;
    let fl = Value::Float;
    // **`Float` JOINED THIS LIST ON 2026-09-18, AND IT SHOULD HAVE BEEN HERE ALL
    // ALONG.**
    //
    // This function is documented as enumerating *"every cell, from the types and
    // the operators"* and it enumerated three of the four scalar types. On
    // 2026-09-17 `Op::Neg` on a `Float` was found emitting INVALID IR under
    // `narrow-float-32` — float negation broken outright in a configuration the
    // gate runs — and **no cell here could have seen it**. It was found by a
    // composite-field probe instead.
    //
    // **The operands are `14.0` and `4.0` for a reason that is not arbitrary.**
    // Every result they produce — 18, 10, 56, 3.5, 2.0 and the six comparisons —
    // is exact in four bytes. See [`is_exact_in_four_bytes`]: the reference runs
    // at f64 in BOTH configurations while this backend lowers at the configured
    // width, so an f32-inexact value would report rounding as a divergence.
    //
    // **Two checks enforce that, and one of them was missing until a perturbation
    // showed it**: `drive` asserts the OPERANDS before either implementation
    // runs, and `Out::checked` asserts each RESULT. A result-only check let a
    // phantom `float %: Disagree` through.
    let types: &[(&str, &str, Value, Value)] = &[
        ("byte", "Byte", b(200), b(7)),
        ("word", "Word", w(200), w(7)),
        ("fixed", "Fixed", fx(200 << 16), fx(7 << 16)),
        ("float", "Float", fl(14.0), fl(4.0)),
    ];
    let mut out = Vec::new();
    for (tn, ty, a, rhs) in types {
        for op in PAIR_OPS {
            out.push((
                format!("{tn} {op}"),
                drive(
                    &format!("fn main(a: {ty}, b: {ty}) -> {ty} {{ a {op} b }}"),
                    &[a.clone(), rhs.clone()],
                ),
            ));
        }
        for op in SHIFT_OPS {
            // Three amount forms, recorded apart, so a refusal names the type it
            // is really about.
            out.push((
                format!("{tn} {op} [Word amt]"),
                drive(
                    &format!("fn main(a: {ty}, b: Word) -> {ty} {{ a {op} b }}"),
                    &[a.clone(), w(3)],
                ),
            ));
            // **EXHAUSTIVE, NOT A CATCH-ALL.** The `_ => fx(..)` arm this
            // replaces would have handed a `Fixed` value to a `Float`
            // parameter the moment `float` joined the list, and the reference
            // would have refused it for the wrong reason — a refusal
            // describing the harness and reading like a fact about the
            // program, which is a shape this package has recorded before.
            let same = match *tn {
                "byte" => b(3),
                "word" => w(3),
                "fixed" => fx(3 << 16),
                "float" => fl(3.0),
                other => panic!("no same-type shift amount is defined for `{other}`"),
            };
            out.push((
                format!("{tn} {op} [same-type amt]"),
                drive(
                    &format!("fn main(a: {ty}, b: {ty}) -> {ty} {{ a {op} b }}"),
                    &[a.clone(), same],
                ),
            ));
            out.push((
                format!("{tn} {op} [literal amt]"),
                drive(
                    &format!("fn main(a: {ty}) -> {ty} {{ a {op} 3 }}"),
                    std::slice::from_ref(a),
                ),
            ));
        }
        for op in ["-", "bnot"] {
            let src = if op == "-" {
                format!("fn main(a: {ty}) -> {ty} {{ -a }}")
            } else {
                format!("fn main(a: {ty}) -> {ty} {{ bnot a }}")
            };
            out.push((
                format!("{tn} unary{op}"),
                drive(&src, std::slice::from_ref(a)),
            ));
        }
        for op in CMP_OPS {
            out.push((
                format!("{tn} {op}"),
                drive(
                    &format!("fn main(a: {ty}, b: {ty}) -> bool {{ a {op} b }}"),
                    &[a.clone(), rhs.clone()],
                ),
            ));
        }
    }
    for op in ["and", "or", "xor", "andalso", "orelse"] {
        out.push((
            format!("bool {op}"),
            drive(
                &format!("fn main(a: bool, b: bool) -> bool {{ a {op} b }}"),
                &[Value::Bool(true), Value::Bool(false)],
            ),
        ));
    }
    out.push((
        "bool unarynot".to_string(),
        drive("fn main(a: bool) -> bool { not a }", &[Value::Bool(true)]),
    ));
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Fewer cells than this means the enumeration broke, not that the surface
/// shrank.
const CELL_FLOOR: usize = 112;

/// Every cell and the class it was observed in.
///
/// **`fixed %` reads `VmTrapsNativeRefuses` because the emitter was fixed.** It
/// was `VmTrapsNativeComputes` when this file was written, which is the defect
/// the denominator found.
const RECORDED: &[(&str, Cell)] = &[
    ("bool and", Cell::Agree),
    ("bool andalso", Cell::Agree),
    ("bool or", Cell::Agree),
    ("bool orelse", Cell::Agree),
    ("bool unarynot", Cell::Agree),
    ("bool xor", Cell::Agree),
    ("byte -", Cell::Agree),
    ("byte !=", Cell::Agree),
    ("byte *", Cell::Agree),
    ("byte /", Cell::Agree),
    ("byte %", Cell::Agree),
    ("byte +", Cell::Agree),
    ("byte <", Cell::Agree),
    ("byte <=", Cell::Agree),
    ("byte ==", Cell::Agree),
    ("byte >", Cell::Agree),
    ("byte >=", Cell::Agree),
    ("byte asl [literal amt]", Cell::Agree),
    ("byte asl [same-type amt]", Cell::RefRejects),
    ("byte asl [Word amt]", Cell::Agree),
    ("byte asr [literal amt]", Cell::Agree),
    ("byte asr [same-type amt]", Cell::RefRejects),
    ("byte asr [Word amt]", Cell::Agree),
    ("byte band", Cell::Agree),
    ("byte bor", Cell::Agree),
    ("byte bxor", Cell::Agree),
    ("byte lsl [literal amt]", Cell::Agree),
    ("byte lsl [same-type amt]", Cell::RefRejects),
    ("byte lsl [Word amt]", Cell::Agree),
    ("byte lsr [literal amt]", Cell::Agree),
    ("byte lsr [same-type amt]", Cell::RefRejects),
    ("byte lsr [Word amt]", Cell::Agree),
    ("byte unary-", Cell::Agree),
    ("byte unarybnot", Cell::Agree),
    ("fixed -", Cell::Agree),
    ("fixed !=", Cell::Agree),
    ("fixed *", Cell::Agree),
    ("fixed /", Cell::Agree),
    ("fixed %", Cell::VmTrapsNativeRefuses),
    ("fixed +", Cell::Agree),
    ("fixed <", Cell::Agree),
    ("fixed <=", Cell::Agree),
    ("fixed ==", Cell::Agree),
    ("fixed >", Cell::Agree),
    ("fixed >=", Cell::Agree),
    ("fixed asl [literal amt]", Cell::RefRejects),
    ("fixed asl [same-type amt]", Cell::RefRejects),
    ("fixed asl [Word amt]", Cell::RefRejects),
    ("fixed asr [literal amt]", Cell::RefRejects),
    ("fixed asr [same-type amt]", Cell::RefRejects),
    ("fixed asr [Word amt]", Cell::RefRejects),
    ("fixed band", Cell::RefRejects),
    ("fixed bor", Cell::RefRejects),
    ("fixed bxor", Cell::RefRejects),
    ("fixed lsl [literal amt]", Cell::RefRejects),
    ("fixed lsl [same-type amt]", Cell::RefRejects),
    ("fixed lsl [Word amt]", Cell::RefRejects),
    ("fixed lsr [literal amt]", Cell::RefRejects),
    ("fixed lsr [same-type amt]", Cell::RefRejects),
    ("fixed lsr [Word amt]", Cell::RefRejects),
    ("fixed unary-", Cell::Agree),
    ("fixed unarybnot", Cell::RefRejects),
    // **THE FLOAT ROWS, ADDED 2026-09-18.** Twelve `Agree` and sixteen
    // `RefRejects`, and **no cell in either defect class** — the backend answers
    // where the reference answers and refuses nothing the reference runs.
    //
    // `float unary-` is the cell that matters most. `Op::Neg` on a `Float` emitted
    // INVALID IR under `narrow-float-32` until 2026-09-17; `drive` now verifies the
    // lowered module before the JIT sees it, so that defect would fail this cell
    // rather than escaping to a probe.
    //
    // The sixteen refusals are the REFERENCE COMPILER declining bitwise and shift
    // operators on a float, which is the language's own surface and not a backend
    // gap. Recorded as classification, not as a finding.
    ("float !=", Cell::Agree),
    ("float %", Cell::Agree),
    ("float *", Cell::Agree),
    ("float +", Cell::Agree),
    ("float -", Cell::Agree),
    ("float /", Cell::Agree),
    ("float <", Cell::Agree),
    ("float <=", Cell::Agree),
    ("float ==", Cell::Agree),
    ("float >", Cell::Agree),
    ("float >=", Cell::Agree),
    ("float asl [Word amt]", Cell::RefRejects),
    ("float asl [literal amt]", Cell::RefRejects),
    ("float asl [same-type amt]", Cell::RefRejects),
    ("float asr [Word amt]", Cell::RefRejects),
    ("float asr [literal amt]", Cell::RefRejects),
    ("float asr [same-type amt]", Cell::RefRejects),
    ("float band", Cell::RefRejects),
    ("float bor", Cell::RefRejects),
    ("float bxor", Cell::RefRejects),
    ("float lsl [Word amt]", Cell::RefRejects),
    ("float lsl [literal amt]", Cell::RefRejects),
    ("float lsl [same-type amt]", Cell::RefRejects),
    ("float lsr [Word amt]", Cell::RefRejects),
    ("float lsr [literal amt]", Cell::RefRejects),
    ("float lsr [same-type amt]", Cell::RefRejects),
    ("float unary-", Cell::Agree),
    ("float unarybnot", Cell::RefRejects),
    ("word -", Cell::Agree),
    ("word !=", Cell::Agree),
    ("word *", Cell::Agree),
    ("word /", Cell::Agree),
    ("word %", Cell::Agree),
    ("word +", Cell::Agree),
    ("word <", Cell::Agree),
    ("word <=", Cell::Agree),
    ("word ==", Cell::Agree),
    ("word >", Cell::Agree),
    ("word >=", Cell::Agree),
    ("word asl [literal amt]", Cell::Agree),
    ("word asl [same-type amt]", Cell::Agree),
    ("word asl [Word amt]", Cell::Agree),
    ("word asr [literal amt]", Cell::Agree),
    ("word asr [same-type amt]", Cell::Agree),
    ("word asr [Word amt]", Cell::Agree),
    ("word band", Cell::Agree),
    ("word bor", Cell::Agree),
    ("word bxor", Cell::Agree),
    ("word lsl [literal amt]", Cell::Agree),
    ("word lsl [same-type amt]", Cell::Agree),
    ("word lsl [Word amt]", Cell::Agree),
    ("word lsr [literal amt]", Cell::Agree),
    ("word lsr [same-type amt]", Cell::Agree),
    ("word lsr [Word amt]", Cell::Agree),
    ("word unary-", Cell::Agree),
    ("word unarybnot", Cell::Agree),
];

#[test]
fn every_cell_is_classified_and_the_class_still_holds() {
    let observed = observe();
    assert!(
        observed.len() >= CELL_FLOOR,
        "the enumeration produced {} cells, below the floor of {CELL_FLOOR}. A \
         short enumeration passes every check below while testing nothing, so \
         this is a broken probe rather than a result.",
        observed.len()
    );
    let recorded: std::collections::BTreeMap<&str, Cell> = RECORDED.iter().copied().collect();
    assert_eq!(
        recorded.len(),
        RECORDED.len(),
        "the record contains a duplicate cell label"
    );
    assert_eq!(
        observed.len(),
        RECORDED.len(),
        "the enumeration produced {} cells and {} are recorded. Classify the new \
         cells by what they do; do not resize the record to match.",
        observed.len(),
        RECORDED.len()
    );

    let mut moved = Vec::new();
    for (label, got) in &observed {
        match recorded.get(label.as_str()) {
            None => moved.push(format!("{label}: unclassified, observed {got:?}")),
            Some(&want) if want != *got => {
                moved.push(format!("{label}: recorded {want:?}, observed {got:?}"))
            }
            Some(_) => {}
        }
    }
    assert!(
        moved.is_empty(),
        "{} cell(s) changed class: {moved:?}. Say in the record what changed and \
         why; do not edit the class to match the run.",
        moved.len()
    );
}

/// **The two classes that are defects, asserted apart from the record**, so that
/// re-recording a cell cannot quietly make a defect acceptable.
#[test]
fn no_cell_answers_where_the_reference_errors() {
    let bad: Vec<String> = observe()
        .into_iter()
        .filter(|(_, c)| matches!(c, Cell::VmTrapsNativeComputes | Cell::Disagree))
        .map(|(l, c)| format!("{l}: {c:?}"))
        .collect();
    assert!(
        bad.is_empty(),
        "{} cell(s) diverge from the reference: {bad:?}. `VmTrapsNativeComputes` \
         means native code returns a value where the reference raises an error, \
         which is the `Fixed % Fixed` defect this file was built to find. An \
         arithmetically correct answer is still wrong when the reference does not \
         produce it.",
        bad.len()
    );
}

/// The `Fixed` refusal, asserted on its own rather than only through the matrix.
///
/// The matrix records a CLASS; this records the CAUSE, so a refusal arriving for
/// an unrelated reason is a failure rather than a pass.
#[test]
fn fixed_modulo_is_refused_with_a_reason_naming_the_operand() {
    let m = common::build("fn main(a: Fixed, b: Fixed) -> Fixed { a % b }");
    let text = format!("{:?}", module_refusals(&m, LowerOptions::default()));
    assert!(
        text.contains("Fixed operand"),
        "`Fixed % Fixed` is no longer refused for having a Fixed operand: {text}"
    );
}

/// **The `Fixed` scalar tag is a number agreed with a file this line does not
/// own.** A silent renumbering upstream would make every `Fixed` parameter read
/// as some other kind, the refusal above would stop firing, and the divergence
/// would reopen — failing OPEN, which is the direction that matters.
#[test]
fn the_fixed_scalar_tag_still_matches_upstream() {
    assert_eq!(
        keleusma::value_layout::ScalarKind::Fixed.to_tag(),
        4,
        "`ScalarKind::Fixed` no longer encodes as 4. `SCALAR_FIXED_TAG` in the \
         emitter must follow, or every Fixed operand reads as an unrecognised \
         kind and `Fixed % Fixed` is lowered again."
    );
}

/// **THE FOUR-BYTE EXACTNESS PREDICATE IS NOT VACUOUS.**
///
/// Two callers apply this during a matrix run: [`drive`] to every float OPERAND
/// before either implementation sees it, and `Out::checked` to every float
/// RESULT either one produces. That is the real coverage, and it is why there is
/// no second list of operands here to drift out of step with [`observe`].
///
/// What this test adds is the half that coverage cannot supply: **evidence that
/// the predicate can say no.** A checker that accepted everything would make
/// every `checked` call pass while establishing nothing, which is the shape this
/// package has recorded more than once — a fidelity check that discriminated
/// nothing, and a census blind to its own subject.
#[test]
fn the_four_byte_exactness_check_can_fail() {
    for ok in [0.0, 14.0, 4.0, 3.0, 3.5, 56.0, -14.0, 2.0, 18.0, 10.0] {
        assert!(
            is_exact_in_four_bytes(ok),
            "{ok} is exact in four bytes and the predicate denies it; the matrix \
             would then refuse its own subjects"
        );
    }
    for bad in [0.1, 1.0 / 3.0, 1e300, f64::MIN_POSITIVE] {
        assert!(
            !is_exact_in_four_bytes(bad),
            "{bad} is NOT exact in four bytes and the predicate accepts it. Then \
             `Out::checked` guards nothing, and under `narrow-float-32` an f64 \
             reference compared against an f32 backend would report rounding as \
             a divergence."
        );
    }
}

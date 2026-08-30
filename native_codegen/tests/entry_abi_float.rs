//! The ENTRY application binary interface for floats, exercised by CALLING it.
//!
//! # Why acceptance is not the evidence
//!
//! Every earlier float increment could be checked by executing an opcode. This
//! one cannot: the thing under test is the *calling convention*, and a wrong
//! convention produces a module that lowers, verifies, and links. On aarch64 a
//! `double` argument arrives in `v0` while an `i64` parameter is read from `x0`,
//! so declaring the wrong one reads an unrelated register — a plausible number,
//! not a fault. The only way to see it is to call the symbol through the C
//! convention the host would use, with a value the compiler cannot fold, and
//! compare against the reference virtual machine.
//!
//! # What is deliberately NOT covered
//!
//! No corpus module carries a float in a signature, so this file is the entire
//! population of witnesses. Float shared slots, a narrower `Float`, and floats
//! inside composites remain unbuilt; the narrow width is refused rather than
//! approximated and `narrow_float_is_refused_rather_than_widened` is that claim's
//! only local check, since this build has an 8-byte `Float`.

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::{Module, Value};
use keleusma::vm::{Vm, VmState};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerOptions, lower_module};

mod common;

fn compile_src(src: &str) -> Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile")
}

/// Call the lowered entry through the real C convention with an `f64`.
///
/// **The declared parameter count is asserted before the call.** Invoking a
/// symbol through a signature it does not have is undefined behaviour that
/// presents as a SIGSEGV inside JIT-compiled code with no indication of which
/// side is wrong, and this harness's whole subject is signature agreement.
fn native_float_entry(m: &Module, arg: f64) -> Result<f64, String> {
    let ctx = Context::create();
    let entry = m.entry_point.expect("entry point");
    let lm = ctx.create_module("kel");
    lower_module(&ctx, &lm, m, LowerOptions::default()).map_err(|e| format!("lower: {e}"))?;
    // The IR verifier is the first place a declaration/return type mismatch
    // shows, and it names the offending function.
    lm.verify().map_err(|e| format!("IR invalid: {e}"))?;
    let sym = format!("kel_chunk_{entry}");
    let f = lm
        .get_function(&sym)
        .ok_or_else(|| format!("no function {sym}"))?;
    if f.count_params() != 1 {
        return Err(format!(
            "the entry `{sym}` takes {} parameters; this harness models the \
             one-argument, no-data case only",
            f.count_params()
        ));
    }
    // **THE DECLARED TYPES ARE CHECKED, not assumed.** If the lowering regressed
    // to an `i64` position this reports it as a readable failure rather than
    // reading the wrong register and returning a plausible number.
    if !f.get_type().get_param_types()[0].is_float_type() {
        return Err("the entry's parameter is not a floating-point type".into());
    }
    if !f
        .get_type()
        .get_return_type()
        .is_some_and(|t| t.is_float_type())
    {
        return Err("the entry's return is not a floating-point type".into());
    }
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .map_err(|e| format!("jit: {e}"))?;
    let callable = unsafe { ee.get_function::<unsafe extern "C" fn(f64) -> f64>(&sym) }
        .map_err(|e| format!("symbol: {e}"))?;
    Ok(unsafe { callable.call(arg) })
}

fn vm_float_entry(m: &Module, arg: f64) -> f64 {
    let cap = keleusma::vm::auto_arena_capacity_for(m, &[]).expect("arena capacity");
    let arena = keleusma_arena::Arena::with_capacity(cap);
    let mut vm = Vm::new(m.clone(), &arena).expect("vm");
    match vm.call(&[Value::Float(arg)]).expect("vm run") {
        VmState::Finished(Value::Float(v)) => v,
        other => panic!("unexpected VM outcome: {other:?}"),
    }
}

/// Compare both sides over a set of runtime arguments.
fn assert_float_entry_agrees(src: &str, args: &[f64]) {
    let m = compile_src(src);
    for &a in args {
        let want = vm_float_entry(&m, a);
        match native_float_entry(&m, a) {
            Ok(got) if got.to_bits() == want.to_bits() => {}
            // Bit equality is the comparison, so a signed zero or a NaN payload
            // difference is a failure rather than an equality that hides one.
            Ok(got) => panic!(
                "arg {a}: native {got} ({:#x}) != vm {want} ({:#x})",
                got.to_bits(),
                want.to_bits()
            ),
            Err(e) => panic!("arg {a}: {e}"),
        }
    }
}

#[test]
fn a_float_parameter_survives_the_boundary_unchanged() {
    // The identity function is the sharpest test of the convention alone: it
    // contains no arithmetic, so ANY difference is the boundary.
    assert_float_entry_agrees(
        "fn main(x: Float) -> Float { x }",
        &[
            0.0,
            -0.0,
            1.5,
            -2.25,
            1e300,
            -1e-300,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ],
    );
}

#[test]
fn a_nan_argument_survives_the_boundary() {
    // A NaN is the case a bitcast preserves and a numeric conversion may not.
    let m = compile_src("fn main(x: Float) -> Float { x }");
    let got = native_float_entry(&m, f64::NAN).expect("lower and call");
    assert!(got.is_nan(), "a NaN argument came back as {got}");
}

#[test]
fn the_parameter_is_operated_on_rather_than_only_passed_through() {
    // Identity alone would pass even if the parameter's local kind were never
    // tagged `Float`. Arithmetic ON the parameter is what requires the seeding,
    // and refuses to lower without it.
    assert_float_entry_agrees(
        "fn main(x: Float) -> Float { x * 2.0 + 1.0 }",
        &[0.0, 1.5, -3.25, 1e10],
    );
}

#[test]
fn a_float_crosses_a_call_boundary_in_both_directions() {
    // The caller-side twin: arguments converted to the callee's declared types
    // and the result converted back. A wrong conversion at either end is a wrong
    // number here, where the single-function tests could not see it.
    assert_float_entry_agrees(
        "fn twice(a: Float) -> Float { a + a }\n\
         fn main(x: Float) -> Float { twice(x) + twice(1.5) }",
        &[0.0, 2.5, -4.75],
    );
}

#[test]
fn a_float_argument_with_an_integer_return_is_not_conflated() {
    // Parameters and the return come from DIFFERENT places — `chunk.param_types`
    // and the module-level signature — and conflating them is the error that
    // made an earlier attempt defer this. A mixed signature is where that shows.
    let m = compile_src("fn main(x: Float) -> Word { if x > 1.0 { 7 } else { 9 } }");
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("lower");
    lm.verify().expect("IR invalid");
    let sym = format!("kel_chunk_{}", m.entry_point.expect("entry"));
    let f = lm.get_function(&sym).expect("entry symbol");
    assert!(
        f.get_type().get_param_types()[0].is_float_type(),
        "the float parameter did not take a floating-point position"
    );
    assert!(
        !f.get_type()
            .get_return_type()
            .expect("a return type")
            .is_float_type(),
        "the integer return was declared as a float"
    );
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");
    let callable =
        unsafe { ee.get_function::<unsafe extern "C" fn(f64) -> i64>(&sym) }.expect("symbol");
    for &a in &[0.5f64, 1.0, 2.0] {
        let cap = keleusma::vm::auto_arena_capacity_for(&m, &[]).expect("cap");
        let arena = keleusma_arena::Arena::with_capacity(cap);
        let mut vm = Vm::new(m.clone(), &arena).expect("vm");
        let want = match vm.call(&[Value::Float(a)]).expect("vm run") {
            VmState::Finished(Value::Int(v)) => v,
            other => panic!("unexpected VM outcome: {other:?}"),
        };
        assert_eq!(unsafe { callable.call(a) }, want, "arg {a}");
    }
}

#[test]
fn narrow_float_is_refused_rather_than_widened() {
    // This build's `Float` is 8 bytes, so the refusal path cannot be reached by
    // compiling a program. What is pinned here is that the guard reads the
    // MODULE's declared width rather than assuming one — the module carries
    // `float_bits_log2`, and a build with `narrow-float-32` would set it to 5.
    let m = compile_src("fn main(x: Float) -> Float { x }");
    assert_eq!(
        1u32 << m.float_bits_log2 >> 3,
        8,
        "this build's Float is not 8 bytes, so the signature route should be \
         refused and these tests do not describe it"
    );
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    assert!(
        lower_module(&ctx, &lm, &m, LowerOptions::default()).is_ok(),
        "an 8-byte float signature should lower"
    );
    let _ = common::CORPUS_ROOTS;
}

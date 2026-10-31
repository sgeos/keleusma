//! Native-call host failures under the operator rulings of 2026-10-20 and
//! 2026-10-24.
//!
//! Every native call is reified, so a soft host failure arrives as an outcome.
//! A `host_error` arm, or its alias `error`, handles it. Without a covering arm
//! the compiler emits an explicit `Trap(HostError)`, which surfaces the host's
//! own error. A call site naming an unregistered native is rejected before
//! execution. The trap analyses classify a reified call as clear, so a fully
//! handled program is trap-free and a bare call is possibly trapping, while an
//! unreified call, which only hand-built bytecode can contain, stays outside
//! the first-pass proof.
//!
//! The memory-bound controls pin a defect this increment repaired. The
//! worst-case memory walk read the raw count byte, including the reify flag,
//! as the argument count, and so understated the bound of every handled call.
//!
//! The self-hosted compiler does not compile native calls, so no byte-identity
//! comparison applies. The Keleusma analyses are compared where they exist.

#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::Arena;
use keleusma::bytecode::{BlockType, Module, Op, TrapKind};
use keleusma::trap_analysis::{
    TrapAnalysisError, TrapVerdict, UnsupportedTrapOperation, classify_traps,
};
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Value, VmError};

fn compile(source: &str) -> Module {
    let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
    keleusma::compiler::compile(&ast).unwrap()
}

fn agree(module: &Module) -> Result<TrapVerdict, TrapAnalysisError> {
    let reference = classify_traps(module);
    #[cfg(feature = "self-host")]
    assert_eq!(
        keleusma::selfhost::classify_traps_via_kel(module),
        reference
    );
    reference
}

fn host_error_traps(module: &Module) -> usize {
    module
        .chunks
        .iter()
        .flat_map(|c| c.ops.iter())
        .filter(|op| **op == Op::Trap(TrapKind::HostError.code()))
        .count()
}

fn native_calls(module: &Module) -> Vec<u8> {
    module
        .chunks
        .iter()
        .flat_map(|c| c.ops.iter())
        .filter_map(|op| match op {
            Op::CallVerifiedNative(_, n) | Op::CallExternalNative(_, n) => Some(*n),
            _ => None,
        })
        .collect()
}

/// `host::risky` fails with a coded error on 0, a message-only error on 1, a
/// VM-internal error on 2 and a halting host error on 3, and otherwise returns
/// its argument.
fn run(source: &str, x: i64) -> Result<Value, VmError> {
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(compile(source), &arena).unwrap();
    vm.register_native("host::risky", |args| match args[0] {
        Value::Int(0) => Err(VmError::NativeErrorCode {
            code: 42,
            message: String::from("boom"),
        }),
        Value::Int(1) => Err(VmError::NativeError(String::from("plain"))),
        Value::Int(2) => Err(VmError::DivisionByZero),
        Value::Int(3) => Err(VmError::HostHalt(String::from("stop"))),
        ref v => Ok(v.clone()),
    });
    match vm.call(&[Value::Int(x)])? {
        VmState::Finished(v) => Ok(v),
        other => panic!("expected a finished call, got {other:?}"),
    }
}

const BARE: &str = "use host::risky\nfn main(x: Word) -> Word { host::risky(x) }";

#[test]
fn every_native_call_is_reified() {
    for source in [
        BARE,
        "use host::risky\nfn main(x: Word) -> Word { x |> host::risky() }",
        "use host::risky\nfn main(x: Word) -> Word { host::risky(x) { ok(v) => v } }",
        "use host::risky\nfn main(x: Word) -> Word { host::risky(x) { ok(v) => v, host_error(c) => c } }",
    ] {
        let calls = native_calls(&compile(source));
        assert_eq!(calls, [0x81], "{source}");
    }
}

#[test]
fn a_bare_failure_surfaces_the_hosts_own_error() {
    let module = compile(BARE);
    assert_eq!(host_error_traps(&module), 1);
    match run(BARE, 0) {
        Err(VmError::NativeErrorCode { code, message }) => {
            assert_eq!((code, message.as_str()), (42, "boom"));
        }
        other => panic!("expected the host's coded error, got {other:?}"),
    }
    match run(BARE, 1) {
        Err(VmError::NativeError(message)) => assert_eq!(message, "plain"),
        other => panic!("expected the host's message-only error, got {other:?}"),
    }
    assert_eq!(run(BARE, 7).unwrap(), Value::Int(7));
}

#[test]
fn a_pipeline_call_reaches_the_same_default_trap() {
    let source = "use host::risky\nfn main(x: Word) -> Word { x |> host::risky() }";
    assert_eq!(host_error_traps(&compile(source)), 1);
    assert!(matches!(
        run(source, 0),
        Err(VmError::NativeErrorCode { code: 42, .. })
    ));
    assert_eq!(run(source, 9).unwrap(), Value::Int(9));
}

#[test]
fn an_internal_fault_still_propagates_unreified() {
    // Only soft host failures are reified. A native returning a VM-internal
    // error propagates it as before, through neither a handler nor the trap.
    let handled = "use host::risky\nfn main(x: Word) -> Word { host::risky(x) { ok(v) => v, host_error => 5 } }";
    for source in [BARE, handled] {
        assert!(
            matches!(run(source, 2), Err(VmError::DivisionByZero)),
            "{source}"
        );
    }
}

#[test]
fn a_halting_host_error_is_never_reified() {
    // B10, ruled on 2026-10-31. A native that must end the run returns
    // `HostHalt`. A covering handler does not catch it, the default trap does
    // not rewrite it, and its category tells the host to halt.
    let handled = "use host::risky\nfn main(x: Word) -> Word { host::risky(x) { ok(v) => v, host_error => 5 } }";
    let bound = "use host::risky\nfn main(x: Word) -> Word { host::risky(x) { ok(v) => v, host_error(c) => c } }";
    for source in [BARE, handled, bound] {
        match run(source, 3) {
            Err(e @ VmError::HostHalt(_)) => {
                assert_eq!(
                    e.category(),
                    keleusma::vm::VmErrorCategory::Halt,
                    "{source}"
                );
                assert!(
                    matches!(e, VmError::HostHalt(ref m) if m == "stop"),
                    "{source}"
                );
            }
            other => panic!("{source}: expected the halting host error, got {other:?}"),
        }
    }
    // The soft failures stay catchable by the same handler.
    assert_eq!(run(handled, 0).unwrap(), Value::Int(5));
    assert_eq!(run(handled, 1).unwrap(), Value::Int(5));
}

#[test]
fn a_host_error_arm_binds_the_code() {
    let source = "use host::risky\nfn main(x: Word) -> Word { host::risky(x) { ok(v) => v, host_error(c) => c + 1 } }";
    assert_eq!(run(source, 0).unwrap(), Value::Int(43));
    // A message-only failure reifies the sentinel code -1.
    assert_eq!(run(source, 1).unwrap(), Value::Int(0));
    assert_eq!(run(source, 7).unwrap(), Value::Int(7));
}

#[test]
fn a_bare_host_error_arm_handles_without_binding() {
    let source = "use host::risky\nfn main(x: Word) -> Word { host::risky(x) { ok(v) => v, host_error => 5 } }";
    assert_eq!(run(source, 0).unwrap(), Value::Int(5));
    assert_eq!(run(source, 1).unwrap(), Value::Int(5));
    assert_eq!(host_error_traps(&compile(source)), 0);
}

#[test]
fn the_error_alias_compiles_identically() {
    let spelled = |arm: &str| {
        compile(&format!(
            "use host::risky\nfn main(x: Word) -> Word {{ host::risky(x) {{ ok(v) => v, {arm}(c) => c }} }}"
        ))
        .to_bytes()
        .unwrap()
    };
    assert_eq!(spelled("error"), spelled("host_error"));
}

#[test]
fn an_uncovering_handler_falls_back_to_the_default_trap() {
    for arm in ["host_error(c) when c == 7 => c", "host_error(7) => 0"] {
        let source = format!(
            "use host::risky\nfn main(x: Word) -> Word {{ host::risky(x) {{ ok(v) => v, {arm} }} }}"
        );
        assert_eq!(host_error_traps(&compile(&source)), 1, "{arm}");
        match run(&source, 0) {
            Err(VmError::NativeErrorCode { code, message }) => {
                assert_eq!((code, message.as_str()), (42, "boom"), "{arm}");
            }
            other => panic!("{arm}: expected the host's coded error, got {other:?}"),
        }
        assert_eq!(run(&source, 4).unwrap(), Value::Int(4), "{arm}");
    }
    // With no host_error arm at all, the failure reaches the same trap.
    let source = "use host::risky\nfn main(x: Word) -> Word { host::risky(x) { ok(v) => v } }";
    assert_eq!(host_error_traps(&compile(source)), 1);
    assert!(matches!(
        run(source, 0),
        Err(VmError::NativeErrorCode { code: 42, .. })
    ));
}

#[test]
fn trap_classification_follows_handler_coverage() {
    for (source, verdict_traps) in [
        (BARE, true),
        (
            "use host::risky\nfn main(x: Word) -> Word { host::risky(x) { ok(v) => v } }",
            true,
        ),
        (
            "use host::risky\nfn main(x: Word) -> Word { host::risky(x) { ok(v) => v, host_error(c) when c == 7 => c } }",
            true,
        ),
        (
            "use host::risky\nfn main(x: Word) -> Word { host::risky(x) { ok(v) => v, host_error(_) => 0 } }",
            false,
        ),
        (
            "use host::risky\nfn main(x: Word) -> Word { host::risky(x) { ok(v) => v, host_error => 0 } }",
            false,
        ),
    ] {
        match agree(&compile(source)).unwrap() {
            TrapVerdict::TrapFree => assert!(!verdict_traps, "{source}"),
            TrapVerdict::MayTrap { .. } => assert!(verdict_traps, "{source}"),
        }
    }
}

#[test]
fn an_unreified_call_remains_outside_the_proof() {
    // Strip the reify flag and the default trap from a compiled bare call.
    // The result is valid bytecode whose failure would propagate internally.
    let mut module = compile(BARE);
    let ops = &mut module.chunks[0].ops;
    let call = ops
        .iter()
        .position(|op| matches!(op, Op::CallVerifiedNative(..)))
        .unwrap();
    let Op::CallVerifiedNative(idx, n) = ops[call] else {
        unreachable!()
    };
    assert_eq!(
        ops[call + 1..call + 6]
            .iter()
            .map(core::mem::discriminant)
            .collect::<Vec<_>>(),
        [Op::Const(0), Op::CmpEq, Op::If(0), Op::Trap(0), Op::EndIf]
            .iter()
            .map(core::mem::discriminant)
            .collect::<Vec<_>>()
    );
    ops[call] = Op::CallVerifiedNative(idx, n & 0x7F);
    ops.drain(call + 1..call + 6);
    assert!(keleusma::verify::verify(&module).is_ok());
    match agree(&module) {
        Err(TrapAnalysisError::Unsupported { operation, .. }) => {
            assert_eq!(operation, UnsupportedTrapOperation::NativeCall);
        }
        other => panic!("expected an unsupported native call, got {other:?}"),
    }
}

#[test]
fn an_unregistered_native_is_rejected_before_execution() {
    // The native lies on a path this call does not take. Before the ruling the
    // call finished, because the site was checked only when reached.
    let source =
        "use host::risky\nfn main(x: Word) -> Word { if x == 0 { host::risky(x) } else { x } }";
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(compile(source), &arena).unwrap();
    match vm.call(&[Value::Int(5)]) {
        Err(VmError::VerifyError(message)) => {
            assert!(message.contains("host::risky"), "{message}");
            assert!(message.contains("not registered"), "{message}");
        }
        other => panic!("expected a pre-execution rejection, got {other:?}"),
    }
    // Registering clears the cached verdict, and the same call then runs.
    vm.register_native("host::risky", |args| Ok(args[0].clone()));
    assert!(matches!(
        vm.call(&[Value::Int(5)]),
        Ok(VmState::Finished(Value::Int(5)))
    ));
}

fn stream_stack_bound(module: &Module) -> u32 {
    let bounds = keleusma::verify::module_wcmu(module, &[]).unwrap();
    module
        .chunks
        .iter()
        .zip(bounds)
        .filter(|(c, _)| c.block_type == BlockType::Stream)
        .map(|(_, (stack, _))| stack)
        .max()
        .unwrap()
}

#[test]
fn the_memory_bound_counts_both_slots_of_a_reified_call() {
    // The native result sits beneath three further operands. A walk that read
    // the raw count byte moved its offset by 1 - 129 at the call, so the later
    // operands never raised the peak above the program without the call.
    let without = compile("loop main(x: Word) -> Word { yield x + (x + (x + x)) }");
    let with = compile(
        "use host::risky\nloop main(x: Word) -> Word { yield host::risky(x) + (x + (x + (x + x))) }",
    );
    assert!(
        stream_stack_bound(&with) > stream_stack_bound(&without),
        "with {}, without {}",
        stream_stack_bound(&with),
        stream_stack_bound(&without)
    );
}

#[cfg(feature = "self-host")]
#[test]
fn keleusma_resource_admission_agrees_on_native_programs() {
    use keleusma::selfhost::{VerificationVerdict, admit_module_via_kel};
    for source in [
        "use host::risky\nloop main(x: Word) -> Word { yield host::risky(x) + (x + (x + (x + x))) }",
        "use host::risky\nloop main(x: Word) -> Word { yield host::risky(x) { ok(v) => v + (x + x), host_error(c) => c } }",
    ] {
        let module = compile(source);
        let bounds = keleusma::verify::module_wcmu(&module, &[]).unwrap();
        let limit = module
            .chunks
            .iter()
            .zip(bounds)
            .filter(|(c, _)| c.block_type == BlockType::Stream)
            .map(|(_, (s, h))| s as usize + h as usize)
            .max()
            .unwrap();
        for capacity in [limit.saturating_sub(1), limit, limit + 1] {
            let reference = keleusma::verify::verify_resource_bounds(&module, capacity).is_ok();
            let actual = admit_module_via_kel(&module, capacity).unwrap();
            assert_eq!(
                matches!(actual, VerificationVerdict::Accepted),
                reference,
                "{source}: capacity {capacity}, verdict {actual:?}"
            );
        }
    }
}

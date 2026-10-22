//! Proof-gated execution retains all unproved guards and validates reentry.
#![cfg(all(feature = "compile", feature = "verify"))]
use keleusma::Arena;
use keleusma::bytecode::{ConstValue, Module, Op, Value};
use keleusma::vm::{
    DEFAULT_ARENA_CAPACITY, ExecutionChecks, TrapPolicy, Vm, VmError, VmOptions, VmState,
};
fn compile(source: &str) -> Module {
    keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap(),
    )
    .unwrap()
}
fn options(checks: ExecutionChecks) -> VmOptions {
    VmOptions {
        trap_policy: TrapPolicy::RequireTrapFree,
        execution_checks: checks,
        ..VmOptions::default()
    }
}
fn run(module: Module, checks: ExecutionChecks, args: &[Value]) -> Result<Value, VmError> {
    let need = keleusma::vm::required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).unwrap();
    let (mut vm, _) = Vm::new_with_options(module, &arena, options(checks))?;
    let mut shared = vec![0; vm.shared_data_bytes()];
    match vm.call_with_shared(&mut shared, args)? {
        VmState::Finished(v) => Ok(v),
        other => panic!("{other:?}"),
    }
}
#[test]
fn checked_and_proven_arithmetic_bounds_calls_and_branches_agree() {
    for source in [
        "fn main(x: Word) -> Word { x + 7 }",
        "private data d { n: Word } fn main(x: Word) -> Word { for i in 0..4 { d.n = d.n + i; } d.n + x }",
        "fn f(x: Word) -> Word { x * 3 } fn main(x: Word) -> Word { if x > 0 { f(x) } else { f(0 - x) } }",
        "fn main(x: Word) -> Word { 7 / x { ok(v) => v, zero_divisor(n) => n } }",
        "fn main(x: Word) -> Word { let a = [7, 8]; a[x] { ok(v) => v, invalid_index(i) => i } }",
        "shared data d { a: [Byte; 2] } fn main(x: Word) -> Word { d.a[x] { ok(v) => v as Word, invalid_index(i) => i } }",
    ] {
        for value in [-1, 0, 1, 2, 3] {
            let module = compile(source);
            let checked = run(
                module.clone(),
                ExecutionChecks::Checked,
                &[Value::Int(value)],
            )
            .unwrap();
            let proven = run(module, ExecutionChecks::Proven, &[Value::Int(value)]).unwrap();
            assert_eq!(checked, proven, "{source} with {value}");
        }
    }
}
#[test]
fn an_empty_return_needs_a_stronger_proof_than_trap_freedom() {
    let mut module = compile("fn main() -> Word { 7 }");
    module.chunks[0].ops = vec![Op::Return];
    assert_eq!(
        keleusma::trap_analysis::classify_traps(&module).unwrap(),
        keleusma::trap_analysis::TrapVerdict::TrapFree
    );
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    assert!(matches!(
        Vm::new_with_options(module.clone(), &arena, options(ExecutionChecks::Proven)),
        Err(VmError::VerifyError(_))
    ));
    let (mut checked, _) =
        Vm::new_with_options(module, &arena, options(ExecutionChecks::Checked)).unwrap();
    assert!(matches!(checked.call(&[]), Err(VmError::StackUnderflow)));
}
#[test]
fn proven_mode_requires_policy_and_cannot_keep_a_weakened_policy() {
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let module = compile("fn main() -> Word { 7 }");
    let bad = VmOptions {
        execution_checks: ExecutionChecks::Proven,
        ..VmOptions::default()
    };
    assert!(Vm::new_with_options(module.clone(), &arena, bad).is_err());
    let (mut vm, _) =
        Vm::new_with_options(module, &arena, options(ExecutionChecks::Proven)).unwrap();
    assert!(vm.set_trap_policy(TrapPolicy::Allow).is_err());
    assert_eq!(vm.trap_policy(), TrapPolicy::RequireTrapFree);
    vm.set_execution_checks(ExecutionChecks::Checked).unwrap();
    vm.set_trap_policy(TrapPolicy::Allow).unwrap();
}
#[test]
fn yielded_reset_and_breakpoint_states_require_the_correct_entry() {
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let module = compile("yield main(x: Word) -> Word { let y = yield x + 1; y + 2 }");
    let (mut vm, _) =
        Vm::new_with_options(module, &arena, options(ExecutionChecks::Proven)).unwrap();
    assert!(matches!(
        vm.call(&[Value::Int(3)]).unwrap(),
        VmState::Yielded(Value::Int(4))
    ));
    assert!(vm.resume_from_breakpoint().is_err());
    assert!(vm.call(&[Value::Int(3)]).is_err());
    assert!(matches!(
        vm.resume(Value::Int(5)).unwrap(),
        VmState::Finished(Value::Int(7))
    ));
    vm.replace_module(
        compile("loop main(x: Word) -> Word { yield x + 1 }"),
        vec![],
    )
    .unwrap();
    assert!(matches!(
        vm.call(&[Value::Int(3)]).unwrap(),
        VmState::Yielded(Value::Int(4))
    ));
    assert!(matches!(vm.resume(Value::Int(5)).unwrap(), VmState::Reset));
    assert!(matches!(
        vm.resume(Value::Int(7)).unwrap(),
        VmState::Yielded(Value::Int(8))
    ));
    vm.reset_after_error();
    vm.replace_module(compile("fn main() -> Word { 3 + 4 }"), vec![])
        .unwrap();
    vm.set_breakpoint(0, 0);
    assert!(matches!(
        vm.call(&[]).unwrap(),
        VmState::BreakpointHit { .. }
    ));
    assert!(vm.resume(Value::Int(0)).is_err());
    assert!(matches!(
        vm.resume_from_breakpoint().unwrap(),
        VmState::Finished(Value::Int(7))
    ));
}
#[test]
fn returned_errors_poison_reentry_and_reset_reestablishes_freshness() {
    let mut module = compile("fn main() -> Word { 7 }");
    module.chunks[0].constants = vec![ConstValue::Bool(false), ConstValue::Int(1)];
    module.chunks[0].ops = vec![
        Op::Const(0),
        Op::Const(1),
        Op::CheckedAdd,
        Op::PopN(2),
        Op::Return,
    ];
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let (mut vm, _) =
        Vm::new_with_options(module, &arena, options(ExecutionChecks::Proven)).unwrap();
    assert!(matches!(vm.call(&[]), Err(VmError::TypeError(_))));
    assert!(matches!(
        vm.resume(Value::Int(0)),
        Err(VmError::VerifyError(_))
    ));
    assert!(matches!(
        vm.resume_from_breakpoint(),
        Err(VmError::VerifyError(_))
    ));
    assert!(vm.call(&[]).is_err());
    vm.reset_after_error();
    assert!(matches!(vm.call(&[]), Err(VmError::TypeError(_))));
    vm.replace_module(compile("fn main() -> Word { 7 }"), vec![])
        .unwrap();
    assert!(matches!(
        vm.call(&[]).unwrap(),
        VmState::Finished(Value::Int(7))
    ));
}
#[test]
fn failed_replacement_preserves_the_old_proof_and_suspension() {
    let module = compile("yield main(x: Word) -> Word { let y = yield x; y + 1 }");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let (mut vm, _) =
        Vm::new_with_options(module, &arena, options(ExecutionChecks::Proven)).unwrap();
    assert!(matches!(
        vm.call(&[Value::Int(3)]).unwrap(),
        VmState::Yielded(Value::Int(3))
    ));
    let mut empty_return = compile("fn main() -> Word { 0 }");
    empty_return.chunks[0].ops = vec![Op::Return];
    assert!(vm.replace_module_unchecked(empty_return, vec![]).is_err());
    assert!(
        vm.replace_module(compile("fn main() -> Word { 7 / 0 }"), vec![])
            .is_err()
    );
    assert_eq!(vm.execution_checks(), ExecutionChecks::Proven);
    assert!(matches!(
        vm.resume(Value::Int(4)).unwrap(),
        VmState::Finished(Value::Int(5))
    ));
}
#[test]
fn borrowed_and_trust_skip_loading_obtain_real_pop_proofs() {
    for invalid in [false, true] {
        let mut module = compile("fn main() -> Word { 7 }");
        if invalid {
            module.chunks[0].ops = vec![Op::Return];
        }
        let mut bytes = rkyv::util::AlignedVec::<8>::new();
        bytes.extend_from_slice(&module.to_bytes().unwrap());
        for borrowed in [false, true] {
            let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
            // The empty Return remains structurally valid under legacy verification.
            // Its runtime underflow is exactly what the additional proof must reject.
            let vm = unsafe {
                if borrowed {
                    Vm::view_bytes_zero_copy(&bytes, &arena)
                } else {
                    Vm::new_unchecked(module.clone(), &arena)
                }
            }
            .unwrap();
            let result = vm
                .with_trap_policy(TrapPolicy::RequireTrapFree)
                .unwrap()
                .with_execution_checks(ExecutionChecks::Proven);
            if invalid {
                assert!(result.is_err());
            } else {
                let mut vm = result.unwrap();
                assert!(matches!(
                    vm.call(&[]).unwrap(),
                    VmState::Finished(Value::Int(7))
                ));
            }
        }
    }
}

#[test]
fn reset_restart_cannot_reuse_operands_from_before_stream() {
    let mut module = compile("loop main(x: Word) -> Word { yield x }");
    module.chunks[0].ops = vec![
        Op::PushImmediate(11),
        Op::PushImmediate(12),
        Op::Stream,
        Op::PopN(2),
        Op::PushImmediate(11),
        Op::Yield,
        Op::PopN(1),
        Op::Reset,
    ];
    assert!(keleusma::verify::verify(&module).is_ok());
    assert_eq!(
        keleusma::trap_analysis::classify_traps(&module).unwrap(),
        keleusma::trap_analysis::TrapVerdict::TrapFree
    );
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let (mut checked, _) =
        Vm::new_with_options(module.clone(), &arena, options(ExecutionChecks::Checked)).unwrap();
    assert!(matches!(
        checked.call(&[Value::Int(0)]).unwrap(),
        VmState::Yielded(Value::Int(7))
    ));
    assert!(matches!(
        checked.resume(Value::Int(0)).unwrap(),
        VmState::Reset
    ));
    match Vm::new_with_options(module, &arena, options(ExecutionChecks::Proven)) {
        Err(VmError::VerifyError(message)) => {
            assert!(message.contains("operand-pop proof"), "{message}")
        }
        Err(other) => panic!("wrong refusal {other:?}"),
        Ok(_) => panic!("restart proof unexpectedly succeeded"),
    }
}

#[test]
fn peek_and_range_guards_have_execution_controls() {
    for ops in [
        vec![
            Op::PopN(0),
            Op::PushImmediate(7),
            Op::Dup,
            Op::CheckedAdd,
            Op::PopN(2),
            Op::Return,
        ],
        vec![
            Op::PushImmediate(11),
            Op::IsStruct(0),
            Op::PopN(1),
            Op::Return,
        ],
        vec![
            Op::PushImmediate(11),
            Op::IsEnum(0, 1, 2),
            Op::PopN(1),
            Op::Return,
        ],
    ] {
        let mut module = compile("fn main() -> Word { 0 }");
        module.chunks[0].constants = vec![
            ConstValue::StaticStr("E".into()),
            ConstValue::StaticStr("A".into()),
            ConstValue::Int(0),
        ];
        module.chunks[0].ops = ops;
        let checked = run(module.clone(), ExecutionChecks::Checked, &[]).unwrap();
        let proven = run(module, ExecutionChecks::Proven, &[]).unwrap();
        assert_eq!(checked, proven);
    }
    for op in [Op::Dup, Op::PopN(1)] {
        let mut module = compile("fn main() -> Word { 0 }");
        module.chunks[0].ops = vec![op, Op::PushImmediate(11), Op::Return];
        let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        assert!(Vm::new_with_options(module, &arena, options(ExecutionChecks::Proven)).is_err());
    }
}

#[test]
fn completed_calls_and_named_entrypoints_keep_the_certificate() {
    let module =
        compile("fn helper(x: Word) -> Word { x + 1 } fn main(x: Word) -> Word { helper(x) }");
    let helper = module
        .chunks
        .iter()
        .position(|c| c.name == "helper")
        .unwrap();
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let (mut vm, _) =
        Vm::new_with_options(module, &arena, options(ExecutionChecks::Proven)).unwrap();
    for n in 0..3 {
        assert!(
            matches!(vm.call(&[Value::Int(n)]).unwrap(), VmState::Finished(Value::Int(v)) if v == n + 1)
        );
        assert!(
            matches!(vm.call_function(helper, &[Value::Int(n)]).unwrap(), VmState::Finished(Value::Int(v)) if v == n + 1)
        );
    }
}

#[cfg(feature = "signatures")]
#[test]
fn authenticated_loading_and_replacement_reprove_execution() {
    let signer = ed25519_dalek::SigningKey::from_bytes(&[71; 32]);
    let keys = [signer.verifying_key()];
    let module = compile("fn main(x: Word) -> Word { x + 1 }");
    let bytes = keleusma::wire_format::module_to_signed_wire_bytes(&module, &signer).unwrap();
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let (mut vm, _) =
        Vm::load_signed_bytes_with_options(&bytes, &arena, &keys, options(ExecutionChecks::Proven))
            .unwrap();
    assert!(matches!(
        vm.call(&[Value::Int(3)]).unwrap(),
        VmState::Finished(Value::Int(4))
    ));
    vm.replace_module_from_bytes(&bytes, vec![]).unwrap();
    assert_eq!(vm.execution_checks(), ExecutionChecks::Proven);
    assert_eq!(vm.verifying_keys_len(), 1);
    assert!(matches!(
        vm.call(&[Value::Int(5)]).unwrap(),
        VmState::Finished(Value::Int(6))
    ));
    #[cfg(feature = "encryption")]
    {
        let private = [72; 32];
        let public = keleusma::encryption::public_key_from_private(&private);
        let encrypted = keleusma::wire_format::module_to_encrypted_signed_wire_bytes(
            &module, &signer, &public, &[73; 32],
        )
        .unwrap();
        let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (mut vm, _) = Vm::load_encrypted_signed_bytes_with_options(
            &encrypted,
            &arena,
            &keys,
            &private,
            options(ExecutionChecks::Proven),
        )
        .unwrap();
        assert_eq!(vm.execution_checks(), ExecutionChecks::Proven);
        assert!(matches!(
            vm.call(&[Value::Int(5)]).unwrap(),
            VmState::Finished(Value::Int(6))
        ));
    }
}

#[test]
fn proven_execution_preserves_an_eight_bit_runtime_layout() {
    let ast = keleusma::parser::parse(
        &keleusma::lexer::tokenize("fn main(x: Word) -> Word { x + 7 }").unwrap(),
    )
    .unwrap();
    let module =
        keleusma::compiler::compile_with_target(&ast, &keleusma::target::Target::embedded_8())
            .unwrap();
    for checks in [ExecutionChecks::Checked, ExecutionChecks::Proven] {
        let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (mut vm, _) = keleusma::vm::GenericVm::<i8, u16, f32>::new_with_options(
            module.clone(),
            &arena,
            options(checks),
        )
        .unwrap();
        for input in [-128_i8, -1, 0, 127] {
            assert!(
                matches!(vm.call(&[keleusma::GenericValue::Int(input)]).unwrap(), keleusma::vm::GenericVmState::Finished(keleusma::GenericValue::Int(value)) if value == input.wrapping_add(7))
            );
        }
    }
}

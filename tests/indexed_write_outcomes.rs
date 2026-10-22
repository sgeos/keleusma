//! Indexed writes expose failure without touching storage.
#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::Arena;
use keleusma::bytecode::{ConstValue, Module, Op, Value};
use keleusma::vm::{Vm, VmError, VmState};

fn compile(source: &str) -> Module {
    keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap(),
    )
    .unwrap()
}

#[test]
fn indexed_store_returns_a_flag_and_never_writes_on_failure() {
    for byte in [false, true] {
        for execution_checks in [
            keleusma::vm::ExecutionChecks::Checked,
            keleusma::vm::ExecutionChecks::Proven,
        ] {
            for (base, len, index) in [
                (0, 2, -1_i64),
                (0, 2, 0),
                (0, 2, 1),
                (0, 2, 2),
                (0, 2, i64::MAX),
                (2, 0, 0),
            ] {
                let mut module = compile(if byte {
                    "shared data d { x: [Byte; 2] } fn main() -> Word { 0 }"
                } else {
                    "shared data d { x: [Word; 2] } fn main() -> Word { 0 }"
                });
                module.chunks[0].constants = vec![
                    if byte {
                        ConstValue::Byte(7)
                    } else {
                        ConstValue::Int(7)
                    },
                    ConstValue::Int(index),
                ];
                module.chunks[0].ops = vec![
                    Op::Const(0),
                    Op::Const(1),
                    Op::SetDataIndexed(base, len),
                    Op::Return,
                ];
                let arena = Arena::with_capacity(16384);
                let (mut vm, _) = Vm::new_with_options(
                    module,
                    &arena,
                    keleusma::vm::VmOptions {
                        trap_policy: keleusma::vm::TrapPolicy::RequireTrapFree,
                        execution_checks,
                        ..keleusma::vm::VmOptions::default()
                    },
                )
                .unwrap();
                let mut shared = vec![0; vm.shared_data_bytes()];
                let invalid = index < 0 || index >= i64::from(len);
                assert!(
                    matches!(vm.call_with_shared(&mut shared, &[]).unwrap(), VmState::Finished(Value::Int(flag)) if flag == i64::from(invalid))
                );
                for slot in 0..2 {
                    let n = if !invalid && slot == base as usize + index as usize {
                        7
                    } else {
                        0
                    };
                    assert_eq!(
                        vm.get_shared(&shared, slot).unwrap(),
                        if byte {
                            Value::Byte(n as u8)
                        } else {
                            Value::Int(n)
                        }
                    );
                }
            }
        }
    }
}

#[cfg(feature = "self-host")]
#[test]
fn both_compilers_preserve_store_artifacts_and_explicit_failures() {
    for (source, expected) in [
        (
            "shared data d { x: [Word; 2] } fn main() -> Word { d.x[1] = 7; d.x[1] }",
            Ok(7_i64),
        ),
        (
            "shared data d { x: [Word; 2] } fn main() -> Word { d.x[-1] = 7; 0 }",
            Err(-1_i64),
        ),
        (
            "shared data d { x: [Byte; 2] } fn main() -> Word { d.x[2] = 7 as Byte; 0 }",
            Err(2_i64),
        ),
        (
            // 19 until 2026-10-21, when the value was evaluated first. The
            // operator moved every indexed write to index-first evaluation, so
            // the trace is 2 then 21, plus the stored 7.
            "private data d { x: [Word; 2], trace: Word } fn idx() -> Word { d.trace = d.trace * 10 + 2; 1 } fn val() -> Word { d.trace = d.trace * 10 + 1; 7 } fn main() -> Word { d.x[idx()] = val(); d.trace + d.x[1] }",
            Ok(28_i64),
        ),
    ] {
        let reference = compile(source);
        let actual =
            keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host())
                .unwrap();
        assert_eq!(
            actual.to_bytes().unwrap(),
            reference.to_bytes().unwrap(),
            "{source}"
        );
        assert!(
            actual
                .chunks
                .iter()
                .flat_map(|c| &c.ops)
                .any(|op| *op == Op::Trap(8)),
            "{source}"
        );
        let need = keleusma::vm::required_persistent_capacity_for(&actual);
        let ops = actual
            .chunks
            .iter()
            .map(|c| c.ops.clone())
            .collect::<Vec<_>>();
        let mut arena = Arena::with_capacity(16384 + need);
        arena.resize_persistent(need).unwrap();
        let mut vm = Vm::new(actual, &arena).unwrap();
        let mut shared = vec![0; vm.shared_data_bytes()];
        match vm.call_with_shared(&mut shared, &[]) {
            Ok(VmState::Finished(Value::Int(n))) => assert_eq!(expected, Ok(n), "{source}"),
            Err(VmError::IndexOutOfBounds(index, 2)) => {
                assert_eq!(expected, Err(index), "{source}");
                let (chunk, ip) = vm.fault_location().unwrap();
                assert_eq!(ops[chunk][ip], Op::Trap(8));
                assert!(shared.iter().all(|&b| b == 0));
            }
            other => panic!("{source}: {other:?}"),
        }
    }
}

#[test]
fn handled_private_store_failure_can_continue_to_a_valid_write() {
    for byte in [false, true] {
        let mut module = compile(if byte {
            "private data d { x: [Byte; 2] } fn main() -> Byte { d.x[0] = 1 as Byte; 0 as Byte }"
        } else {
            "private data d { x: [Word; 2] } fn main() -> Word { d.x[0] = 1; 0 }"
        });
        module.chunks[0].constants = vec![
            if byte {
                ConstValue::Byte(7)
            } else {
                ConstValue::Int(7)
            },
            ConstValue::Int(-1),
            ConstValue::Int(1),
        ];
        module.chunks[0].ops = vec![
            Op::Const(0),
            Op::Const(1),
            Op::SetDataIndexed(0, 2),
            Op::PopN(6),
            Op::Const(0),
            Op::Const(2),
            Op::SetDataIndexed(0, 2),
            Op::PopN(6),
            Op::GetData(1),
            Op::Return,
        ];
        let need = keleusma::vm::required_persistent_capacity_for(&module);
        let mut arena = Arena::with_capacity(16384 + need);
        arena.resize_persistent(need).unwrap();
        let mut vm = Vm::new(module, &arena).unwrap();
        match vm.call(&[]).unwrap() {
            VmState::Finished(value) => {
                assert_eq!(value, if byte { Value::Byte(7) } else { Value::Int(7) })
            }
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn malformed_store_spans_and_missing_operands_are_rejected() {
    for ops in [
        vec![Op::Const(0), Op::SetDataIndexed(0, 2), Op::Return],
        vec![
            Op::Const(0),
            Op::Const(0),
            Op::SetDataIndexed(2, 1),
            Op::Return,
        ],
    ] {
        let mut module = compile("shared data d { x: [Word; 2] } fn main() -> Word { 0 }");
        module.chunks[0].constants = vec![ConstValue::Int(0)];
        module.chunks[0].ops = ops;
        assert!(keleusma::verify::verify(&module).is_err());
        #[cfg(feature = "self-host")]
        assert!(matches!(
            keleusma::selfhost::verify_module_via_kel(&module).unwrap(),
            keleusma::selfhost::VerificationVerdict::Rejected { .. }
        ));
    }
}

#[test]
fn valid_store_outcomes_are_accepted_by_both_verifiers() {
    let mut module = compile("shared data d { x: [Word; 2] } fn main() -> Word { 0 }");
    module.chunks[0].constants = vec![ConstValue::Int(0)];
    module.chunks[0].ops = vec![
        Op::Const(0),
        Op::Const(0),
        Op::SetDataIndexed(0, 2),
        Op::Return,
    ];
    assert!(keleusma::verify::verify(&module).is_ok());
    #[cfg(feature = "self-host")]
    assert_eq!(
        keleusma::selfhost::verify_module_via_kel(&module).unwrap(),
        keleusma::selfhost::VerificationVerdict::Accepted
    );
    assert_eq!(
        keleusma::verify::op_depth_effect(&Op::SetDataIndexed(0, 2), &module.chunks[0]),
        (2, 4)
    );
    assert_eq!(Op::SetDataIndexed(0, 2).stack_growth(), 4);
    assert_eq!(Op::SetDataIndexed(0, 2).stack_shrink(), 0);
}

#[test]
fn a_nonword_store_index_remains_a_contract_error() {
    for execution_checks in [
        keleusma::vm::ExecutionChecks::Checked,
        keleusma::vm::ExecutionChecks::Proven,
    ] {
        let mut module = compile("shared data d { x: [Word; 2] } fn main() -> Word { 0 }");
        module.chunks[0].constants = vec![ConstValue::Int(7), ConstValue::Bool(false)];
        module.chunks[0].ops = vec![
            Op::Const(0),
            Op::Const(1),
            Op::SetDataIndexed(0, 2),
            Op::Return,
        ];
        let arena = Arena::with_capacity(16384);
        let (mut vm, _) = Vm::new_with_options(
            module,
            &arena,
            keleusma::vm::VmOptions {
                trap_policy: keleusma::vm::TrapPolicy::RequireTrapFree,
                execution_checks,
                ..keleusma::vm::VmOptions::default()
            },
        )
        .unwrap();
        let mut shared = vec![0; vm.shared_data_bytes()];
        assert!(matches!(
            vm.call_with_shared(&mut shared, &[]),
            Err(VmError::TypeError(_))
        ));
        assert!(shared.iter().all(|&byte| byte == 0));
        if execution_checks == keleusma::vm::ExecutionChecks::Proven {
            assert!(matches!(
                vm.resume_with_shared(&mut shared, Value::Int(0)),
                Err(VmError::VerifyError(_))
            ));
        }
    }
}

#[test]
fn indexed_store_diagnostics_work_on_eight_bit_words() {
    let ast = keleusma::parser::parse(
        &keleusma::lexer::tokenize("shared data d { x: [Word; 2] } fn main() -> Word { 0 }")
            .unwrap(),
    )
    .unwrap();
    let mut module =
        keleusma::compiler::compile_with_target(&ast, &keleusma::target::Target::embedded_8())
            .unwrap();
    module.chunks[0].constants = vec![ConstValue::Int(7), ConstValue::Int(-1)];
    module.chunks[0].ops = vec![
        Op::Const(0),
        Op::Const(1),
        Op::SetDataIndexed(0, 2),
        Op::PopN(1),
        Op::Trap(8),
    ];
    let arena = Arena::with_capacity(16384);
    let mut vm: keleusma::vm::GenericVm<'_, '_, i8, u16, f32> =
        keleusma::vm::GenericVm::new(module, &arena).unwrap();
    let mut shared = vec![0; vm.shared_data_bytes()];
    assert!(matches!(
        vm.call_with_shared(&mut shared, &[]),
        Err(VmError::IndexOutOfBounds(-1, 2))
    ));
    assert!(shared.iter().all(|&byte| byte == 0));
}

#[cfg(feature = "self-host")]
#[test]
fn text_after_an_indexed_store_keeps_module_flags() {
    let source = "shared data d { x: [Word; 1] } fn main() -> Text { d.x[0] = 7; \"retained\" }";
    let reference = compile(source);
    let actual =
        keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host()).unwrap();
    assert_eq!(actual.to_bytes().unwrap(), reference.to_bytes().unwrap());
}

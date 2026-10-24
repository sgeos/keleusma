//! Legacy division opcodes use explicit outcomes, including hand-built bytecode.
#![cfg(all(feature = "compile", feature = "verify"))]
use keleusma::Arena;
use keleusma::bytecode::{ConstValue, Module, Op, Value};
use keleusma::vm::{Vm, VmState};

fn module(operation: Op, numerator: ConstValue, denominator: ConstValue, drop: u8) -> Module {
    let ast =
        keleusma::parser::parse(&keleusma::lexer::tokenize("fn main() -> Word { 0 }").unwrap())
            .unwrap();
    let mut module = keleusma::compiler::compile(&ast).unwrap();
    module.chunks[0].constants = vec![numerator, denominator];
    module.chunks[0].ops = vec![
        Op::Const(0),
        Op::Const(1),
        operation,
        Op::PopN(drop),
        Op::Return,
    ];
    module
}

#[test]
fn legacy_integer_division_reifies_zero_and_preserves_the_numerator() {
    for operation in [Op::Div, Op::Mod] {
        for drop in [0, 2] {
            let m = module(operation, ConstValue::Int(7), ConstValue::Int(0), drop);
            let arena = Arena::with_capacity(16384);
            let mut vm = Vm::new(m, &arena).unwrap();
            let expected = if drop == 0 { 3 } else { 7 };
            assert!(
                matches!(vm.call(&[]).unwrap(), VmState::Finished(Value::Int(n)) if n == expected)
            );
        }
    }
}

fn execute(m: Module, args: &[Value]) -> Result<Value, keleusma::vm::VmError> {
    let need = keleusma::vm::required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(keleusma::vm::DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).unwrap();
    let mut vm = Vm::new(m, &arena)?;
    match vm.call(args)? {
        VmState::Finished(value) => Ok(value),
        other => panic!("unexpected state {other:?}"),
    }
}

#[test]
fn legacy_numeric_outcomes_match_the_checked_family() {
    for (legacy, checked) in [(Op::Div, Op::CheckedDiv(0)), (Op::Mod, Op::CheckedMod)] {
        for a in [i64::MIN, -7, 0, 7, i64::MAX] {
            for b in [-1, 0, 1, 2] {
                for drop in 0..=2 {
                    let expected = execute(
                        module(checked, ConstValue::Int(a), ConstValue::Int(b), drop),
                        &[],
                    )
                    .unwrap();
                    let actual = execute(
                        module(legacy, ConstValue::Int(a), ConstValue::Int(b), drop),
                        &[],
                    )
                    .unwrap();
                    assert_eq!(actual, expected, "{legacy:?} {a}/{b}, output {drop}");
                }
            }
        }
        for b in [0, 2 << 16] {
            for drop in 0..=2 {
                let expected = execute(
                    module(
                        checked,
                        ConstValue::Fixed(7 << 16),
                        ConstValue::Fixed(b),
                        drop,
                    ),
                    &[],
                )
                .unwrap();
                let actual = execute(
                    module(
                        legacy,
                        ConstValue::Fixed(7 << 16),
                        ConstValue::Fixed(b),
                        drop,
                    ),
                    &[],
                )
                .unwrap();
                assert_eq!(actual, expected);
            }
        }
        for a in [0, 7, 255] {
            for b in [0, 2, 255] {
                for drop in 0..=2 {
                    let expected = execute(
                        module(checked, ConstValue::Byte(a), ConstValue::Byte(b), drop),
                        &[],
                    )
                    .unwrap();
                    let actual = execute(
                        module(legacy, ConstValue::Byte(a), ConstValue::Byte(b), drop),
                        &[],
                    )
                    .unwrap();
                    assert_eq!(actual, expected);
                }
            }
        }
    }
}

#[test]
fn legacy_division_observes_declared_and_runtime_word_widths() {
    let ast =
        keleusma::parser::parse(&keleusma::lexer::tokenize("fn main() -> Word { 0 }").unwrap())
            .unwrap();
    for (target, min) in [
        (keleusma::target::Target::embedded_8(), -128),
        (keleusma::target::Target::embedded_16(), -32768),
    ] {
        for (operation, expected) in [(Op::Div, min), (Op::Mod, 0)] {
            let mut m = keleusma::compiler::compile_with_target(&ast, &target).unwrap();
            let fixture = module(operation, ConstValue::Int(min), ConstValue::Int(-1), 2);
            m.chunks[0].constants = fixture.chunks[0].constants.clone();
            m.chunks[0].ops = fixture.chunks[0].ops.clone();
            assert_eq!(execute(m.clone(), &[]).unwrap(), Value::Int(expected));
            if min == -128 {
                let arena = Arena::with_capacity(16384);
                let mut vm: keleusma::vm::GenericVm<'_, '_, i8, u16, f32> =
                    keleusma::vm::GenericVm::new(m, &arena).unwrap();
                assert!(
                    matches!(vm.call(&[]).unwrap(), keleusma::vm::GenericVmState::Finished(keleusma::bytecode::GenericValue::Int(n)) if i64::from(n) == expected)
                );
            }
        }
    }
}

#[cfg(feature = "floats")]
#[test]
fn legacy_float_outcomes_preserve_ieee_results() {
    for operation in [Op::Div, Op::Mod] {
        for denominator in [0.0, 2.0] {
            let result = execute(
                module(
                    operation,
                    ConstValue::Float(7.0),
                    ConstValue::Float(denominator),
                    2,
                ),
                &[],
            )
            .unwrap();
            let Value::Float(result) = result else {
                panic!("Float expected")
            };
            let expected = if operation == Op::Div {
                7.0 / denominator
            } else {
                7.0 % denominator
            };
            assert!(result == expected || (result.is_nan() && expected.is_nan()));
            let flag = if result.is_nan() {
                4
            } else if result.is_infinite() {
                1
            } else {
                0
            };
            assert_eq!(
                execute(
                    module(
                        operation,
                        ConstValue::Float(7.0),
                        ConstValue::Float(denominator),
                        0
                    ),
                    &[]
                )
                .unwrap(),
                Value::Int(flag)
            );
        }
    }
    assert!(matches!(
        execute(
            module(
                Op::CheckedMod,
                ConstValue::Float(7.0),
                ConstValue::Float(2.0),
                2
            ),
            &[]
        ),
        Err(keleusma::vm::VmError::TypeError(_))
    ));
}

#[test]
fn fixed_remainder_source_runs_and_traps_explicitly() {
    let source = "fn main(a: Fixed<16>, b: Fixed<16>) -> Fixed<16> { a % b }";
    let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
    let reference = keleusma::compiler::compile(&ast).unwrap();
    assert!(reference.chunks[0].ops.contains(&Op::Trap(5)));
    assert_eq!(
        execute(
            reference.clone(),
            &[Value::Fixed(7 << 16), Value::Fixed(2 << 16)]
        )
        .unwrap(),
        Value::Fixed(1 << 16)
    );
    assert!(matches!(
        execute(reference.clone(), &[Value::Fixed(7 << 16), Value::Fixed(0)]),
        Err(keleusma::vm::VmError::DivisionByZero)
    ));
    #[cfg(feature = "self-host")]
    {
        let actual =
            keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host())
                .unwrap();
        assert_eq!(actual.to_bytes().unwrap(), reference.to_bytes().unwrap());
        assert_eq!(
            execute(
                actual.clone(),
                &[Value::Fixed(7 << 16), Value::Fixed(2 << 16)]
            )
            .unwrap(),
            Value::Fixed(1 << 16)
        );
        assert!(matches!(
            execute(actual, &[Value::Fixed(7 << 16), Value::Fixed(0)]),
            Err(keleusma::vm::VmError::DivisionByZero)
        ));
    }
}

#[test]
fn malformed_operands_remain_contract_errors() {
    for operation in [Op::Div, Op::Mod] {
        assert!(matches!(
            execute(
                module(operation, ConstValue::Bool(true), ConstValue::Int(0), 2),
                &[]
            ),
            Err(keleusma::vm::VmError::TypeError(_))
        ));
        assert!(matches!(
            execute(
                module(operation, ConstValue::Byte(7), ConstValue::Int(0), 2),
                &[]
            ),
            Err(keleusma::vm::VmError::TypeError(_))
        ));
    }
}

#[test]
fn legacy_outcomes_survive_wire_round_trip_and_both_verifiers() {
    for operation in [Op::Div, Op::Mod] {
        let m = module(operation, ConstValue::Int(7), ConstValue::Int(0), 2);
        let bytes = m.to_bytes().unwrap();
        let decoded = Module::from_bytes(&bytes).unwrap();
        assert_eq!(execute(decoded, &[]).unwrap(), Value::Int(7));
        #[cfg(feature = "self-host")]
        {
            assert_eq!(
                keleusma::selfhost::verify_module_via_kel(&m).unwrap(),
                keleusma::selfhost::VerificationVerdict::Accepted
            );
            assert_eq!(
                keleusma::selfhost::wire_module_artifact_via_kel(&m).unwrap(),
                bytes
            );
        }
        assert_eq!(
            keleusma::verify::op_depth_effect(&operation, &m.chunks[0]),
            (2, 1)
        );
        assert_eq!(operation.stack_growth(), 1);
        assert_eq!(operation.stack_shrink(), 0);
        assert_eq!(operation.cost(), 5);
    }
}

#[test]
fn multiword_zero_divisors_reach_the_explicit_trap() {
    for fraction in [0, 32] {
        for operator in ["/", "%"] {
            let source = format!(
                "fn main() -> Word {{ let a = (7, 0) as Multiword<2, {fraction}>; let b = (0, 0) as Multiword<2, {fraction}>; let r = a {operator} b; r[0] }}"
            );
            let ast =
                keleusma::parser::parse(&keleusma::lexer::tokenize(&source).unwrap()).unwrap();
            let m = keleusma::compiler::compile(&ast).unwrap();
            let ops = &m.chunks[0].ops;
            assert!(ops.contains(&Op::Trap(5)));
            assert!(!ops.contains(&Op::Div));
            assert!(matches!(
                execute(m, &[]),
                Err(keleusma::vm::VmError::DivisionByZero)
            ));
        }
    }
}

/// `FixedDiv` reports a zero divisor as the outcome triple of a Fixed `Mod`
/// since 2026-10-22, instead of faulting. The fraction-bit count is sixteen.
#[test]
fn fixed_division_reifies_zero_and_saturates_with_flag_zero() {
    let fixed = |bits: i64| ConstValue::Fixed(bits);
    // (numerator, denominator, (low, high, flag))
    for (a, b, (low, high, flag)) in [
        // Zero divisor: the numerator, zero, and flag 3.
        (7 << 16, 0, (7 << 16, 0, 3)),
        // 7 / 2 = 3.5 in Q16.
        (7 << 16, 2 << 16, (229_376, 0, 0)),
        // A quotient beyond the word saturates and reports flag 0.
        (i64::MAX, 1, (i64::MAX, 0, 0)),
        (i64::MIN, 1, (i64::MIN, 0, 0)),
    ] {
        for (drop, expected) in [
            (0, Value::Int(flag)),
            (1, Value::Fixed(high)),
            (2, Value::Fixed(low)),
        ] {
            let actual = execute(module(Op::FixedDiv(16), fixed(a), fixed(b), drop), &[]).unwrap();
            assert_eq!(actual, expected, "{a} / {b}, output {drop}");
        }
    }
}

/// Source Fixed division reaches the compiler's explicit zero-divisor trap by
/// default, and a handled division receives the numerator instead.
#[test]
fn source_fixed_division_traps_by_default_and_is_handled_on_request() {
    let compile = |source: &str| {
        let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
        keleusma::compiler::compile(&ast).unwrap()
    };
    let one = Value::Fixed(1 << 16);
    let zero = Value::Fixed(0);
    let bare = compile("fn main(a: Fixed<16>, b: Fixed<16>) -> Fixed<16> { a / b }");
    assert!(
        bare.chunks
            .iter()
            .flat_map(|c| &c.ops)
            .any(|op| matches!(op, Op::Trap(_)))
    );
    assert_eq!(
        execute(
            bare.clone(),
            &[Value::Fixed(6 << 16), Value::Fixed(4 << 16)]
        )
        .unwrap(),
        Value::Fixed(98_304)
    );
    assert!(matches!(
        execute(bare, &[one.clone(), zero.clone()]),
        Err(keleusma::vm::VmError::DivisionByZero)
    ));
    let handled = compile(
        "fn main(a: Fixed<16>, b: Fixed<16>) -> Fixed<16> { a / b { ok(v) => v, zero_divisor(n) => n } }",
    );
    assert_eq!(execute(handled, &[one.clone(), zero]).unwrap(), one);
}

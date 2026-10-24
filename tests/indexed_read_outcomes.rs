//! Total Word and Byte array reads and their explicit source traps.
#![cfg(all(feature = "compile", feature = "verify"))]
use keleusma::Arena;
use keleusma::bytecode::{ArrayElem, ConstValue, Module, NewCompositeOperand, Op, Value};
use keleusma::value_layout::{CompositeKind, ScalarKind};
use keleusma::vm::{Vm, VmError, VmState};
fn compile(source: &str) -> Module {
    keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap(),
    )
    .unwrap()
}
fn fixture(byte: bool, empty: bool, index: i64, flag: bool) -> Module {
    let mut m = compile("fn main() -> Word { 0 }");
    m.chunks[0].constants = vec![
        if byte {
            ConstValue::Byte(7)
        } else {
            ConstValue::Int(7)
        },
        ConstValue::Int(index),
    ];
    let count = if empty { 0 } else { 1 };
    let mut ops = if empty { vec![] } else { vec![Op::Const(0)] };
    ops.extend([
        Op::NewComposite(NewCompositeOperand::Flat {
            kind: CompositeKind::Array,
            count,
            byte_size: count * if byte { 1 } else { 8 },
        }),
        Op::Const(1),
        Op::GetIndex(ArrayElem::Flat {
            kind: if byte {
                ScalarKind::Byte
            } else {
                ScalarKind::Int
            },
        }),
    ]);
    if !flag {
        ops.push(Op::PopN(6));
    }
    ops.push(Op::Return);
    m.chunks[0].ops = ops;
    m
}
#[test]
fn scalar_array_reads_return_flags_and_typed_fallbacks() {
    for byte in [false, true] {
        for empty in [false, true] {
            for index in [-1, 0, 1, 1_i64 << 32, i64::MAX] {
                let invalid = empty || index != 0;
                for flag in [false, true] {
                    let arena = Arena::with_capacity(16384);
                    let mut vm = Vm::new(fixture(byte, empty, index, flag), &arena).unwrap();
                    let expected = if flag {
                        Value::Int(i64::from(invalid))
                    } else if byte {
                        Value::Byte(if invalid { 0 } else { 7 })
                    } else {
                        Value::Int(if invalid { 0 } else { 7 })
                    };
                    assert!(
                        matches!(vm.call(&[]).unwrap(),VmState::Finished(actual) if actual==expected)
                    );
                }
            }
        }
    }
}
#[test]
fn source_index_handlers_omit_unreachable_traps() {
    for source in [
        "fn main() -> Word { [7][2] { ok(v) => v, invalid_index(i) => i } }",
        "fn main() -> Byte { [7 as Byte][2] { ok(v) => v, invalid_index(_) => 3 as Byte } }",
        "fn main() -> bool { [7][2] { ok(_) => true, invalid_index(_) => false } }",
    ] {
        let module = compile(source);
        assert!(
            !module
                .chunks
                .iter()
                .flat_map(|c| &c.ops)
                .any(|op| matches!(op, Op::Trap(_))),
            "{source}"
        );
        let arena = Arena::with_capacity(16384);
        let mut vm = Vm::new(module, &arena).unwrap();
        assert!(matches!(vm.call(&[]).unwrap(), VmState::Finished(_)));
    }
}
#[cfg(feature = "self-host")]
#[test]
fn both_bare_lowerings_preserve_artifacts_and_faults() {
    for (source, expected) in [
        ("fn main() -> Word { let a = [7, 9]; a[1] }", Ok(9_i64)),
        ("fn main() -> Word { let a = [7, 9]; a[-1] }", Err(-1_i64)),
        ("fn main() -> Word { let a = [7, 9]; a[2] }", Err(2_i64)),
    ] {
        let reference = compile(source);
        let actual =
            keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host())
                .unwrap();
        assert_eq!(actual.to_bytes().unwrap(), reference.to_bytes().unwrap());
        let ops = actual.chunks[0].ops.clone();
        let arena = Arena::with_capacity(16384);
        let mut vm = Vm::new(actual, &arena).unwrap();
        match vm.call(&[]) {
            Ok(VmState::Finished(Value::Int(n))) => assert_eq!(expected, Ok(n)),
            Err(VmError::IndexOutOfBounds(i, 2)) => {
                assert_eq!(expected, Err(i));
                let (_, ip) = vm.fault_location().unwrap();
                assert_eq!(ops[ip], Op::Trap(8));
            }
            other => panic!("{other:?}"),
        }
    }
}

#[cfg(feature = "self-host")]
#[test]
fn checked_read_sources_match_reference_artifacts_and_execution() {
    fn execute(module: Module) -> String {
        let need = keleusma::vm::required_persistent_capacity_for(&module);
        let mut arena = Arena::with_capacity(32768 + need);
        arena.resize_persistent(need).unwrap();
        let mut vm = Vm::new(module, &arena).unwrap();
        format!("{:?}", vm.call(&[]))
    }
    for source in [
        "fn main() -> Word { [7][2] { ok(v) => v, invalid_index(i) => i } }",
        "fn main() -> Byte { [7 as Byte][2] { ok(v) => v, invalid_index(_) => 3 as Byte } }",
        "fn main() -> bool { [7][2] { ok(_) => true, invalid_index(_) => false } }",
        "fn main() -> Word { let a = [7, 9]; a[-1] { invalid_index(i) => i, ok(v) => v } }",
        "fn main() -> Word { let a = [7, 9]; a[1] { ok(v) => v } }",
        "fn main() -> Word { let a = [7, 9]; a[2] { ok(v) => v } }",
        "fn main() -> Word { let a = [7, 9]; a[2] { ok(v) => v, invalid_index(i) when i == 3 => i } }",
        "fn main() -> Word { let a = [7, 9]; a[2] { ok(v) => v, invalid_index(2) => 3, invalid_index(_) => 4 } }",
        "fn main() -> Word { let a = [7, 9]; a[2] { ok(v) => v, invalid_index(3) => 3 } }",
        "fn main() -> Word { let a = [7, 9]; a[2] { ok(v) => v, invalid_index(i) => a[-1] { ok(v) => v, invalid_index(j) => i + j } } }",
        "fn main() -> Byte { let a = [7 as Byte]; a[2] { ok(v) => v, invalid_index(i) when i == 2 => 9 as Byte, invalid_index(_) => 3 as Byte } }",
    ] {
        let reference = compile(source);
        let actual =
            keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host())
                .unwrap_or_else(|e| panic!("{source}\n{e}"));
        assert_eq!(
            actual.to_bytes().unwrap(),
            reference.to_bytes().unwrap(),
            "{source}"
        );
        assert_eq!(execute(actual), execute(reference), "{source}");
    }
}

#[cfg(feature = "self-host")]
#[test]
fn parser_rejects_invalid_index_outcome_shapes_independently() {
    for source in [
        "fn main() -> Word { [7][1] { ok(v) => v, overflow(h, l) => l } }",
        "fn main() -> Word { [7][1] { ok(v) => v, invalid_index(h, l) => l } }",
        "fn main() -> Word { [7][1] { invalid_index(i) => i } }",
        "fn main() -> Word { [7][1] { ok(v) => v, invalid_index(i) => i, invalid_index(_) => 0 } }",
        "fn main() -> Word { 1 + 2 { ok(v) => v, invalid_index(i) => i } }",
        "fn main() -> Word { [7][1] { ok(v) => v, invalid_index(i) => i }; i }",
    ] {
        assert!(
            keleusma::selfhost::try_parse_functions(source).is_err(),
            "{source}"
        );
    }
}

#[test]
fn guarded_and_literal_handlers_keep_the_explicit_failure() {
    for source in [
        "fn main() -> Word { [7][2] { ok(v) => v } }",
        "fn main() -> Word { [7][2] { ok(v) => v, invalid_index(1) => 9 } }",
        "fn main() -> Word { [7][2] { ok(v) => v, invalid_index(i) when i == 1 => 9 } }",
    ] {
        let m = compile(source);
        let ops = m.chunks[0].ops.clone();
        let arena = Arena::with_capacity(16384);
        let mut vm = Vm::new(m, &arena).unwrap();
        assert!(matches!(vm.call(&[]), Err(VmError::IndexOutOfBounds(2, 1))));
        let (_, ip) = vm.fault_location().unwrap();
        assert_eq!(ops[ip], Op::Trap(8));
    }
}

#[test]
fn scalar_array_diagnostics_survive_an_eight_bit_runtime() {
    let ast =
        keleusma::parser::parse(&keleusma::lexer::tokenize("fn main() -> Word { 0 }").unwrap())
            .unwrap();
    let mut m =
        keleusma::compiler::compile_with_target(&ast, &keleusma::target::Target::embedded_8())
            .unwrap();
    m.chunks[0].constants = vec![ConstValue::Int(7), ConstValue::Int(-1)];
    m.chunks[0].ops = vec![
        Op::Const(0),
        Op::NewComposite(NewCompositeOperand::Flat {
            kind: CompositeKind::Array,
            count: 1,
            byte_size: 1,
        }),
        Op::Const(1),
        Op::GetIndex(ArrayElem::Flat {
            kind: ScalarKind::Int,
        }),
        Op::PopN(1),
        Op::Trap(8),
    ];
    let arena = Arena::with_capacity(16384);
    let mut vm: keleusma::vm::GenericVm<'_, '_, i8, u16, f32> =
        keleusma::vm::GenericVm::new(m, &arena).unwrap();
    assert!(matches!(
        vm.call(&[]),
        Err(VmError::IndexOutOfBounds(-1, 1))
    ));
}

#[test]
fn bare_read_trap_preserves_the_exact_source_span() {
    let source = "fn main() -> Word { let a = [7]; a[2] }";
    let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
    let m = keleusma::compiler::compile_with_options(
        &ast,
        &keleusma::target::Target::host(),
        &keleusma::compiler::CompileOptions { emit_debug: true },
    )
    .unwrap()
    .0;
    let arena = Arena::with_capacity(16384);
    let mut vm = Vm::new(m, &arena).unwrap();
    assert!(matches!(vm.call(&[]), Err(VmError::IndexOutOfBounds(2, 1))));
    let location = vm.fault_source_location().unwrap();
    assert!(location.exact);
    assert_eq!(location.byte_offset as usize, source.find("a[2]").unwrap());
    assert_eq!(location.byte_length, 4);
}

#[test]
fn malformed_array_read_operands_are_not_language_outcomes() {
    let mut wrong_index = fixture(false, false, 0, true);
    wrong_index.chunks[0].constants[1] = ConstValue::Bool(false);
    let arena = Arena::with_capacity(16384);
    let mut vm = Vm::new(wrong_index, &arena).unwrap();
    assert!(matches!(vm.call(&[]), Err(VmError::TypeError(_))));
    let mut wrong_stride = fixture(true, false, 0, true);
    wrong_stride.chunks[0].ops[3] = Op::GetIndex(ArrayElem::Flat {
        kind: ScalarKind::Int,
    });
    assert!(keleusma::verify::verify(&wrong_stride).is_err());
    #[cfg(feature = "self-host")]
    assert!(matches!(
        keleusma::selfhost::verify_module_via_kel(&wrong_stride).unwrap(),
        keleusma::selfhost::VerificationVerdict::Rejected { .. }
    ));
}

#[test]
fn operand_forms_keep_an_explicit_first_pass_boundary() {
    assert!(
        ArrayElem::Flat {
            kind: ScalarKind::Int
        }
        .has_bounds_outcome()
    );
    assert!(
        ArrayElem::Flat {
            kind: ScalarKind::Byte
        }
        .has_bounds_outcome()
    );
    assert!(
        !ArrayElem::Flat {
            kind: ScalarKind::Bool
        }
        .has_bounds_outcome()
    );
    // Nested composite and boxed reads joined the total contract on
    // 2026-10-24. Other flat scalar kinds keep the faulting contract.
    assert!(ArrayElem::Boxed.has_bounds_outcome());
    assert!(
        ArrayElem::FlatNested {
            size: 8,
            variant: CompositeKind::Array
        }
        .has_bounds_outcome()
    );
    let m = compile("fn main() -> Word { 0 }");
    assert_eq!(
        keleusma::verify::op_depth_effect(
            &Op::GetIndex(ArrayElem::Flat {
                kind: ScalarKind::Int
            }),
            &m.chunks[0]
        ),
        (2, 5)
    );
    assert_eq!(
        keleusma::verify::op_depth_effect(
            &Op::GetIndex(ArrayElem::Flat {
                kind: ScalarKind::Bool
            }),
            &m.chunks[0]
        ),
        (2, -1)
    );
}

#[test]
fn malformed_host_array_stride_keeps_its_runtime_guard_in_both_modes() {
    use keleusma::vm::{ExecutionChecks, TrapPolicy, VmOptions};
    let source = "fn main(a: [Word; 1]) -> Word { a[0] { ok(v) => v, invalid_index(_) => 0 } }";
    for execution_checks in [ExecutionChecks::Checked, ExecutionChecks::Proven] {
        let arena = Arena::with_capacity(65536);
        let (mut vm, _) = Vm::new_with_options(
            compile(source),
            &arena,
            VmOptions {
                trap_policy: TrapPolicy::RequireTrapFree,
                execution_checks,
                ..VmOptions::default()
            },
        )
        .unwrap();
        let body = keleusma::flat_value::FlatComposite::build_in_arena(&arena, 3, |bytes| {
            bytes.fill(0);
            Ok(())
        })
        .unwrap()
        .unwrap();
        let value = Value::Array(keleusma::bytecode::ArrayBody::Flat(body));
        assert!(
            matches!(vm.call(&[value]), Err(VmError::InvalidBytecode(message)) if message.contains("multiple of its element stride"))
        );
    }
}

//! Indexed data reads carry an explicit fallback value for every span size.
#![cfg(all(feature = "compile", feature = "verify"))]
use keleusma::Arena;
use keleusma::bytecode::{ConstValue, Module, Op, Value};
use keleusma::vm::{Vm, VmError, VmState};
fn compile(src: &str) -> Module {
    keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(src).unwrap()).unwrap(),
    )
    .unwrap()
}
#[test]
fn data_read_preserves_fallback_kind_and_reifies_invalid_indices() {
    for byte in [false, true] {
        for (base, len, index) in [
            (0, 2, -1_i64),
            (0, 2, 0),
            (0, 2, 1),
            (0, 2, 2),
            (0, 2, 1_i64 << 32),
            (2, 0, 0),
        ] {
            for flag in [false, true] {
                let mut m = compile(if byte {
                    "shared data d { x: [Byte; 2] } fn main() -> Word { 0 }"
                } else {
                    "shared data d { x: [Word; 2] } fn main() -> Word { 0 }"
                });
                m.chunks[0].constants = vec![
                    ConstValue::Int(index),
                    if byte {
                        ConstValue::Byte(77)
                    } else {
                        ConstValue::Int(77)
                    },
                ];
                m.chunks[0].ops = vec![Op::Const(0), Op::Const(1), Op::GetDataIndexed(base, len)];
                if !flag {
                    m.chunks[0].ops.push(Op::PopN(6));
                }
                m.chunks[0].ops.push(Op::Return);
                let arena = Arena::with_capacity(16384);
                let mut vm = Vm::new(m, &arena).unwrap();
                let mut shared = vec![0; vm.shared_data_bytes()];
                for slot in 0..2 {
                    vm.set_shared(
                        &mut shared,
                        slot,
                        if byte {
                            Value::Byte(9 + slot as u8)
                        } else {
                            Value::Int(9 + slot as i64)
                        },
                    )
                    .unwrap();
                }
                let invalid = index < 0 || index >= i64::from(len);
                let value = if invalid { 77 } else { 9 + index };
                let expected = if flag {
                    Value::Int(i64::from(invalid))
                } else if byte {
                    Value::Byte(value as u8)
                } else {
                    Value::Int(value)
                };
                assert!(
                    matches!(vm.call_with_shared(&mut shared,&[]).unwrap(),VmState::Finished(actual) if actual==expected)
                );
            }
        }
    }
}
#[cfg(feature = "self-host")]
#[test]
fn data_reads_and_handlers_match_both_compilers() {
    for (source, expected) in [
        (
            "shared data d { x: [Word; 2] } fn main() -> Word { d.x[0] }",
            Ok(0_i64),
        ),
        (
            "shared data d { x: [Word; 2] } fn main() -> Word { d.x[-1] }",
            Err(-1_i64),
        ),
        (
            "shared data d { x: [Word; 2] } fn main() -> Word { d.x[2] { ok(v) => v, invalid_index(i) => i } }",
            Ok(2_i64),
        ),
        (
            "shared data d { x: [Byte; 2] } fn main() -> Word { d.x[2] { ok(v) => v as Word, invalid_index(i) => i } }",
            Ok(2_i64),
        ),
        (
            "shared data d { x: [Word; 2] } fn main() -> Word { d.x[2] { ok(v) => v, invalid_index(i) when i == 3 => 9 } }",
            Err(2_i64),
        ),
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
        let ops = actual.chunks[0].ops.clone();
        let arena = Arena::with_capacity(32768);
        let mut vm = Vm::new(actual, &arena).unwrap();
        let mut shared = vec![0; vm.shared_data_bytes()];
        match vm.call_with_shared(&mut shared, &[]) {
            Ok(VmState::Finished(Value::Int(v))) => assert_eq!(expected, Ok(v)),
            Err(VmError::IndexOutOfBounds(i, 2)) => {
                assert_eq!(expected, Err(i));
                let (_, ip) = vm.fault_location().unwrap();
                assert_eq!(ops[ip], Op::Trap(8));
            }
            other => panic!("{source}: {other:?}"),
        }
    }
}

#[test]
fn empty_data_span_needs_no_declared_storage() {
    let mut m = compile("fn main() -> Byte { 0 as Byte }");
    m.chunks[0].constants = vec![ConstValue::Int(0), ConstValue::Byte(7)];
    m.chunks[0].ops = vec![
        Op::Const(0),
        Op::Const(1),
        Op::GetDataIndexed(0, 0),
        Op::PopN(6),
        Op::Return,
    ];
    assert!(keleusma::verify::verify(&m).is_ok());
    #[cfg(feature = "self-host")]
    assert_eq!(
        keleusma::selfhost::verify_module_via_kel(&m).unwrap(),
        keleusma::selfhost::VerificationVerdict::Accepted
    );
    let arena = Arena::with_capacity(16384);
    let mut vm = Vm::new(m, &arena).unwrap();
    assert!(matches!(
        vm.call(&[]).unwrap(),
        VmState::Finished(Value::Byte(7))
    ));
}

#[test]
fn invalid_fallback_and_span_remain_contract_failures() {
    for fallback in [
        ConstValue::Bool(false),
        ConstValue::StaticStr("invalid".into()),
    ] {
        let mut m = compile("fn main() -> Word { 0 }");
        m.chunks[0].constants = vec![ConstValue::Int(0), fallback];
        m.chunks[0].ops = vec![
            Op::Const(0),
            Op::Const(1),
            Op::GetDataIndexed(0, 0),
            Op::Return,
        ];
        let arena = Arena::with_capacity(16384);
        let mut vm = Vm::new(m, &arena).unwrap();
        assert!(matches!(vm.call(&[]), Err(VmError::TypeError(_))));
    }
    let mut m = compile("fn main() -> Word { 0 }");
    m.chunks[0].ops = vec![
        Op::Const(0),
        Op::Const(0),
        Op::GetDataIndexed(1, 0),
        Op::Return,
    ];
    assert!(keleusma::verify::verify(&m).is_err());
    #[cfg(feature = "self-host")]
    assert!(matches!(
        keleusma::selfhost::verify_module_via_kel(&m).unwrap(),
        keleusma::selfhost::VerificationVerdict::Rejected { .. }
    ));
}

#[test]
fn data_read_diagnostics_survive_eight_bit_words() {
    let ast =
        keleusma::parser::parse(&keleusma::lexer::tokenize("fn main() -> Word { 0 }").unwrap())
            .unwrap();
    let mut m =
        keleusma::compiler::compile_with_target(&ast, &keleusma::target::Target::embedded_8())
            .unwrap();
    m.chunks[0].constants = vec![ConstValue::Int(-1), ConstValue::Int(0)];
    m.chunks[0].ops = vec![
        Op::Const(0),
        Op::Const(1),
        Op::GetDataIndexed(0, 0),
        Op::PopN(1),
        Op::Trap(8),
    ];
    let arena = Arena::with_capacity(16384);
    let mut vm: keleusma::vm::GenericVm<'_, '_, i8, u16, f32> =
        keleusma::vm::GenericVm::new(m, &arena).unwrap();
    assert!(matches!(
        vm.call(&[]),
        Err(VmError::IndexOutOfBounds(-1, 0))
    ));
}

#[cfg(feature = "self-host")]
#[test]
fn checked_data_reads_have_no_fallback_trap_when_covered() {
    for source in [
        "shared data d { x: [Word; 2] } fn main() -> Word { d.x[2] { invalid_index(i) => i, ok(v) => v } }",
        "shared data d { x: [Byte; 2] } fn main() -> Byte { d.x[2] { ok(v) => v, invalid_index(2) => 7 as Byte, invalid_index(_) => 0 as Byte } }",
        "shared data d { x: [Word; 2] } fn main() -> Word { d.x[2] { ok(v) => v, invalid_index(i) => d.x[-1] { ok(v) => v, invalid_index(j) => i + j } } }",
    ] {
        let reference = compile(source);
        let actual =
            keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host())
                .unwrap();
        assert_eq!(actual.to_bytes().unwrap(), reference.to_bytes().unwrap());
        assert!(
            !actual
                .chunks
                .iter()
                .flat_map(|c| &c.ops)
                .any(|op| matches!(op, Op::Trap(_))),
            "{source}"
        );
    }
}

#[cfg(feature = "self-host")]
#[test]
fn private_data_read_observes_index_effects_once() {
    let source = "private data d { x: [Word; 2], count: Word } fn idx() -> Word { d.x[0] = 9; d.count = d.count + 1; 0 } fn main() -> Word { let n = d.x[idx()] { ok(v) => v, invalid_index(_) => 0 }; n + d.count }";
    let reference = compile(source);
    let actual =
        keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host()).unwrap();
    assert_eq!(actual.to_bytes().unwrap(), reference.to_bytes().unwrap());
    let need = keleusma::vm::required_persistent_capacity_for(&actual);
    let mut arena = Arena::with_capacity(32768 + need);
    arena.resize_persistent(need).unwrap();
    let mut vm = Vm::new(actual, &arena).unwrap();
    assert!(matches!(
        vm.call(&[]).unwrap(),
        VmState::Finished(Value::Int(10))
    ));
}

#[cfg(feature = "self-host")]
#[test]
fn opcode_profile_accepts_its_exact_bound_and_refuses_one_more() {
    let mut m = compile("fn main() -> Word { 0 }");
    m.chunks[0].ops = vec![Op::Return; keleusma::selfhost_host::VERIFY_OP_CAP];
    assert_eq!(
        keleusma::selfhost::verify_module_via_kel(&m).unwrap(),
        keleusma::selfhost::VerificationVerdict::Accepted
    );
    m.chunks[0].ops.push(Op::Return);
    assert!(matches!(
        keleusma::selfhost::verify_module_via_kel(&m),
        Err(keleusma::selfhost::VerificationError::Capacity {
            resource: "opcodes",
            ..
        })
    ));
}

#[test]
fn a_narrow_runtime_preserves_a_bound_larger_than_its_word() {
    let ast = keleusma::parser::parse(
        &keleusma::lexer::tokenize("shared data d { x: [Byte; 256] } fn main() -> Word { 0 }")
            .unwrap(),
    )
    .unwrap();
    let mut m =
        keleusma::compiler::compile_with_target(&ast, &keleusma::target::Target::embedded_8())
            .unwrap();
    m.chunks[0].constants = vec![ConstValue::Int(-1), ConstValue::Byte(0)];
    m.chunks[0].ops = vec![
        Op::Const(0),
        Op::Const(1),
        Op::GetDataIndexed(0, 256),
        Op::PopN(1),
        Op::Trap(8),
    ];
    let arena = Arena::with_capacity(32768);
    let mut vm: keleusma::vm::GenericVm<'_, '_, i8, u16, f32> =
        keleusma::vm::GenericVm::new(m, &arena).unwrap();
    let mut shared = vec![0; vm.shared_data_bytes()];
    assert!(matches!(
        vm.call_with_shared(&mut shared, &[]),
        Err(VmError::IndexOutOfBounds(-1, 256))
    ));
}

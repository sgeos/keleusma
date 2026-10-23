//! Bounds predicates return outcomes; explicit traps carry portable diagnostics.
#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::Arena;
use keleusma::bytecode::{ConstValue, Module, Op, Value};
use keleusma::target::Target;
use keleusma::vm::{Vm, VmError, VmState};

fn fixture(ops: Vec<Op>, constants: Vec<ConstValue>, target: Target) -> Module {
    let ast =
        keleusma::parser::parse(&keleusma::lexer::tokenize("fn main() -> Word { 0 }").unwrap())
            .unwrap();
    let mut module = keleusma::compiler::compile_with_target(&ast, &target).unwrap();
    module.chunks[0].ops = ops;
    module.chunks[0].constants = constants;
    module
}

#[test]
fn bounds_predicate_reifies_all_boundary_outcomes() {
    for bound in [0_u16, 1, 3, u16::MAX] {
        for index in [-1_i64, 0, 1, i64::from(bound), i64::MAX] {
            let module = fixture(
                vec![Op::Const(0), Op::BoundsCheck(bound), Op::Return],
                vec![ConstValue::Int(index)],
                Target::host(),
            );
            let arena = Arena::with_capacity(16384);
            let mut vm = Vm::new(module, &arena).unwrap();
            let expected = i64::from(index < 0 || index >= i64::from(bound));
            match vm.call(&[]).unwrap() {
                VmState::Finished(Value::Int(flag)) => {
                    assert_eq!(flag, expected, "{index}, {bound}")
                }
                other => panic!("{other:?}"),
            }
        }
    }
}

#[test]
fn bounds_predicate_preserves_the_index_below_its_outcome() {
    for index in [-1_i64, 0, 3, i64::MAX] {
        let module = fixture(
            vec![Op::Const(0), Op::BoundsCheck(3), Op::PopN(5), Op::Return],
            vec![ConstValue::Int(index)],
            Target::host(),
        );
        let arena = Arena::with_capacity(16384);
        let mut vm = Vm::new(module, &arena).unwrap();
        match vm.call(&[]).unwrap() {
            VmState::Finished(Value::Int(actual)) => assert_eq!(actual, index),
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn bounds_trap_requires_all_five_payload_words() {
    for words in 0..5 {
        let mut ops = vec![Op::Const(0); words];
        ops.push(Op::Trap(8));
        let module = fixture(ops, vec![ConstValue::Int(0)], Target::host());
        assert!(
            keleusma::verify::verify(&module).is_err(),
            "payload length {words}"
        );
        #[cfg(feature = "self-host")]
        assert!(matches!(
            keleusma::selfhost::verify_module_via_kel(&module).unwrap(),
            keleusma::selfhost::VerificationVerdict::Rejected { .. }
        ));
    }
}

#[test]
fn portable_bounds_trap_round_trips_the_diagnostic() {
    for bound in [0_u32, 3, 65535, 0x1234ffff, u32::MAX] {
        let constants = core::iter::once(ConstValue::Int(-7))
            .chain(
                bound
                    .to_le_bytes()
                    .map(|byte| ConstValue::Int(i64::from(byte as i8))),
            )
            .collect();
        let mut ops: Vec<_> = (0..5).map(Op::Const).collect();
        ops.push(Op::Trap(8));
        let module = fixture(ops, constants, Target::embedded_8());
        let arena = Arena::with_capacity(16384);
        let mut vm: keleusma::vm::GenericVm<'_, '_, i8, u16, f32> =
            keleusma::vm::GenericVm::new(module, &arena).unwrap();
        assert!(
            matches!(vm.call(&[]), Err(VmError::IndexOutOfBounds(-7, actual)) if actual == bound as usize)
        );
    }
}

#[test]
fn predicate_payload_survives_an_eight_bit_word_runtime() {
    let module = fixture(
        vec![
            Op::Const(0),
            Op::BoundsCheck(u16::MAX),
            Op::PopN(1),
            Op::Trap(8),
        ],
        vec![ConstValue::Int(-1)],
        Target::embedded_8(),
    );
    let arena = Arena::with_capacity(16384);
    let mut vm: keleusma::vm::GenericVm<'_, '_, i8, u16, f32> =
        keleusma::vm::GenericVm::new(module, &arena).unwrap();
    assert!(matches!(
        vm.call(&[]),
        Err(VmError::IndexOutOfBounds(-1, 65535))
    ));
}

#[test]
fn malformed_operand_kinds_remain_runtime_contract_errors() {
    for ops in [
        vec![Op::Const(0), Op::BoundsCheck(3), Op::Return],
        vec![Op::Const(0); 5]
            .into_iter()
            .chain([Op::Trap(8)])
            .collect(),
    ] {
        let module = fixture(ops, vec![ConstValue::Bool(false)], Target::host());
        let arena = Arena::with_capacity(16384);
        let mut vm = Vm::new(module, &arena).unwrap();
        assert!(matches!(vm.call(&[]), Err(VmError::TypeError(_))));
    }
}

#[test]
fn per_dimension_source_failures_reach_explicit_traps() {
    for source in [
        "private data d { a: [[Word; 2]; 2] } fn main() -> Word { d.a[0][0] = 1; d.a[-1][0] }",
        "private data d { a: [[Word; 2]; 2] } fn main() -> Word { d.a[0][2] = 7; 0 }",
        "private data d { a: [[Byte; 2]; 2] } fn main() -> Byte { d.a[0][0] = 1 as Byte; d.a[0][2] }",
    ] {
        let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
        let module = keleusma::compiler::compile(&ast).unwrap();
        let ops = module.chunks[0].ops.clone();
        #[cfg(feature = "self-host")]
        assert_eq!(
            keleusma::selfhost::verify_module_via_kel(&module).unwrap(),
            keleusma::selfhost::VerificationVerdict::Accepted
        );
        let need = keleusma::vm::required_persistent_capacity_for(&module);
        let mut arena = Arena::with_capacity(16384 + need);
        arena.resize_persistent(need).unwrap();
        let mut vm = Vm::new(module, &arena).unwrap();
        assert!(
            matches!(vm.call(&[]), Err(VmError::IndexOutOfBounds(_, 2))),
            "{source}"
        );
        let (chunk, ip) = vm.fault_location().unwrap();
        assert_eq!(chunk, 0);
        assert_eq!(ops[ip], Op::Trap(8));
    }
}

#[test]
fn terminal_payloads_cannot_consume_an_enclosing_loop_frame() {
    for (inside, accepted) in [(4_usize, false), (5, true)] {
        let mut ops = vec![Op::Const(0), Op::Loop(0)];
        ops.extend(vec![Op::Const(0); inside]);
        ops.extend([Op::Trap(8), Op::EndLoop(2), Op::Return]);
        ops[1] = Op::Loop((ops.len() - 1) as u16);
        let module = fixture(ops, vec![ConstValue::Int(0)], Target::host());
        assert_eq!(keleusma::verify::verify(&module).is_ok(), accepted);
        #[cfg(feature = "self-host")]
        assert_eq!(
            matches!(
                keleusma::selfhost::verify_module_via_kel(&module).unwrap(),
                keleusma::selfhost::VerificationVerdict::Accepted
            ),
            accepted
        );
    }
}

#[test]
fn valid_multidimensional_access_preserves_results() {
    let source =
        "private data d { a: [[Word; 2]; 2] } fn main() -> Word { d.a[1][1] = 37; d.a[1][1] }";
    let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
    let module = keleusma::compiler::compile(&ast).unwrap();
    let need = keleusma::vm::required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(16384 + need);
    arena.resize_persistent(need).unwrap();
    let mut vm = Vm::new(module, &arena).unwrap();
    assert!(matches!(
        vm.call(&[]).unwrap(),
        VmState::Finished(Value::Int(37))
    ));
}

#[test]
fn wire_round_trip_keeps_the_new_outcome_contract() {
    let module = fixture(
        vec![
            Op::Const(0),
            Op::BoundsCheck(65535),
            Op::PopN(1),
            Op::Trap(8),
        ],
        vec![ConstValue::Int(-1)],
        Target::host(),
    );
    let bytes = module.to_bytes().unwrap();
    let decoded = Module::from_bytes(&bytes).unwrap();
    assert_eq!(decoded.chunks[0].ops, module.chunks[0].ops);
    let arena = Arena::with_capacity(16384);
    let mut vm = Vm::new(decoded, &arena).unwrap();
    assert!(matches!(
        vm.call(&[]),
        Err(VmError::IndexOutOfBounds(-1, 65535))
    ));
    #[cfg(feature = "self-host")]
    {
        let encoded = keleusma::selfhost::wire_module_artifact_via_kel(&module).unwrap();
        assert_eq!(encoded, bytes);
        let decoded = keleusma::selfhost::wire_decode_module_via_kel(&encoded).unwrap();
        assert_eq!(decoded.to_bytes().unwrap(), bytes);
    }
}

#[test]
fn bounds_metadata_agrees_with_operand_effects() {
    let module = fixture(
        vec![Op::Const(0), Op::Return],
        vec![ConstValue::Int(0)],
        Target::host(),
    );
    for (op, req, net, growth, shrink, cost) in [
        (Op::BoundsCheck(3), 1, 5, 5, 0, 7),
        (Op::Trap(8), 5, -5, 0, 5, 6),
    ] {
        assert_eq!(
            keleusma::verify::op_depth_effect(&op, &module.chunks[0]),
            (req, net)
        );
        assert_eq!(op.stack_growth(), growth);
        assert_eq!(op.stack_shrink(), shrink);
        assert_eq!(keleusma::bytecode::nominal_op_cycles(&op), cost);
    }
}

#[test]
fn explicit_bounds_trap_has_an_exact_index_source_span() {
    let source =
        "private data d { a: [[Word; 2]; 2] } fn main() -> Word { d.a[0][0] = 1; d.a[3][0] }";
    let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
    let module = keleusma::compiler::compile_with_options(
        &ast,
        &Target::host(),
        &keleusma::compiler::CompileOptions { emit_debug: true },
    )
    .unwrap()
    .0;
    let need = keleusma::vm::required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(16384 + need);
    arena.resize_persistent(need).unwrap();
    let mut vm = Vm::new(module, &arena).unwrap();
    assert!(matches!(vm.call(&[]), Err(VmError::IndexOutOfBounds(3, 2))));
    let location = vm.fault_source_location().unwrap();
    assert!(location.exact);
    assert_eq!(
        location.byte_offset as usize,
        source.find("d.a[3]").unwrap() + 4
    );
    assert_eq!(location.byte_length, 1);
}

//! Record-level controls for the checked-arithmetic lowering stages.
use super::*;
use crate::vm::Vm;

fn assert_lowering(source: &str, records: &[(i64, i64)]) {
    let reference = compile_src(source);
    let body = reconstruct_via_kel(records, 0, 0, "main");
    let (ops, pool, local_count) = run_codegen(&body, 0);
    let chunk = &reference.chunks[0];
    assert_eq!(ops, chunk.ops, "{source}");
    assert_eq!(pool_to_constants(&pool, &[]), chunk.constants, "{source}");
    assert_eq!(local_count, i64::from(chunk.local_count), "{source}");
    let mut actual = reference.clone();
    actual.chunks[0].ops = ops;
    actual.chunks[0].constants = pool_to_constants(&pool, &[]);
    actual.chunks[0].local_count = local_count as u16;
    assert!(actual.to_bytes().unwrap() == reference.to_bytes().unwrap());
    crate::verify::verify(&actual).unwrap();
    let execute = |module: Module| {
        let need = required_persistent_capacity_for(&module);
        let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
        arena.resize_persistent(need).unwrap();
        let mut vm = Vm::new(module, &arena).unwrap();
        match vm.call(&[]) {
            Ok(VmState::Finished(Value::Int(n))) => (3, n),
            Ok(VmState::Finished(Value::Byte(n))) => (2, i64::from(n)),
            Err(crate::vm::VmError::DivisionByZero) => (-5, 0),
            other => panic!("unexpected result {other:?}"),
        }
    };
    assert_eq!(execute(actual), execute(reference), "{source}");
}

#[test]
fn checked_zero_divisor_records_match_reference_lowering() {
    let records = [
        (1, 7),
        (1, 0),
        (3, 4),
        (118, 3 * 65536),
        (125, 3 * 65536 + 1),
        (2, 3),
        (119, 0),
        (5, 3),
        (123, 0),
        (125, 3 * 65536 + 1),
        (2, 4),
        (119, 3),
        (5, 4),
        (123, 0),
        (124, 0),
    ];
    assert_lowering(
        "fn main() -> Word { 7 / 0 { ok(v) => v, zero_divisor(n) => n } }",
        &records,
    );
}

#[test]
fn guarded_zero_divisor_records_retain_the_reference_fallback() {
    let records = [
        (1, 7),
        (1, 0),
        (3, 4),
        (118, 3 * 65536),
        (125, 3 * 65536 + 1),
        (2, 3),
        (119, 0),
        (5, 3),
        (123, 0),
        (125, 3 * 65536 + 1),
        (2, 4),
        (1, 9),
        (3, 6),
        (2, 4),
        (119, 7),
        (5, 4),
        (123, 0),
        (124, 0),
    ];
    assert_lowering(
        "fn main() -> Word { 7 / 0 { ok(v) => v, zero_divisor(n) when n == 9 => n } }",
        &records,
    );
}

#[test]
fn word_overflow_records_bind_high_then_low() {
    let records = [
        (1, i64::MAX),
        (1, 1),
        (3, 1),
        (118, 3 * 65536),
        (125, 3 * 65536 + 2),
        (125, 3 * 65536 + 1),
        (2, 3),
        (2, 4),
        (3, 1),
        (119, 1),
        (5, 4),
        (5, 3),
        (123, 0),
        (125, 3 * 65536 + 1),
        (2, 5),
        (119, 0),
        (5, 5),
        (123, 0),
        (124, 0),
    ];
    assert_lowering(
        "fn main() -> Word { 9223372036854775807 + 1 { overflow(h, l) => h + l, ok(v) => v } }",
        &records,
    );
}

#[test]
fn byte_overflow_records_preserve_the_single_byte_binding() {
    let records = [
        (1, 255),
        (26, 1),
        (1, 1),
        (26, 1),
        (45, 1),
        (118, 2 * 65536),
        (125, 2 * 65536 + 1),
        (2, 3),
        (119, 0),
        (5, 3),
        (123, 0),
        (125, 2 * 65536 + 1),
        (1, 7),
        (26, 1),
        (119, 1),
        (5, 4),
        (123, 0),
        (124, 0),
    ];
    assert_lowering(
        "fn main() -> Byte { (255 as Byte) + (1 as Byte) { ok(v) => v, overflow(n) => 7 as Byte } }",
        &records,
    );
}

#[test]
fn literal_patterns_keep_the_zero_divisor_fallback() {
    for numerator in [7, 8] {
        let records = [
            (1, numerator),
            (1, 0),
            (3, 5),
            (118, 3 * 65536),
            (125, 3 * 65536 + 1),
            (2, 3),
            (119, 0),
            (5, 3),
            (123, 0),
            (1, 9),
            (119, 11),
            (120, 7),
            (123, 0),
            (124, 0),
        ];
        assert_lowering(
            &format!(
                "fn main() -> Word {{ {numerator} % 0 {{ ok(v) => v, zero_divisor(7) => 9 }} }}"
            ),
            &records,
        );
    }
}

#[test]
fn every_word_arithmetic_operation_uses_the_checked_result_contract() {
    for (operator, code) in [("+", 1), ("*", 2), ("-", 3), ("/", 4), ("%", 5)] {
        let records = [
            (1, 7),
            (1, 2),
            (3, code),
            (118, 3 * 65536),
            (125, 3 * 65536 + 1),
            (2, 3),
            (119, 0),
            (5, 3),
            (123, 0),
            (124, 0),
        ];
        assert_lowering(
            &format!("fn main() -> Word {{ 7 {operator} 2 {{ ok(v) => v }} }}"),
            &records,
        );
    }
    assert_lowering(
        "fn main() -> Word { -7 { ok(v) => v } }",
        &[
            (1, 7),
            (10, 0),
            (118, 3 * 65536),
            (125, 3 * 65536 + 1),
            (2, 3),
            (119, 0),
            (5, 3),
            (123, 0),
            (124, 0),
        ],
    );
}

fn reconstruct_code(records: &[(i64, i64)]) -> i64 {
    let module = reconstruct_kel_module();
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).unwrap();
    let mut vm = Vm::new(module, &arena).unwrap();
    let mut shared = seed_reconstruct_shared(&vm, records, 0, 0);
    assert!(matches!(
        vm.call_with_shared(&mut shared, &[Value::Int(0)]),
        Ok(VmState::Yielded(Value::Int(_)))
    ));
    match vm.get_shared(&shared, RC_ERR_CODE).unwrap() {
        Value::Int(n) => n,
        other => panic!("{other:?}"),
    }
}

#[test]
fn malformed_checked_records_fail_through_the_stage_diagnostic() {
    let prefix = [(1, 7), (1, 0), (3, 4), (118, 3 * 65536)];
    for suffix in [
        vec![(1, 9), (119, 0), (123, 0), (122, 0)], // Reserved tag cannot close.
        vec![(1, 9), (119, 8), (126, -1)],          // Negative complement is malformed.
        vec![(1, 9), (119, 16), (127, -1)],
        vec![],                                     // Unclosed context.
        vec![(124, 0)],                             // No exhaustive ok arm.
        vec![(123, 0)],                             // No arm wrapper.
        vec![(1, 9), (119, 8), (123, 0)],           // Missing literal payload.
        vec![(1, 9), (119, 0), (120, 7), (123, 0)], // Unexpected literal.
        vec![(1, 9), (119, 2), (123, 0)],           // Division cannot underflow.
    ] {
        let records: Vec<_> = prefix.into_iter().chain(suffix).collect();
        assert_eq!(reconstruct_code(&records), 6, "{records:?}");
    }
    assert_eq!(reconstruct_code(&[(1, 7), (118, 3 * 65536)]), 6);
    assert_eq!(reconstruct_code(&[(1, 7), (123, 0)]), 6);
}

#[test]
fn checked_arm_storage_accepts_its_boundary_and_refuses_the_next_arm() {
    for (arms, expected) in [(50, 0), (51, 6)] {
        let mut records = vec![(1, 7), (1, 2), (3, 1), (118, 3 * 65536)];
        for index in 0..arms - 1 {
            records.extend([(1, 7), (119, 8), (120, index), (123, 0)]);
        }
        records.extend([(1, 7), (119, 0), (123, 0), (124, 0)]);
        assert_eq!(reconstruct_code(&records), expected);
        if arms == 50 {
            let mut arms_source = (0..49).map(|i| format!("ok({i}) => 7")).collect::<Vec<_>>();
            arms_source.push("ok(_) => 7".into());
            assert_lowering(
                &format!(
                    "fn main() -> Word {{ 7 + 2 {{ {} }} }}",
                    arms_source.join(", ")
                ),
                &records,
            );
        }
    }
}

#[test]
fn nested_checked_records_keep_enclosing_bindings_and_temporaries() {
    let records = [
        (1, 7),
        (1, 0),
        (3, 4),
        (118, 3 * 65536),
        (125, 3 * 65536 + 1),
        (2, 3),
        (119, 0),
        (5, 3),
        (123, 0),
        (125, 3 * 65536 + 1),
        (2, 4),
        (1, 2),
        (3, 1),
        (118, 3 * 65536 + 5),
        (125, 3 * 65536 + 6),
        (2, 8),
        (119, 0),
        (5, 8),
        (123, 0),
        (124, 0),
        (119, 3),
        (5, 4),
        (123, 0),
        (124, 0),
    ];
    assert_lowering(
        "fn main() -> Word { 7 / 0 { ok(v) => v, zero_divisor(n) => n + 2 { ok(w) => w } } }",
        &records,
    );
}

#[test]
fn checked_records_expose_guard_and_result_type_constraints() {
    let records = [
        (1, 7),
        (1, 2),
        (3, 4),
        (118, 3 * 65536),
        (1, 42),
        (20, 1),
        (119, 4),
        (123, 0),
        (1, 9),
        (119, 0),
        (123, 0),
        (124, 0),
    ];
    let body = reconstruct_via_kel(&records, 0, 0, "main");
    let slots = alloc::collections::BTreeMap::new();
    let mut rows = Vec::new();
    let results = expression_rows_from_body(
        &body,
        |index| source_operand(&body, index, &slots, &[]),
        &mut rows,
        true,
    );
    assert!(rows.iter().any(|(kind, lhs, _)| *kind == 3 && lhs.0 == 1));
    assert!(
        rows.iter()
            .any(|(kind, lhs, rhs)| *kind == 4 && lhs.0 != rhs.0 && [lhs.0, rhs.0].contains(&2))
    );
    assert!(results.contains_key(&body.root));
    for scalar in [2, 3] {
        let binding = Body {
            nodes: vec![Node {
                kind: 80,
                arg: scalar * 65536 + 1,
                lhs: 0,
                rhs: 0,
            }],
            call_args: vec![],
            for_parts: vec![],
            match_parts: vec![],
            limit_parts: vec![],
            head_parts: vec![],
            category: 0,
            root: 0,
        };
        assert_eq!(
            source_operand(&binding, 0, &slots, &[]).0,
            scalar_tag_of_layout_kind(scalar)
        );
    }
}

#[test]
fn checked_shift_records_intern_the_multiplier_before_the_operand() {
    for amount in [0, 1, 62] {
        let records = [
            (1, 3),
            (1, amount),
            (3, 37),
            (118, 3 * 65536),
            (125, 3 * 65536 + 1),
            (2, 3),
            (119, 0),
            (5, 3),
            (123, 0),
            (124, 0),
        ];
        assert_lowering(
            &format!("fn main() -> Word {{ 3 asl {amount} {{ ok(v) => v }} }}"),
            &records,
        );
    }
}

#[test]
fn checked_shift_records_refuse_unproved_multipliers() {
    for (kind, amount, scalar, op) in [
        (1, -1, 3, 37),
        (1, 63, 3, 37),
        (1, 64, 3, 37),
        (2, 0, 3, 37),
        (1, 2, 2, 37),
        (1, 2, 3, 27),
        (1, 2, 3, 37 + 64),
    ] {
        let records = [(1, 3), (kind, amount), (3, op), (118, scalar * 65536)];
        assert_eq!(reconstruct_code(&records), 6, "{records:?}");
    }
}

#[test]
fn signed_literal_records_preserve_full_words_and_reject_negative_complements() {
    for value in [-1_i64, -i64::MAX] {
        assert_lowering(
            &format!("const data d {{ n: Word = {value} }} fn main() -> Word {{ d.n }}"),
            &[(128, !value)],
        );
    }
    assert_eq!(reconstruct_code(&[(128, -1)]), 7);
}

#[test]
fn checked_array_records_preserve_index_bindings_and_seven_temporaries() {
    assert_lowering(
        "fn main() -> Word { [7][2] { ok(v) => v, invalid_index(i) => i } }",
        &[
            (1, 7),
            (17, 8193),
            (1, 2),
            (31, 3),
            (118, 3 * 65536),
            (125, 3 * 65536 + 1),
            (2, 7),
            (119, 0),
            (5, 7),
            (123, 0),
            (125, 3 * 65536 + 2),
            (2, 8),
            (119, 1),
            (5, 8),
            (123, 0),
            (124, 0),
        ],
    );
    assert_lowering(
        "fn main() -> Byte { [7 as Byte][2] { ok(v) => v, invalid_index(i) => 3 as Byte } }",
        &[
            (1, 7),
            (26, 1),
            (17, 1025),
            (1, 2),
            (31, 2),
            (118, 2 * 65536),
            (125, 2 * 65536 + 1),
            (2, 7),
            (119, 0),
            (5, 7),
            (123, 0),
            (125, 3 * 65536 + 2),
            (1, 3),
            (26, 1),
            (119, 1),
            (5, 8),
            (123, 0),
            (124, 0),
        ],
    );
}

#[test]
fn malformed_index_outcome_records_fail_closed() {
    let prefix = [
        (1, 7),
        (26, 1),
        (17, 1025),
        (1, 2),
        (31, 2),
        (118, 2 * 65536),
        (1, 0),
        (26, 1),
        (119, 0),
        (123, 0),
    ];
    for suffix in [
        vec![
            (125, 2 * 65536 + 2),
            (1, 0),
            (26, 1),
            (119, 1),
            (5, 7),
            (123, 0),
            (124, 0),
        ],
        vec![
            (125, 3 * 65536 + 1),
            (1, 0),
            (26, 1),
            (119, 1),
            (5, 7),
            (123, 0),
            (124, 0),
        ],
        vec![(1, 0), (26, 1), (119, 17), (121, 0), (123, 0), (124, 0)],
    ] {
        let records: Vec<_> = prefix.into_iter().chain(suffix).collect();
        assert_eq!(reconstruct_code(&records), 6, "{records:?}");
    }
    assert_eq!(
        reconstruct_code(&[(1, 7), (17, 8193), (1, 2), (31, 3), (118, 3 * 65536 + 2042)]),
        6
    );
}

//! Production transport boundaries are errors, never language rejection verdicts.
#![cfg(feature = "self-host")]

use keleusma::selfhost::{
    CallTypeBatch, CallTypeInputs, TypeCheckError, type_check_inputs_via_kel,
    type_check_source_via_kel,
};

fn empty_batch() -> CallTypeBatch {
    CallTypeBatch {
        sites: Vec::new(),
        arguments: Vec::new(),
        bindings: Vec::new(),
        expressions: Vec::new(),
    }
}

fn input(batch: CallTypeBatch) -> CallTypeInputs {
    CallTypeInputs {
        declarations: vec![("f".into(), 0, vec![])],
        batches: vec![batch],
    }
}

/// Each selected column admits the authorized cap and explicitly refuses cap plus one.
#[test]
fn production_transport_checks_selected_cap_boundaries() {
    for (column, table) in [
        (0, "dparams"),
        (1, "csite"),
        (2, "ekind"),
        (3, "bname"),
        (4, "ekind"),
    ] {
        for count in [640, 641] {
            let mut inputs = input(empty_batch());
            match column {
                0 => {
                    inputs.declarations = (0..count).map(|i| (format!("f{i}"), 0, vec![])).collect()
                }
                1 => inputs.batches[0].sites = vec![("f".into(), 0); count],
                2 => {
                    inputs.batches[0].expressions =
                        vec![(8, (1, 0, String::new()), (1, 0, String::new())); count]
                }
                3 => {
                    inputs.batches[0].bindings = (0..count)
                        .map(|i| (format!("b{i}"), 1, 0, String::new()))
                        .collect()
                }
                4 => {
                    inputs.batches[0].expressions =
                        vec![(8, (1, 0, String::new()), (1, 0, String::new())); 639];
                    inputs.batches[0].arguments = vec![(1, (1, 0, String::new())); count - 639];
                }
                _ => unreachable!(),
            }
            match type_check_inputs_via_kel(&inputs) {
                Ok(true) if count == 640 => {}
                Err(TypeCheckError::Capacity {
                    table: actual_table,
                    actual,
                    maximum,
                }) if count == 641 => {
                    assert_eq!(actual_table, format!("ty.{table}"));
                    assert_eq!((actual, maximum), (641, 640));
                }
                other => panic!("column {column}, count {count}: {other:?}"),
            }
        }
    }
}

/// Malformed transport must not be presented as a rejection of the source language.
#[test]
fn production_transport_rejects_malformed_facts_as_errors() {
    for case in 0..8 {
        let mut inputs = input(empty_batch());
        let batch = &mut inputs.batches[0];
        match case {
            0 => inputs.declarations[0].1 = 1,
            1 => batch.sites.push(("missing".into(), 0)),
            2 => batch.sites.push(("f".into(), -1)),
            3 => batch.arguments.push((-1, (0, 0, String::new()))),
            4 => batch.arguments.push((1, (0, 2, String::new()))),
            5 => batch.bindings.push(("b".into(), 0, 2, String::new())),
            6 => batch
                .expressions
                .push((19, (0, 0, String::new()), (0, 0, String::new()))),
            7 => batch.bindings.push(("b".into(), -1, 0, String::new())),
            _ => unreachable!(),
        }
        assert!(
            matches!(
                type_check_inputs_via_kel(&inputs),
                Err(TypeCheckError::Protocol(_))
            ),
            "case {case}"
        );
    }
}

/// Identical local names in separate source heads cannot share private solver state.
#[test]
fn production_transport_isolates_batches_and_repeated_calls() {
    let mut first = empty_batch();
    first.bindings.push(("local".into(), 1, 0, String::new()));
    first.arguments.push((1, (0, 1, "local".into())));
    let mut second = empty_batch();
    second.bindings.push(("local".into(), 2, 0, String::new()));
    second.arguments.push((2, (0, 1, "local".into())));
    let mut inputs = input(first);
    inputs.batches.push(second);
    assert!(type_check_inputs_via_kel(&inputs).unwrap());
    inputs.batches[1].arguments[0].0 = 1;
    assert!(!type_check_inputs_via_kel(&inputs).unwrap());
    inputs.batches[1].arguments[0].0 = 2;
    assert!(type_check_inputs_via_kel(&inputs).unwrap());
}

/// An unsupported parser form is unavailable evidence rather than a source rejection.
#[test]
fn production_types_preserve_unavailable_parser_diagnostics() {
    let source = "fn main(n: bool) -> Word { match n { true => 1, false => 2 } }";
    match type_check_source_via_kel(source) {
        Err(TypeCheckError::Parse(error)) => assert!(!error.is_source_rejection()),
        other => panic!("expected unsupported pattern diagnostic, got {other:?}"),
    }
}

/// A known scalar array must retain both dimensions, including the 32-bit encoding boundary.
#[test]
fn production_array_type_tags_retain_element_and_length() {
    for element in [1, 2, 3] {
        for length in [0, 1, 2, 134_217_726] {
            let mut batch = empty_batch();
            batch
                .expressions
                .push((16, (element, 0, String::new()), (length, 0, String::new())));
            batch.bindings.push(("array".into(), 0, 2, String::new()));
            let expected = (length + 1) * 16 + element;
            batch.arguments.push((expected, (0, 1, "array".into())));
            let mut inputs = input(batch);
            assert!(type_check_inputs_via_kel(&inputs).unwrap());
            inputs.batches[0].arguments[0].0 = expected + 16;
            assert!(
                !type_check_inputs_via_kel(&inputs).unwrap(),
                "length {length}, element {element}"
            );
            inputs.batches[0].arguments[0].0 = (length + 1) * 16 + (element % 3 + 1);
            assert!(
                !type_check_inputs_via_kel(&inputs).unwrap(),
                "element {element}"
            );
        }
    }
}

/// Equal byte sizes cannot identify tuples, arrays or differently nested arrays.
#[test]
fn structural_type_identities_are_exact() {
    fn row(
        batch: &mut CallTypeBatch,
        kind: i64,
        a: keleusma::selfhost::ExprOperand,
        b: keleusma::selfhost::ExprOperand,
    ) -> keleusma::selfhost::ExprOperand {
        let index = batch.expressions.len();
        let name = format!("type{index}");
        batch.expressions.push((kind, a, b));
        batch
            .bindings
            .push((name.clone(), index as i64, 2, String::new()));
        (0, 1, name)
    }
    let atom = |tag| (tag, 0, String::new());
    for changed in [false, true] {
        let mut batch = empty_batch();
        let tail = row(&mut batch, 17, atom(2), atom(7));
        let first = row(&mut batch, 17, atom(1), tail);
        let tail = row(&mut batch, 17, atom(if changed { 1 } else { 2 }), atom(7));
        let second = row(&mut batch, 17, atom(1), tail);
        batch.expressions.push((8, first, second));
        assert_eq!(type_check_inputs_via_kel(&input(batch)).unwrap(), !changed);
    }
    for changed in [false, true] {
        let mut batch = empty_batch();
        let inner = row(&mut batch, 16, atom(1), atom(2));
        let first = row(&mut batch, 16, inner, atom(3));
        let inner = row(&mut batch, 16, atom(1), atom(if changed { 3 } else { 2 }));
        let second = row(&mut batch, 16, inner, atom(if changed { 2 } else { 3 }));
        batch.expressions.push((8, first, second));
        assert_eq!(type_check_inputs_via_kel(&input(batch)).unwrap(), !changed);
    }
    let mut batch = empty_batch();
    let tail = row(&mut batch, 17, atom(1), atom(7));
    let tuple = row(&mut batch, 17, atom(1), tail);
    let array = row(&mut batch, 16, atom(1), atom(2));
    batch.expressions.push((8, tuple, array));
    assert!(!type_check_inputs_via_kel(&input(batch)).unwrap());
}

/// Source-independent ordering can require more than four productive solver rounds.
#[test]
fn structural_dependencies_resolve_in_reverse_order() {
    let mut batch = empty_batch();
    for index in 0..24 {
        let tail = if index == 23 {
            (7, 0, String::new())
        } else {
            (0, 1, format!("b{}", index + 1))
        };
        batch.expressions.push((17, (1, 0, String::new()), tail));
        batch
            .bindings
            .push((format!("b{index}"), index, 2, String::new()));
    }
    batch.arguments.push((24 * 16 + 4, (0, 1, "b0".into())));
    let mut inputs = input(batch);
    assert!(type_check_inputs_via_kel(&inputs).unwrap());
    inputs.batches[0].arguments[0].0 = 1;
    assert!(
        !type_check_inputs_via_kel(&inputs).unwrap(),
        "unknown would incorrectly accept"
    );
}

#[test]
fn structural_construction_rows_obey_the_selected_capacity() {
    for count in [639, 640, 641] {
        let mut batch = empty_batch();
        for index in 0..count {
            batch.expressions.push((
                17,
                (index * 16 + 8, 0, String::new()),
                (7, 0, String::new()),
            ));
            batch
                .bindings
                .push((format!("b{index}"), index, 2, String::new()));
        }
        let mut inputs = input(batch);
        if count == 641 {
            assert!(matches!(
                type_check_inputs_via_kel(&inputs),
                Err(TypeCheckError::Capacity {
                    actual: 641,
                    maximum: 640,
                    ..
                })
            ));
        } else {
            assert!(type_check_inputs_via_kel(&inputs).unwrap());
            if count == 639 {
                inputs.batches[0].arguments.push((1, (0, 1, "b638".into())));
                assert!(
                    !type_check_inputs_via_kel(&inputs).unwrap(),
                    "last constructed identity must be known"
                );
            }
        }
    }
}

/// Projection rules must distinguish known composites from known scalar operands.
#[test]
fn projection_rules_admit_composite_identity_classes() {
    for kind in [5, 6] {
        for tag in [0, 1, 2, 3, 4, 17, 18, 19, 20, 22, 24, 25] {
            let mut batch = empty_batch();
            batch
                .expressions
                .push((kind, (tag, 0, String::new()), (0, 0, String::new())));
            assert_eq!(
                type_check_inputs_via_kel(&input(batch)).unwrap(),
                tag == 0 || tag >= 16,
                "kind {kind}, tag {tag}"
            );
        }
    }
}

/// Element projection follows structural identity rather than scalar layout width.
#[test]
fn array_projection_resolves_exact_element_types() {
    for element in [1, 2, 3, 24, 49] {
        let mut batch = empty_batch();
        batch
            .expressions
            .push((16, (element, 0, String::new()), (2, 0, String::new())));
        batch.bindings.push(("array".into(), 0, 2, String::new()));
        batch
            .expressions
            .push((18, (0, 1, "array".into()), (0, 0, String::new())));
        batch.bindings.push(("element".into(), 1, 2, String::new()));
        batch.arguments.push((element, (0, 1, "element".into())));
        let mut inputs = input(batch);
        assert!(type_check_inputs_via_kel(&inputs).unwrap());
        inputs.batches[0].arguments[0].0 = element + 16;
        assert!(
            !type_check_inputs_via_kel(&inputs).unwrap(),
            "element {element}"
        );
    }
    for base in [0, 1, 2, 3, 20, 22, 24, 25] {
        let mut batch = empty_batch();
        batch
            .expressions
            .push((18, (base, 0, String::new()), (0, 0, String::new())));
        assert_eq!(
            type_check_inputs_via_kel(&input(batch)).unwrap(),
            base == 0,
            "base {base}"
        );
    }
}

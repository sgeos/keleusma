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
                .push((16, (0, 0, String::new()), (0, 0, String::new()))),
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

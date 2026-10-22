//! Tuple projections must carry source member identity independently of byte layout.
#![cfg(feature = "self-host")]

#[test]
fn tuple_projection_verdicts_preserve_valid_sources_and_reject_wrong_members() {
    let cases = [
        (
            "tuple bool scalar",
            "fn main(t: (Word, bool)) -> Word { t.1 }",
            false,
        ),
        (
            "tuple bool valid",
            "fn main(t: (Word, bool)) -> bool { t.1 }",
            true,
        ),
        (
            "nested tuple identity",
            "fn f(t: (Word, bool)) -> Word { 7 } fn main(t: ((Word, Word), Word)) -> Word { f(t.0) }",
            false,
        ),
        (
            "nested tuple valid",
            "fn f(t: (Word, Word)) -> Word { 7 } fn main(t: ((Word, Word), Word)) -> Word { f(t.0) }",
            true,
        ),
        (
            "tuple array dimension",
            "fn f(xs: [Word; 3]) -> Word { 7 } fn main(t: ([Word; 2], Word)) -> Word { f(t.0) }",
            false,
        ),
        (
            "tuple array valid",
            "fn f(xs: [Word; 2]) -> Word { 7 } fn main(t: ([Word; 2], Word)) -> Word { f(t.0) }",
            true,
        ),
        (
            "tuple nominal identity",
            "struct P { x: Word } struct Q { x: Word } fn f(q: Q) -> Word { 7 } fn main(t: (P, Word)) -> Word { f(t.0) }",
            false,
        ),
        (
            "tuple nominal valid",
            "struct P { x: Word } fn f(p: P) -> Word { 7 } fn main(t: (P, Word)) -> Word { f(t.0) }",
            true,
        ),
        (
            "tuple wrong scalar first",
            "fn main(t: (bool, Word)) -> Word { t.0 }",
            false,
        ),
        (
            "empty array identity",
            "fn f(xs: [Word; 1]) -> Word { 7 } fn main(t: ([Word; 0], Word)) -> Word { f(t.0) }",
            false,
        ),
        (
            "zero array declaration",
            "fn f(xs: [Word; 0]) -> Word { 7 } fn main(t: ([Word; 0], Word)) -> Word { f(t.0) }",
            false,
        ),
        (
            "nominal alias",
            "struct P { x: Word } struct Q { x: Word } fn f(q: Q) -> Word { 7 } fn main(t: (P, Word)) -> Word { let a = t.0; let b = a; f(b) }",
            false,
        ),
        (
            "tuple index bounds",
            "fn main(t: (Word, Word)) -> Word { t.2 }",
            false,
        ),
        (
            "nested array dimension",
            "fn f(xs: [[Word; 3]; 2]) -> Word { 7 } fn main(t: ([[Word; 2]; 2], Word)) -> Word { f(t.0) }",
            false,
        ),
    ];
    let mut gaps = Vec::new();
    for (label, source, expected) in cases {
        let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
        assert_eq!(
            keleusma::compiler::compile(&ast).is_ok(),
            expected,
            "reference control {label}"
        );
        let actual = keleusma::selfhost::type_check_source_via_kel(source);
        eprintln!("{label}: expected {expected}, independent {actual:?}");
        if !matches!(actual,Ok(v) if v==expected) {
            gaps.push(label);
        }
        if !expected && matches!(actual, Ok(false)) {
            assert!(
                matches!(
                    keleusma::selfhost::self_hosted_compile(
                        source,
                        &keleusma::target::Target::host()
                    ),
                    Err(keleusma::selfhost::SelfHostError::PipelineRejected { .. })
                ),
                "reference masked {label}"
            );
        }
    }
    assert!(gaps.is_empty(), "tuple projection gaps {gaps:?}");
}

#[test]
fn tuple_projection_artifacts_and_host_supplied_values_match_reference() {
    use keleusma::bytecode::Value;
    use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
    let cases = [
        (
            "array",
            "fn f(xs: [Word; 2]) -> Word { xs[0] } fn main(t: ([Word; 2], Word)) -> Word { f(t.0) }",
            vec![
                Value::array(vec![Value::Int(7), Value::Int(2)]),
                Value::Int(9),
            ],
        ),
        (
            "nested tuple",
            "fn f(t: (Word, Word)) -> Word { t.0 } fn main(t: ((Word, Word), Word)) -> Word { f(t.0) }",
            vec![
                Value::tuple(vec![Value::Int(7), Value::Int(2)]),
                Value::Int(9),
            ],
        ),
        (
            "nominal",
            "struct P { x: Word } fn f(p: P) -> Word { p.x } fn main(t: (P, Word)) -> Word { f(t.0) }",
            vec![Value::Int(7), Value::Int(9)],
        ),
        (
            "array alias",
            "fn f(xs: [Word; 2]) -> Word { xs[0] } fn main(t: ([Word; 2], Word)) -> Word { let a = t.0; let b = a; f(b) }",
            vec![
                Value::array(vec![Value::Int(7), Value::Int(2)]),
                Value::Int(9),
            ],
        ),
        (
            "bool",
            "fn main(t: (Word, bool)) -> Word { if t.1 { t.0 } else { 0 } }",
            vec![Value::Int(7), Value::Bool(true)],
        ),
        (
            "returned tuple",
            "fn relay(t: ([Word; 2], Word)) -> ([Word; 2], Word) { t } fn main(t: ([Word; 2], Word)) -> Word { let a = relay(t); let b = a.0; b[0] }",
            vec![
                Value::array(vec![Value::Int(7), Value::Int(2)]),
                Value::Int(9),
            ],
        ),
    ];
    for (label, source, values) in cases {
        eprintln!("artifact control {label}");
        let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
        let expected = keleusma::compiler::compile(&ast).unwrap();
        let actual =
            keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host())
                .unwrap_or_else(|e| panic!("{label}: {e:?}"));
        let bytes = keleusma::selfhost::wire_module_artifact_via_kel(&actual).unwrap();
        assert_eq!(bytes, expected.to_bytes().unwrap(), "{label}");
        for module in [actual, expected] {
            let arena = keleusma::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
            let mut vm = Vm::new(module, &arena).unwrap();
            let argument = Value::tuple_in_arena(values.clone(), 8, 8, 8, &arena).unwrap();
            assert!(
                matches!(
                    vm.call(&[argument]).unwrap(),
                    VmState::Finished(Value::Int(7))
                ),
                "{label}"
            );
        }
    }
}

#[test]
fn tuple_projection_source_positions_do_not_come_from_layout_offsets() {
    let source = "fn main(t: ((), Word)) -> Word { let a = t.0; t.1 }";
    let tag = keleusma::selfhost_host::PARSE_TUPLE_PROJECTION_TAG;
    let (_, records) = keleusma::selfhost::parse_record_trace(source);
    let mut ordinals = Vec::new();
    let mut offsets = Vec::new();
    for (at, &(kind, ordinal, _)) in records.iter().enumerate() {
        if kind == tag {
            assert_eq!(records[at + 1].0, 37);
            ordinals.push(ordinal);
            offsets.push(records[at + 1].1 % 65536);
        }
    }
    assert_eq!(ordinals, [0, 1]);
    assert_eq!(
        offsets,
        [0, 0],
        "zero-size member must make layout offsets ambiguous"
    );
}

#[test]
fn nested_header_metadata_retains_array_dimensions_and_tuple_boundaries() {
    let source = "fn main(t: (([Word; 2], bool), [Byte; 3])) -> Word { 7 }";
    let (names, records) = keleusma::selfhost::parse_record_trace(source);
    let actual: Vec<_> = records
        .into_iter()
        .filter(|(kind, _, _)| matches!(kind, 8 | 94..=96))
        .map(|(kind, value, _)| {
            (
                kind,
                if kind == 95 {
                    names[value as usize].clone()
                } else {
                    value.to_string()
                },
            )
        })
        .collect();
    assert_eq!(
        actual,
        [
            (94, "0"),
            (94, "0"),
            (95, "Word"),
            (8, "2"),
            (95, "bool"),
            (96, "0"),
            (95, "Byte"),
            (8, "3"),
            (96, "0")
        ]
        .into_iter()
        .map(|(k, v)| (k, v.into()))
        .collect::<Vec<_>>()
    );
}

#[test]
fn repeated_tuple_headers_reuse_layouts_without_changing_admission() {
    for parameters in [false, true] {
        let mut source = String::new();
        for i in 0..80 {
            if parameters {
                source.push_str(&format!("fn f{i}(t: (Word, Word)) -> Word {{ t.0 }}\n"));
            } else {
                source.push_str(&format!("fn f{i}() -> (Word, Word) {{ (1,2) }}\n"));
            }
        }
        source.push_str("fn main() -> Word { 7 }");
        let reference = keleusma::selfhost::compile_src(&source);
        let actual =
            keleusma::selfhost::self_hosted_compile(&source, &keleusma::target::Target::host())
                .unwrap();
        assert_eq!(actual.to_bytes().unwrap(), reference.to_bytes().unwrap());
    }
}

#[test]
fn nested_tuple_equality_executes_in_independently_assembled_modules() {
    use keleusma::bytecode::Value;
    use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
    let source = "fn main(a: ((Word, Word), Word), b: ((Word, Word), Word)) -> bool { a == b }";
    let reference = keleusma::selfhost::compile_src(source);
    let actual =
        keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host()).unwrap();
    assert_eq!(actual.to_bytes().unwrap(), reference.to_bytes().unwrap());
    for (second, expected) in [(2, true), (3, false)] {
        let arena = keleusma::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(actual.clone(), &arena).unwrap();
        let a = Value::tuple_in_arena(
            vec![
                Value::tuple(vec![Value::Int(1), Value::Int(2)]),
                Value::Int(9),
            ],
            8,
            8,
            8,
            &arena,
        )
        .unwrap();
        let b = Value::tuple_in_arena(
            vec![
                Value::tuple(vec![Value::Int(1), Value::Int(second)]),
                Value::Int(9),
            ],
            8,
            8,
            8,
            &arena,
        )
        .unwrap();
        assert!(
            matches!(vm.call(&[a,b]).unwrap(),VmState::Finished(Value::Bool(value)) if value==expected)
        );
    }
}

#[test]
fn tuple_equality_preserves_struct_members_at_multiple_depths() {
    for shape in [
        "(P, Word)",
        "((P, Word), Word)",
        "(((P, Word), Word), Word)",
        "(Word, ((Word, P), Word))",
    ] {
        let source = format!(
            "struct P {{ x: Word, y: Word }} fn main(a: {shape}, b: {shape}) -> bool {{ a == b }}"
        );
        let reference = keleusma::selfhost::compile_src(&source);
        let actual =
            keleusma::selfhost::self_hosted_compile(&source, &keleusma::target::Target::host())
                .unwrap_or_else(|error| panic!("{shape}: {error:?}"));
        assert_eq!(
            keleusma::selfhost::wire_module_artifact_via_kel(&actual).unwrap(),
            reference.to_bytes().unwrap(),
            "{shape}"
        );
    }
}

#[test]
fn tuple_header_capacity_errors_remain_distinct_from_source_rejection() {
    use keleusma::selfhost::try_call_type_inputs_from_pipeline;
    use keleusma::selfhost_host::{PARSE_DIAG_TAG_BASE, ParseRecordError};

    for depth in [32, 33] {
        let shape = format!("{}Word{}", "(".repeat(depth), ")".repeat(depth));
        let source = format!("fn main(t: {shape}) -> Word {{ 7 }}");
        // The reference parser stops at depth 24. Exercise the iterative stage directly.
        let result = try_call_type_inputs_from_pipeline(&source);
        if depth == 32 {
            result.expect("header at the nesting limit");
        } else {
            let error = result.expect_err("header beyond the nesting limit");
            assert!(!error.is_source_rejection());
            assert!(matches!(error, ParseRecordError::Diagnostic { code, .. }
                if code == PARSE_DIAG_TAG_BASE - 28));
        }
    }

    for count in [32, 33] {
        let mut source = String::new();
        for i in 0..count {
            let members: Vec<_> = (0..6)
                .map(|bit| if i & (1 << bit) == 0 { "Word" } else { "bool" })
                .collect();
            source.push_str(&format!(
                "fn f{i}(t: ({})) -> Word {{ 7 }}\n",
                members.join(", ")
            ));
        }
        keleusma::selfhost::compile_src(&source);
        let result = try_call_type_inputs_from_pipeline(&source);
        if count == 32 {
            result.expect("header at the distinct layout limit");
        } else {
            let error = result.expect_err("header beyond the distinct layout limit");
            assert!(!error.is_source_rejection());
            assert!(matches!(error, ParseRecordError::Diagnostic { code, .. }
                if code == PARSE_DIAG_TAG_BASE - 29));
        }
    }
}

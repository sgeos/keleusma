//! Written composite let types must reach independent production checking.
#![cfg(feature = "self-host")]

#[test]
fn composite_let_annotations_agree_with_reference() {
    let cases = [
        (
            "array element",
            "fn main() -> Word { let x: [Word; 2] = [true, false]; 7 }",
            false,
        ),
        (
            "array length",
            "fn main() -> Word { let x: [Word; 2] = [1]; 7 }",
            false,
        ),
        (
            "array scalar",
            "fn main() -> Word { let x: [Word; 2] = 1; 7 }",
            false,
        ),
        (
            "array alias",
            "fn main() -> Word { let a = [1]; let x: [Word; 2] = a; 7 }",
            false,
        ),
        (
            "array return value",
            "fn make() -> [Word; 1] { [1] } fn main() -> Word { let x: [Word; 2] = make(); 7 }",
            false,
        ),
        (
            "nested array inner length",
            "fn main() -> Word { let x: [[Word; 2]; 2] = [[1], [2]]; 7 }",
            false,
        ),
        (
            "nested array outer length",
            "fn main() -> Word { let x: [[Word; 2]; 2] = [[1, 2]]; 7 }",
            false,
        ),
        (
            "tuple member",
            "fn main() -> Word { let x: (Word, bool) = (1, 2); 7 }",
            false,
        ),
        (
            "tuple scalar",
            "fn main() -> Word { let x: (Word, Word) = 1; 7 }",
            false,
        ),
        (
            "array nominal element",
            "struct P { x: Word } struct Q { x: Word } fn main() -> Word { let x: [P; 1] = [Q { x: 1 }]; 7 }",
            false,
        ),
        (
            "array tuple element",
            "fn main() -> Word { let x: [(Word, bool); 2] = [(1, 2), (3, 4)]; 7 }",
            false,
        ),
        (
            "grouped scalar mismatch",
            "fn main() -> Word { let x: ((Word)) = true; 7 }",
            false,
        ),
        (
            "grouped array mismatch",
            "fn main() -> Word { let x: ([(Word); 2]) = [true, false]; 7 }",
            false,
        ),
        (
            "valid grouped scalar",
            "fn main() -> Word { let x: ((Word)) = 7; x }",
            true,
        ),
        (
            "valid grouped array",
            "fn main() -> Word { let x: ([(Word); 2]) = [7, 2]; x[0] }",
            true,
        ),
        (
            "unit mismatch",
            "fn main() -> Word { let x: () = 1; 7 }",
            false,
        ),
        (
            "valid unit",
            "fn main() -> Word { let x: () = (); 7 }",
            true,
        ),
        (
            "valid array",
            "fn main() -> Word { let x: [Word; 2] = [7, 2]; x[0] }",
            true,
        ),
        (
            "valid nested array",
            "fn main() -> Word { let x: [[Word; 2]; 2] = [[7, 2], [3, 4]]; x[0][0] }",
            true,
        ),
        (
            "valid tuple",
            "fn main() -> Word { let x: (Word, Word) = (7, 2); x.0 }",
            true,
        ),
        (
            "valid array tuple",
            "fn main() -> Word { let x: [(Word, bool); 2] = [(1, true), (3, false)]; 7 }",
            true,
        ),
        (
            "valid nominal array",
            "struct P { x: Word } fn main() -> Word { let x: [P; 1] = [P { x: 7 }]; x[0].x }",
            true,
        ),
        (
            "valid shadowed annotation",
            "fn main() -> Word { let x: [Word; 2] = [7, 2]; let y = if true { let x: (Word, Word) = (1, 2); x.0 } else { let x: Word = 2; x }; x[0] }",
            true,
        ),
    ];
    let mut gaps = Vec::new();
    for (label, source, expected) in cases {
        let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
        assert_eq!(
            keleusma::compiler::compile(&ast).is_ok(),
            expected,
            "bad control {label}"
        );
        let actual = keleusma::selfhost::type_check_source_via_kel(source);
        eprintln!("{label}: reference {expected}, pipeline {actual:?}");
        if !matches!(actual, Ok(value) if value == expected) {
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
    assert!(gaps.is_empty(), "composite annotation gaps {gaps:?}");
}

#[test]
fn annotation_metadata_retains_structure_and_binding_order() {
    use keleusma::selfhost_host::{
        PARSE_ANNOTATION_ARRAY_TAG, PARSE_ANNOTATION_END_TAG, PARSE_ANNOTATION_REF_TAG,
        PARSE_ANNOTATION_START_TAG, PARSE_LET_NAME_TAG,
    };
    let source = "fn main() -> Word { let a: [(Word, [bool; 3]); 2] = 1; let b: [Word; 1099511627776] = 2; 7 }";
    let (names, records) = keleusma::selfhost::parse_record_trace(source);
    let mut annotations = Vec::new();
    let mut active = None;
    let mut references = Vec::new();
    for (at, &(kind, value, _)) in records.iter().enumerate() {
        match kind {
            PARSE_ANNOTATION_START_TAG => {
                assert!(active.replace(value).is_none());
                annotations.push((value, Vec::new()));
            }
            PARSE_ANNOTATION_END_TAG => assert_eq!(active.take(), Some(value)),
            94..=96 | PARSE_ANNOTATION_ARRAY_TAG if active.is_some() => {
                annotations.last_mut().unwrap().1.push((
                    kind,
                    if kind == 95 {
                        names[value as usize].clone()
                    } else {
                        value.to_string()
                    },
                ));
            }
            PARSE_ANNOTATION_REF_TAG => {
                assert!(active.is_none());
                assert_eq!(records[at - 1].0, PARSE_LET_NAME_TAG);
                assert_eq!(records[at + 1].0, 5);
                references.push(value);
            }
            _ => {}
        }
    }
    assert!(active.is_none());
    assert_eq!(annotations.len(), 2);
    assert_eq!(references, [annotations[1].0, annotations[0].0]);
    assert_eq!(
        annotations[0].1,
        [
            (94, "0"),
            (95, "Word"),
            (95, "bool"),
            (100, "3"),
            (96, "0"),
            (100, "2")
        ]
        .map(|(kind, value)| (kind, value.to_owned()))
    );
    assert_eq!(
        annotations[1].1,
        [(95, "Word"), (100, "1099511627776")].map(|(kind, value)| (kind, value.to_owned()))
    );
    let (_, bare) = keleusma::selfhost::parse_record_trace("fn main() -> Word { let a = 7; a }");
    assert!(bare.iter().all(|(kind, _, _)| !matches!(*kind, 99..=102)));
}

#[test]
fn annotation_capacity_is_guarded_before_writing() {
    use keleusma::selfhost::try_call_type_inputs_from_pipeline;
    use keleusma::selfhost_host::{
        PARSE_ANNOTATION_DEPTH_CAP, PARSE_DIAG_TAG_BASE, ParseRecordError,
    };
    let stage = include_str!("../src/selfhost/kel/parse.kel");
    let cap: usize = stage
        .lines()
        .find(|line| line.starts_with("fn pe_cap_annotation() -> Word {"))
        .and_then(|line| line.split('{').nth(1))
        .and_then(|line| line.split('}').next())
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_eq!(cap, PARSE_ANNOTATION_DEPTH_CAP);
    let storage = stage
        .split("private data ann {")
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap();
    for field in ["kind", "phase", "length"] {
        assert!(storage.contains(&format!("{field}: [Word; {cap}]")));
    }
    for depth in [cap - 1, cap, cap + 1] {
        let mut declared = "Word".to_owned();
        for _ in 0..depth {
            declared = format!("[{declared}; 1]");
        }
        let source = format!("fn main() -> Word {{ let a: {declared} = 1; 7 }}");
        // This checks the iterative stage's bound directly. The reference parser has
        // a separate recursion limit of 24 and cannot serve as this boundary oracle.
        if depth <= cap {
            try_call_type_inputs_from_pipeline(&source).expect("annotation within capacity");
            assert!(!keleusma::selfhost::type_check_source_via_kel(&source).unwrap());
        } else {
            let error = try_call_type_inputs_from_pipeline(&source).expect_err("depth cap");
            assert!(!error.is_source_rejection());
            assert!(
                matches!(error, ParseRecordError::Diagnostic { code, detail }
                if code == PARSE_DIAG_TAG_BASE - 28 && detail == depth as i64)
            );
        }
    }
}

#[test]
fn unsupported_annotation_syntax_is_unavailable() {
    use keleusma::selfhost::try_call_type_inputs_from_pipeline;
    use keleusma::selfhost_host::{PARSE_DIAG_TAG_BASE, ParseRecordError};
    let source = "fn main() -> Word { let a: [Word; 1 + 1] = [7, 2]; a[0] }";
    let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
    keleusma::compiler::compile(&ast).expect("valid const dimension");
    let error = try_call_type_inputs_from_pipeline(source).expect_err("unsupported dimension");
    assert!(!error.is_source_rejection());
    assert!(matches!(error, ParseRecordError::Diagnostic { code, .. }
        if code == PARSE_DIAG_TAG_BASE - 27));
}

#[test]
fn composite_annotations_preserve_artifacts_and_execution() {
    for source in [
        "fn main() -> Word { let x: () = (); 7 }",
        "fn main() -> Word { let x: [Word; 2] = [7, 2]; x[0] }",
        "fn main() -> Word { let x: [[Word; 2]; 2] = [[7, 2], [3, 4]]; x[0][0] }",
        "fn main() -> Word { let x: (Word, Word) = (7, 2); x.0 }",
        "fn main() -> Word { let x: [(Word, Word); 2] = [(7, 2), (3, 4)]; x[0].0 }",
        "struct P { x: Word } fn main() -> Word { let x: [P; 1] = [P { x: 7 }]; x[0].x }",
        "fn main() -> Word { let x: [Word; 2] = [7, 2]; let y = if true { let x: (Word, Word) = (1, 2); x.0 } else { let x: Word = 2; x }; x[0] }",
    ] {
        let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
        let reference = keleusma::compiler::compile(&ast).unwrap();
        let actual =
            keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host())
                .unwrap_or_else(|error| panic!("{source}: {error:?}"));
        assert_eq!(
            actual.to_bytes().unwrap(),
            reference.to_bytes().unwrap(),
            "{source}"
        );
        for module in [reference, actual] {
            let persistent = keleusma::vm::required_persistent_capacity_for(&module);
            let mut arena =
                keleusma::Arena::with_capacity(keleusma::vm::DEFAULT_ARENA_CAPACITY + persistent);
            arena.resize_persistent(persistent).unwrap();
            let entry = module
                .chunks
                .iter()
                .position(|chunk| chunk.name == "main")
                .unwrap();
            let mut vm = keleusma::vm::Vm::new(module, &arena).unwrap();
            assert!(
                matches!(
                    vm.call_function(entry, &[]).unwrap(),
                    keleusma::vm::VmState::Finished(keleusma::bytecode::Value::Int(7))
                ),
                "{source}"
            );
        }
    }
}

#[test]
fn literal_tuple_layout_exhaustion_is_unavailable() {
    use keleusma::selfhost::try_call_type_inputs_from_pipeline;
    use keleusma::selfhost_host::{PARSE_DIAG_TAG_BASE, ParseRecordError};
    // Separate layout-count, member-count, and total-field-storage boundaries.
    for (bindings, members, fits) in [
        (32, 2, true),
        (33, 2, false),
        (1, 32, true),
        (1, 33, false),
        (8, 32, true),
        (9, 32, false),
    ] {
        let tuple = format!("({})", vec!["1"; members].join(", "));
        let mut source = "fn main() -> Word { ".to_owned();
        for i in 0..bindings {
            source.push_str(&format!("let t{i} = {tuple}; "));
        }
        source.push_str("7 }");
        let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(&source).unwrap()).unwrap();
        keleusma::compiler::compile(&ast).expect("valid tuple layouts");
        let result = try_call_type_inputs_from_pipeline(&source);
        if fits {
            result.expect("layout within bounds");
        } else {
            let error = result.expect_err("layout capacity");
            assert!(!error.is_source_rejection());
            assert!(matches!(error, ParseRecordError::Diagnostic { code, .. }
                if code == PARSE_DIAG_TAG_BASE - 29));
        }
    }
}

//! Tuple construction must preserve scalar widths and nested member layouts.
#![cfg(feature = "self-host")]

#[test]
fn tuple_construction_and_nested_headers_preserve_artifacts() {
    let cases = [
        (
            "mixed literal projection",
            "fn main() -> Word { let t = (true, 7); t.1 }",
        ),
        (
            "mixed literal return",
            "fn main() -> (bool, Word) { (true, 7) }",
        ),
        (
            "nested literal projection",
            "fn main() -> Word { let t = ((1, 2), 3); t.0 .1 }",
        ),
        (
            "nested mixed projection",
            "fn main() -> Word { let t = ((true, 7), 3); t.0 .1 }",
        ),
        (
            "later nested member",
            "fn main() -> Word { let t = (true, (7, false)); t.1 .0 }",
        ),
        (
            "scalar parameters",
            "fn main(b: bool, n: Word) -> Word { let t = (b, n); t.1 }",
        ),
        (
            "scalar aliases",
            "fn main() -> Word { let b = true; let n = 7; let t = (b, n); t.1 }",
        ),
        (
            "comparison member",
            "fn main(n: Word) -> Word { let t = (n == 1, 7); t.1 }",
        ),
        (
            "call member",
            "fn yes() -> bool { true } fn main() -> Word { let t = (yes(), 7); t.1 }",
        ),
        ("unit member", "fn main() -> Word { let t = ((), 7); t.1 }"),
        (
            "byte member",
            "fn main(b: Byte) -> Word { let t = (b, 7); t.1 }",
        ),
        (
            "struct member",
            "struct P { x: Word } fn main() -> Word { let t = (P { x: 7 }, false); t.0.x }",
        ),
        (
            "array of mixed tuples",
            "fn main() -> Word { let xs = [(true, 7), (false, 8)]; xs[0].1 }",
        ),
        (
            "tuple alias",
            "fn main() -> Word { let t = (true, 7); let u = t; u.1 }",
        ),
        (
            "nested tuple alias",
            "fn main() -> Word { let t = ((true, 7), 3); let u = t.0; u.1 }",
        ),
        (
            "repeated tuple members",
            "fn main() -> Word { let t = (true, 7); let u = (t, t); u.1 .1 }",
        ),
        (
            "direct literal projection",
            "fn main() -> Word { (true, 7).1 }",
        ),
        (
            "array member",
            "fn main() -> Word { let t = ([1, 7], false); t.0[1] }",
        ),
        (
            "array parameter member",
            "fn main(xs: [Word; 2]) -> Word { let t = (xs, false); t.0[1] }",
        ),
        (
            "data member",
            "private data d { n: Word } fn main() -> Word { d.n = 7; let t = (true, d.n); t.1 }",
        ),
        (
            "string member",
            "fn main() -> Word { let t = (\"hello\", 7); t.1 }",
        ),
        (
            "boolean field member",
            "struct P { b: bool } fn main(p: P) -> Word { let t = (p.b, 7); t.1 }",
        ),
        (
            "conditional member",
            "fn main(c: bool) -> Word { let t = (if c { true } else { false }, 7); t.1 }",
        ),
        (
            "tuple array element alias",
            "fn main(xs: [(bool, Word); 2]) -> Word { let t = xs[0]; t.1 }",
        ),
        (
            "layout survives function boundaries",
            "fn pair() -> (bool, Word) { (true, 7) } fn take(t: (bool, Word)) -> Word { let u = t; u.1 } fn main() -> Word { let t = pair(); take(t) }",
        ),
        (
            "indexed data member",
            "private data d { xs: [Word; 2] } fn main() -> Word { d.xs[1] = 7; let t = (true, d.xs[1]); t.1 }",
        ),
        (
            "bool array member",
            "fn main() -> Word { let t = ([true, false], 7); t.1 }",
        ),
        (
            "nested struct field member",
            "struct P { x: Word } struct Q { p: P } fn main(q: Q) -> Word { let t = (q.p, false); t.0.x }",
        ),
        (
            "split nested array indexing",
            "fn main() -> Word { let a = [[1, 2], [3, 4]]; let b = a[0]; b[1] }",
        ),
        (
            "array of nested tuple header",
            "fn main(xs: [((Word, Word), Word); 2]) -> Word { xs[0].0 .1 }",
        ),
    ];
    let mut gaps = Vec::new();
    for (name, source) in cases {
        let reference = keleusma::selfhost::compile_src(source);
        let verdict = keleusma::selfhost::type_check_source_via_kel(source);
        let result =
            keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host());
        let agrees = matches!(&result, Ok(module) if module.to_bytes().unwrap() == reference.to_bytes().unwrap());
        eprintln!("{name}: type {verdict:?}, artifact identity {agrees}");
        if !matches!(verdict, Ok(true)) || !agrees {
            eprintln!(
                "{name}: {}",
                result.err().map(|e| format!("{e:?}")).unwrap_or_default()
            );
            gaps.push(name);
        }
    }
    assert!(gaps.is_empty(), "tuple construction gaps {gaps:?}");
}

#[test]
fn constructed_tuples_execute_after_keleusma_serialization() {
    use keleusma::bytecode::Value;
    use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};
    for source in [
        "fn main() -> Word { let t = (true, 7); t.1 }",
        "fn main() -> Word { let t = ((true, 7), 3); t.0 .1 }",
        "fn main() -> Word { let t = (true, 7); let u = (t, t); u.1 .1 }",
        "fn main() -> Word { let xs = [(true, 7), (false, 8)]; xs[0].1 }",
        "fn main() -> Word { ((), 7).1 }",
    ] {
        let module =
            keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host())
                .unwrap();
        let bytes = keleusma::selfhost::wire_module_artifact_via_kel(&module).unwrap();
        assert_eq!(
            bytes,
            keleusma::selfhost::compile_src(source).to_bytes().unwrap()
        );
        let module = keleusma::selfhost::wire_decode_module_via_kel(&bytes).unwrap();
        let persistent = required_persistent_capacity_for(&module);
        let mut arena = keleusma::Arena::with_capacity(DEFAULT_ARENA_CAPACITY + persistent);
        arena.resize_persistent(persistent).unwrap();
        let mut vm = Vm::new(module, &arena).unwrap();
        assert!(
            matches!(vm.call(&[]).unwrap(), VmState::Finished(Value::Int(7))),
            "{source}"
        );
    }
}

#[test]
fn invalid_tuple_members_are_rejected_independently() {
    for source in [
        "fn main() -> Word { let t = (true, 7); t.0 }",
        "fn main() -> Word { let t: (Word, Word) = (true, 7); t.1 }",
        "fn main() -> Word { let t = ((true, 7), 3); let n: Word = t.0 .0; n }",
        "fn main() -> (Word, bool) { (true, 7) }",
        "fn main(xs: [((Word, Word), Word); 2]) -> bool { xs[0].0 .1 }",
    ] {
        let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
        assert!(keleusma::compiler::compile(&ast).is_err(), "{source}");
        assert!(
            !keleusma::selfhost::type_check_source_via_kel(source).unwrap(),
            "{source}"
        );
        assert!(
            matches!(
                keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host()),
                Err(keleusma::selfhost::SelfHostError::PipelineRejected { .. })
            ),
            "{source}"
        );
    }
}

#[test]
fn tuple_context_overflow_is_a_parser_diagnostic() {
    for depth in [8, 9] {
        let mut value = "7".to_owned();
        for _ in 0..depth {
            value = format!("(true, {value})");
        }
        let source = format!("fn main() -> Word {{ let t = {value}; 7 }}");
        keleusma::selfhost::compile_src(&source);
        let parsed = keleusma::selfhost::try_call_type_inputs_from_pipeline(&source);
        if depth == 8 {
            assert!(parsed.is_ok(), "{parsed:?}");
        } else {
            assert!(
                matches!(
                    parsed,
                    Err(keleusma::selfhost_host::ParseRecordError::Diagnostic { code, detail: 9 }) if code == keleusma::selfhost_host::PARSE_DIAG_TAG_BASE - 29
                ),
                "{parsed:?}"
            );
        }
    }
}

#[test]
fn temporary_tuple_descriptors_do_not_accumulate_between_functions() {
    let mut source = String::new();
    for i in 0..400 {
        source.push_str(&format!("fn f{i}() -> Word {{ let t = (true, 7); t.1 }}\n"));
    }
    source.push_str("fn main() -> Word { f399() }");
    let reference = keleusma::selfhost::compile_src(&source);
    let actual =
        keleusma::selfhost::self_hosted_compile(&source, &keleusma::target::Target::host())
            .unwrap();
    assert_eq!(actual.to_bytes().unwrap(), reference.to_bytes().unwrap());
}

#[test]
fn unavailable_forward_return_layout_is_distinct_from_capacity_and_type_rejection() {
    use keleusma::selfhost_host::{PARSE_DIAG_TAG_BASE, ParseRecordError};
    let source = "fn main() -> Word { let t = (yes(), 7); t.1 } fn yes() -> bool { true }";
    keleusma::selfhost::compile_src(source);
    let error = keleusma::selfhost::try_call_type_inputs_from_pipeline(source)
        .expect_err("forward return layout remains unavailable");
    assert!(!error.is_source_rejection());
    assert!(
        matches!(error, ParseRecordError::Diagnostic { code, .. } if code == PARSE_DIAG_TAG_BASE - 38)
    );
    assert!(matches!(
        keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host()),
        Err(keleusma::selfhost::SelfHostError::TypeCheckUnavailable { .. })
    ));
}

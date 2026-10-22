//! Differential admission of source composite facts through the production type path.
#![cfg(feature = "self-host")]

#[test]
fn composite_source_facts_agree_with_reference() {
    let cases = [
        (
            "struct argument",
            "struct P { x: Word } fn f(p: P) -> Word { 1 } fn main() -> Word { f(1) }",
            false,
        ),
        (
            "struct return",
            "struct P { x: Word } fn f() -> P { 1 } fn main() -> Word { 1 }",
            false,
        ),
        (
            "struct field",
            "struct P { x: Word } fn main() -> Word { let p = P { x: true }; 1 }",
            false,
        ),
        (
            "struct identity",
            "struct P { x: Word } struct Q { x: Word } fn f(p: P) -> Word { 1 } fn main() -> Word { f(Q { x: 1 }) }",
            false,
        ),
        (
            "enum argument",
            "enum E { A, B } fn f(e: E) -> Word { 1 } fn main() -> Word { f(1) }",
            false,
        ),
        (
            "enum return",
            "enum E { A, B } fn f() -> E { 1 } fn main() -> Word { 1 }",
            false,
        ),
        (
            "array argument",
            "fn f(a: [Word; 2]) -> Word { 1 } fn main() -> Word { f(1) }",
            false,
        ),
        (
            "array length",
            "fn f(a: [Word; 2]) -> Word { 1 } fn main() -> Word { f([1]) }",
            false,
        ),
        (
            "array element",
            "fn f(a: [Word; 2]) -> Word { 1 } fn main() -> Word { f([true, false]) }",
            false,
        ),
        (
            "array return",
            "fn f() -> [Word; 2] { 1 } fn main() -> Word { 1 }",
            false,
        ),
        (
            "scalar from array",
            "fn f(a: [Word; 2]) -> Word { a } fn main() -> Word { 1 }",
            false,
        ),
        (
            "array parameter valid",
            "fn f(a: [Word; 2]) -> [Word; 2] { a } fn main() -> Word { 1 }",
            true,
        ),
        (
            "struct parameter valid",
            "struct P { x: Word } fn f(p: P) -> P { p } fn main() -> Word { f(P { x: 1 }).x }",
            true,
        ),
        (
            "array literal valid",
            "fn f(a: [Word; 2]) -> Word { a[0] } fn main() -> Word { f([1, 2]) }",
            true,
        ),
        (
            "tuple before scalar valid",
            "fn f(t: (Word, Word), b: bool) -> bool { b } fn main() -> bool { f((1, 2), true) }",
            true,
        ),
        (
            "tuple before scalar invalid",
            "fn f(t: (Word, Word), b: bool) -> bool { b } fn main() -> bool { f((1, 2), 1) }",
            false,
        ),
    ];
    let mut gaps = Vec::new();
    for (label, source, expected) in cases {
        let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
        let reference = keleusma::compiler::compile(&ast).is_ok();
        assert_eq!(reference, expected, "bad control {label}");
        let actual = keleusma::selfhost::type_check_source_via_kel(source);
        if expected && !matches!(actual, Ok(true)) {
            eprintln!(
                "facts {:?}",
                keleusma::selfhost::try_call_type_inputs_from_pipeline(source)
            );
        }
        eprintln!("{label}: reference {reference}, pipeline {actual:?}");
        if !matches!(actual, Ok(value) if value == expected) {
            gaps.push(label);
        }
        if !expected {
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
    assert!(gaps.is_empty(), "composite gaps {gaps:?}");
}

/// Positive controls guard against losing nominal and structural identities through aliases.
#[test]
fn composite_aliases_and_nested_controls_agree() {
    let cases = [
        "enum E { A, B } fn f(e: E) -> Word { e as Word } fn main() -> Word { f(E::A()) }",
        "struct P { x: Word } fn main() -> Word { P { x: 7 }.x }",
        "struct P { x: Word, b: bool } fn main() -> Word { let p = P { b: true, x: 7 }; p.x }",
        "struct Q { x: Word } struct P { q: Q, b: bool } fn main() -> Word { let p = P { b: true, q: Q { x: 7 } }; p.q.x }",
        "struct P { x: Word } fn f(p: P) -> Word { p.x } fn main() -> Word { let p = P { x: 7 }; let q = p; f(q) }",
        "struct P { x: Word } fn f() -> P { P { x: 7 } } fn main() -> Word { f().x }",
        "struct P { x: Word } fn f(p: P) -> P { p } fn main() -> Word { f(f(P { x: 7 })).x }",
        "struct P { a: [Word; 2], b: bool } fn main() -> Word { let p = P { b: true, a: [1, 2] }; p.a[0] }",
        "struct P { a: (Word, bool), b: Word } fn main() -> Word { let p = P { b: 7, a: (1, true) }; p.b }",
        "fn f(a: [Word; 2]) -> [Word; 2] { a } fn main() -> Word { let a = [1, 2]; let b = a; let c = f(b); c[0] }",
        "fn f(a: [bool; 2]) -> bool { a[0] } fn main() -> bool { f([true, false]) }",
        "fn f(a: [Byte; 2]) -> Byte { a[0] } fn main() -> Byte { f([1 as Byte, 2 as Byte]) }",
        "fn f(a: [Word; 2]) -> Word { a[0] } fn main() -> Word { f(if true { [1, 2] } else { [3, 4] }) }",
        "fn f(a: [Word; 2]) -> Word { a[0] } fn main() -> Word { f([1 + 2, 3 * 4]) }",
    ];
    let mut gaps = Vec::new();
    for source in cases {
        let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
        assert!(
            keleusma::compiler::compile(&ast).is_ok(),
            "invalid positive {source}"
        );
        let actual = keleusma::selfhost::type_check_source_via_kel(source);
        eprintln!("positive {source}: {actual:?}");
        if !matches!(actual, Ok(true)) {
            gaps.push(source);
        }
    }
    assert!(gaps.is_empty(), "positive gaps {gaps:?}");
}

/// Entire array fields and array signatures preserve complete artifacts and executable values.
#[test]
fn array_signatures_and_whole_fields_preserve_artifacts() {
    for source in [
        "struct P { xs: [Word; 2] } fn take(xs: [Word; 2]) -> Word { xs[0] } fn main() -> Word { let p = P { xs: [7, 2] }; take(p.xs) }",
        "fn f(a: [Word; 2]) -> [Word; 2] { a } fn main() -> Word { let a = f([7, 2]); a[0] }",
        "struct P { x: Word } fn main() -> Word { P { x: 7 }.x }",
    ] {
        let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
        let reference = keleusma::compiler::compile(&ast).unwrap();
        let actual =
            keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host())
                .unwrap();
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
            let entry = module.chunks.iter().position(|c| c.name == "main").unwrap();
            let mut vm = keleusma::vm::Vm::new(module, &arena).unwrap();
            assert!(matches!(
                vm.call_function(entry, &[]).unwrap(),
                keleusma::vm::VmState::Finished(keleusma::bytecode::Value::Int(7))
            ));
        }
    }
}

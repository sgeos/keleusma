//! Enum construction facts must reach independent production rejection.
#![cfg(feature = "self-host")]

#[test]
fn enum_constructors_and_payloads_agree_with_reference() {
    let cases = [
        (
            "unit identity",
            "enum E { A } enum F { A } fn f(e: E) -> Word { 7 } fn main() -> Word { f(F::A()) }",
            false,
        ),
        (
            "payload identity",
            "enum E { A(Word) } enum F { A(Word) } fn f(e: E) -> Word { 7 } fn main() -> Word { f(F::A(1)) }",
            false,
        ),
        (
            "scalar payload",
            "enum E { A(Word) } fn main() -> Word { let e = E::A(true); 7 }",
            false,
        ),
        (
            "missing payload",
            "enum E { A(Word) } fn main() -> Word { let e = E::A(); 7 }",
            false,
        ),
        (
            "extra payload",
            "enum E { A(Word) } fn main() -> Word { let e = E::A(1, 2); 7 }",
            false,
        ),
        (
            "unit with payload",
            "enum E { A } fn main() -> Word { let e = E::A(1); 7 }",
            false,
        ),
        (
            "unknown variant",
            "enum E { A } fn main() -> Word { let e = E::Missing(); 7 }",
            false,
        ),
        (
            "cast missing payload",
            "enum E { A(Word) } fn main() -> Word { E::A() as Word }",
            false,
        ),
        (
            "constructor return",
            "enum E { A } enum F { A } fn f() -> E { F::A() } fn main() -> Word { 7 }",
            false,
        ),
        (
            "struct payload",
            "struct P { x: Word } struct Q { x: Word } enum E { A(P) } fn main() -> Word { let e = E::A(Q { x: 1 }); 7 }",
            false,
        ),
        (
            "array payload element",
            "enum E { A([Word; 2]) } fn main() -> Word { let e = E::A([true, false]); 7 }",
            false,
        ),
        (
            "array payload length",
            "enum E { A([Word; 2]) } fn main() -> Word { let e = E::A([1]); 7 }",
            false,
        ),
        (
            "alias identity",
            "enum E { A } enum F { A } fn f(e: E) -> Word { 7 } fn main() -> Word { let a = F::A(); let b = a; f(b) }",
            false,
        ),
        (
            "valid unit",
            "enum E { A } fn f(e: E) -> Word { 7 } fn main() -> Word { f(E::A()) }",
            true,
        ),
        (
            "valid payload",
            "enum E { A(Word, bool) } fn main() -> Word { let e = E::A(1 + 2, true); 7 }",
            true,
        ),
        (
            "valid struct payload",
            "struct P { x: Word } enum E { A(P) } fn main() -> Word { let e = E::A(P { x: 1 }); 7 }",
            true,
        ),
        (
            "valid array payload",
            "enum E { A([Word; 2]) } fn main() -> Word { let e = E::A([1, 2]); 7 }",
            true,
        ),
        (
            "tuple before scalar payload",
            "enum E { A((Word, bool), Byte) } fn main() -> Word { let e = E::A((1, true), 2 as Byte); 7 }",
            true,
        ),
        (
            "tuple before wrong scalar payload",
            "enum E { A((Word, bool), Byte) } fn main() -> Word { let e = E::A((1, true), true); 7 }",
            false,
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
        let shipping = if expected {
            true
        } else {
            matches!(
                keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host()),
                Err(keleusma::selfhost::SelfHostError::PipelineRejected { .. })
            )
        };
        eprintln!(
            "{label}: reference {expected}, pipeline {actual:?}, independent shipping {shipping}"
        );
        if !matches!(actual, Ok(value) if value == expected) || !shipping {
            gaps.push(label);
        }
    }
    assert!(gaps.is_empty(), "enum fact gaps {gaps:?}");
}

#[test]
fn enum_variant_metadata_precedes_each_constructor() {
    let source = "enum E { A, B(Word) } enum F { C } fn main() -> Word { let a = E::B(7); let b = F::C(); let c = E::A(); 7 }";
    let tag = keleusma::selfhost_host::PARSE_ENUM_VARIANT_TAG;
    let declared: i64 = include_str!("../src/selfhost/kel/parse.kel")
        .lines()
        .find(|line| line.starts_with("fn tag_enum_variant() -> Word {"))
        .and_then(|line| line.split('{').nth(1))
        .and_then(|body| body.split('}').next())
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_eq!(tag, declared);
    let (_, records) = keleusma::selfhost::parse_record_trace(source);
    let variants: Vec<_> = records
        .iter()
        .enumerate()
        .filter(|(_, (kind, _, _))| *kind == tag)
        .map(|(at, (_, variant, _))| {
            assert_eq!(records[at + 1].0, 18, "metadata must precede EnumInit");
            *variant
        })
        .collect();
    assert_eq!(variants, [1, 2, 0]);
    assert!(keleusma::selfhost::type_check_source_via_kel(source).unwrap());
}

#[test]
fn enum_artifacts_and_execution_agree() {
    for source in [
        "enum E { A } fn take(e: E) -> Word { 7 } fn main() -> Word { take(E::A()) }",
        "enum E { A(Word), B } fn main() -> Word { let e = E::A(7); match e { E::A(x) => x, E::B() => 0 } }",
        "enum E { A([Word; 2]), B } fn main() -> Word { let e = E::A([7, 2]); match e { E::A(xs) => xs[0], E::B() => 0 } }",
        "struct P { x: Word } enum E { A(P), B } fn main() -> Word { let e = E::A(P { x: 7 }); match e { E::A(p) => p.x, E::B() => 0 } }",
        "enum E { A = 7, B = 11 } fn main() -> Word { let e = E::A(); e as Word }",
        "enum E { A = 7, B = 11 } fn take(e: E) -> Word { e as Word } fn main() -> Word { take(E::A()) }",
        "enum E { A = 7, B = 11 } fn make() -> E { E::A() } fn main() -> Word { make() as Word }",
        "enum E { A = 7, B = 11 } fn main() -> Word { let a = E::A(); let b = a; b as Word }",
        "enum E { A(Word) = 7, B = 11 } fn main() -> Word { E::A(1 + 2) as Word }",
        "enum E { A = 7, B = 11 } fn main() -> Word { let e = E::A(); (7 as Byte) as Word }",
        "enum E { A = 7, B = 11 } fn take(e: E) -> Word { let a = e; let b = a; b as Word } fn main() -> Word { take(E::A()) }",
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

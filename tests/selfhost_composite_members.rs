//! Declared composite member requirements must reach independent production rejection.
#![cfg(feature = "self-host")]

#[test]
fn composite_member_requirements_agree_with_reference() {
    let cases = [
        (
            "enum tuple scalar",
            "enum E { A((Word, bool)) } fn main() -> Word { let e = E::A(1); 7 }",
            false,
        ),
        (
            "enum tuple member",
            "enum E { A((Word, bool)) } fn main() -> Word { let e = E::A((1, 2)); 7 }",
            false,
        ),
        (
            "enum tuple arity",
            "enum E { A((Word, bool)) } fn main() -> Word { let e = E::A((1, true, 3)); 7 }",
            false,
        ),
        (
            "enum tuple alias",
            "enum E { A((Word, bool)) } fn main() -> Word { let x = (true, 1); let y = x; let e = E::A(y); 7 }",
            false,
        ),
        (
            "enum tuple return",
            "enum E { A((Word, bool)) } fn make() -> (Word, Word) { (1, 2) } fn main() -> Word { let e = E::A(make()); 7 }",
            false,
        ),
        (
            "enum tuple between fields",
            "enum E { A(Word, (Word, bool), Byte) } fn main() -> Word { let e = E::A(1, (true, 2), 3 as Byte); 7 }",
            false,
        ),
        (
            "enum field after tuple",
            "enum E { A((Word, bool), Byte) } fn main() -> Word { let e = E::A((1, true), false); 7 }",
            false,
        ),
        (
            "valid enum tuple",
            "enum E { A((Word, bool)) } fn main() -> Word { let e = E::A((1, true)); 7 }",
            true,
        ),
        (
            "valid enum tuple between fields",
            "enum E { A(Word, (Word, bool), Byte) } fn main() -> Word { let e = E::A(1, (2, true), 3 as Byte); 7 }",
            true,
        ),
        (
            "valid enum multiple tuple fields",
            "enum E { A((Word, bool), (Byte, Word)) } fn main() -> Word { let e = E::A((1, true), (2 as Byte, 3)); 7 }",
            true,
        ),
        (
            "struct tuple scalar",
            "struct P { x: (Word, bool) } fn main() -> Word { let p = P { x: 1 }; 7 }",
            false,
        ),
        (
            "struct tuple member",
            "struct P { x: (Word, bool) } fn main() -> Word { let p = P { x: (1, 2) }; 7 }",
            false,
        ),
        (
            "struct tuple arity",
            "struct P { x: (Word, bool) } fn main() -> Word { let p = P { x: (1, true, 3) }; 7 }",
            false,
        ),
        (
            "struct tuple alias",
            "struct P { x: (Word, bool) } fn main() -> Word { let x = (true, 1); let y = x; let p = P { x: y }; 7 }",
            false,
        ),
        (
            "struct array element",
            "struct P { x: [Word; 2] } fn main() -> Word { let p = P { x: [true, false] }; 7 }",
            false,
        ),
        (
            "struct array length",
            "struct P { x: [Word; 2] } fn main() -> Word { let p = P { x: [1] }; 7 }",
            false,
        ),
        (
            "valid struct tuple",
            "struct P { x: (Word, bool) } fn main() -> Word { let p = P { x: (1, true) }; 7 }",
            true,
        ),
        (
            "valid struct tuple reordered",
            "struct P { x: (Word, bool), y: Byte } fn main() -> Word { let p = P { y: 2 as Byte, x: (1, true) }; 7 }",
            true,
        ),
        (
            "valid struct array",
            "struct P { x: [Word; 2] } fn main() -> Word { let p = P { x: [1, 2] }; 7 }",
            true,
        ),
        (
            "valid enum named tuple member",
            "struct P { x: Word } enum E { A((P, Word)) } fn main() -> Word { let e = E::A((P { x: 1 }, 2)); 7 }",
            true,
        ),
        (
            "tuple payload binder scalar",
            "enum E { A((Word, Word)) } fn main(e: E) -> Word { match e { E::A(t) => t } }",
            false,
        ),
        (
            "tuple payload binder argument",
            "enum E { A((Word, Word)) } fn f(t: (Word, bool)) -> Word { 7 } fn main(e: E) -> Word { match e { E::A(t) => f(t) } }",
            false,
        ),
        (
            "array payload binder scalar",
            "enum E { A([Word; 2]) } fn main(e: E) -> Word { match e { E::A(xs) => xs } }",
            false,
        ),
        (
            "array payload binder dimension",
            "enum E { A([Word; 2]) } fn f(xs: [Word; 3]) -> Word { 7 } fn main(e: E) -> Word { match e { E::A(xs) => f(xs) } }",
            false,
        ),
        (
            "nominal payload binder identity",
            "struct P { x: Word } struct Q { x: Word } enum E { A(P) } fn f(p: Q) -> Word { 7 } fn main(e: E) -> Word { match e { E::A(p) => f(p) } }",
            false,
        ),
        (
            "valid tuple payload binder argument",
            "enum E { A((Word, Word)) } fn f(t: (Word, Word)) -> Word { 7 } fn main(e: E) -> Word { match e { E::A(t) => f(t) } }",
            true,
        ),
        (
            "valid array payload binder argument",
            "enum E { A([Word; 2]) } fn f(xs: [Word; 2]) -> Word { 7 } fn main(e: E) -> Word { match e { E::A(xs) => f(xs) } }",
            true,
        ),
        (
            "valid nominal payload binder identity",
            "struct P { x: Word } enum E { A(P) } fn f(p: P) -> Word { 7 } fn main(e: E) -> Word { match e { E::A(p) => f(p) } }",
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
    assert!(gaps.is_empty(), "composite member gaps {gaps:?}");
}

#[test]
fn composite_members_preserve_artifacts_and_execution() {
    for source in [
        "enum E { A((Word, Word)), B } fn main() -> Word { let e = E::A((7, 2)); match e { E::A(t) => t.0, E::B() => 0 } }",
        "enum E { A(Word, (Word, Word), Word), B } fn main() -> Word { let e = E::A(1, (4, 2), 2); match e { E::A(a, t, b) => a + t.0 + b, E::B() => 0 } }",
        "enum E { A((Word, Word)), B } fn main() -> Word { let t = (7, 2); let u = t; let e = E::A(u); match e { E::A(v) => v.0, E::B() => 0 } }",
        "struct P { x: (Word, Word) } fn main() -> Word { let p = P { x: (7, 2) }; p.x.0 }",
        "struct P { x: (Word, Word), y: Word } fn main() -> Word { let p = P { y: 3, x: (4, 2) }; p.x.0 + p.y }",
        "struct P { x: [Word; 2] } fn main() -> Word { let p = P { x: [7, 2] }; p.x[0] }",
        "enum E { A((Word, Word)), B } fn take(t: (Word, Word)) -> Word { t.0 } fn main() -> Word { let e = E::A((7, 2)); match e { E::A(t) => take(t), E::B() => 0 } }",
        "enum E { A([Word; 2]), B } fn take(xs: [Word; 2]) -> Word { xs[0] } fn main() -> Word { let e = E::A([7, 2]); match e { E::A(xs) => take(xs), E::B() => 0 } }",
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
fn composite_declarations_retain_ordered_tuple_members() {
    let source = "enum E { A((Word, bool), Byte) } struct P { x: (Byte, Word), y: [Word; 2] } fn main() -> Word { 7 }";
    let (names, records) = keleusma::selfhost::parse_record_trace(source);
    let tuples: Vec<_> = records
        .iter()
        .filter(|(kind, _, _)| (94..=96).contains(kind))
        .map(|(kind, value, _)| {
            (
                *kind,
                if *kind == 95 {
                    names[*value as usize].as_str()
                } else {
                    ""
                },
            )
        })
        .collect();
    assert_eq!(
        tuples,
        [
            (94, ""),
            (95, "Word"),
            (95, "bool"),
            (96, ""),
            (94, ""),
            (95, "Byte"),
            (95, "Word"),
            (96, "")
        ]
    );
}

/// Nested matches must retain declaration coordinates before each binding node.
#[test]
fn payload_binding_metadata_preserves_variant_and_position() {
    let source = "enum E { A((Word, Word), Word), B([Word; 2]) } enum F { C(Word) } fn main(e: E, f: F) -> Word { match e { E::A(t, x) => match f { F::C(y) => t.0 + x + y }, E::B(xs) => xs[0] } }";
    let tag = keleusma::selfhost_host::PARSE_ENUM_PAYLOAD_BIND_TAG;
    let declared: i64 = include_str!("../src/selfhost/kel/parse.kel")
        .lines()
        .find(|line| line.starts_with("fn tag_enum_payload_bind() -> Word {"))
        .and_then(|line| line.split('{').nth(1))
        .and_then(|body| body.split('}').next())
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_eq!(tag, declared);
    let (_, records) = keleusma::selfhost::parse_record_trace(source);
    let coordinates: Vec<_> = records
        .iter()
        .enumerate()
        .filter(|(_, (kind, _, _))| *kind == tag)
        .map(|(at, (_, value, _))| {
            assert_eq!(
                records[at + 1].0,
                42,
                "payload metadata must precede EnumBind"
            );
            *value
        })
        .collect();
    assert_eq!(coordinates, [0, 1, 2048, 1024]);
    assert!(keleusma::selfhost::type_check_source_via_kel(source).unwrap());
}

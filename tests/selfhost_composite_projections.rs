//! Complete composite values must retain source type identity through projections.
#![cfg(feature = "self-host")]

#[test]
fn composite_projection_types_agree_with_reference() {
    let cases = [
        (
            "nested array scalar",
            "fn main(xs: [[Word; 2]; 2]) -> Word { xs[0] }",
            false,
        ),
        (
            "nested array dimension",
            "fn f(xs: [Word; 3]) -> Word { 7 } fn main(xs: [[Word; 2]; 2]) -> Word { f(xs[0]) }",
            false,
        ),
        (
            "nested array alias",
            "fn f(xs: [Word; 3]) -> Word { 7 } fn main(xs: [[Word; 2]; 2]) -> Word { let a = xs[0]; let b = a; f(b) }",
            false,
        ),
        (
            "array tuple member",
            "fn f(t: (Word, bool)) -> Word { 7 } fn main(xs: [(Word, Word); 2]) -> Word { f(xs[0]) }",
            false,
        ),
        (
            "array nominal identity",
            "struct P { x: Word } struct Q { x: Word } fn f(q: Q) -> Word { 7 } fn main(xs: [P; 2]) -> Word { f(xs[0]) }",
            false,
        ),
        (
            "struct array scalar",
            "struct P { x: [Word; 2] } fn main(p: P) -> Word { p.x }",
            false,
        ),
        (
            "struct array dimension",
            "struct P { x: [Word; 2] } fn f(xs: [Word; 3]) -> Word { 7 } fn main(p: P) -> Word { f(p.x) }",
            false,
        ),
        (
            "struct tuple member",
            "struct P { x: (Word, Word) } fn f(t: (Word, bool)) -> Word { 7 } fn main(p: P) -> Word { f(p.x) }",
            false,
        ),
        (
            "struct nominal identity",
            "struct P { x: Word } struct Q { x: Word } struct R { p: P } fn f(q: Q) -> Word { 7 } fn main(r: R) -> Word { f(r.p) }",
            false,
        ),
        (
            "struct field alias",
            "struct P { x: [Word; 2] } fn f(xs: [Word; 3]) -> Word { 7 } fn main(p: P) -> Word { let a = p.x; let b = a; f(b) }",
            false,
        ),
        (
            "struct array nominal element",
            "struct P { x: Word } struct Q { x: Word } struct R { ps: [P; 2] } fn f(q: Q) -> Word { 7 } fn main(r: R) -> Word { f(r.ps[0]) }",
            false,
        ),
        (
            "valid nested array argument",
            "fn f(xs: [Word; 2]) -> Word { 7 } fn main(xs: [[Word; 2]; 2]) -> Word { f(xs[0]) }",
            true,
        ),
        (
            "valid array tuple argument",
            "fn f(t: (Word, Word)) -> Word { 7 } fn main(xs: [(Word, Word); 2]) -> Word { f(xs[0]) }",
            true,
        ),
        (
            "valid array nominal identity",
            "struct P { x: Word } fn f(p: P) -> Word { 7 } fn main(xs: [P; 2]) -> Word { f(xs[0]) }",
            true,
        ),
        (
            "valid struct array argument",
            "struct P { x: [Word; 2] } fn f(xs: [Word; 2]) -> Word { 7 } fn main(p: P) -> Word { f(p.x) }",
            true,
        ),
        (
            "valid struct tuple argument",
            "struct P { x: (Word, Word) } fn f(t: (Word, Word)) -> Word { 7 } fn main(p: P) -> Word { f(p.x) }",
            true,
        ),
        (
            "valid struct nested field",
            "struct P { x: Word } struct R { p: P } fn main(r: R) -> Word { r.p.x }",
            true,
        ),
        (
            "valid struct array nominal element",
            "struct P { x: Word } struct R { ps: [P; 2] } fn main(r: R) -> Word { r.ps[0].x }",
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
    assert!(gaps.is_empty(), "composite projection gaps {gaps:?}");
}

#[test]
fn composite_projections_preserve_artifacts_and_execution() {
    for source in [
        "fn take(xs: [Word; 2]) -> Word { xs[0] } fn main() -> Word { let xs = [[7, 2], [3, 4]]; let a = xs[0]; let b = a; take(b) }",
        "fn take(t: (Word, Word)) -> Word { t.0 } fn choose(xs: [(Word, Word); 2]) -> Word { take(xs[0]) } fn main() -> Word { choose([(7, 2), (3, 4)]) }",
        "struct P { xs: [Word; 2] } fn take(xs: [Word; 2]) -> Word { xs[0] } fn main() -> Word { let p = P { xs: [7, 2] }; take(p.xs) }",
        "struct P { t: (Word, Word) } fn take(t: (Word, Word)) -> Word { t.0 } fn main() -> Word { let p = P { t: (7, 2) }; take(p.t) }",
        "struct P { x: Word } struct R { ps: [P; 2] } fn main() -> Word { let r = R { ps: [P { x: 7 }, P { x: 2 }] }; r.ps[0].x }",
        "struct P { x: Word } struct R { p: P } fn main() -> Word { let r = R { p: P { x: 7 } }; r.p.x }",
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
fn field_projection_metadata_retains_source_coordinates() {
    let source = "struct P { x: Word } struct R { ps: [P; 2], p: P, t: (Word, Word) } fn main(r: R) -> Word { r.ps[0].x + r.p.x + r.t.0 }";
    let tag = keleusma::selfhost_host::PARSE_STRUCT_PROJECTION_TAG;
    let declared: i64 = include_str!("../src/selfhost/kel/parse.kel")
        .lines()
        .find(|line| line.starts_with("fn tag_struct_projection() -> Word {"))
        .and_then(|line| line.split('{').nth(1))
        .and_then(|body| body.split('}').next())
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_eq!(tag, declared);
    let (names, records) = keleusma::selfhost::parse_record_trace(source);
    let fields: Vec<_> = records
        .iter()
        .enumerate()
        .filter(|(_, (kind, _, _))| *kind == tag)
        .map(|(at, (_, coordinates, _))| {
            assert!(
                matches!(records[at + 1].0, 28 | 30),
                "metadata must precede its field read"
            );
            (
                names[(*coordinates / 1024) as usize].as_str(),
                *coordinates % 1024,
            )
        })
        .collect();
    assert_eq!(fields, [("R", 0), ("P", 0), ("R", 1), ("P", 0), ("R", 2)]);
    assert!(keleusma::selfhost::type_check_source_via_kel(source).unwrap());
}

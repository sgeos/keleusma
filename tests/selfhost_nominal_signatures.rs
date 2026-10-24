//! Complete signatures must describe nested values without borrowing reference layouts.
#![cfg(feature = "self-host")]

#[test]
fn nested_nominal_signatures_preserve_complete_artifacts() {
    let cases = [
        (
            "nested parameter",
            "struct Q { x: Word } struct P { q: Q } fn main(p: P) -> Word { p.q.x }",
        ),
        (
            "nested return",
            "struct Q { x: Word } struct P { q: Q } fn main(p: P) -> P { p }",
        ),
        (
            "tuple parameter",
            "struct Q { x: Word } struct P { q: Q } fn main(t: (P, Word)) -> Word { t.0.q.x }",
        ),
        (
            "tuple field",
            "struct Q { x: Word } struct P { q: (Q, Word) } fn main(p: P) -> P { p }",
        ),
        (
            "array field",
            "struct Q { x: Word } struct P { q: [Q; 2] } fn main(p: P) -> P { p }",
        ),
        (
            "enum field",
            "struct Q { x: Word } enum E { A(Q), B } struct P { e: E } fn main(p: P) -> P { p }",
        ),
        (
            "three levels",
            "struct Q { x: Byte, y: bool } struct P { q: Q, z: Word } struct R { p: P } fn main(r: R) -> R { r }",
        ),
        ("empty struct", "struct P {} fn main(p: P) -> P { p }"),
        (
            "forward struct",
            "struct P { q: Q } struct Q { x: Word } fn main(p: P) -> P { p }",
        ),
        #[cfg(feature = "floats")]
        (
            "float signature",
            "fn f(a: Float) -> Float { a } fn main(x: Float) -> Float { f(x) }",
        ),
        #[cfg(feature = "floats")]
        (
            "float field",
            "struct P { x: Float } fn main(p: P) -> P { p }",
        ),
        (
            "nominal array",
            "struct P { x: Word } fn main(xs: [P; 2]) -> [P; 2] { xs }",
        ),
        (
            "nested nominal array",
            "struct P { x: Word } fn main(xs: [[P; 2]; 3]) -> [[P; 2]; 3] { xs }",
        ),
        (
            "tuple nominal array",
            "struct P { x: Word } fn main(xs: [(P, Word); 2]) -> [(P, Word); 2] { xs }",
        ),
        #[cfg(feature = "floats")]
        (
            "float array",
            "fn main(xs: [Float; 2]) -> [Float; 2] { xs }",
        ),
        (
            "large tuple array",
            "fn main(t: ([Word; 10000], Word)) -> ([Word; 10000], Word) { t }",
        ),
    ];
    let mut gaps = Vec::new();
    for (label, source) in cases {
        let reference = keleusma::selfhost::compile_src(source);
        let actual =
            keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host());
        match actual {
            Ok(actual) => {
                let equal = actual.to_bytes().unwrap() == reference.to_bytes().unwrap();
                eprintln!(
                    "{label}: artifact identity {equal}; pipeline {:?}; reference {:?}",
                    actual.signatures, reference.signatures
                );
                if !equal {
                    gaps.push(label);
                }
            }
            Err(error) => {
                eprintln!("{label}: shipping refused {error:?}");
                gaps.push(label);
            }
        }
    }
    assert!(gaps.is_empty(), "nominal signature gaps {gaps:?}");
}

#[test]
fn forward_struct_offsets_execute_against_host_values() {
    use keleusma::bytecode::Value;
    use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
    let source = "struct P { pad: Byte, q: Q, tail: Word } struct Q { x: Word } fn main(p: P) -> Word { p.q.x + p.tail }";
    let reference = keleusma::selfhost::compile_src(source);
    let actual =
        keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host()).unwrap();
    assert_eq!(
        keleusma::selfhost::wire_module_artifact_via_kel(&actual).unwrap(),
        reference.to_bytes().unwrap()
    );
    for module in [actual, reference] {
        let arena = keleusma::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        let q = Value::struct_in_arena(
            "Q".into(),
            vec![("x".into(), Value::Int(7))],
            8,
            8,
            8,
            &arena,
        )
        .unwrap();
        let p = Value::struct_in_arena(
            "P".into(),
            vec![
                ("pad".into(), Value::Byte(3)),
                ("q".into(), q),
                ("tail".into(), Value::Int(5)),
            ],
            8,
            8,
            8,
            &arena,
        )
        .unwrap();
        assert!(matches!(
            vm.call(&[p]).unwrap(),
            VmState::Finished(Value::Int(12))
        ));
    }
}

#[test]
fn nominal_resolution_reaches_the_last_reverse_dependency() {
    let mut source = String::new();
    for index in 0..63 {
        source.push_str(&format!("struct S{index} {{ value: S{} }}\n", index + 1));
    }
    source.push_str("struct S63 { value: Word } fn main(s: S0) -> S0 { s }");
    let reference = keleusma::selfhost::compile_src(&source);
    let actual =
        keleusma::selfhost::self_hosted_compile(&source, &keleusma::target::Target::host())
            .unwrap();
    assert_eq!(actual.to_bytes().unwrap(), reference.to_bytes().unwrap());
}

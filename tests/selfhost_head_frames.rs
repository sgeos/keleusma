//! Head-local frames must include parameter copies, guards and body allocations.
#![cfg(feature = "self-host")]

#[test]
fn head_frames_preserve_complete_artifacts() {
    let cases = [
        (
            "two local heads",
            "fn f(x: Word) -> Word when x == 0 { let a = 3; let b = 4; a + b } fn f(y: Word) -> Word { let a = y; a } fn main() -> Word { f(0) }",
        ),
        (
            "second head execution",
            "fn f(x: Word) -> Word when x == 0 { let a = 3; let b = 4; a + b } fn f(y: Word) -> Word { let a = y; a } fn main() -> Word { f(7) }",
        ),
        (
            "guard temporaries",
            "fn f(x: Word, k: Word) -> Word when (x lsr k) == 0 { let a = 7; a } fn f(y: Word, n: Word) -> Word { let b = y lsr n; b } fn main() -> Word { f(0, 1) }",
        ),
        (
            "body shift temporaries",
            "fn f(x: Word, k: Word) -> Word when x == 0 { x lsr k } fn f(y: Word, n: Word) -> Word { let b = 7; b } fn main() -> Word { f(1, 1) }",
        ),
        (
            "loop locals",
            "fn f(x: Word) -> Word when x == 0 { for i in 0..2 limit 2 { let j = i; } let a = 7; a } fn f(y: Word) -> Word { let b = 7; b } fn main() -> Word { f(1) }",
        ),
        (
            "match locals",
            "fn f(x: Word) -> Word when x == 0 { let a = match x { 0 => 7, _ => 8 }; a } fn f(y: Word) -> Word { let b = 7; b } fn main() -> Word { f(0) }",
        ),
        (
            "guard equality temporaries",
            "fn f(a: (Word, Word), b: (Word, Word)) -> Word when a == b { let n = 7; n } fn f(x: (Word, Word), y: (Word, Word)) -> Word { let n = 8; n } fn main() -> Word { f((1, 2), (1, 2)) }",
        ),
        (
            "shift initializer and later loop",
            "fn main(n: Word, k: Word) -> Word { let a = n lsr k; for i in 0..2 limit 2 { let b = i; } a }",
        ),
        (
            "boolean initializer",
            "fn main(a: bool, b: bool) -> bool { let x = a and b; let y = x; y }",
        ),
        (
            "shift under match",
            "fn main(n: Word, k: Word) -> Word { let a = match (n lsr k) { 0 => 7, _ => 8 }; a }",
        ),
        (
            "shift loop bound",
            "fn main(n: Word, k: Word) -> Word { for i in (n lsr k)..2 limit 2 {} 7 }",
        ),
        (
            "nested shifts",
            "fn main(n: Word, k: Word) -> Word { let a = (n lsr k) lsr k; a }",
        ),
        (
            "unguarded trailing head locals",
            "fn f(x: Word) -> Word when x == 0 { let a = 1; a } fn f(y: Word) -> Word when y == 1 { let b = 2; let c = 3; b + c } fn f(z: Word) -> Word { let d = 7; d } fn main() -> Word { f(2) }",
        ),
    ];
    let mut gaps = Vec::new();
    for (label, source) in cases {
        let reference = keleusma::selfhost::compile_src(source);
        let actual =
            keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host());
        let equal =
            matches!(&actual, Ok(m) if m.to_bytes().unwrap() == reference.to_bytes().unwrap());
        eprintln!("{label}: artifact identity {equal}");
        if !equal {
            eprintln!("{label}: {:?}", actual.err());
            gaps.push(label);
        }
    }
    assert!(gaps.is_empty(), "head frame gaps {gaps:?}");
}

#[test]
fn head_frames_execute_after_keleusma_serialization() {
    use keleusma::bytecode::Value;
    use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};
    for source in [
        "fn f(x: Word, k: Word) -> Word when (x lsr k) == 0 { let a = 7; a } fn f(y: Word, n: Word) -> Word { let b = y lsr n; b } fn main(x: Word) -> Word { f(x, 1) }",
        "fn f(x: Word) -> Word when x == 0 { for i in 0..2 limit 2 { let j = i; } let a = 7; a } fn f(y: Word) -> Word { let b = y / 2; b } fn main(x: Word) -> Word { f(x) }",
    ] {
        let module =
            keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host())
                .unwrap();
        let bytes = keleusma::selfhost::wire_module_artifact_via_kel(&module).unwrap();
        assert_eq!(
            bytes,
            keleusma::selfhost::compile_src(source).to_bytes().unwrap()
        );
        for input in [0, 14] {
            let module = keleusma::selfhost::wire_decode_module_via_kel(&bytes).unwrap();
            let persistent = required_persistent_capacity_for(&module);
            let mut arena = keleusma::Arena::with_capacity(DEFAULT_ARENA_CAPACITY + persistent);
            arena.resize_persistent(persistent).unwrap();
            let mut vm = Vm::new(module, &arena).unwrap();
            assert!(
                matches!(
                    vm.call(&[Value::Int(input)]).unwrap(),
                    VmState::Finished(Value::Int(7))
                ),
                "{source}, input {input}"
            );
        }
    }
}

#[test]
fn unknown_generic_layout_is_deferred_only_when_no_layout_is_needed() {
    use keleusma::selfhost::SelfHostError;
    let plain = "fn id<T>(x: T) -> T { x } fn main() -> Word { id(1) }";
    let error = keleusma::selfhost::self_hosted_compile(plain, &keleusma::target::Target::host())
        .unwrap_err();
    assert!(matches!(error, SelfHostError::Unsupported { .. }));
    assert!(error.to_string().contains("diverges"));
    let tuple = "fn id<T>(x: T) -> T { x } fn main() -> Word { let t = (id(true), 7); t.1 }";
    keleusma::selfhost::compile_src(tuple);
    assert!(matches!(
        keleusma::selfhost::self_hosted_compile(tuple, &keleusma::target::Target::host()),
        Err(SelfHostError::TypeCheckUnavailable { .. })
    ));
}

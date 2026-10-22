//! Data accesses use the final slot partition even when declarations follow functions.
#![cfg(feature = "self-host")]

#[test]
fn late_shared_declarations_preserve_complete_artifacts() {
    let cases = [
        (
            "scalar read and write",
            "private data d { n: Word } fn early() -> Word { d.n = 1; d.n } shared data s { n: Byte } fn main() -> Word { early() }",
        ),
        (
            "indexed read and write",
            "private data d { xs: [Word; 2] } fn early(i: Word) -> Word { d.xs[i] = 7; d.xs[i] } shared data s { n: Byte } fn main() -> Word { early(1) }",
        ),
        (
            "handled indexed write",
            "private data d { xs: [Word; 2] } fn early(i: Word) -> Word { d.xs[i] = 7 { ok(o, n) => o + n, invalid_index(j) => j } } shared data s { n: Byte } fn main() -> Word { early(1) }",
        ),
        (
            "initializer",
            "private data d { n: Word = 5 } fn early() -> Word { d.n = d.n + 1; d.n } shared data s { n: Byte } fn main() -> Word { early() }",
        ),
        (
            "multiple private fields",
            "private data a { pad: [Word; 3], n: Word = 5 } fn early() -> Word { a.n = a.n + 1; a.n } shared data s { n: [Byte; 4] } private data b { n: Word = 8 } fn main() -> Word { b.n = 0; early() }",
        ),
        (
            "shared declaration at eof",
            "private data d { n: Word = 5 } fn main() -> Word { d.n = d.n + 1; d.n } shared data s { n: Byte }",
        ),
        (
            "different head snapshots",
            "private data d { n: Word = 2 } fn f(x: Word) -> Word when d.n == x { d.n = 5; d.n } shared data s { n: Byte } fn f(x: Word) -> Word { d.n = 7; d.n } fn main() -> Word { f(2) }",
        ),
        (
            "unchanged addresses",
            "shared data s { n: Byte } private data d { n: Word = 5 } fn main() -> Word { d.n = d.n + 1; d.n }",
        ),
    ];
    let mut gaps = Vec::new();
    for (label, source) in cases {
        let reference = keleusma::selfhost::compile_src(source);
        assert!(
            keleusma::selfhost::type_check_source_via_kel(source).unwrap(),
            "{label}"
        );
        match keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host()) {
            Ok(actual) => {
                let equal = actual.to_bytes().unwrap() == reference.to_bytes().unwrap();
                eprintln!("{label}: complete artifact identity {equal}");
                if !equal {
                    gaps.push(label);
                }
            }
            Err(error) => {
                eprintln!("{label}: {error:?}");
                gaps.push(label);
            }
        }
    }
    assert!(gaps.is_empty(), "data relocation gaps {gaps:?}");
}

#[test]
fn relocated_modules_execute_and_all_compile_paths_agree() {
    use keleusma::bytecode::Value;
    use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};
    for (source, expected) in [
        (
            "private data d { n: Word = 5 } fn early() -> Word { d.n = d.n + 1; d.n } shared data s { n: Byte } fn main() -> Word { early() }",
            6,
        ),
        (
            "private data d { xs: [Word; 2] } fn early(i: Word) -> Word { d.xs[i] = 7; d.xs[i] } shared data s { n: Byte } fn main() -> Word { early(1) }",
            7,
        ),
        // A handled write's data operand travels on its root record, not on a
        // statement record, so it is relocated separately.
        (
            "private data d { xs: [Word; 2] } fn early(i: Word) -> Word { d.xs[i] = 7 { ok(o, n) => o + n, invalid_index(j) => j } } shared data s { n: Byte } fn main() -> Word { early(1) }",
            7,
        ),
        (
            "private data d { n: Word = 2 } fn f(x: Word) -> Word when d.n == x { d.n = 5; d.n } shared data s { n: Byte } fn f(x: Word) -> Word { d.n = 7; d.n } fn main() -> Word { f(2) }",
            5,
        ),
    ] {
        let reference = keleusma::selfhost::compile_src(source);
        let expected_bytes = reference.to_bytes().unwrap();
        for module in [
            keleusma::selfhost::self_host_compile(source),
            keleusma::selfhost::self_host_compile_fused(source),
            keleusma::selfhost::self_host_compile_full(source),
            keleusma::selfhost::self_host_compile_scratch(source),
            keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host())
                .unwrap(),
        ] {
            let bytes = keleusma::selfhost::wire_module_artifact_via_kel(&module).unwrap();
            assert_eq!(bytes, expected_bytes, "{source}");
            let module = keleusma::selfhost::wire_decode_module_via_kel(&bytes).unwrap();
            let persistent = required_persistent_capacity_for(&module);
            let mut arena = keleusma::Arena::with_capacity(DEFAULT_ARENA_CAPACITY + persistent);
            arena.resize_persistent(persistent).unwrap();
            let mut vm = Vm::new(module, &arena).unwrap();
            let mut shared = vec![0; vm.shared_data_bytes()];
            assert!(
                matches!(vm.call_with_shared(&mut shared, &[]).unwrap(), VmState::Finished(Value::Int(value)) if value == expected),
                "{source}"
            );
        }
    }
}

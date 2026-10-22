//! Structural type identity must preserve tuple positions and array dimensions.
#![cfg(feature = "self-host")]

#[test]
fn tuple_and_nested_array_facts_agree_with_reference() {
    let cases = [
        (
            "tuple argument",
            "fn f(x: (Word, bool)) -> Word { 7 } fn main() -> Word { f(1) }",
            false,
        ),
        (
            "tuple element",
            "fn f(x: (Word, bool)) -> Word { 7 } fn main() -> Word { f((1, 2)) }",
            false,
        ),
        (
            "tuple arity",
            "fn f(x: (Word, bool)) -> Word { 7 } fn main() -> Word { f((1, true, 3)) }",
            false,
        ),
        (
            "tuple return",
            "fn f() -> (Word, bool) { (true, 1) } fn main() -> Word { 7 }",
            false,
        ),
        (
            "tuple scalar return",
            "fn f(x: (Word, bool)) -> Word { x } fn main() -> Word { 7 }",
            false,
        ),
        (
            "tuple branch identity",
            "fn main() -> Word { let x = if true { (1, true) } else { (true, 1) }; 7 }",
            false,
        ),
        (
            "valid tuple",
            "fn f(x: (Word, bool)) -> Word { 7 } fn main() -> Word { f((1, true)) }",
            true,
        ),
        (
            "valid tuple alias",
            "fn f(x: (Word, bool)) -> (Word, bool) { x } fn main() -> Word { let a = (1, true); let b = a; let c = f(b); 7 }",
            true,
        ),
        (
            "nested array element",
            "fn f(x: [[Word; 2]; 2]) -> Word { 7 } fn main() -> Word { f([[true, false], [false, true]]) }",
            false,
        ),
        (
            "nested array inner length",
            "fn f(x: [[Word; 2]; 2]) -> Word { 7 } fn main() -> Word { f([[1], [2]]) }",
            false,
        ),
        (
            "nested array outer length",
            "fn f(x: [[Word; 2]; 2]) -> Word { 7 } fn main() -> Word { f([[1, 2]]) }",
            false,
        ),
        (
            "nested array scalar",
            "fn f(x: [[Word; 2]; 2]) -> Word { 7 } fn main() -> Word { f(1) }",
            false,
        ),
        (
            "valid nested array",
            "fn f(x: [[Word; 2]; 2]) -> Word { 7 } fn main() -> Word { f([[1, 2], [3, 4]]) }",
            true,
        ),
        (
            "array of tuple",
            "fn f(x: [(Word, bool); 2]) -> Word { 7 } fn main() -> Word { f([(1, 2), (3, 4)]) }",
            false,
        ),
        (
            "valid array of tuple",
            "fn f(x: [(Word, bool); 2]) -> Word { 7 } fn main() -> Word { f([(1, true), (3, false)]) }",
            true,
        ),
        (
            "nested array return",
            "fn f() -> [[Word; 2]; 2] { [[true, false], [false, true]] } fn main() -> Word { 7 }",
            false,
        ),
        (
            "valid nested array return",
            "fn f() -> [[Word; 2]; 2] { [[1, 2], [3, 4]] } fn main() -> Word { 7 }",
            true,
        ),
        (
            "array of tuple return",
            "fn f() -> [(Word, bool); 2] { [(1, 2), (3, 4)] } fn main() -> Word { 7 }",
            false,
        ),
        (
            "valid array of tuple return",
            "fn f() -> [(Word, bool); 2] { [(1, true), (3, false)] } fn main() -> Word { 7 }",
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
    assert!(gaps.is_empty(), "structural gaps {gaps:?}");
}

#[test]
fn structural_signatures_preserve_artifacts_and_execution() {
    for source in [
        "fn take(x: (Word, Word)) -> Word { x.0 } fn main() -> Word { take((7, 2)) }",
        "fn take(x: [[Word; 2]; 2]) -> Word { x[0][0] } fn main() -> Word { take([[7, 2], [3, 4]]) }",
        "fn take(x: [(Word, Word); 2]) -> Word { 7 } fn main() -> Word { take([(7, 2), (3, 4)]) }",
        "fn make() -> (Word, Word) { (7, 2) } fn main() -> Word { let x = make(); x.0 }",
        "fn make() -> [[Word; 2]; 2] { [[7, 2], [3, 4]] } fn main() -> Word { let x = make(); 7 }",
        "fn make() -> [(Word, Word); 2] { [(7, 2), (3, 4)] } fn main() -> Word { let x = make(); 7 }",
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

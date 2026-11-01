//! Empty literals retain their known shape and length while their element stays unknown.
#![cfg(feature = "self-host")]

#[test]
fn empty_array_constraints_preserve_valid_sources_and_reject_length_mismatches() {
    for (label, source, expected) in [
        ("unused", "fn main() -> Word { let xs = []; 7 }", true),
        (
            "argument",
            "fn f(xs: [Word; 2]) -> Word { 7 } fn main() -> Word { f([]) }",
            false,
        ),
        ("return", "fn main() -> Word { [] }", false),
        (
            "different lengths",
            "fn main() -> bool { [] == [1] }",
            false,
        ),
        ("empty equality", "fn main() -> bool { [] == [] }", true),
        (
            "alias argument",
            "fn f(xs: [Word; 2]) -> Word { 7 } fn main() -> Word { let xs = []; let ys = xs; f(ys) }",
            false,
        ),
        (
            "annotation",
            "fn main() -> Word { let xs: [Word; 1] = []; 7 }",
            false,
        ),
        (
            "branch length",
            "fn main() -> Word { let xs = if true { [] } else { [1] }; 7 }",
            false,
        ),
        (
            "unused alias",
            "fn main() -> Word { let xs = []; let ys = xs; 7 }",
            true,
        ),
        (
            "unknown element",
            "fn main() -> Word { let xs = []; xs[0] }",
            true,
        ),
    ] {
        let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
        assert_eq!(
            keleusma::compiler::compile(&ast).is_ok(),
            expected,
            "{label}"
        );
        assert_eq!(
            keleusma::selfhost::type_check_source_via_kel(source).unwrap(),
            expected,
            "{label}"
        );
        if !expected {
            assert!(
                matches!(
                    keleusma::selfhost::self_hosted_compile(
                        source,
                        &keleusma::target::Target::host()
                    ),
                    Err(keleusma::selfhost::SelfHostError::PipelineRejected { .. })
                ),
                "{label}"
            );
        }
    }
}

#[test]
fn empty_array_controls_preserve_artifacts_and_execution() {
    use keleusma::bytecode::Value;
    use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmError, VmState};
    for (source, expected) in [
        ("fn main() -> Word { let xs = []; 7 }", Some(Value::Int(7))),
        (
            "fn main() -> Word { let xs = []; let ys = xs; 7 }",
            Some(Value::Int(7)),
        ),
        // The reference accepts this source but retains a runtime comparison fault.
        ("fn main() -> bool { [] == [] }", None),
    ] {
        let reference = keleusma::selfhost::compile_src(source);
        let actual =
            keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host())
                .unwrap();
        assert_eq!(
            keleusma::selfhost::wire_module_artifact_via_kel(&actual).unwrap(),
            reference.to_bytes().unwrap()
        );
        for module in [actual, reference] {
            let arena = keleusma::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
            let mut vm = Vm::new(module, &arena).unwrap();
            let result = vm.call(&[]);
            if let Some(expected) = &expected {
                assert!(
                    matches!(result.unwrap(), VmState::Finished(value) if &value == expected),
                    "{source}"
                );
            } else {
                assert!(
                    matches!(result, Err(VmError::TypeError(message)) if message.starts_with("cannot compare a flat composite")),
                    "{source}"
                );
            }
        }
    }
}

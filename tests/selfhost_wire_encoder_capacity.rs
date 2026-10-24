//! Serializer transport windows must preserve existing production source admission.
#![cfg(feature = "self-host")]
use keleusma::selfhost::{self_hosted_compile, wire_module_artifact_via_kel};
use keleusma::target::Target;

fn assert_source(source: &str, expected_roots: usize) {
    let module = self_hosted_compile(source, &Target::host())
        .expect("production compiler already accepts this source");
    assert_eq!(
        module
            .chunks
            .iter()
            .map(|c| c.constants.len())
            .sum::<usize>(),
        expected_roots
    );
    let expected = module.to_bytes().unwrap();
    let actual = wire_module_artifact_via_kel(&module)
        .expect("Keleusma serialization must retain source admission");
    assert_eq!(actual, expected);
}
#[test]
fn accepted_scalar_forests_cross_the_old_1365_node_boundary() {
    let mut helpers = String::new();
    for i in 0..170 {
        helpers.push_str(&format!(
            "fn f{i}(x: Word) -> Word {{ x + 1 + 2 + 3 + 4 + 5 + 6 + 7 + 8 }}\n"
        ));
    }
    for (tail, roots) in [
        ("f0(0) + 1 + 2 + 3 + 4", 1365),
        ("f0(0) + 1 + 2 + 3 + 4 + 5", 1366),
    ] {
        assert_source(&format!("{helpers}fn main() -> Word {{ {tail} }}"), roots);
    }
}
#[test]
fn accepted_text_crosses_name_and_aggregate_input_windows() {
    for length in [256, 257, 60000] {
        assert_source(
            &format!("fn main() -> Text {{ \"{}\" }}", "x".repeat(length)),
            1,
        );
    }
}

#[test]
fn colliding_text_hashes_remain_distinct_during_execution() {
    use keleusma::bytecode::{Module, Value};
    use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
    let source = "fn main() -> Word { if \"6a2ef2cd\" == \"f41fb47a\" { 0 } else { 7 } }";
    let module = self_hosted_compile(source, &Target::host()).unwrap();
    let bytes = wire_module_artifact_via_kel(&module).unwrap();
    assert_eq!(bytes, module.to_bytes().unwrap());
    drop(module);
    let decoded = Module::from_bytes(&bytes).unwrap();
    let arena = keleusma::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(decoded, &arena).unwrap();
    assert!(matches!(
        vm.call(&[]).unwrap(),
        VmState::Finished(Value::Int(7))
    ));
}

//! Enclosing operators must wait until a conditional operand is complete.
#![cfg(feature = "self-host")]

use keleusma::Arena;
use keleusma::bytecode::{Module, Value};
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

fn execute(module: Module) -> i64 {
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).unwrap();
    let mut vm = Vm::new(module, &arena).unwrap();
    match vm.call(&[]).unwrap() {
        VmState::Finished(Value::Int(value)) => value,
        other => panic!("unexpected result {other:?}"),
    }
}

#[test]
fn conditional_operands_preserve_artifacts_and_values() {
    let cases = [
        (
            "addition",
            "fn main() -> Word { 10 + if true { 1 } else { 2 } }",
            11,
        ),
        (
            "division",
            "fn main() -> Word { 9 / if true { 3 } else { 0 } }",
            3,
        ),
        (
            "modulo",
            "fn main() -> Word { 11 % if false { 0 } else { 3 } }",
            2,
        ),
        (
            "negation",
            "fn main() -> Word { -if true { 4 } else { 6 } }",
            -4,
        ),
        (
            "nested condition",
            "fn main() -> Word { if (if true { 1 } else { 2 }) == 1 { 7 } else { 8 } }",
            7,
        ),
        (
            "call argument",
            "fn f(a: Word, b: Word) -> Word { a + b } fn main() -> Word { f(1 + if true { 2 } else { 3 }, 5) }",
            8,
        ),
        (
            "branch statements",
            "fn main() -> Word { 10 + if true { let x = 2; x * 3 } else { let x = 8; x } }",
            16,
        ),
        (
            "boolean",
            "fn main() -> Word { if false orelse if true { true } else { false } { 7 } else { 8 } }",
            7,
        ),
    ];
    let mut failures = Vec::new();
    for (name, source, expected) in cases {
        let reference = keleusma::selfhost::compile_src(source);
        assert_eq!(execute(reference.clone()), expected, "reference {name}");
        match keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host()) {
            Ok(actual) => {
                if actual.to_bytes().unwrap() != reference.to_bytes().unwrap() {
                    failures.push(format!("{name}: different artifacts"));
                } else {
                    assert_eq!(execute(actual), expected, "pipeline {name}");
                }
            }
            Err(error) => failures.push(format!("{name}: {error}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn conditional_divisor_preserves_left_to_right_effects() {
    let source = "private data d { trace: Word } fn f(n: Word) -> Word { d.trace = d.trace * 10 + n; n } fn main() -> Word { let result = f(1) / if f(2) > 0 { f(3) } else { f(4) }; d.trace + result }";
    let reference = keleusma::selfhost::compile_src(source);
    assert_eq!(execute(reference.clone()), 123);
    let actual =
        keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host()).unwrap();
    assert!(actual.to_bytes().unwrap() == reference.to_bytes().unwrap());
    assert_eq!(execute(actual), 123);
}

#[test]
fn a_conditional_zero_divisor_reaches_the_explicit_trap() {
    use keleusma::bytecode::{Op, TrapKind};
    use keleusma::vm::VmError;
    let source = "fn main() -> Word { 9 / if false { 3 } else { 0 } }";
    let actual =
        keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host()).unwrap();
    assert!(
        actual.to_bytes().unwrap() == keleusma::selfhost::compile_src(source).to_bytes().unwrap()
    );
    let ops = actual.chunks[0].ops.clone();
    let need = required_persistent_capacity_for(&actual);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).unwrap();
    let mut vm = Vm::new(actual, &arena).unwrap();
    assert!(matches!(vm.call(&[]), Err(VmError::DivisionByZero)));
    let (chunk, op) = vm.fault_location().unwrap();
    assert_eq!(chunk, 0);
    assert!(matches!(ops[op], Op::Trap(kind) if kind == TrapKind::ZeroDivisor.code()));
}

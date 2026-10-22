//! Bare first-pass partial arithmetic must fail through an explicit Trap.
#![cfg(all(feature = "compile", feature = "verify"))]
use keleusma::Arena;
use keleusma::bytecode::{Module, Op, TrapKind, Value};
use keleusma::vm::{
    DEFAULT_ARENA_CAPACITY, Vm, VmError, VmState, required_persistent_capacity_for,
};

fn compile(source: &str) -> Module {
    let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
    keleusma::compiler::compile(&ast).unwrap()
}
fn execute(module: Module) -> Result<Value, VmError> {
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).unwrap();
    let mut vm = Vm::new(module, &arena).unwrap();
    match vm.call(&[])? {
        VmState::Finished(Value::Int(n)) => Ok(Value::Int(n)),
        VmState::Finished(Value::Byte(n)) => Ok(Value::Byte(n)),
        other => panic!("unexpected state {other:?}"),
    }
}
fn explicit_zero_trap(module: &Module) {
    let ops: Vec<_> = module.chunks.iter().flat_map(|c| &c.ops).collect();
    assert!(
        ops.iter()
            .any(|op| matches!(op, Op::Trap(k) if *k == TrapKind::ZeroDivisor.code()))
    );
    assert!(!ops.iter().any(|op| matches!(op, Op::Div | Op::Mod)));
}
const ZERO: &[&str] = &[
    "fn main() -> Word { 7 / 0 }",
    "fn main() -> Word { let z = 0; 7 / z }",
    "fn main() -> Word { let z = 0; 7 % z }",
    "fn main() -> Byte { let n = 7 as Byte; let z = 0 as Byte; n / z }",
    "fn main() -> Byte { let n = 7 as Byte; let z = 0 as Byte; n % z }",
];
#[test]
fn word_and_byte_zero_divisors_reach_an_explicit_trap() {
    for source in ZERO {
        let module = compile(source);
        explicit_zero_trap(&module);
        assert!(
            matches!(execute(module), Err(VmError::DivisionByZero)),
            "{source}"
        );
    }
}
#[test]
fn wrapping_sign_and_byte_results_are_preserved() {
    for (source, expected) in [
        ("fn main() -> Word { (0 - 7) / 3 }", Value::Int(-2)),
        ("fn main() -> Word { (0 - 7) % 3 }", Value::Int(-1)),
        (
            "fn main() -> Word { let n = 0 - 9223372036854775807 - 1; n / (0 - 1) }",
            Value::Int(i64::MIN),
        ),
        (
            "fn main() -> Word { let n = 0 - 9223372036854775807 - 1; n % (0 - 1) }",
            Value::Int(0),
        ),
        (
            "fn main() -> Byte { (255 as Byte) / (2 as Byte) }",
            Value::Byte(127),
        ),
        (
            "fn main() -> Byte { (255 as Byte) % (2 as Byte) }",
            Value::Byte(1),
        ),
    ] {
        let module = compile(source);
        explicit_zero_trap(&module);
        assert_eq!(execute(module).unwrap(), expected, "{source}");
    }
}
#[cfg(feature = "self-host")]
#[test]
fn both_compilers_emit_identical_explicit_division_guards() {
    for source in ZERO.iter().copied().chain([
        "fn main() -> Word { 5 + (20 / 3) * (7 % 3) }",
        "fn main() -> Byte { (255 as Byte) / (2 as Byte) }",
        "fn main() -> Byte { (255 as Byte) % (2 as Byte) }",
        "fn f(n: Word) -> Word { n } fn main() -> Word { f(7) / f(2) }",
    ]) {
        let expected = compile(source);
        let actual = keleusma::selfhost::self_host_compile_scratch(source);
        explicit_zero_trap(&actual);
        assert_eq!(
            actual.to_bytes().unwrap(),
            expected.to_bytes().unwrap(),
            "{source}"
        );
    }
}

#[test]
fn division_respects_the_declared_word_width_on_a_wider_runtime() {
    for (body, expected) in [
        ("let n = 0 - 32767 - 1; n / (0 - 1)", -32768),
        ("let n = 0 - 32767 - 1; n % (0 - 1)", 0),
        (
            "let n = 0 - 32767 - 1; n / (0 - 1) { ok(v) => v, overflow(h, l) => l }",
            -32768,
        ),
    ] {
        let source = format!("fn main() -> Word {{ {body} }}");
        let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(&source).unwrap()).unwrap();
        let module =
            keleusma::compiler::compile_with_target(&ast, &keleusma::target::Target::embedded_16())
                .unwrap();
        assert_eq!(execute(module).unwrap(), Value::Int(expected), "{source}");
    }
}

#[test]
fn handled_arithmetic_has_no_unreachable_zero_divisor_trap() {
    for source in [
        "fn main() -> Word { 7 + 2 { ok(v) => v } }",
        "fn main() -> Word { 7 / 0 { ok(v) => v, zero_divisor(n) => n } }",
        "fn main() -> Word { 7 % 0 { ok(v) => v, zero_divisor(_) => 9 } }",
        "fn main() -> Byte { (7 as Byte) / (0 as Byte) { ok(v) => v, zero_divisor(n) => n } }",
    ] {
        let module = compile(source);
        assert!(
            !module
                .chunks
                .iter()
                .flat_map(|chunk| &chunk.ops)
                .any(|op| matches!(op, Op::Trap(_))),
            "{source}"
        );
        execute(module).unwrap();
    }
}

#[test]
fn guarded_and_refutable_zero_divisor_handlers_retain_the_fallback() {
    for source in [
        "fn main() -> Word { 7 / 0 { ok(v) => v, zero_divisor(n) when n == 9 => n } }",
        "fn main() -> Word { 7 % 0 { ok(v) => v, zero_divisor(9) => 1 } }",
    ] {
        let module = compile(source);
        explicit_zero_trap(&module);
        assert!(matches!(execute(module), Err(VmError::DivisionByZero)));
    }
}

#[cfg(all(feature = "self-host", feature = "floats"))]
#[test]
fn both_compilers_preserve_noninteger_division_paths() {
    for source in [
        "fn main(a: Float, b: Float) -> Float { (a / b) % (a / b) }",
        "fn main(a: Float, b: Float) -> Float { let x = a / b; x / b }",
        "fn f(a: Float, b: Float) -> Float { a / b } fn main(a: Float, b: Float) -> Float { f(a, b) / b }",
        "fn main(a: Fixed<16>, b: Fixed<16>) -> Fixed<16> { (a % b) % b }",
    ] {
        let expected = compile(source);
        let actual = keleusma::selfhost::self_host_compile_scratch(source);
        assert_eq!(
            actual.to_bytes().unwrap(),
            expected.to_bytes().unwrap(),
            "{source}"
        );
        let has_trap = actual
            .chunks
            .iter()
            .flat_map(|c| &c.ops)
            .any(|op| matches!(op, Op::Trap(_)));
        assert_eq!(has_trap, source.contains("Fixed<16>"));
    }
}

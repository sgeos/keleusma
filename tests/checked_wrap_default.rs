//! The wrapping default of a checked arithmetic construct is typed, and it is
//! not emitted where it cannot run.
//!
//! # The defect
//!
//! An outcome class with no unguarded catch-all arm falls to the wrapping
//! default, which pushes the operation's own value. The type checker unified
//! the arm bodies with one another but never with the operand, so
//! `fn main(x: Word) -> bool { x + 1 { ok(v) => v > 0 } }` was admitted, passed
//! the verifier, and returned `Int(i64::MIN)` from a `bool` function on
//! overflow. Measured on 2026-10-29 before the fix.
//!
//! # The rule
//!
//! When the default is reachable, the construct's type must be the operand's.
//! When unguarded catch-all arms cover every class that can arise and no
//! zero-divisor trap is pending, the default is unreachable. The last arm is
//! then an unguarded catch-all, and the compiler omits its class test and the
//! default, so the value after the construct has only the arms' kind and no
//! trap is added. Both compilers decide this in the same way.
//!
//! # What these tests do NOT establish
//!
//! They do not show that the class table matches the virtual machine for every
//! operator and operand family. They pin the families named below. One place
//! where the machine set a flag that no admissible arm covered was found while
//! writing them, `Fixed` division and its underflow, and that arm is now
//! admissible.
#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::Arena;
use keleusma::bytecode::{Module, Op, Value};
use keleusma::vm::{Vm, VmState};

fn compiled(source: &str) -> Result<Module, String> {
    let tokens = keleusma::lexer::tokenize(source).map_err(|e| format!("{e:?}"))?;
    let program = keleusma::parser::parse(&tokens).map_err(|e| format!("{e:?}"))?;
    keleusma::compiler::compile(&program).map_err(|e| e.message)
}

/// Whether the main chunk still carries the wrapping default, which is a read
/// of the low slot followed by a `Break` just before the virtual loop ends.
/// The test programs hold one construct each, so one search suffices.
fn has_default(module: &Module) -> bool {
    let main = module
        .chunks
        .iter()
        .find(|c| c.name == "main")
        .expect("a main chunk");
    main.ops
        .windows(3)
        .any(|w| matches!(w, [Op::GetLocal(_), Op::Break(_), Op::EndLoop(_)]))
}

/// No construct here may add a trap, so a fully handled one stays trap-free.
fn has_trap(module: &Module) -> bool {
    module
        .chunks
        .iter()
        .flat_map(|c| &c.ops)
        .any(|op| matches!(op, Op::Trap(_)))
}

fn run(module: Module, arg: Value) -> Value {
    let arena = Arena::with_capacity(1 << 16);
    let mut vm = Vm::new(module, &arena).expect("the verifier admits the module");
    match vm.call(&[arg]).expect("the call finishes") {
        VmState::Finished(v) => v,
        other => panic!("unexpected state {other:?}"),
    }
}

/// Constructs whose default is reachable and whose arms do not produce the
/// operand's type. Each is refused by the new rule and by nothing else.
const REFUSED: [&str; 5] = [
    // The measured defect.
    "fn main(x: Word) -> bool { x + 1 { ok(v) => v > 0 } }",
    // Underflow uncovered.
    "fn main(x: Word) -> bool { x + 1 { ok(v) => v > 0, overflow(_, _) => false } }",
    // A guarded arm does not cover its class.
    "fn main(x: Word) -> bool { x + 1 { ok(v) => v > 0, overflow(h, _) when h > 0 => false, underflow(_, _) => false } }",
    // A literal pattern does not cover its class.
    "fn main(x: Word) -> bool { x + 1 { ok(v) => v > 0, overflow(0, _) => false, underflow(_, _) => false } }",
    // Byte subtraction can underflow.
    "fn main(x: Byte) -> bool { x - (1 as Byte) { ok(_) => true } }",
];

#[test]
fn a_reachable_default_requires_the_operand_type() {
    for source in REFUSED {
        let message = compiled(source).expect_err(source);
        assert!(
            message.contains("wrapping default"),
            "{source} was refused, but not by the default rule: {message}"
        );
    }
}

#[test]
fn a_reachable_default_with_the_operand_type_is_unchanged() {
    let module = compiled("fn main(x: Word) -> Word { x + 1 { ok(v) => v } }").unwrap();
    assert!(has_default(&module));
    assert_eq!(run(module, Value::Int(i64::MAX)), Value::Int(i64::MIN));
}

/// Covered constructs are admitted with another result type, and they carry
/// neither the default nor a trap.
#[test]
fn a_covered_construct_replaces_the_default_with_the_trap() {
    let cases: [(&str, Value, Value); 4] = [
        (
            "fn main(x: Word) -> bool { x + 1 { ok(v) => v > 0, overflow(_, _) => false, underflow(_, _) => false } }",
            Value::Int(i64::MAX),
            Value::Bool(false),
        ),
        // Byte addition cannot underflow, so overflow alone covers it.
        (
            "fn main(x: Byte) -> bool { x + (1 as Byte) { ok(_) => true, overflow(_) => false } }",
            Value::Byte(255),
            Value::Bool(false),
        ),
        // Word modulo has no wrapping class, and the zero divisor is covered.
        (
            "fn main(x: Word) -> bool { x % 3 { ok(v) => v > 0, zero_divisor(_) => false } }",
            Value::Int(7),
            Value::Bool(true),
        ),
        // Word negation can only overflow.
        (
            "fn main(x: Word) -> bool { -x { ok(_) => true, overflow(_, _) => false } }",
            Value::Int(i64::MIN),
            Value::Bool(false),
        ),
    ];
    for (source, arg, expected) in cases {
        let module = compiled(source).unwrap_or_else(|e| panic!("{source}: {e}"));
        assert!(!has_default(&module), "{source} kept the default");
        assert!(!has_trap(&module), "{source} carries a trap");
        assert_eq!(run(module, arg), expected, "{source}");
    }
}

/// `Fixed` division can set the underflow flag, so it keeps its default
/// until an `underflow` arm covers that class. Before this change no such arm
/// was admissible, so the flag always fell to the default.
#[test]
fn fixed_division_needs_its_underflow_class_covered() {
    let partial =
        "fn main(x: Fixed) -> bool { x / (1 as Fixed) { ok(_) => true, overflow(_) => false } }";
    let message = compiled(partial).expect_err(partial);
    assert!(message.contains("wrapping default"), "{message}");
    let full = "fn main(x: Fixed) -> bool { x / (1 as Fixed) { ok(_) => true, overflow(_) => false, underflow(_) => false, zero_divisor(_) => false } }";
    let module = compiled(full).unwrap_or_else(|e| panic!("{full}: {e}"));
    assert!(!has_default(&module));
    assert!(!has_trap(&module));
}

/// A Float operation can also produce a NaN, which only a `nan` arm covers.
#[cfg(feature = "floats")]
#[test]
fn float_needs_its_nan_class_covered() {
    let partial = "fn main(x: Float) -> bool { x + 1.0 { ok(_) => true, overflow(_) => false, underflow(_) => false } }";
    let message = compiled(partial).expect_err(partial);
    assert!(message.contains("wrapping default"), "{message}");
    let full = "fn main(x: Float) -> bool { x + 1.0 { ok(_) => true, overflow(_) => false, underflow(_) => false, nan(_) => false } }";
    let module = compiled(full).unwrap_or_else(|e| panic!("{full}: {e}"));
    assert!(!has_default(&module));
    assert!(!has_trap(&module));
    assert_eq!(run(module, Value::Float(1.0)), Value::Bool(true));
}

/// The self-hosted compiler agrees, byte for byte where it admits a program,
/// and its independent type stage refuses what the reference refuses.
#[cfg(feature = "self-host")]
#[test]
fn the_self_hosted_compiler_agrees() {
    use keleusma::selfhost::{compile_src, self_hosted_compile};
    use keleusma::target::Target;
    let admitted = [
        "fn main(x: Word) -> Word { x + 1 { ok(v) => v } }",
        "fn main(x: Word) -> bool { x + 1 { ok(v) => v > 0, overflow(_, _) => false, underflow(_, _) => false } }",
        "fn main(x: Byte) -> bool { x + (1 as Byte) { ok(_) => true, overflow(_) => false } }",
        "fn main(x: Word) -> bool { x % 3 { ok(v) => v > 0, zero_divisor(_) => false } }",
        "fn main(x: Word) -> Word { x % 3 { ok(v) => v } }",
        "fn main(x: Word) -> bool { -x { ok(_) => true, overflow(_, _) => false } }",
        "fn main(x: Word) -> Word { x * 2 { ok(v) => v, overflow(h, l) => h + l, underflow(_, l) => l } }",
        "fn main(x: Word) -> Word { x / 0 { ok(v) => v, overflow(_, l) => l, zero_divisor(n) => n } }",
    ];
    for source in admitted {
        let reference = compile_src(source);
        let actual = self_hosted_compile(source, &Target::host())
            .unwrap_or_else(|e| panic!("{source}: {e}"));
        assert_eq!(
            actual.to_bytes().unwrap(),
            reference.to_bytes().unwrap(),
            "{source}"
        );
    }
    // `self_hosted_compile` runs the reference first, so the independent type
    // stage is asked directly. Each verdict is from the stage alone.
    for source in REFUSED {
        assert!(
            !keleusma::selfhost::type_check_source_via_kel(source).unwrap(),
            "the self-hosted type stage admitted {source}"
        );
    }
    for source in admitted {
        assert!(
            keleusma::selfhost::type_check_source_via_kel(source).unwrap(),
            "the self-hosted type stage refused {source}"
        );
    }
}

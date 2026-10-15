//! Checked arithmetic left shifts preserve the bounded multiplier contract.
#![cfg(feature = "self-host")]

use keleusma::Arena;
use keleusma::bytecode::{Module, Op, Value};
use keleusma::selfhost::{compile_src, self_hosted_compile};
use keleusma::target::Target;
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
fn checked_shifts_preserve_artifacts_values_and_pool_order() {
    for source in [
        "fn main() -> Word { 3 asl 2 { ok(v) => v } }",
        "fn main() -> Word { 3 asl 0 { ok(v) => v } }",
        "fn main() -> Word { 1 asl 62 { ok(v) => v } }",
        "fn main() -> Word { 3 asl 62 { overflow(h, l) => h + l, ok(v) => v } }",
        "fn main() -> Word { (1 lsl 63) asl 1 { underflow(-1, l) => l, underflow(h, l) => h + l, ok(v) => v } }",
        "fn main() -> Word { (2 + 3) asl 2 { ok(v) => v } }",
        "fn main() -> Word { (2 + 3) asl 2 { overflow(h, l) when h == 0 => l, ok(v) => v } }",
        "fn main() -> Word { (3 asl 2 { ok(v) => v }) asl 3 { ok(v) => v } }",
        "fn main() -> Word { 3 asl 2 { ok(v) => v asl 1 { ok(w) => w } } }",
        "private data d { trace: Word } fn f(x: Word) -> Word { d.trace = d.trace * 10 + x; x } fn main() -> Word { let n = (f(2) + f(3)) asl 2 { ok(v) => v }; d.trace + n }",
    ] {
        let reference = compile_src(source);
        let actual = self_hosted_compile(source, &Target::host())
            .unwrap_or_else(|error| panic!("{source}\n{error}"));
        assert_eq!(
            actual.to_bytes().unwrap(),
            reference.to_bytes().unwrap(),
            "{source}"
        );
        assert_eq!(execute(actual), execute(reference), "{source}");
    }
}

#[test]
fn checked_shift_has_no_unreachable_trap() {
    let module = self_hosted_compile(
        "fn main() -> Word { 7 asl 2 { ok(v) => v } }",
        &Target::host(),
    )
    .unwrap();
    assert!(
        module.chunks[0]
            .ops
            .iter()
            .any(|op| matches!(op, Op::CheckedMul(0)))
    );
    assert!(
        !module.chunks[0]
            .ops
            .iter()
            .any(|op| matches!(op, Op::Trap(_)))
    );
    assert_eq!(execute(module), 28);
}

#[test]
fn invalid_checked_shift_amounts_and_families_are_refused() {
    for source in [
        "fn main() -> Word { 1 asl 63 { ok(v) => v } }",
        "fn main() -> Word { 1 asl 64 { ok(v) => v } }",
        "fn main() -> Word { 1 asl -1 { ok(v) => v } }",
        "fn f(n: Word) -> Word { 1 asl n { ok(v) => v } } fn main() -> Word { f(2) }",
        "fn main() -> Word { 1 lsl 2 { ok(v) => v } }",
        "fn main() -> Word { 1 asr 2 { ok(v) => v } }",
        "fn main() -> Word { 1 lsr 2 { ok(v) => v } }",
        "fn main() -> Byte { (1 as Byte) asl 2 { ok(v) => v } }",
    ] {
        assert!(
            std::panic::catch_unwind(|| compile_src(source)).is_err(),
            "reference admitted {source}"
        );
        assert!(
            self_hosted_compile(source, &Target::host()).is_err(),
            "pipeline admitted {source}"
        );
    }
}

#[test]
fn bare_arithmetic_shifts_keep_word_and_byte_lowering() {
    for source in [
        "fn main() -> Word { 7 asl 3 }",
        "fn f(n: Word, k: Word) -> Word { n asl k } fn main() -> Word { f(7, 3) }",
        "fn main() -> Byte { (7 as Byte) asl 3 }",
        "fn main() -> Byte { (7 as Byte) lsl 3 }",
        "fn main() -> Byte { (255 as Byte) asr 3 }",
        "fn main() -> Byte { (255 as Byte) lsr 3 }",
    ] {
        let reference = compile_src(source);
        let actual = self_hosted_compile(source, &Target::host()).unwrap();
        assert_eq!(
            actual.to_bytes().unwrap(),
            reference.to_bytes().unwrap(),
            "{source}"
        );
    }
}

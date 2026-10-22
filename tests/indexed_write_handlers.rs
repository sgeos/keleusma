//! Handled indexed writes and index-first evaluation of every indexed write.
//!
//! The operator rulings of 2026-10-20 and 2026-10-21 are recorded in
//! `docs/decisions/ORDER3_HANDLER_SURFACE_PLAN.md`. In short, a handled write
//! `d.x[i] = v { ok(old, new) => .., invalid_index(j) => .. }` is an expression
//! of its arms' type; the index is evaluated and bounds-checked before the
//! value, on bare and handled writes alike, so an invalid index skips the
//! value; the element is read before the write only when `old` is bound;
//! and the first increment admits one index.
//!
//! These tests cover the Rust reference compiler. The self-hosted compiler's
//! byte identity for the same sources is asserted separately under the
//! `self-host` feature.
#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::Arena;
use keleusma::bytecode::{Module, Op, Value};
use keleusma::trap_analysis::{TrapVerdict, classify_traps};
use keleusma::vm::{Vm, VmError, VmState};

fn parse(source: &str) -> Result<keleusma::ast::Program, String> {
    let tokens = keleusma::lexer::tokenize(source).map_err(|e| e.message)?;
    keleusma::parser::parse(&tokens).map_err(|e| e.message)
}

fn try_compile(source: &str) -> Result<Module, String> {
    keleusma::compiler::compile(&parse(source)?).map_err(|e| e.message)
}

fn compile(source: &str) -> Module {
    try_compile(source).unwrap_or_else(|e| panic!("{source}: {e}"))
}

/// Run `main` with a zeroed shared buffer. Returns the result and the buffer.
fn run(module: Module) -> (Result<VmState, VmError>, Vec<u8>, Vec<Vec<Op>>) {
    let ops = module.chunks.iter().map(|c| c.ops.clone()).collect();
    let need = keleusma::vm::required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(1 << 21);
    arena.resize_persistent(need).unwrap();
    let mut vm = Vm::new(module, &arena).unwrap();
    let mut shared = vec![0; vm.shared_data_bytes()];
    let result = vm.call_with_shared(&mut shared, &[]);
    (result, shared, ops)
}

fn run_word(source: &str) -> i64 {
    match run(compile(source)).0 {
        Ok(VmState::Finished(Value::Int(n))) => n,
        other => panic!("{source}: {other:?}"),
    }
}

fn main_ops(module: &Module) -> Vec<Op> {
    module
        .chunks
        .iter()
        .find(|c| c.name == "main")
        .expect("a main chunk")
        .ops
        .clone()
}

const TRACE: &str = "private data d { x: [Word; 2], trace: Word } \
     fn idx(i: Word) -> Word { d.trace = d.trace * 10 + 2; i } \
     fn val() -> Word { d.trace = d.trace * 10 + 1; 7 } ";

#[test]
fn a_handled_write_binds_the_old_and_new_values_and_writes() {
    let source = "private data d { x: [Word; 2] } fn main() -> Word { \
         d.x[1] = 3; \
         d.x[1] = 7 { ok(old, new) => old * 100 + new, invalid_index(j) => j } \
         }";
    assert_eq!(
        run_word(source),
        307,
        "the tail form yields the ok arm's value"
    );
    let source = "private data d { x: [Word; 2] } fn main() -> Word { \
         d.x[1] = 3; \
         d.x[1] = 7 { ok(_, _) => (), invalid_index(_) => () }; \
         d.x[1] }";
    assert_eq!(
        run_word(source),
        7,
        "the statement form writes and discards its value"
    );
}

#[test]
fn an_invalid_index_reaches_its_arm_without_evaluating_the_value() {
    let source = alloc_source(
        "fn main() -> Word { d.x[idx(5)] = val() { ok(_, n) => n, invalid_index(j) => j * 1000 + d.trace } }",
    );
    // `idx` ran once (trace 2) and `val` never ran (it would append 1).
    assert_eq!(run_word(&source), 5002);
    let source = alloc_source(
        "fn main() -> Word { d.x[idx(1)] = val() { ok(o, n) => o * 1000 + n * 100 + d.trace, invalid_index(j) => j } }",
    );
    // Index first, then the value: trace 2 then 21.
    assert_eq!(run_word(&source), 721);
}

fn alloc_source(main: &str) -> String {
    let mut s = String::from(TRACE);
    s.push_str(main);
    s
}

#[test]
fn bare_writes_evaluate_the_index_first_and_short_circuit_a_bad_index() {
    // Previously 19: the value ran first. The operator moved bare writes to
    // index-first evaluation on 2026-10-21.
    let source = alloc_source("fn main() -> Word { d.x[idx(1)] = val(); d.trace + d.x[1] }");
    assert_eq!(run_word(&source), 28);

    // A bad index traps before the value is evaluated. Shared data makes the
    // effect observable after the fault: `val` would set `t` to one.
    let source = "shared data d { x: [Word; 2], t: Word } \
         fn val() -> Word { d.t = 1; 7 } \
         fn main() -> Word { d.x[5] = val(); 0 }";
    let (result, shared, ops) = run(compile(source));
    assert!(
        matches!(result, Err(VmError::IndexOutOfBounds(5, 2))),
        "{result:?}"
    );
    assert!(shared.iter().all(|&b| b == 0), "the value was evaluated");
    let _ = ops;
}

#[test]
fn an_unhandled_invalid_index_takes_the_default_trap() {
    for source in [
        "private data d { x: [Word; 2] } fn main() -> Word { d.x[2] = 7 { ok(_, n) => n } }",
        "private data d { x: [Word; 2] } fn main() -> Word { d.x[2] = 7 { ok(_, n) => n, invalid_index(0) => 0 } }",
        "private data d { x: [Word; 2] } fn main() -> Word { d.x[2] = 7 { ok(_, n) => n, invalid_index(j) when j < 0 => 0 } }",
    ] {
        let module = compile(source);
        assert!(main_ops(&module).contains(&Op::Trap(8)), "{source}");
        assert!(
            matches!(classify_traps(&module), Ok(TrapVerdict::MayTrap { .. })),
            "{source}"
        );
        let (result, _, ops) = run(module);
        assert!(
            matches!(result, Err(VmError::IndexOutOfBounds(2, 2))),
            "{source}: {result:?}"
        );
        let _ = ops;
    }
}

#[test]
fn a_fully_handled_write_emits_no_trap_and_classifies_trap_free() {
    for source in [
        "private data d { x: [Word; 2] } fn main() -> Word { d.x[3] = 7 { ok(_, n) => n, invalid_index(j) => j } }",
        "private data d { x: [Byte; 2] } fn main() -> Word { d.x[1] = 7 as Byte { ok(o, n) => (o as Word) + (n as Word), invalid_index(_) => 0 } }",
        "private data d { x: [Word; 2] } fn main() -> bool { d.x[0] = 1 { ok(_, _) => true, invalid_index(_) => false } }",
    ] {
        let module = compile(source);
        assert!(
            !main_ops(&module).iter().any(|op| matches!(op, Op::Trap(_))),
            "{source}"
        );
        assert_eq!(
            classify_traps(&module),
            Ok(TrapVerdict::TrapFree),
            "{source}"
        );
        let (result, _, _) = run(module);
        assert!(
            matches!(result, Ok(VmState::Finished(_))),
            "{source}: {result:?}"
        );
    }
}

#[test]
fn the_old_value_is_read_only_when_bound() {
    let count_reads = |source: &str| {
        main_ops(&compile(source))
            .iter()
            .filter(|op| matches!(op, Op::GetDataIndexed(_, _)))
            .count()
    };
    let shape = |ok: &str| {
        format!(
            "private data d {{ x: [Word; 2] }} fn main() -> Word {{ d.x[1] = 7 {{ {ok} => 0, invalid_index(_) => 0 }} }}"
        )
    };
    assert_eq!(count_reads(&shape("ok(_, _)")), 0);
    assert_eq!(count_reads(&shape("ok(_, n)")), 0);
    assert_eq!(count_reads(&shape("ok(o, _)")), 1);
    assert_eq!(count_reads(&shape("ok(o, n)")), 1);
    // Keeping a copy of the value is likewise emitted only when `new` is bound.
    let dups = |source: &str| {
        main_ops(&compile(source))
            .iter()
            .filter(|op| **op == Op::Dup)
            .count()
    };
    assert_eq!(dups(&shape("ok(o, _)")), 0);
    assert_eq!(dups(&shape("ok(o, n)")), 1);
}

#[test]
fn a_field_beyond_sixteen_bits_probes_with_the_wide_read() {
    let source = "shared data d { x: [Byte; 70000] } fn main() -> Word { \
         d.x[69999] = 9 as Byte; \
         d.x[70000] = 1 as Byte { ok(_, _) => 0, invalid_index(j) => j } }";
    let module = compile(source);
    let ops = main_ops(&module);
    assert!(
        !ops.iter().any(|op| matches!(op, Op::BoundsCheck(_))),
        "{ops:?}"
    );
    match run(module).0 {
        Ok(VmState::Finished(Value::Int(n))) => assert_eq!(n, 70000),
        other => panic!("{other:?}"),
    }
}

#[test]
fn malformed_handled_writes_are_rejected_with_a_reason() {
    for (source, needle) in [
        (
            "private data d { x: [Word; 2] } fn main() -> Word { d.x[0] = 1 { ok(v) => v } }",
            "write `ok(old, new)`",
        ),
        (
            "private data d { x: [Word; 2] } fn main() -> Word { d.x[0] = 1 { ok(0, n) => n } }",
            "literal pattern is not admissible",
        ),
        (
            "private data d { x: [Word; 2] } fn main() -> Word { d.x[0] = 1 { ok(v, v) => v } }",
            "binds `v` twice",
        ),
        (
            "private data d { x: [Word; 2] } fn main() -> Word { d.x[0] = 1 { ok(_, n) when n > 0 => n } }",
            "non-exhaustive on `ok`",
        ),
        (
            "private data d { x: [Word; 2] } fn main() -> Word { d.x[0] = 1 { ok(_, n) => n, overflow(h, l) => l } }",
            "not admissible for a handled indexed write",
        ),
        (
            "private data d { m: [[Word; 2]; 2] } fn main() -> Word { d.m[0][1] = 1 { ok(_, n) => n } }",
            "exactly one index",
        ),
        (
            "private data d { x: [Word; 2] } fn main() -> Word { d.x[0] = 1 { ok(_, n) => n, ok(o, _) => o } }",
            "unreachable",
        ),
        (
            "const data d { x: [Word; 2] } fn main() -> Word { d.x[0] = 1 { ok(_, n) => n } }",
            "const data",
        ),
    ] {
        let err = try_compile(source).expect_err(source);
        assert!(err.contains(needle), "{source}: {err}");
    }
}

#[test]
fn an_arm_block_after_an_indexed_write_belongs_to_the_write() {
    // Before 2026-10-21 this was a handled read of `a[1]` on the right-hand
    // side. The arm block now handles the write, whose `ok` binds two values.
    let source = "private data d { x: [Word; 2] } fn main() -> Word { \
         let a = [4, 5]; d.x[0] = a[1] { ok(v) => v, invalid_index(_) => 0 }; d.x[0] }";
    let err = try_compile(source).expect_err("the single-binder ok now belongs to the write");
    assert!(err.contains("write `ok(old, new)`"), "{err}");
    // Parenthesizing keeps the handled read on the right-hand side.
    let source = "private data d { x: [Word; 2] } fn main() -> Word { \
         let a = [4, 5]; d.x[0] = (a[1] { ok(v) => v, invalid_index(_) => 0 }); d.x[0] }";
    assert_eq!(run_word(source), 5);
}

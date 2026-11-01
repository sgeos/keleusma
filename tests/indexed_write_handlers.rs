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

/// The lowering writes each bound `old` and `new` slot with the element's zero
/// before the flag test, for the definite-initialization rule. A struct
/// element's zero is a pool constant. The arm still sees the new value, and an
/// invalid index still reaches its arm. The `old` binding of a composite
/// element is asserted by the test that follows.
#[test]
fn a_handled_write_on_a_struct_element_binds_the_new_value() {
    let prefix = "struct P { x: Word, y: Word }\nprivate data d { x: [P; 2] } ";
    for (index, expected) in [(1, 2), (5, 5)] {
        let source = format!(
            "{prefix}fn main() -> Word {{ d.x[1] = P {{ x: 3, y: 4 }}; \
             d.x[{index}] = P {{ x: 1, y: 2 }} {{ ok(o, n) => n.y + o.x * 0, invalid_index(j) => j }} }}"
        );
        assert_eq!(run_word(&source), expected, "{source}");
    }
}

/// The `old` binding of a composite element holds the element as it was
/// before the write.
///
/// A private composite element's body lives at a fixed offset in the
/// persistent pool, and reading the slot yields a handle to that body rather
/// than a copy. The write then overwrites the body in place, so before the
/// repair of 2026-10-31 `old` observed the new value, and every case below
/// yielded the value built from the written element instead. The lowering now
/// copies the element into a fresh arena body before the write.
#[test]
fn a_handled_write_on_a_composite_element_binds_the_old_value() {
    let cases = [
        (
            "struct P { x: Word, y: Word }\nprivate data d { x: [P; 2] } fn main() -> Word { \
             d.x[1] = P { x: 3, y: 4 }; \
             d.x[1] = P { x: 1, y: 2 } { ok(o, n) => o.x * 10 + o.y + n.x * 0, invalid_index(j) => j } }",
            34,
        ),
        (
            "private data d { x: [(Word, Word); 2] } fn main() -> Word { \
             d.x[1] = (3, 4); \
             d.x[1] = (1, 2) { ok(o, _) => o.0 * 10 + o.1, invalid_index(j) => j } }",
            34,
        ),
        (
            "enum E { A(Word), B(Word, Word) }\nprivate data d { x: [E; 2] } fn main() -> Word { \
             d.x[1] = E::B(3, 4); \
             d.x[1] = E::A(1) { ok(o, _) => match o { E::A(a) => a, E::B(a, b) => a * 10 + b }, \
             invalid_index(j) => j } }",
            34,
        ),
        (
            "struct Q { p: (Word, Word), z: Word }\nprivate data d { x: [Q; 2] } fn main() -> Word { \
             d.x[1] = Q { p: (3, 4), z: 5 }; \
             d.x[1] = Q { p: (1, 2), z: 0 } { ok(o, _) => o.p.0 * 100 + o.p.1 * 10 + o.z, \
             invalid_index(j) => j } }",
            345,
        ),
        (
            "struct P { x: Word, y: Word }\nprivate data d { m: [[P; 2]; 2] } fn main() -> Word { \
             d.m[1][0] = P { x: 3, y: 4 }; \
             d.m[1][0] = P { x: 1, y: 2 } { ok(o, _) => o.x * 10 + o.y, invalid_index(j) => j } }",
            34,
        ),
        // The new value and the element read after the write are unaffected.
        (
            "struct P { x: Word, y: Word }\nprivate data d { x: [P; 2] } fn main() -> Word { \
             d.x[1] = P { x: 3, y: 4 }; \
             let s = d.x[1] = P { x: 1, y: 2 } { ok(o, n) => o.x * 1000 + o.y * 100 + n.x * 10 + n.y, \
             invalid_index(j) => j }; \
             s * 100 + d.x[1].x * 10 + d.x[1].y }",
            341_212,
        ),
    ];
    for (source, expected) in cases {
        assert_eq!(run_word(source), expected, "{source}");
    }
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
        // The copy of a composite `old` indexes a one-element array at zero
        // and adds no trap.
        "struct P { x: Word, y: Word }\nprivate data d { x: [P; 2] } fn main() -> Word { d.x[1] = P { x: 1, y: 2 } { ok(o, n) => o.x + n.y, invalid_index(j) => j } }",
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
            "private data d { m: [[Word; 2]; 2] } fn main() -> Word { d.m[0] = 1 { ok(_, n) => n } }",
            "does not descend to a scalar",
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

/// The self-hosted compiler mirrors every accepted shape byte for byte.
///
/// `self_hosted_compile` itself refuses a module that differs from the
/// reference, so success here is the byte-identity verdict. The comparison
/// is repeated explicitly so that a change to that entry cannot quietly
/// weaken the check.
#[cfg(feature = "self-host")]
#[test]
fn the_self_hosted_compiler_matches_every_handled_write_shape() {
    let mut sources: Vec<String> = [
        "private data d { x: [Word; 2] } fn main() -> Word { d.x[1] = 3; d.x[1] = 7 { ok(old, new) => old * 100 + new, invalid_index(j) => j } }",
        "private data d { x: [Word; 2] } fn main() -> Word { d.x[1] = 3; d.x[1] = 7 { ok(_, _) => (), invalid_index(_) => () }; d.x[1] }",
        "private data d { x: [Word; 2] } fn main() -> Word { d.x[2] = 7 { ok(_, n) => n } }",
        "private data d { x: [Word; 2] } fn main() -> Word { d.x[2] = 7 { ok(_, n) => n, invalid_index(0) => 0 } }",
        "private data d { x: [Word; 2] } fn main() -> Word { d.x[2] = 7 { ok(_, n) => n, invalid_index(j) when j < 0 => 0 } }",
        "private data d { x: [Word; 2] } fn main() -> Word { d.x[3] = 7 { ok(_, n) => n, invalid_index(j) => j } }",
        // Not `(o as Word) + (n as Word)`, which the self-hosted compiler mis-lowers
        // to Add outside any handler as well. That gap predates this construct.
        "private data d { x: [Byte; 2] } fn main() -> Byte { d.x[1] = 7 as Byte { ok(o, _) => o, invalid_index(_) => 0 as Byte } }",
        "private data d { x: [Byte; 2] } fn main() -> Byte { d.x[1] = 7 as Byte { ok(_, n) => n, invalid_index(_) => 0 as Byte } }",
        "private data d { x: [Word; 2] } fn main() -> bool { d.x[0] = 1 { ok(_, _) => true, invalid_index(_) => false } }",
        "private data d { x: [Word; 2] } fn main() -> Word { d.x[1] = 7 { ok(o, _) => o, invalid_index(_) => 0 } }",
        "private data d { x: [Word; 2] } fn main() -> Word { let a = [4, 5]; d.x[0] = (a[1] { ok(v) => v, invalid_index(_) => 0 }); d.x[0] }",
        "private data d { x: [Word; 2] } fn main() -> Word { d.x[0] = 1 { ok(o, n) when n > 5 => o, ok(_, n) => n, invalid_index(j) => j } }",
        "private data d { x: [Word; 2] } fn main() -> Word { if true { d.x[0] = 2 { ok(_, n) => n, invalid_index(_) => 0 } } else { 1 } }",
        "private data d { x: [Word; 2] } fn main() -> Word { d.x[0] = 2 { ok(_, n) => n, invalid_index(-1) => 5, invalid_index(_) => 0 } }",
        // Beyond sixteen bits the probe is the wide read, both with and without old.
        "shared data d { x: [Byte; 70000] } fn main() -> Word { d.x[69999] = 9 as Byte; d.x[70000] = 1 as Byte { ok(_, _) => 0, invalid_index(j) => j } }",
        "shared data d { x: [Byte; 70000] } fn main() -> Byte { d.x[5] = 1 as Byte { ok(o, _) => o, invalid_index(_) => 0 as Byte } }",
    ]
    .iter()
    .map(|s| String::from(*s))
    .collect();
    sources.push(alloc_source(
        "fn main() -> Word { d.x[idx(5)] = val() { ok(_, n) => n, invalid_index(j) => j * 1000 + d.trace } }",
    ));
    sources.push(alloc_source(
        "fn main() -> Word { d.x[idx(1)] = val() { ok(o, n) => o * 1000 + n * 100 + d.trace, invalid_index(j) => j } }",
    ));
    sources.push(alloc_source(
        "fn main() -> Word { d.x[idx(1)] = val(); d.trace + d.x[1] }",
    ));
    for source in &sources {
        let reference = compile(source);
        let actual =
            keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host())
                .unwrap_or_else(|e| panic!("{source}: {e:?}"));
        assert_eq!(
            actual.to_bytes().unwrap(),
            reference.to_bytes().unwrap(),
            "{source}"
        );
    }
    // Non-vacuity: every case above reached the comparison.
    assert_eq!(sources.len(), 19);
    // Outside the mirrored subset. The reference compiles a handled write on a
    // bool element, and the self-hosted compiler refuses it rather than emit a
    // module, as it refuses the handled read on one.
    // The handled write on a composite element, whose `old` binding the
    // reference copies before the write, is outside it too, so the copy has no
    // self-hosted twin to keep byte-identical.
    for outside in [
        "private data d { x: [bool; 2] } fn main() -> bool { d.x[0] = true { ok(_, n) => n, invalid_index(_) => false } }",
        "struct P { x: Word, y: Word }\nprivate data d { x: [P; 2] } fn main() -> Word { d.x[1] = P { x: 1, y: 2 } { ok(o, _) => o.x, invalid_index(j) => j } }",
        "private data d { x: [(Word, Word); 2] } fn main() -> Word { d.x[1] = (1, 2) { ok(o, _) => o.0, invalid_index(j) => j } }",
    ] {
        compile(outside);
        assert!(
            keleusma::selfhost::self_hosted_compile(outside, &keleusma::target::Target::host())
                .is_err(),
            "{outside}"
        );
    }
}

/// The self-hosted parser refuses each malformed handled write on its own,
/// without the reference compiler's verdict.
#[cfg(feature = "self-host")]
#[test]
fn the_self_hosted_parser_refuses_every_malformed_handled_write() {
    for source in [
        "private data d { x: [Word; 2] } fn main() -> Word { d.x[0] = 1 { ok(v) => v } }",
        "private data d { x: [Word; 2] } fn main() -> Word { d.x[0] = 1 { ok(0, n) => n } }",
        "private data d { x: [Word; 2] } fn main() -> Word { d.x[0] = 1 { ok(v, v) => v } }",
        "private data d { x: [Word; 2] } fn main() -> Word { d.x[0] = 1 { ok(_, n) when n > 0 => n } }",
        "private data d { x: [Word; 2] } fn main() -> Word { d.x[0] = 1 { ok(_, n) => n, overflow(h, l) => l } }",
        "private data d { x: [Word; 2] } fn main() -> Word { d.x[0] = 1 { ok(_, n) => n, ok(o, _) => o } }",
        "private data d { x: [Word; 2] } fn main() -> Word { d.x[0] = 1 { ok(_, n) => n } + 1 }",
    ] {
        assert!(
            keleusma::selfhost::try_parse_functions(source).is_err(),
            "{source}"
        );
        assert!(
            try_compile(source).is_err(),
            "the reference accepts {source}"
        );
    }
}

/// A handled multi-index write is outside the self-hosted subset, which has
/// no multi-index data access at all. The self-hosted compiler refuses it
/// rather than emitting a module, while the reference compiles it.
#[cfg(feature = "self-host")]
#[test]
fn the_self_hosted_compiler_refuses_a_handled_multi_index_write() {
    let source = "private data d { m: [[Word; 2]; 2] } fn main() -> Word { d.m[0][1] = 1 { ok(_, n) => n, invalid_index(j) => j } }";
    compile(source);
    assert!(
        keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host()).is_err()
    );
}

const MATRIX: &str = "private data d { m: [[Word; 3]; 2] } ";

fn matrix_source(main: &str) -> String {
    let mut s = String::from(MATRIX);
    s.push_str(main);
    s
}

/// The sum of every cell weighted by its flat position plus one, so a write
/// to any other cell than the intended one changes the result.
const MATRIX_DIGEST: &str =
    "d.m[0][0] + d.m[0][1] * 2 + d.m[0][2] * 3 + d.m[1][0] * 4 + d.m[1][1] * 5 + d.m[1][2] * 6";

#[test]
fn a_handled_multi_index_write_writes_the_addressed_cell_and_binds_both_values() {
    let source = matrix_source(&format!(
        "fn w() -> Word {{ d.m[1][2] = 7 {{ ok(old, new) => old * 100 + new, invalid_index(j) => 1000 + j }} }} \
         fn main() -> Word {{ d.m[1][2] = 3; let r = w(); r * 1000 + {MATRIX_DIGEST} }}"
    ));
    // `ok` saw old 3 and new 7, and only the cell at flat position six holds 7.
    assert_eq!(run_word(&source), 307 * 1000 + 42);
    let source = matrix_source(&format!(
        "fn main() -> Word {{ d.m[0][1] = 9 {{ ok(_, _) => (), invalid_index(_) => () }}; {MATRIX_DIGEST} }}"
    ));
    assert_eq!(run_word(&source), 18);
}

#[test]
fn a_failure_at_any_level_binds_that_levels_index_and_writes_nothing() {
    for (target, bound) in [
        // The outer level fails.
        ("d.m[2][0]", 2),
        // The inner level fails while the flat offset, five, is in range.
        // Checking only the flat offset would write the cell `m[1][2]`.
        ("d.m[0][5]", 5),
        // A negative inner index, whose flat offset would be in range too.
        ("d.m[1][-1]", -1),
    ] {
        let source = matrix_source(&format!(
            "fn w() -> Word {{ {target} = 7 {{ ok(_, _) => 0 - 1, invalid_index(j) => j }} }} \
             fn main() -> Word {{ let r = w(); r * 1000 + {MATRIX_DIGEST} }}"
        ));
        assert_eq!(run_word(&source), bound * 1000, "{source}");
    }
}

#[test]
fn a_three_level_write_reaches_its_cell_and_reports_its_middle_level() {
    let cube = "private data d { c: [[[Word; 2]; 2]; 2] } ";
    let digest = "d.c[1][0][1] * 10 + d.c[0][1][1]";
    let source = format!(
        "{cube} fn main() -> Word {{ d.c[1][0][1] = 4 {{ ok(_, n) => n, invalid_index(j) => 100 + j }}; {digest} }}"
    );
    assert_eq!(run_word(&source), 40);
    // The middle level fails. Flat offset 0 * 4 + 2 * 2 + 1 = 5 is in range.
    let source = format!(
        "{cube} fn w() -> Word {{ d.c[0][2][1] = 4 {{ ok(_, n) => n, invalid_index(j) => 100 + j }} }} \
         fn main() -> Word {{ let r = w(); r * 10 + {digest} }}"
    );
    assert_eq!(run_word(&source), 1020);
}

const MATRIX_TRACE: &str = "private data d { m: [[Word; 2]; 2], trace: Word } \
     fn idx(i: Word) -> Word { d.trace = d.trace * 10 + 2; i } \
     fn val() -> Word { d.trace = d.trace * 10 + 1; 7 } ";

#[test]
fn a_failing_level_stops_the_later_indices_and_the_value() {
    let run_main = |main: &str| {
        let mut s = String::from(MATRIX_TRACE);
        s.push_str(main);
        run_word(&s)
    };
    // The outer index fails, so the inner index and the value never run.
    assert_eq!(
        run_main(
            "fn main() -> Word { d.m[idx(5)][idx(0)] = val() { ok(_, n) => n, invalid_index(j) => j * 1000 + d.trace } }"
        ),
        5002
    );
    // The inner index fails after both indices ran, and the value never runs.
    assert_eq!(
        run_main(
            "fn main() -> Word { d.m[idx(1)][idx(4)] = val() { ok(_, n) => n, invalid_index(j) => j * 1000 + d.trace } }"
        ),
        4022
    );
    // Both levels in range: the indices, then the value.
    assert_eq!(
        run_main(
            "fn main() -> Word { d.m[idx(1)][idx(0)] = val() { ok(_, n) => n * 1000 + d.trace, invalid_index(j) => j } }"
        ),
        7221
    );
}

#[test]
fn an_unhandled_multi_index_failure_traps_with_the_bare_writes_payload() {
    for (target, expected) in [("d.m[2][0]", (2, 2)), ("d.m[0][5]", (5, 3))] {
        let handled = matrix_source(&format!(
            "fn main() -> Word {{ {target} = 7 {{ ok(_, n) => n, invalid_index(0) => 0 }} }}"
        ));
        let bare = matrix_source(&format!("fn main() -> Word {{ {target} = 7; 0 }}"));
        for source in [&handled, &bare] {
            let module = compile(source);
            assert!(
                matches!(classify_traps(&module), Ok(TrapVerdict::MayTrap { .. })),
                "{source}"
            );
            let (result, _, _) = run(module);
            assert!(
                matches!(result, Err(VmError::IndexOutOfBounds(i, n)) if (i, n) == expected),
                "{source}: {result:?}"
            );
        }
    }
}

#[test]
fn a_fully_handled_multi_index_write_emits_no_trap_and_classifies_trap_free() {
    // The seeding write is handled too, because a bare one keeps its trap.
    let byte_source = "private data d { m: [[Byte; 2]; 2] } fn main() -> Byte { \
         d.m[1][1] = 7 as Byte { ok(_, _) => (), invalid_index(_) => () }; \
         d.m[1][1] = 9 as Byte { ok(o, _) => o, invalid_index(_) => 0 as Byte } }";
    for source in [
        matrix_source(
            "fn main() -> Word { d.m[1][3] = 7 { ok(_, n) => n, invalid_index(j) => j } }",
        ),
        String::from(byte_source),
    ] {
        let module = compile(&source);
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
    // The old Byte element is read at the flat offset of `m[1][1]`.
    match run(compile(byte_source)).0 {
        Ok(VmState::Finished(Value::Byte(7))) => {}
        other => panic!("{other:?}"),
    }
}

// -- Handled writes in a `let` initializer, operator ruling of 2026-10-24 --

#[test]
fn a_let_initializer_binds_a_handled_write() {
    let source = "private data d { x: [Word; 2] } fn main() -> Word { \
         d.x[1] = 3; \
         let r = d.x[1] = 7 { ok(old, new) => old * 100 + new, invalid_index(j) => j }; \
         r * 10 + d.x[1] }";
    assert_eq!(run_word(source), 3077);
    let source = "private data d { x: [Word; 2] } fn main() -> Word { \
         let r: Word = d.x[5] = 7 { ok(_, n) => n, invalid_index(j) => j * 10 }; \
         r + d.x[0] + d.x[1] }";
    assert_eq!(run_word(source), 50, "an invalid index writes nothing");
}

#[test]
fn a_let_bound_write_with_a_bad_index_skips_its_value() {
    let source = alloc_source(
        "fn main() -> Word { let r = d.x[idx(5)] = val() { ok(_, n) => n, invalid_index(j) => j * 1000 + d.trace }; r }",
    );
    // `idx` ran and `val` did not, so the trace holds only the index's 2.
    assert_eq!(run_word(&source), 5002);
}

#[test]
fn a_let_bound_write_without_coverage_keeps_the_default_trap() {
    let source = "private data d { x: [Word; 2] } fn main() -> Word { \
         let r = d.x[2] = 7 { ok(_, n) => n, invalid_index(j) when j < 0 => 0 }; r }";
    let module = compile(source);
    assert!(matches!(
        classify_traps(&module),
        Ok(TrapVerdict::MayTrap { .. })
    ));
    assert!(matches!(
        run(module).0,
        Err(VmError::IndexOutOfBounds(2, 2))
    ));
    let source = "private data d { x: [Word; 2] } fn main() -> Word { \
         let r = d.x[2] = 7 { ok(_, n) => n, invalid_index(j) => j }; r }";
    assert_eq!(classify_traps(&compile(source)), Ok(TrapVerdict::TrapFree));
}

#[test]
fn a_bare_write_cannot_initialize_a_binding() {
    let error =
        try_compile("private data d { x: [Word; 2] } fn main() -> Word { let r = d.x[1] = 7; 0 }")
            .unwrap_err();
    assert!(
        error.contains("bare indexed write yields no value"),
        "{error}"
    );
}

/// A handled write in a `let` initializer reaches the self-hosted compiler
/// already, with no change to its stages. The operator ruling of 2026-10-24
/// expected a refusal, because the parse stage has one free name of its 1024.
/// Measured, the stage parses the form through its existing pending-write
/// detection, and the complete artifact equals the reference's. Equality is
/// the oracle, so the ruling's fallback of a pinned refusal is not needed.
#[cfg(feature = "self-host")]
#[test]
fn the_self_hosted_compiler_matches_a_let_bound_handled_write() {
    let sources = [
        "private data d { x: [Word; 2] } fn main() -> Word { let r = d.x[1] = 7 { ok(_, n) => n, invalid_index(j) => j }; r }",
        "private data d { x: [Word; 2] } fn main() -> Word { d.x[1] = 3; let r = d.x[1] = 7 { ok(old, new) => old * 100 + new, invalid_index(j) => j }; r * 10 + d.x[1] }",
        "private data d { x: [Word; 2] } fn main() -> Word { let r: Word = d.x[5] = 7 { ok(_, n) => n, invalid_index(j) => j * 10 }; r }",
        "private data d { x: [Word; 2] } fn main() -> Word { let r = d.x[2] = 7 { ok(_, n) => n, invalid_index(j) when j < 0 => 0 }; r }",
        "private data d { x: [Byte; 2] } fn main() -> Byte { let r = d.x[1] = 7 as Byte { ok(o, _) => o, invalid_index(_) => 0 as Byte }; r }",
    ];
    for source in sources {
        let reference = compile(source);
        let actual =
            keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host())
                .unwrap_or_else(|e| panic!("{source}: {e:?}"));
        assert_eq!(
            actual.to_bytes().unwrap(),
            reference.to_bytes().unwrap(),
            "{source}"
        );
    }
    // The bare form is refused on the shipping path, but only by the reference
    // cross-check. The self-hosted parser alone accepts it, a leniency that
    // predates this construct and is recorded in the Order 3 audit.
    let bare = "private data d { x: [Word; 2] } fn main() -> Word { let r = d.x[1] = 7; 0 }";
    assert!(matches!(
        keleusma::selfhost::self_hosted_compile(bare, &keleusma::target::Target::host()),
        Err(keleusma::selfhost::SelfHostError::ReferenceRejected { .. })
    ));
    assert!(keleusma::selfhost::try_parse_functions(bare).is_ok());
}

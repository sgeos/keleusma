#![cfg(all(feature = "compile", feature = "verify"))]
//! The instruction-set specification's trap-or-reify claims, checked against
//! the implementation by running it.
//!
//! # Why this file exists
//!
//! `docs/spec/INSTRUCTION_SET.md` stated that `CheckedMod` "Traps on
//! divide-by-zero". **The implementation has never done that** — it reifies a
//! zero divisor as flag `3` carrying the numerator, mirroring `CheckedDiv`,
//! and says so in its own comment. The row sat directly beside `CheckedDiv`,
//! which describes the reifying behaviour correctly, and the same wrong claim
//! had propagated to a hand-written book page that gave the wrong MECHANISM
//! as well.
//!
//! Nothing caught it, because **no test asserted the specification's prose
//! against the implementation for this class**. The one guard that exists in
//! the neighbourhood regenerates the book chapter from the spec and fails on
//! drift, which protects the COPY and says nothing about whether the original
//! is true. `tests/push_order_claims.rs` guards a different prose class the
//! same way this file guards its own.
//!
//! # The population
//!
//! Taken from the specification's own rows rather than from a hand-picked
//! list. Scanning every row whose prose makes a trap-or-reify claim yields:
//!
//! | row | the claim |
//! |---|---|
//! | `Div` | a zero divisor reifies as flag `3` |
//! | `Mod` | a zero divisor reifies as flag `3` |
//! | `CheckedDiv` | a zero divisor reifies as flag `3`, rather than trapping |
//! | `CheckedMod` | the same, mirroring `CheckedDiv` |
//! | `BoundsCheck` | reifies an invalid index as flag `1` |
//!
//! `CheckedNeg` names an overflow case but makes no trap-or-reify claim, and
//! `Trap` is the trap itself. Neither is in this class.
//!
//! # What is asserted, and what deliberately is not
//!
//! The BEHAVIOUR the row claims, never the row's wording. A reworded row must
//! not fail this file; a reworded row that becomes untrue must.
//!
//! Every case asserts that it REACHED the opcode under test. A module that is
//! refused before the opcode executes has tested nothing, and this suite's
//! recurring failure mode is exactly that: an outcome that looks like a pass
//! while testing nothing.

extern crate alloc;

use keleusma::Arena;
use keleusma::bytecode::{ConstValue, Module, Op, Value};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{
    DEFAULT_ARENA_CAPACITY, Vm, VmError, VmState, required_persistent_capacity_for,
};

fn compiled(src: &str) -> Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile")
}

/// Every opcode in the module, flattened.
fn ops(m: &Module) -> alloc::vec::Vec<Op> {
    m.chunks
        .iter()
        .flat_map(|c| c.ops.iter().copied())
        .collect()
}

/// Runs `m`, requiring that it verifies first.
///
/// Verification failing is a failure of the CASE, not a pass: a module the
/// virtual machine never accepted cannot demonstrate anything about an
/// opcode's behaviour.
fn run(what: &str, m: Module) -> Result<VmState, VmError> {
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena
        .resize_persistent(need)
        .unwrap_or_else(|e| panic!("{what}: arena: {e:?}"));
    let mut vm = Vm::new(m, &arena).unwrap_or_else(|e| {
        panic!("{what}: the module did not verify, so the opcode was never reached: {e:?}")
    });
    vm.call(&[])
}

/// Build a straight-line opcode fixture independently of surface lowering.
/// Both division contracts remain executable even though bare source operators
/// now select the checked family and an explicit Trap.
fn division_fixture(operation: Op, divisor: i64, discard_outcome: bool) -> Module {
    let mut module = compiled("fn main() -> Word { 7 }");
    let chunk = &mut module.chunks[0];
    chunk.constants = alloc::vec![ConstValue::Int(7), ConstValue::Int(divisor)];
    chunk.ops = alloc::vec![Op::Const(0), Op::Const(1), operation];
    if discard_outcome {
        chunk.ops.push(Op::PopN(2));
    }
    chunk.ops.push(Op::Return);
    module
}

/// Legacy opcodes expose the same total integer outcome shape.
#[test]
fn the_plain_divide_and_modulo_reify_a_zero_divisor_as_the_spec_says() {
    for operation in [Op::Div, Op::Mod] {
        for divisor in [0, 2] {
            let module = division_fixture(operation, divisor, true);
            assert!(ops(&module).contains(&operation));
            let expected = if divisor == 0 {
                7
            } else if operation == Op::Div {
                3
            } else {
                1
            };
            assert!(
                matches!(run("total legacy division", module), Ok(VmState::Finished(Value::Int(n))) if n == expected)
            );
        }
    }
}

/// Discarding an explicit outcome must leave the numerator on zero and the
/// quotient or remainder otherwise. Verification and execution must succeed.
#[test]
fn the_checked_divide_and_modulo_reify_a_zero_divisor_rather_than_trapping() {
    for (operation, valid_result) in [(Op::CheckedDiv(0), 3), (Op::CheckedMod, 1)] {
        for (divisor, expected) in [(0, 7), (2, valid_result)] {
            let module = division_fixture(operation, divisor, true);
            assert!(ops(&module).contains(&operation));
            match run("checked division", module).expect("total arithmetic executes") {
                VmState::Finished(Value::Int(value)) => assert_eq!(value, expected),
                other => panic!("unexpected result {other:?}"),
            }
        }
        // Read flag directly. Returning consumes the top slot; leave only
        // that result on the operand stack through a local.
        let mut module = division_fixture(operation, 0, false);
        let chunk = &mut module.chunks[0];
        chunk.local_count = 1;
        chunk.ops.pop();
        chunk
            .ops
            .extend([Op::SetLocal(0), Op::PopN(2), Op::GetLocal(0), Op::Return]);
        assert!(matches!(
            run("zero-divisor flag", module),
            Ok(VmState::Finished(Value::Int(3)))
        ));
    }
}

/// The predicate returns a flag; source lowering owns the explicit failure.
#[test]
fn the_bounds_check_reifies_failure_and_source_lowering_traps() {
    let mut module = compiled("fn main() -> Word { 0 }");
    module.chunks[0].constants = alloc::vec![ConstValue::Int(-1)];
    module.chunks[0].ops = alloc::vec![Op::Const(0), Op::BoundsCheck(2), Op::Return];
    assert!(matches!(
        run("BoundsCheck outcome", module),
        Ok(VmState::Finished(Value::Int(1)))
    ));
    let module = compiled(
        "private data d { xs: [[Word; 2]; 2] } fn main() -> Word { d.xs[0][0] = 1; d.xs[5][0] }",
    );
    assert!(ops(&module).contains(&Op::BoundsCheck(2)));
    assert!(ops(&module).contains(&Op::Trap(
        keleusma::bytecode::TrapKind::IndexOutOfBounds.code()
    )));
    assert!(matches!(
        run("explicit bounds failure", module),
        Err(VmError::IndexOutOfBounds(5, 2))
    ));
}

/// The population this file covers is the one the specification states.
///
/// Guards the census itself: if a row gains or loses a trap-or-reify claim,
/// the set below no longer matches the specification and the file's coverage
/// claim has gone stale.
#[test]
fn the_population_matches_the_rows_that_make_such_a_claim() {
    const SPEC: &str = include_str!("../docs/spec/INSTRUCTION_SET.md");
    let mut claiming: alloc::vec::Vec<&str> = alloc::vec::Vec::new();
    for line in SPEC.lines() {
        let Some(rest) = line.strip_prefix("| ") else {
            continue;
        };
        let Some((name, body)) = rest.split_once('|') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() || !name.chars().next().is_some_and(char::is_uppercase) {
            continue;
        }
        let low = body.to_ascii_lowercase();
        if low.contains("traps on") || low.contains("trap if") || low.contains("reifies as flag") {
            claiming.push(name);
        }
    }
    claiming.sort_unstable();
    claiming.dedup();
    assert_eq!(
        claiming,
        alloc::vec!["BoundsCheck", "CheckedDiv", "CheckedMod", "Div", "Mod"],
        "the set of specification rows making a trap-or-reify claim has changed. This file \
         covers a fixed population; a new claiming row is unguarded until it is added here."
    );
}

//! Artifacts in the shape compilers emitted before this cycle's contract changes.
//!
//! The V0.2.3 cycle changed the stack contracts of several existing opcodes in
//! place, with `BYTECODE_VERSION` and the format fingerprint held fixed by the
//! operator rulings of 2026-10-13 and 2026-10-20. An artifact built before a
//! change therefore still passes the version and fingerprint checks. This file
//! records, per changed contract, whether such an artifact is now rejected at
//! load or loads and runs with a changed meaning. It is a pin of the accepted
//! hazard, which strengthening item 14 asked to make visible, not a repair.
//!
//! Each artifact is the current compiler output for a small program, with the
//! operation's lowering rewritten to the earlier single-result shape. The
//! earlier shapes are taken from the contract documents under
//! docs/decisions/ORDER3_*_CONTRACTS.md and the handler surface plan.

#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::Arena;
use keleusma::bytecode::{ArrayElem, Module, Op, Value};
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};

fn compile(source: &str) -> Module {
    let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
    keleusma::compiler::compile(&ast).unwrap()
}

fn with_main(source: &str, ops: Vec<Op>) -> Module {
    let mut module = compile(source);
    let main = module.chunks.iter_mut().find(|c| c.name == "main").unwrap();
    main.ops = ops;
    module
}

/// What happens to an artifact: refused by `verify`, refused by `Vm::new`, or
/// run to the given result.
#[derive(Debug, PartialEq)]
enum Outcome {
    RejectedByVerify,
    RejectedAtLoad,
    Ran(Result<Value, String>),
}

fn outcome(module: Module, args: &[Value]) -> Outcome {
    if keleusma::verify::verify(&module).is_err() {
        return Outcome::RejectedByVerify;
    }
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let Ok(mut vm) = Vm::new(module, &arena) else {
        return Outcome::RejectedAtLoad;
    };
    vm.register_native("host::f", |args| Ok(args[0].clone()));
    Outcome::Ran(match vm.call(args) {
        Ok(VmState::Finished(v)) => Ok(v),
        Ok(other) => Err(format!("{other:?}")),
        Err(e) => Err(format!("{e:?}")),
    })
}

fn without_tail(source: &str, is_op: impl Fn(&Op) -> bool, tail: usize) -> Module {
    let mut ops = compile(source)
        .chunks
        .into_iter()
        .find(|c| c.name == "main")
        .unwrap()
        .ops;
    let at = ops.iter().position(is_op).unwrap();
    ops.drain(at + 1..at + 1 + tail);
    with_main(source, ops)
}

/// Legacy `Div` once pushed the quotient alone and now pushes low, high and
/// flag. An old artifact still verifies and returns the flag, zero, in place
/// of the quotient three. This is the accepted hazard: changed meaning with
/// no rejection.
#[test]
fn an_old_division_artifact_runs_with_a_changed_result() {
    let div = with_main(
        "fn main(a: Word, b: Word) -> Word { a + b }",
        vec![Op::GetLocal(0), Op::GetLocal(1), Op::Div, Op::Return],
    );
    assert_eq!(
        outcome(div, &[Value::Int(7), Value::Int(2)]),
        Outcome::Ran(Ok(Value::Int(0)))
    );
}

/// A flat Word read once pushed the element alone. An old artifact still
/// verifies and returns the outcome flag, zero, in place of the element 20.
#[test]
fn an_old_flat_read_artifact_runs_with_a_changed_result() {
    let module = without_tail(
        "fn main(i: Word) -> Word { let a = [10, 20]; a[i] }",
        |op| matches!(op, Op::GetIndex(ArrayElem::Flat { .. })),
        6,
    );
    assert_eq!(
        outcome(module, &[Value::Int(1)]),
        Outcome::Ran(Ok(Value::Int(0)))
    );
}

/// A nested composite read followed by a field read is rejected at
/// verification, because the field read now meets the scalar flag.
#[test]
fn an_old_nested_read_artifact_is_rejected_by_verification() {
    let module = without_tail(
        "struct P { a: Word, b: Word } fn main(i: Word) -> Word { let ps = [P { a: 1, b: 2 }]; ps[i].b }",
        |op| matches!(op, Op::GetIndex(ArrayElem::FlatNested { .. })),
        6,
    );
    assert_eq!(outcome(module, &[Value::Int(0)]), Outcome::RejectedByVerify);
}

/// An unreified native call keeps its earlier contract, so an old artifact
/// keeps its meaning.
#[test]
fn an_old_native_call_artifact_keeps_its_meaning() {
    let source = "use host::f\nfn main(x: Word) -> Word { host::f(x) }";
    let mut ops = compile(source)
        .chunks
        .into_iter()
        .find(|c| c.name == "main")
        .unwrap()
        .ops;
    let at = ops
        .iter()
        .position(|op| matches!(op, Op::CallVerifiedNative(..)))
        .unwrap();
    let Op::CallVerifiedNative(i, n) = ops[at] else {
        unreachable!()
    };
    ops[at] = Op::CallVerifiedNative(i, n & 0x7F);
    ops.drain(at + 1..at + 6);
    assert_eq!(
        outcome(with_main(source, ops), &[Value::Int(3)]),
        Outcome::Ran(Ok(Value::Int(3)))
    );
}

//! A hot swap seeds a composite private value.
//!
//! # The defect
//!
//! `replace_module` wrote its `initial_data` straight into the private slots
//! and then cleared the persistent composite pool and reset the arena. A seeded
//! struct was left boxed, so the new module's flat-baked field read refused it.
//! Measured on 2026-10-30, a read of `d.p.x` after a swap seeded with
//! `P { x: 3, y: 4 }` failed with "GetField operand form does not match struct
//! body". A pykeleusma finding.
//!
//! # What these tests do NOT establish
//!
//! A host still cannot read a private slot, so carrying state forward needs the
//! host to know the values. Whether it should be able to is an open question.
#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::Arena;
use keleusma::bytecode::Value;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

fn module(source: &str) -> keleusma::bytecode::Module {
    let tokens = keleusma::lexer::tokenize(source).expect("lex");
    keleusma::compiler::compile(&keleusma::parser::parse(&tokens).expect("parse")).expect("compile")
}

const DECL: &str = "struct P { x: Word, y: Word }\nprivate data d { p: P, n: Word }\n";

#[test]
fn a_seeded_struct_is_read_after_the_swap() {
    let first = module(&format!("{DECL}fn main() -> Word {{ d.n = d.n + 1; d.n }}"));
    let second = module(&format!(
        "{DECL}fn main() -> Word {{ d.n = d.n + 1; d.p.x * 100 + d.p.y * 10 + d.n }}"
    ));
    let need =
        required_persistent_capacity_for(&first).max(required_persistent_capacity_for(&second));
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(first, &arena).expect("verifies");
    let seeded = Value::struct_value(
        "P".into(),
        vec![("x".into(), Value::Int(3)), ("y".into(), Value::Int(4))],
    );
    vm.replace_module(second, vec![seeded, Value::Int(5)])
        .expect("the swap is admitted");
    // Twice, because the persisted body must survive the arena's reuse.
    for expected in [346, 347] {
        match vm.call(&[]).expect("runs") {
            VmState::Finished(Value::Int(v)) => assert_eq!(v, expected),
            other => panic!("unexpected {other:?}"),
        }
    }
}

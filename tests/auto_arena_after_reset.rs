//! The auto-sized arena holds a stream across its resets.
//!
//! # The defect
//!
//! `Vm::resume` wrote a stream's input into slot 0 and then also pushed it, as
//! it does to complete a yield. After a `Reset` there is no yield to complete,
//! so every iteration after the first ran one operand slot above the bound the
//! verifier computed, and an arena sized by `auto_arena_capacity_for` ran out.
//! Measured on 2026-10-30, `loop main(e: Word) -> Word { let n = yield e + 1;
//! n }` got 184 bytes and failed with `OutOfArena` on the resume after its first
//! reset. A pykeleusma finding.
//!
//! # What these tests do NOT establish
//!
//! Composite host inputs are copied into the arena and are outside the auto
//! size by its documented contract. That part of the finding is open.
#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::Arena;
use keleusma::bytecode::Value;
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};

#[test]
fn an_auto_sized_stream_survives_many_resets() {
    let src = "loop main(e: Word) -> Word { let n = yield e + 1; n }";
    let tokens = keleusma::lexer::tokenize(src).expect("lex");
    let module = keleusma::compiler::compile(&keleusma::parser::parse(&tokens).expect("parse"))
        .expect("compile");
    let capacity = auto_arena_capacity_for(&module, &[]).expect("bounded");
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(capacity + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(module, &arena).expect("verifies");
    assert!(matches!(
        vm.call(&[Value::Int(10)]).expect("call"),
        VmState::Yielded(Value::Int(11))
    ));
    let mut resets = 0usize;
    let mut yields = 0usize;
    for i in 0..200 {
        match vm
            .resume(Value::Int(i))
            .expect("an auto-sized arena holds every iteration")
        {
            VmState::Reset => resets += 1,
            VmState::Yielded(Value::Int(v)) => {
                // After a reset the body reads the resumed input from slot 0.
                assert_eq!(v, i + 1);
                yields += 1;
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!((resets, yields), (100, 100));
}

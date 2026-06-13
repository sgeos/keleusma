#![cfg(all(feature = "compile", feature = "verify"))]
//! Private persistent composite data slots store their body in the arena
//! persistent region (B28 P3 item 5, item 3a).
//!
//! A private `.data` slot holding a flat composite (struct, tuple, enum) gets a
//! fixed body offset assigned by the compiler within the persistent composite
//! pool, which follows the private-slot `Value` array. `Op::SetDataComposite`
//! copies the body once into that fixed `.data`-style location and stores a
//! region-aware handle that survives RESET in place, with no global-heap
//! `Inline` body. `GetData` reads it in place. These tests pin write-then-read
//! within a call, survival across a RESET in a loop, and a nested composite.

extern crate alloc;

use keleusma::Arena;
use keleusma::bytecode::Value;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

/// Compile, size the arena's persistent region to the module's requirement
/// (which now includes the persistent composite body pool), and run `main`.
fn run_word(src: &str) -> i64 {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize_persistent");
    let mut vm = Vm::new(m, &arena).expect("verify");
    match vm.call(&[]).expect("call") {
        VmState::Finished(Value::Int(n)) => n,
        other => panic!("expected Int, got {:?}", other),
    }
}

#[test]
fn private_struct_slot_write_then_read() {
    let src = "struct Point { x: Word, y: Word }\n\
               private data d { p: Point }\n\
               fn main() -> Word { d.p = Point { x: 3, y: 4 }; d.p.x + d.p.y }";
    assert_eq!(run_word(src), 7);
}

#[test]
fn private_tuple_slot_write_then_read() {
    let src = "private data d { t: (Word, Word) }\n\
               fn main() -> Word { d.t = (10, 20); d.t.0 + d.t.1 }";
    assert_eq!(run_word(src), 30);
}

#[test]
fn private_nested_struct_slot_round_trips() {
    let src = "struct Inner { a: Word, b: Word }\n\
               struct Outer { inner: Inner, c: Word }\n\
               private data d { o: Outer }\n\
               fn main() -> Word { \
                   d.o = Outer { inner: Inner { a: 1, b: 2 }, c: 3 }; \
                   d.o.inner.a + d.o.inner.b + d.o.c }";
    assert_eq!(run_word(src), 6);
}

#[test]
fn private_composite_survives_reset() {
    // Iteration 1 writes the slot; resuming past the yield reaches the loop
    // body end, which RESETs the ephemeral arena. The restarted stream reads
    // the slot without rewriting it (seed != 0), so a correct read requires
    // the composite body to have survived the RESET in the persistent region.
    let src = "struct Point { x: Word, y: Word }\n\
               private data d { p: Point }\n\
               loop main(seed: Word) -> Word { \
                   if seed == 0 { d.p = Point { x: 11, y: 22 }; }; \
                   let _ = yield d.p.x + d.p.y; \
                   0 \
               }";
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize_persistent");
    let mut vm = Vm::new(m, &arena).expect("verify");
    // First call: main(0) writes the slot, yields 11 + 22 = 33.
    match vm.call(&[Value::Int(0)]).expect("call") {
        VmState::Yielded(Value::Int(n)) => assert_eq!(n, 33),
        other => panic!("iter 1 expected Yielded(33), got {:?}", other),
    }
    // Resume past the yield: the body end RESETs the ephemeral arena.
    match vm.resume(Value::Int(1)).expect("resume") {
        VmState::Reset => {}
        other => panic!("expected Reset at loop body end, got {:?}", other),
    }
    // Restart the stream with seed 1: no rewrite occurs, so the read of the
    // slot must observe the body that survived the RESET, yielding 33 again.
    match vm.resume(Value::Int(1)).expect("resume") {
        VmState::Yielded(Value::Int(n)) => {
            assert_eq!(n, 33, "private composite must survive RESET")
        }
        other => panic!("iter 2 expected Yielded(33), got {:?}", other),
    }
}

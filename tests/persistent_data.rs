#![cfg(all(feature = "compile", feature = "verify"))]
//! Private persistent composite data slots store their body in the arena
//! persistent region (B28 P3 item 5, item 3a).
//!
//! A private `.data` slot holding a flat composite (struct, tuple, enum) gets a
//! fixed body offset assigned by the compiler within the persistent composite
//! pool, which follows the private-slot `Value` array, and recorded in the
//! module's private-composite layout table. `Op::SetData` dispatches on the
//! value at run time: a flat composite copies its body once into that fixed
//! `.data`-style location at the table offset and stores a region-aware handle
//! that survives RESET in place, with no global-heap `Inline` body. `GetData`
//! reads it in place. These tests pin write-then-read within a call, survival
//! across a RESET in a loop, and a nested composite.

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
fn d4_private_composite_write_bounded_by_slot_region() {
    // Audit D4: a private-composite layout table whose offsets are spaced closer
    // than the bodies (a crafted table that still passes the ascending-offset
    // validation) must not let one slot's write overrun into the next slot's
    // region. Two Point slots each occupy a 16-byte body. Shrinking slot a's
    // region to one byte by moving slot b's pool offset down to 1 keeps the
    // table strictly ascending, so it loads, but the 16-byte write to a now
    // overruns [0, 1) and must fault rather than overwrite b's region.
    let src = "struct Point { x: Word, y: Word }\n\
               private data d { a: Point, b: Point }\n\
               fn main() -> Word { d.a = Point { x: 1, y: 2 }; 0 }";
    let mut m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    {
        let dl = m.data_layout.as_mut().expect("data layout");
        assert!(
            dl.private_composite_layout.len() >= 2,
            "expected two private composite slots"
        );
        dl.private_composite_layout[0].offset = 0;
        dl.private_composite_layout[1].offset = 1;
    }
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize_persistent");
    let mut vm = Vm::new(m, &arena).expect("verify");
    let err = vm
        .call(&[])
        .expect_err("the overrunning composite write must fault, not corrupt the neighbour");
    let msg = alloc::format!("{err:?}");
    assert!(
        msg.contains("overruns its pool slot region"),
        "expected a D4 pool-region fault, got: {msg}"
    );
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

#[test]
fn private_array_of_struct_write_then_read() {
    // An array-of-composite private field: each element slot is a flat
    // composite that B28 item 2 step 6A places in the persistent composite pool
    // through the private-composite layout table (the linker-style fixed-address
    // model), persisted at the `Op::SetDataIndexed` write with no global-heap
    // body. Distinct elements must occupy distinct pool offsets, so writing two
    // elements and summing their fields reads each back independently.
    let src = "struct Point { x: Word, y: Word }\n\
               private data d { arr: [Point; 4] }\n\
               fn main() -> Word { \
                   d.arr[0] = Point { x: 1, y: 2 }; \
                   d.arr[3] = Point { x: 30, y: 40 }; \
                   d.arr[0].x + d.arr[0].y + d.arr[3].x + d.arr[3].y }";
    assert_eq!(run_word(src), 1 + 2 + 30 + 40);
}

#[test]
fn private_array_of_struct_survives_reset() {
    // The array-element pool bodies must survive a RESET in place, exactly as a
    // single composite slot does (B28 item 2 step 6A). Iteration 1 writes two
    // elements; the restarted stream reads them without rewriting, so a correct
    // read requires both element bodies to have survived the RESET at their
    // distinct persistent pool offsets.
    let src = "struct Point { x: Word, y: Word }\n\
               private data d { arr: [Point; 4] }\n\
               loop main(seed: Word) -> Word { \
                   if seed == 0 { \
                       d.arr[1] = Point { x: 5, y: 6 }; \
                       d.arr[2] = Point { x: 7, y: 8 }; \
                   }; \
                   let _ = yield d.arr[1].x + d.arr[1].y + d.arr[2].x + d.arr[2].y; \
                   0 \
               }";
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize_persistent");
    let mut vm = Vm::new(m, &arena).expect("verify");
    // Sum kept within the 8-bit range so the assertion holds under the
    // narrow-word-8 runtime (`--all-features`), where integer arithmetic wraps
    // at eight bits. The distinct element values still prove distinct pool
    // offsets per element slot.
    let want = 5 + 6 + 7 + 8;
    // First call writes the two element slots, yields their field sum.
    match vm.call(&[Value::Int(0)]).expect("call") {
        VmState::Yielded(Value::Int(n)) => assert_eq!(n, want),
        other => panic!("iter 1 expected Yielded({want}), got {:?}", other),
    }
    // Resume past the yield: the body end RESETs the ephemeral arena.
    match vm.resume(Value::Int(1)).expect("resume") {
        VmState::Reset => {}
        other => panic!("expected Reset at loop body end, got {:?}", other),
    }
    // Restart with seed 1: no rewrite, so the reads must observe both element
    // bodies surviving the RESET at their distinct pool offsets.
    match vm.resume(Value::Int(1)).expect("resume") {
        VmState::Yielded(Value::Int(n)) => {
            assert_eq!(n, want, "array-of-composite elements must survive RESET")
        }
        other => panic!("iter 2 expected Yielded({want}), got {:?}", other),
    }
}

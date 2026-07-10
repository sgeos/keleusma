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

// --- Private-data `.data`-section load-time initialization ---------------
//
// A private scalar slot initializes at load to its declared `= literal` or the
// type's zero, exactly like an assembler `.data` section, invisible to the
// host. This makes read-before-write on a private scalar well-defined (it reads
// the zero) rather than faulting on the old `Unit` sentinel. Composite private
// slots keep the write-before-read contract (they initialize to `Unit`).

/// Compile a program whose entry is a zero-argument `loop` and drive one
/// iteration, returning the yielded `Word`. Mirrors `run_word` for the stream
/// entry shape used by the counter idiom.
fn compile_ok(src: &str) -> keleusma::bytecode::Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile")
}

// Each program below both reads and writes its private block, so the
// "private data is never mutated; use const data" lint is satisfied; the
// return value is the value *read before the write*, which is precisely the
// load-time `.data` initialization under test.

#[test]
fn private_scalar_reads_zero_before_write() {
    // The exact playground "Counter" bug: reading a private `Word` before any
    // write must observe 0, not a `Unit` type fault.
    let src = "private data d { total: Word }\n\
               fn main() -> Word { let seen = d.total; d.total = 1; seen }";
    assert_eq!(run_word(src), 0);
}

#[test]
fn private_scalar_accumulates_from_a_zero_start() {
    // Read-add-write against the zero-initialized slot within one call. The RHS
    // reads the load-time zero, so the stored (and returned) value is 5.
    let src = "private data d { total: Word }\n\
               fn main() -> Word { d.total = d.total + 5; d.total }";
    assert_eq!(run_word(src), 5);
}

#[test]
fn private_scalar_explicit_initializer_is_baked() {
    // An explicit `= literal` on a private scalar field is the load-time value.
    let src = "private data d { total: Word = 42 }\n\
               fn main() -> Word { let seen = d.total; d.total = 0; seen }";
    assert_eq!(run_word(src), 42);
}

#[test]
fn private_byte_reads_zero_before_write() {
    let src = "private data d { b: Byte }\n\
               fn main() -> Word { let seen = d.b as Word; d.b = 1 as Byte; seen }";
    assert_eq!(run_word(src), 0);
}

#[test]
fn private_bool_reads_false_before_write() {
    let src = "private data d { flag: bool }\n\
               fn main() -> Word { let seen = if d.flag { 1 } else { 0 }; d.flag = true; seen }";
    assert_eq!(run_word(src), 0);
}

#[test]
fn private_scalar_array_reads_zero_before_write() {
    // Every element slot of a scalar array initializes to the element zero.
    let src = "private data d { xs: [Word; 3] }\n\
               fn main() -> Word { let sum = d.xs[0] + d.xs[1] + d.xs[2]; d.xs[0] = 1; sum }";
    assert_eq!(run_word(src), 0);
}

#[test]
fn private_scalar_zero_init_survives_the_zero_copy_path() {
    // The zero-copy `view_bytes_zero_copy` constructor initializes private
    // slots from the archived `.data` table, matching the owned-`Module` path.
    let src = "private data d { total: Word = 9 }\n\
               fn main() -> Word { let seen = d.total; d.total = 0; seen }";
    let module = compile_ok(src);
    let bytes = module.to_bytes().expect("serialize");
    let mut aligned: rkyv::util::AlignedVec<8> = rkyv::util::AlignedVec::with_capacity(bytes.len());
    aligned.extend_from_slice(&bytes);
    // Persistent region must hold the private-slot array; size it from the
    // owned module before it is consumed by serialization above (already done).
    let need = required_persistent_capacity_for(&compile_ok(src));
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize_persistent");
    let mut vm = unsafe { Vm::view_bytes_zero_copy(aligned.as_slice(), &arena) }.expect("view");
    match vm.call(&[]).expect("call") {
        VmState::Finished(Value::Int(n)) => assert_eq!(n, 9),
        other => panic!("expected Int(9), got {:?}", other),
    }
}

#[test]
fn shared_field_initializer_is_rejected() {
    // Shared data is host-initialized; a `= literal` on a shared field is an
    // error (only private scalar fields admit initializers).
    let src = "shared data d { total: Word = 1 }\n\
               fn main() -> Word { 0 }";
    let err = compile(&parse(&tokenize(src).expect("lex")).expect("parse"))
        .expect_err("shared initializer must be rejected");
    assert!(
        err.message.contains("shared data field") && err.message.contains("host-initialized"),
        "unexpected error: {}",
        err.message
    );
}

#[test]
fn private_composite_field_initializer_is_rejected() {
    // A composite private field keeps write-before-read; an initializer on it
    // is rejected (only scalar private fields admit `= literal`).
    let src = "struct Point { x: Word, y: Word }\n\
               private data d { p: Point = Point { x: 1, y: 2 } }\n\
               fn main() -> Word { 0 }";
    let err = compile(&parse(&tokenize(src).expect("lex")).expect("parse"))
        .expect_err("composite private initializer must be rejected");
    assert!(
        err.message.contains("private data field") && err.message.contains("scalar"),
        "unexpected error: {}",
        err.message
    );
}

#[test]
fn counter_loop_accumulates_from_zero_initialized_private_scalar() {
    // The exact playground "Counter" scenario: a `loop` whose first iteration
    // reads a private scalar before writing it. The read observes the load-time
    // zero, so `add(value, state.total)` is well-defined on iteration one, and
    // the accumulated total persists across the loop-body RESET.
    let src = "private data state { total: Word }\n\
               fn add(a: Word, b: Word) -> Word { a + b }\n\
               loop main(value: Word) -> Word { \
                   state.total = add(value, state.total); \
                   yield state.total \
               }";
    let m = compile_ok(src);
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize_persistent");
    let mut vm = Vm::new(m, &arena).expect("verify");
    // Iteration 1: total = add(5, 0) = 5.
    match vm.call(&[Value::Int(5)]).expect("call") {
        VmState::Yielded(Value::Int(n)) => assert_eq!(n, 5),
        other => panic!("iter 1 expected Yielded(5), got {:?}", other),
    }
    // Resume past the yield to the loop-body RESET.
    match vm.resume(Value::Int(0)).expect("resume") {
        VmState::Reset => {}
        other => panic!("expected Reset at loop body end, got {:?}", other),
    }
    // Iteration 2 restarts with value = 3: total = add(3, 5) = 8, proving the
    // private scalar survived the RESET and accumulated from the zero start.
    match vm.resume(Value::Int(3)).expect("resume") {
        VmState::Yielded(Value::Int(n)) => assert_eq!(n, 8, "private scalar must accumulate"),
        other => panic!("iter 2 expected Yielded(8), got {:?}", other),
    }
}

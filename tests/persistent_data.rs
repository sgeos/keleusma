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
    // Audit D4: a private composite field whose declared element length is
    // shorter than the body written into it must not let the write overrun
    // into the next field's region. Two Point fields each occupy a 16-byte
    // body. Declaring field a's element one byte long keeps every placement
    // ascending and disjoint, so it loads, but the 16-byte write to a now
    // overruns [0, 1) and must fault rather than overwrite b's region.
    let src = "struct Point { x: Word, y: Word }\n\
               private data d { a: Point, b: Point }\n\
               fn main() -> Word { d.a = Point { x: 1, y: 2 }; 0 }";
    let mut m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    {
        let dl = m.data_layout.as_mut().expect("data layout");
        let pooled: Vec<_> = dl
            .fields
            .iter_mut()
            .filter(|f| f.offset.is_some())
            .collect();
        assert!(pooled.len() >= 2, "expected two private composite fields");
        pooled.into_iter().next().unwrap().elem_len = 1;
    }
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize_persistent");
    // Construction writes each composite slot's zero initializer into its pool
    // region, which is the first overrunning write, so the fault now arises at
    // `Vm::new` rather than at the program's own write.
    let err = Vm::new(m, &arena)
        .err()
        .expect("the overrunning composite write must fault, not corrupt the neighbour");
    let msg = alloc::format!("{err:?}");
    assert!(
        msg.contains("overruns its pool slot region"),
        "expected a D4 pool-region fault, got: {msg}"
    );
}

/// A placement table whose fields overlap in the pool is refused when the
/// module is verified, before any write could reach it. The record this
/// replaced carried no element length, so only the runtime write could catch
/// the same overlap.
#[test]
fn overlapping_private_composite_placements_are_refused_at_verification() {
    let src = "struct Point { x: Word, y: Word }\n\
               private data d { a: Point, b: Point }\n\
               fn main() -> Word { d.a = Point { x: 1, y: 2 }; 0 }";
    let mut m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    {
        let dl = m.data_layout.as_mut().expect("data layout");
        let b = dl
            .fields
            .iter_mut()
            .filter(|f| f.offset.is_some())
            .nth(1)
            .expect("two private composite fields");
        b.offset = Some(1);
    }
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize_persistent");
    let err = Vm::new(m, &arena)
        .err()
        .expect("an overlapping placement must be refused");
    let msg = alloc::format!("{err:?}");
    assert!(
        msg.contains("not ascending and disjoint"),
        "expected a placement refusal, got: {msg}"
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

// PACKED PRIVATE STORAGE, S1 OF THE ARRAY WIRE-SLOT DESIGN.
//
// Ruled on 2026-10-31 as C3.5. A private array of more than one element whose
// elements do not start as `Unit` lies at element width, a scalar array in the
// packed region after the `Value` slots and a composite array in its pool
// bodies alone. Every other private field keeps a `Value` slot per element.
// These tests pin the declared size per element kind, the persistent figure
// by its smallest admitted value and the value one byte below it, survival
// across a RESET for every packed kind, and the two writes packing refuses.
// Each declared-size and persistent figure below is smaller than the figure of
// the element-slot storage that preceded packing, so each fails on the old
// code.

fn compiled(src: &str) -> keleusma::bytecode::Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile")
}

#[test]
fn the_declared_private_size_packs_each_array_at_element_width() {
    use keleusma::bytecode::VALUE_SLOT_SIZE_BYTES as SLOT;
    // `z` is a private scalar written by `main`, one `Value` slot in every case.
    for (decl, packed, pool) in [
        ("a: Word", SLOT, 0),
        ("a: [Word; 10]", 80, 0),
        ("a: [Byte; 10]", 10, 0),
        ("a: [bool; 10]", 10, 0),
        ("a: [Fixed; 10]", 80, 0),
        ("a: [Float; 10]", 80, 0),
        ("a: [[Word; 3]; 4]", 96, 0),
        // One element is a scalar in all but its declared type.
        ("a: [Word; 1]", SLOT, 0),
        // A composite array occupies its pool bodies alone.
        ("a: [P; 10]", 0, 160),
        // A single composite keeps its `Value` handle beside its body.
        ("a: P", SLOT, 16),
    ] {
        let src = format!(
            "struct P {{ x: Word, y: Word }}\nprivate data d {{ {decl}, z: Word }}\n\
             fn main() -> Word {{ d.z = 1; 0 }}"
        );
        let m = compiled(&src);
        assert_eq!(
            m.private_data_bytes,
            packed + SLOT,
            "declared size of {decl}"
        );
        assert_eq!(m.persistent_composite_bytes, pool, "pool of {decl}");
        assert_eq!(
            required_persistent_capacity_for(&m),
            (packed + SLOT + pool) as usize,
            "persistent figure of {decl}"
        );
    }
}

const PACKED_KINDS: &str = "struct P { x: Word, y: Word }\n\
    private data d { w: [Word; 5], b: [Byte; 3], t: [bool; 2], f: [Float; 2], p: [P; 4], s: Word }\n\
    loop main(seed: Word) -> Word { \
        if seed == 0 { \
            d.w[4] = 7; d.b[2] = 9 as Byte; d.t[1] = true; d.f[1] = 2.5; \
            d.p[3] = P { x: 1, y: 2 }; d.s = 5; \
        }; \
        let t = if d.t[1] { 100 } else { 0 }; \
        let f = if d.f[1] == 2.5 { 1000 } else { 0 }; \
        let _ = yield d.w[4] + (d.b[2] as Word) + d.p[3].x + d.p[3].y + d.s + t + f; \
        0 \
    }";

/// Construct `m` with exactly `persistent` bytes of persistent region, run two
/// iterations across a RESET, and return what each read.
fn packed_run(
    m: &keleusma::bytecode::Module,
    persistent: usize,
) -> Result<(i64, i64), keleusma::vm::VmError> {
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + persistent);
    arena
        .resize_persistent(persistent)
        .expect("resize_persistent");
    let mut vm = Vm::new(m.clone(), &arena)?;
    let first = match vm.call(&[Value::Int(0)])? {
        VmState::Yielded(Value::Int(n)) => n,
        other => panic!("iteration 1 yielded {other:?}"),
    };
    match vm.resume(Value::Int(1))? {
        VmState::Reset => {}
        other => panic!("expected Reset, got {other:?}"),
    }
    let second = match vm.resume(Value::Int(1))? {
        VmState::Yielded(Value::Int(n)) => n,
        other => panic!("iteration 2 yielded {other:?}"),
    };
    Ok((first, second))
}

#[test]
fn every_packed_kind_survives_a_reset() {
    let m = compiled(PACKED_KINDS);
    let need = required_persistent_capacity_for(&m);
    let want = 7 + 9 + 1 + 2 + 5 + 100 + 1000;
    assert_eq!(packed_run(&m, need).expect("runs"), (want, want));
}

/// The persistent figure is tight. One `Value` slot for `s`, forty bytes of
/// `Word`, three of `Byte`, two of `bool`, sixteen of `Float` and four
/// sixteen-byte pool bodies. The element-slot storage that preceded packing
/// needed 17 slots of 32 bytes plus the pool, 608 bytes.
#[test]
fn the_persistent_figure_for_packed_arrays_is_tight() {
    let m = compiled(PACKED_KINDS);
    let need = required_persistent_capacity_for(&m);
    assert_eq!(need, 32 + 40 + 3 + 2 + 16 + 64);
    assert!(packed_run(&m, need).is_ok());
    match packed_run(&m, need - 1) {
        Err(keleusma::vm::VmError::VerifyError(message)) => {
            assert!(message.contains("persistent_capacity"), "{message}")
        }
        other => panic!("one byte below the figure must refuse, got {other:?}"),
    }
}

/// A packed element has room for its own kind only. Before packing, a `Value`
/// slot stored a value of another kind and a later read observed it, so this
/// is a host-observable change for bytecode that skipped the compiler.
#[test]
fn a_packed_element_refuses_a_value_of_another_kind() {
    use keleusma::bytecode::{ConstValue, Op};
    let mut m = compiled("private data d { x: [Word; 2] } fn main() -> Word { d.x[0] = 1; 0 }");
    m.chunks[0].constants = vec![ConstValue::Byte(7), ConstValue::Int(0)];
    m.chunks[0].ops = vec![
        Op::Const(0),
        Op::Const(1),
        Op::SetDataIndexed(0, 2),
        Op::PopN(6),
        Op::Const(1),
        Op::Return,
    ];
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize_persistent");
    let mut vm = Vm::new(m, &arena).expect("the module verifies");
    match vm.call(&[]) {
        Err(keleusma::vm::VmError::TypeError(message)) => {
            assert!(message.contains("packed private array"), "{message}")
        }
        other => panic!("a Byte written into a Word array must refuse, got {other:?}"),
    }
}

/// A hot swap seeds packed elements in element order, the host's index space.
/// A seed a packed element cannot hold refuses the swap and keeps the running
/// module.
#[test]
fn a_hot_swap_seeds_packed_elements_and_refuses_a_mismatched_seed() {
    const DECL: &str = "struct P { x: Word, y: Word }\n\
        private data d { a: [Word; 3], p: [P; 2], s: Word }\n";
    let first = compiled(&format!("{DECL}fn main() -> Word {{ d.s = d.s + 1; d.s }}"));
    let second = compiled(&format!(
        "{DECL}fn main() -> Word {{ d.s = d.s + 1; d.a[2] * 100 + d.p[1].y * 10 + d.s }}"
    ));
    let need =
        required_persistent_capacity_for(&first).max(required_persistent_capacity_for(&second));
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(first, &arena).expect("verifies");
    let p = |y: i64| {
        Value::struct_value(
            "P".into(),
            vec![("x".into(), Value::Int(0)), ("y".into(), Value::Int(y))],
        )
    };
    let refused = vm.replace_module(
        second.clone(),
        vec![
            Value::Int(1),
            Value::Byte(2),
            Value::Int(3),
            p(0),
            p(4),
            Value::Int(5),
        ],
    );
    assert!(
        matches!(refused, Err(keleusma::vm::VmError::TypeError(_))),
        "a Byte seed for a Word element must refuse, got {refused:?}"
    );
    // The running module is kept.
    match vm.call(&[]).expect("the first module still runs") {
        VmState::Finished(Value::Int(1)) => {}
        other => panic!("unexpected {other:?}"),
    }
    vm.replace_module(
        second,
        vec![
            Value::Int(1),
            Value::Int(2),
            Value::Int(3),
            p(0),
            p(4),
            Value::Int(5),
        ],
    )
    .expect("the swap is admitted");
    for expected in [346, 347] {
        match vm.call(&[]).expect("runs") {
            VmState::Finished(Value::Int(v)) => assert_eq!(v, expected),
            other => panic!("unexpected {other:?}"),
        }
    }
}

//! A private composite read is copied exactly where a later write can reach it.
//!
//! A private data slot holding a flat composite keeps its body at a fixed
//! offset in the persistent pool, and reading it pushes a handle to that body.
//! A later write to the slot overwrites the body in place, so before the repair
//! of 2026-10-31 a value read before the write and used after it observed the
//! written value. The operator ruled that the compiler copies such a read only
//! where a write to an overlapping slot can follow it while the value is live.
//! The rule is documented in `src/compiler/alias_copy.rs`.
//!
//! These tests cover the reference compiler, namely the values the repaired
//! programs compute, which reads are copied and which are not, and what one
//! copy costs.
//! The self-hosted compiler's agreement is checked in
//! `tests/selfhost_private_read_alias.rs`, which runs in the heavy tier.
#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::Arena;
use keleusma::bytecode::{ArrayElem, Module, NewCompositeOperand, Op, Value};
use keleusma::value_layout::CompositeKind;
use keleusma::vm::{Vm, VmState};

const P: &str = "struct P { x: Word, y: Word }\n";

fn compile(source: &str) -> Module {
    let tokens = keleusma::lexer::tokenize(source).unwrap_or_else(|e| panic!("{source}: {e:?}"));
    let program = keleusma::parser::parse(&tokens).unwrap_or_else(|e| panic!("{source}: {e:?}"));
    keleusma::compiler::compile(&program).unwrap_or_else(|e| panic!("{source}: {}", e.message))
}

fn run_module(module: Module) -> i64 {
    let need = keleusma::vm::required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(1 << 21);
    arena.resize_persistent(need).expect("persistent region");
    let mut vm = Vm::new(module, &arena).expect("the module verifies");
    let mut shared = vec![0; vm.shared_data_bytes()];
    match vm.call_with_shared(&mut shared, &[]) {
        Ok(VmState::Finished(Value::Int(n))) => n,
        other => panic!("unexpected outcome {other:?}"),
    }
}

fn run(source: &str) -> i64 {
    run_module(compile(source))
}

fn chunk_ops<'a>(module: &'a Module, name: &str) -> &'a [Op] {
    &module
        .chunks
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("no chunk `{name}`"))
        .ops
}

/// The four-op copy `emit_detach_data_element` emits.
fn is_copy_at(ops: &[Op], i: usize) -> bool {
    matches!(
        ops.get(i..i + 4),
        Some([
            Op::NewComposite(NewCompositeOperand::Flat {
                kind: CompositeKind::Array,
                count: 1,
                ..
            }),
            Op::PushImmediate(4),
            Op::GetIndex(ArrayElem::FlatNested { .. }),
            Op::PopN(6),
        ])
    )
}

fn copies(ops: &[Op]) -> usize {
    (0..ops.len()).filter(|&i| is_copy_at(ops, i)).count()
}

fn main_copies(source: &str) -> usize {
    copies(chunk_ops(&compile(source), "main"))
}

/// The two reproducers of the defect, each of which returned 12.
#[test]
fn a_read_used_after_a_write_to_its_slot_keeps_its_value() {
    let whole = format!(
        "{P}private data d {{ s: P }} fn main() -> Word {{ \
         d.s = P {{ x: 3, y: 4 }}; let o = d.s; d.s = P {{ x: 1, y: 2 }}; o.x * 10 + o.y }}"
    );
    let indexed = format!(
        "{P}private data d {{ x: [P; 2] }} fn main() -> Word {{ \
         d.x[1] = P {{ x: 3, y: 4 }}; let o = d.x[1]; d.x[1] = P {{ x: 1, y: 2 }}; o.x * 10 + o.y }}"
    );
    for source in [&whole, &indexed] {
        assert_eq!(run(source), 34, "{source}");
        assert_eq!(main_copies(source), 1, "{source}");
    }
}

/// Every composite shape that a private slot can hold aliases its pool body.
#[test]
fn tuple_enum_and_nested_struct_reads_keep_their_value() {
    let cases = [
        (
            "private data d { s: (Word, Word) } fn main() -> Word { \
             d.s = (3, 4); let o = d.s; d.s = (1, 2); o.0 * 10 + o.1 }"
                .to_string(),
            34,
        ),
        (
            "enum E { A(Word), B(Word) }\nprivate data d { e: E } fn main() -> Word { \
             d.e = E::A(3); let o = d.e; d.e = E::B(1); match o { E::A(v) => v, E::B(w) => w * 10 } }"
                .to_string(),
            3,
        ),
        // A view of a nested struct is still a view of the slot's body.
        (
            "struct I { a: Word }\nstruct Q { i: I, y: Word }\nprivate data d { s: Q } \
             fn main() -> Word { d.s = Q { i: I { a: 3 }, y: 4 }; let o = d.s.i; \
             d.s = Q { i: I { a: 1 }, y: 2 }; o.a }"
                .to_string(),
            3,
        ),
        // A handled indexed read binds its element in an arm.
        (
            format!(
                "{P}private data d {{ x: [P; 2] }} fn main() -> Word {{ d.x[1] = P {{ x: 3, y: 4 }}; \
                 d.x[1] {{ ok(o) => if true {{ d.x[1] = P {{ x: 1, y: 2 }}; o.x * 10 + o.y }} else {{ 0 }}, \
                 invalid_index(j) => j }} }}"
            ),
            34,
        ),
        // The loop variable of a `for` over a data array.
        (
            format!(
                "{P}private data d {{ x: [P; 2] }} fn main() -> Word {{ d.x[0] = P {{ x: 3, y: 4 }}; \
                 for p in d.x {{ d.x[0] = P {{ x: 1, y: 2 }}; d.x[1] = P {{ x: p.x, y: p.y }}; }} \
                 d.x[1].x * 10 + d.x[1].y }}"
            ),
            34,
        ),
    ];
    for (source, expected) in &cases {
        assert_eq!(run(source), *expected, "{source}");
        assert!(main_copies(source) >= 1, "{source}");
    }
}

/// A handled indexed read with an invalid index reaches its arm. The copy is
/// guarded to the in-range path, since the slot then holds the scalar
/// fallback rather than an element.
#[test]
fn a_handled_read_with_an_invalid_index_skips_the_copy() {
    let source = format!(
        "{P}private data d {{ x: [P; 2] }} fn main() -> Word {{ d.x[0] = P {{ x: 1, y: 2 }}; \
         d.x[5] {{ ok(o) => o.x, invalid_index(j) => j * 100 }} }}"
    );
    assert_eq!(run(&source), 500);
    assert_eq!(main_copies(&source), 1, "the copy is emitted, guarded");
}

/// A value that leaves the chunk, as a result or an argument, is copied,
/// because the caller or the callee may write the slot.
#[test]
fn a_value_escaping_through_a_return_or_an_argument_is_copied() {
    let getter = format!(
        "{P}private data d {{ s: P }} fn get() -> P {{ d.s }} fn main() -> Word {{ \
         d.s = P {{ x: 3, y: 4 }}; let o = get(); d.s = P {{ x: 1, y: 2 }}; o.x * 10 + o.y }}"
    );
    assert_eq!(run(&getter), 34);
    assert_eq!(copies(chunk_ops(&compile(&getter), "get")), 1);
    assert_eq!(copies(chunk_ops(&compile(&getter), "main")), 0);

    let argument = format!(
        "{P}private data d {{ s: P }} fn f(o: P) -> Word {{ d.s = P {{ x: 1, y: 2 }}; o.x * 10 + o.y }} \
         fn main() -> Word {{ d.s = P {{ x: 3, y: 4 }}; f(d.s) }}"
    );
    assert_eq!(run(&argument), 34);
    assert_eq!(main_copies(&argument), 1);
}

/// A call between the read and a later use counts as a write, since the
/// callee may write the slot. Here it does.
#[test]
fn a_callee_write_is_a_write() {
    let source = format!(
        "{P}private data d {{ s: P }} fn w() -> Word {{ d.s = P {{ x: 1, y: 2 }}; 0 }} \
         fn main() -> Word {{ d.s = P {{ x: 3, y: 4 }}; let o = d.s; let z = w(); o.x * 10 + o.y + z }}"
    );
    assert_eq!(run(&source), 34);
    assert_eq!(main_copies(&source), 1);
}

/// Loops. A value bound before a loop and used inside it outlives every
/// iteration, so a write anywhere in the loop reaches it. A value bound inside
/// the loop is rebound each iteration, so a write after its last use does not.
#[test]
fn loops_extend_the_value_over_every_iteration_that_uses_it() {
    let prefix = format!("{P}private data d {{ s: P, t: Word }} fn main() -> Word {{ ");
    let outside = format!(
        "{prefix}d.s = P {{ x: 3, y: 4 }}; d.t = 0; let o = d.s; \
         for i in 0..2 limit 2 {{ d.t = d.t + o.x * 10 + o.y; d.s = P {{ x: 1, y: 2 }}; }} d.t }}"
    );
    assert_eq!(run(&outside), 68);
    assert_eq!(main_copies(&outside), 1);

    let after = format!(
        "{prefix}d.s = P {{ x: 3, y: 4 }}; let o = d.s; \
         for i in 0..3 limit 3 {{ d.s = P {{ x: i, y: i }}; }} o.x * 10 + o.y }}"
    );
    assert_eq!(run(&after), 34);
    assert_eq!(main_copies(&after), 1);

    let inside = format!(
        "{prefix}d.s = P {{ x: 3, y: 4 }}; d.t = 0; \
         for i in 0..2 limit 2 {{ let o = d.s; d.t = d.t + o.x * 10 + o.y; d.s = P {{ x: 1, y: 2 }}; }} d.t }}"
    );
    assert_eq!(run(&inside), 46);
    assert_eq!(main_copies(&inside), 0);
}

/// Branches. A write in one arm of an `if` can precede a use after the `if`.
#[test]
fn a_write_in_a_branch_reaches_a_later_use() {
    let source = format!(
        "{P}private data d {{ s: P }} fn main() -> Word {{ d.s = P {{ x: 3, y: 4 }}; let o = d.s; \
         if o.x > 0 {{ d.s = P {{ x: 1, y: 2 }}; 0 }} else {{ 0 }}; o.x * 10 + o.y }}"
    );
    assert_eq!(run(&source), 34);
    assert_eq!(main_copies(&source), 1);
}

/// Writing a slot's own value back copied overlapping bytes with
/// `copy_nonoverlapping`, which aborts a debug build. The compiled form now
/// copies the value first, and the runtime tolerates the overlap when the
/// bytecode does not.
#[test]
fn writing_a_slot_back_to_itself_is_defined() {
    let source = format!(
        "{P}private data d {{ s: P }} fn main() -> Word {{ \
         d.s = P {{ x: 3, y: 4 }}; d.s = d.s; d.s.x * 10 + d.s.y }}"
    );
    assert_eq!(run(&source), 34);
    let mut module = compile(&source);
    let main = module
        .chunks
        .iter_mut()
        .find(|c| c.name == "main")
        .expect("main");
    let at = (0..main.ops.len())
        .find(|&i| is_copy_at(&main.ops, i))
        .expect("the self-assignment is copied");
    // Remove the copy. The chunk has no jumps, so no target moves.
    assert!(!main.ops.iter().any(|op| matches!(
        op,
        Op::If(_) | Op::Else(_) | Op::Loop(_) | Op::EndLoop(_) | Op::Break(_) | Op::BreakIf(_)
    )));
    main.ops.drain(at..at + 4);
    assert!(matches!(
        main.ops[at - 1..=at],
        [Op::GetData(0), Op::SetData(0)]
    ));
    assert_eq!(run_module(module), 34);
}

/// The reads that need no copy. A read reduced to a scalar at once, a value
/// whose last use precedes every write, a value used only in the value of the
/// write that replaces it, a function with no write, a copy into another slot,
/// and a shared slot, which already copies on read.
#[test]
fn a_read_no_write_can_reach_is_not_copied() {
    let private = format!(
        "{P}private data d {{ s: P, u: P }} fn main() -> Word {{ d.s = P {{ x: 3, y: 4 }}; "
    );
    let cases = [
        format!("{private}let v = d.s.x * 10 + d.s.y; d.s = P {{ x: 1, y: 2 }}; v }}"),
        format!("{private}let o = d.s; o.x * 10 + o.y }}"),
        format!("{private}let o = d.s; let v = o.x * 10 + o.y; d.s = P {{ x: 1, y: 2 }}; v }}"),
        format!(
            "{private}let p = d.s; d.s = P {{ x: p.x + 1, y: p.y }}; d.s.x * 10 + d.s.y - 10 }}"
        ),
        format!("{private}d.u = d.s; d.s = P {{ x: 1, y: 2 }}; d.u.x * 10 + d.u.y }}"),
        format!(
            "{P}shared data d {{ s: P }} fn main() -> Word {{ \
             d.s = P {{ x: 3, y: 4 }}; let o = d.s; d.s = P {{ x: 1, y: 2 }}; o.x * 10 + o.y }}"
        ),
    ];
    for source in &cases {
        assert_eq!(run(source), 34, "{source}");
        assert_eq!(main_copies(source), 0, "{source}");
    }
}

/// The cost of one copy, measured as the difference it makes to the chunk's
/// own worst-case bounds. The four ops cost fifteen nominal cycles, and the
/// heap bound grows by twice the element's byte size.
#[test]
fn one_copy_costs_four_ops_and_one_element_allocation() {
    let source = format!(
        "{P}private data d {{ s: P }} fn main() -> Word {{ \
         d.s = P {{ x: 3, y: 4 }}; let o = d.s; d.s = P {{ x: 1, y: 2 }}; o.x * 10 + o.y }}"
    );
    let module = compile(&source);
    let main = module
        .chunks
        .iter()
        .find(|c| c.name == "main")
        .expect("main")
        .clone();
    let at = (0..main.ops.len())
        .find(|&i| is_copy_at(&main.ops, i))
        .expect("one copy");
    let mut bare = main.clone();
    bare.ops.drain(at..at + 4);
    let cycles: u32 = main.ops[at..at + 4].iter().map(Op::cost).sum();
    let wcet = keleusma::verify::wcet_whole_chunk(&main).expect("wcet");
    let bare_wcet = keleusma::verify::wcet_whole_chunk(&bare).expect("wcet");
    assert_eq!(wcet - bare_wcet, cycles);
    assert_eq!(cycles, COPY_CYCLES);
    let (stack, heap) = keleusma::verify::wcmu_whole_chunk(&main).expect("wcmu");
    let (bare_stack, bare_heap) = keleusma::verify::wcmu_whole_chunk(&bare).expect("wcmu");
    assert_eq!(
        heap - bare_heap,
        COPY_HEAP_BYTES_FOR_16,
        "heap bytes for a 16-byte struct"
    );
    // After its `GetIndex` the copy holds the element, the index, four bound
    // limbs and a flag, six slots where the read held one, until `PopN(6)`
    // drops five, so the operand peak can rise by up to five slots. This
    // chunk's peak rises by four 32-byte slots, as measured. Where a copy sits
    // further below the chunk's peak it rises by less or not at all.
    assert_eq!(stack - bare_stack, 128);
    println!(
        "one copy of a 16-byte struct: {cycles} cycles, {} heap bytes, {} stack bytes",
        heap - bare_heap,
        stack - bare_stack
    );
}

/// The nominal cycles of the four copy ops.
const COPY_CYCLES: u32 = 15;
/// The worst-case heap bytes one copy of a 16-byte struct adds. The bound
/// charges the one-element array's 16 bytes and, again, the 16-byte
/// placeholder that `GetIndex` would allocate on an invalid index, which the
/// constant index zero never takes. The charge is twice the element size.
const COPY_HEAP_BYTES_FOR_16: u32 = 32;

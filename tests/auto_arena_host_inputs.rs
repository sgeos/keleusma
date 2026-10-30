//! The auto-sized arena holds the composite host inputs of the entry chunk.
//!
//! # The defect
//!
//! `auto_arena_capacity_for` summed the operand stack, the call frames, the
//! opaque registry and the program's heap, and documented that host inputs
//! were outside the figure. `Vm::call` copies each composite entry argument
//! into the arena top region, and `Vm::resume` copies each composite resume
//! value there, and the copies stay until the next `Reset`. An arena of the
//! reported size therefore failed with `OutOfArena` at the first composite
//! input. A nested host composite cost more still, because each nested child
//! was packed into its own body before its parent copied it, and the dead
//! child bodies stayed until `Reset`.
//!
//! # What these tests establish
//!
//! - An arena of exactly the returned size admits a composite entry argument
//!   and every composite resume value of several iterations, including one
//!   resume per pass of a bounded loop and one through a `yield fn` callee.
//! - An arena of the figure without the input term fails, so the term is
//!   load-bearing.
//! - A scalar-only module gets the same figure as before.
//! - A text-bearing composite input needs the host's text allowance, and an
//!   arena sized with it admits the inputs.
//!
//! # What they do NOT establish
//!
//! The figure is not shown to be tight. The input term charges one transient
//! body even when an input has no nested composite. Non-entry chunks called
//! through `Vm::call_function` are outside the figure by its contract.
#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::Arena;
use keleusma::bytecode::{Module, Value};
use keleusma::vm::{
    Vm, VmError, VmState, auto_arena_capacity_for, auto_arena_capacity_with_input_text,
    host_input_arena_bytes_for, required_persistent_capacity_for,
};

fn compile(src: &str) -> Module {
    let tokens = keleusma::lexer::tokenize(src).expect("lex");
    keleusma::compiler::compile(&keleusma::parser::parse(&tokens).expect("parse")).expect("compile")
}

fn word_pair(name: &str, x: i64, y: i64) -> Value {
    Value::struct_value(
        String::from(name),
        vec![
            (String::from("x"), Value::Int(x)),
            (String::from("y"), Value::Int(y)),
        ],
    )
}

/// A host-built `Q { a: P, b: P }`, boxed and nested, as a host constructs it
/// without an arena.
fn nested(k: i64) -> Value {
    Value::struct_value(
        String::from("Q"),
        vec![
            (String::from("a"), word_pair("P", k, k + 1)),
            (String::from("b"), word_pair("P", k + 2, k + 3)),
        ],
    )
}

/// A Stream with two straight-line yields and a yield in a three-pass loop,
/// so five resume values arrive between one `Reset` and the next.
const NESTED_STREAM: &str = "\
struct P { x: Word, y: Word }
struct Q { a: P, b: P }
loop main(e: Q) -> Word {
    let r: Q = yield e.a.x;
    let s: Q = yield r.b.y;
    for i in 0..3 {
        let t: Q = yield i + s.a.x;
    }
    0
}";

/// Run `iterations` full Stream iterations with nested composite inputs,
/// returning the first error.
fn drive_nested(module: Module, capacity: usize, iterations: usize) -> Result<usize, VmError> {
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(capacity + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(module, &arena)?;
    let mut state = vm.call(&[nested(10)])?;
    let mut resets = 0usize;
    let mut k = 20i64;
    while resets < iterations {
        match state {
            VmState::Yielded(_) => {}
            VmState::Reset => resets += 1,
            other => panic!("unexpected {other:?}"),
        }
        state = vm.resume(nested(k))?;
        k += 1;
    }
    Ok(resets)
}

#[test]
fn the_resume_count_covers_straight_line_and_loop_yields() {
    let module = compile(NESTED_STREAM);
    let entry = module.entry_point.expect("entry");
    let resumes = keleusma::verify::max_resumes_before_reset(&module, entry).expect("bounded");
    // Two straight-line yields plus three loop passes. The walk may charge
    // more, never fewer.
    assert!(
        resumes >= 5,
        "resume bound {resumes} is below the five yields"
    );
}

#[test]
fn an_arena_of_exactly_the_figure_holds_nested_composite_inputs() {
    let module = compile(NESTED_STREAM);
    let capacity = auto_arena_capacity_for(&module, &[]).expect("bounded");
    let resets = drive_nested(module, capacity, 4).expect("the auto-sized arena holds every input");
    assert_eq!(resets, 4);
}

#[test]
fn the_figure_without_its_input_term_is_too_small() {
    let module = compile(NESTED_STREAM);
    let capacity = auto_arena_capacity_for(&module, &[]).expect("bounded");
    let inputs = host_input_arena_bytes_for(&module, 0).expect("bounded");
    // Q is four words, so each input is a 32-byte body, and the term counts
    // the entry argument, at least five resumes, and one transient body.
    assert!(
        inputs >= 7 * 32,
        "input term {inputs} is below seven bodies"
    );
    let old_figure = capacity - inputs;
    match drive_nested(module, old_figure, 4) {
        Err(VmError::OutOfArena(_)) => {}
        other => panic!("the old figure should run out of arena, got {other:?}"),
    }
}

#[test]
fn a_scalar_only_module_keeps_its_figure() {
    let module = compile("loop main(e: Word) -> Word { let n = yield e + 1; n }");
    assert_eq!(host_input_arena_bytes_for(&module, 0).expect("bounded"), 0);
    assert_eq!(
        host_input_arena_bytes_for(&module, 1000).expect("bounded"),
        0
    );
    // Measured before the input term existed, at the same tree.
    assert_eq!(auto_arena_capacity_for(&module, &[]).expect("bounded"), 184);
}

/// A `yield fn` callee whose two yields are charged at its call site.
const CALLEE_STREAM: &str = "\
struct P { x: Word, y: Word }
yield twice(v: Word) -> Word {
    let a = yield v;
    let b = yield v + 1;
    v
}
loop main(e: P) -> Word {
    let r = twice(e.x);
    let s: P = yield e.y;
    0
}";

#[test]
fn a_yield_fn_callee_is_counted_at_its_call_site() {
    let module = compile(CALLEE_STREAM);
    let entry = module.entry_point.expect("entry");
    let resumes = keleusma::verify::max_resumes_before_reset(&module, entry).expect("bounded");
    assert!(
        resumes >= 3,
        "resume bound {resumes} misses the callee's yields"
    );
    let capacity = auto_arena_capacity_for(&module, &[]).expect("bounded");
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(capacity + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(module, &arena).expect("verifies");
    let mut state = vm.call(&[word_pair("P", 1, 2)]).expect("call");
    let mut resets = 0usize;
    let mut k = 0i64;
    while resets < 3 {
        match state {
            VmState::Yielded(_) => {}
            VmState::Reset => resets += 1,
            other => panic!("unexpected {other:?}"),
        }
        state = vm
            .resume(word_pair("P", k, k + 1))
            .expect("the auto-sized arena holds every resume");
        k += 1;
    }
}

const TEXT_STREAM: &str = "\
struct M { tag: Text, n: Word }
loop main(e: M) -> Word {
    let r: M = yield e.n;
    0
}";

fn message(text: &str, n: i64) -> Value {
    Value::struct_value(
        String::from("M"),
        vec![
            (String::from("tag"), Value::StaticStr(String::from(text))),
            (String::from("n"), Value::Int(n)),
        ],
    )
}

fn drive_text(module: Module, capacity: usize, text: &str) -> Result<(), VmError> {
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(capacity + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(module, &arena)?;
    let mut state = vm.call(&[message(text, 1)])?;
    let mut resets = 0usize;
    while resets < 3 {
        match state {
            VmState::Yielded(_) => {}
            VmState::Reset => resets += 1,
            other => panic!("unexpected {other:?}"),
        }
        state = vm.resume(message(text, 2))?;
    }
    Ok(())
}

#[test]
fn text_inside_a_composite_input_needs_the_host_allowance() {
    let text = "a host string longer than any slack in the figure, sixty-four bytes";
    let module = compile(TEXT_STREAM);
    let without = auto_arena_capacity_for(&module, &[]).expect("bounded");
    let with = auto_arena_capacity_with_input_text(&module, &[], text.len()).expect("bounded");
    assert!(with > without);
    drive_text(module.clone(), with, text).expect("the allowance covers the text");
    match drive_text(module, without, text) {
        Err(VmError::OutOfArena(_)) => {}
        other => panic!("text with no allowance should run out of arena, got {other:?}"),
    }
}

/// Read the little-endian words of a flat struct body.
fn flat_words(v: &Value, arena: &Arena) -> Vec<i64> {
    let Value::Struct(keleusma::bytecode::StructBody::Flat(body)) = v else {
        panic!("expected a flat struct, got {v:?}");
    };
    let bytes = body.resolve(arena).expect("live");
    assert_eq!(bytes.len() % 8, 0, "a body of whole words");
    bytes
        .as_chunks::<8>()
        .0
        .iter()
        .map(|w| i64::from_le_bytes(*w))
        .collect()
}

#[test]
fn a_nested_input_holds_only_its_own_flat_size() {
    let arena = Arena::with_capacity(4096);
    let canonical = nested(5)
        .into_arena_canonical(8, 8, 8, &arena)
        .expect("fits");
    // Before the compaction the two dead 16-byte child bodies stayed beside
    // the 32-byte parent, 64 bytes in all.
    assert_eq!(arena.top_used(), 32);
    assert_eq!(flat_words(&canonical, &arena), vec![5, 6, 7, 8]);
}

#[test]
fn a_deep_single_field_chain_holds_only_its_own_flat_size() {
    // Each wrapper adds no bytes, so the dead child bodies of an uncompacted
    // chain would grow with the depth while the flat size stays eight.
    let mut v = Value::Int(42);
    for depth in 0..12 {
        v = Value::struct_value(format!("W{depth}"), vec![(String::from("f"), v)]);
    }
    let arena = Arena::with_capacity(4096);
    let canonical = v.into_arena_canonical(8, 8, 8, &arena).expect("fits");
    assert_eq!(arena.top_used(), 8);
    assert_eq!(flat_words(&canonical, &arena), vec![42]);
}

#[test]
fn compaction_keeps_promoted_text_alive() {
    let text = "kept across the compaction";
    let outer = Value::tuple(vec![message(text, 9), Value::Int(3)]);
    let arena = Arena::with_capacity(4096);
    let canonical = outer.into_arena_canonical(8, 8, 8, &arena).expect("fits");
    // The text bytes, then one 32-byte body holding the pointer, length and
    // word of `M` and the word of the tuple. The dead 24-byte body of `M` is
    // reclaimed.
    assert_eq!(arena.top_used(), text.len() + 32);
    let Value::Tuple(keleusma::bytecode::TupleBody::Flat(body)) = &canonical else {
        panic!("expected a flat tuple, got {canonical:?}");
    };
    let bytes = body.resolve(&arena).expect("live");
    let word = |i: usize| u64::from_le_bytes(bytes[i * 8..i * 8 + 8].try_into().expect("eight"));
    let (ptr, len) = (word(0) as usize, word(1) as usize);
    assert_eq!(len, text.len());
    // SAFETY: the pointer and length were written by the packer from a live
    // arena string, and the arena has not been reset or rewound since.
    let stored = unsafe { core::slice::from_raw_parts(ptr as *const u8, len) };
    assert_eq!(stored, text.as_bytes());
    assert_eq!((word(2), word(3)), (9, 3));
}

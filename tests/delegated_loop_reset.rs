//! A `loop` called from a `loop` receives the resume value in its parameter.
//!
//! # The defect
//!
//! A `loop` may call a `loop`, which `category_can_call` in `src/typecheck.rs`
//! admits and `tests/stream_never_returns.rs` pins as constructible. The callee
//! never returns, so its `Reset` restarts the callee in place with the caller's
//! frame beneath it. `Op::Reset` clears every local of the resetting frame, the
//! parameter included. `Vm::resume` then wrote the host's input into slot 0 of
//! the BASE frame, which belongs to the caller and is never resumed, so the
//! callee's parameter stayed `Unit` on every iteration after the first.
//!
//! The definite-initialization verifier, `src/verify_init.rs`, assumes that
//! each resume of a stream with a parameter writes slot 0 of that stream, so it
//! admitted the read. Measured on 2026-10-31 before the fix, a tuple built from
//! the `Unit` parameter faulted with `InvalidBytecode("flat scalar codec:
//! OutOfBounds")` on the resume after the first reset, or yielded the wrong
//! field. Written into a `private data` composite, it stored zeros, which is
//! why the second finding first looked like a defect in the private read alias
//! copy of pull request 690. The zeros vanish with this fix alone, which is
//! evidence, not proof, that the alias copy is not involved.
//!
//! # The intended semantics, and how sure this file is of them
//!
//! A delegated `loop` behaves exactly as it would as the entry loop. Its
//! parameter holds the call argument until the first resume, and from then on
//! it holds the latest resume value, both after a `Reset` and at a `yield`.
//! The resume value is checked against the callee's own parameter type.
//!
//! The after-reset half is decided with high confidence. It is the premise
//! `src/verify_init.rs` already verifies against, and it matches the statement
//! in `docs/architecture/LANGUAGE_DESIGN.md` that the parameter slot of a loop
//! function is updated with the host's value on each resume. The mid-iteration
//! half is an inference from the same sentence and from making the delegated
//! loop indistinguishable from the entry loop. It is recorded as an open
//! question for the operator, not as a settled ruling.
//! `docs/spec/GRAMMAR.md` section 6.3 still says only one `loop` may exist per
//! script, which the implementation does not enforce. That conflict predates
//! this file and is not resolved here.
//!
//! # The oracle
//!
//! Every delegated case that can be is paired with the same body written as
//! `loop main` alone, which behaved correctly before the fix, and the two
//! sequences must be equal as well as equal to the pinned one.
#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::Arena;
use keleusma::bytecode::Value;
use keleusma::vm::{Vm, VmError, VmState, required_persistent_capacity_for};

/// The first argument the host passes to `main`.
const FIRST: i64 = 5;
/// The reply the host passes to every resume unless a test supplies its own.
const REPLY: i64 = 7;
/// The operand arena above the persistent region. The program sizes here are
/// tiny, so a fixed generous figure keeps arena sizing out of the question.
const ARENA: usize = 1 << 16;

fn compile(src: &str) -> keleusma::bytecode::Module {
    let tokens = keleusma::lexer::tokenize(src).expect("lex");
    keleusma::compiler::compile(&keleusma::parser::parse(&tokens).expect("parse")).expect("compile")
}

/// Drive `src` with `FIRST`, then resume with `replies(i)` for the `i`th
/// resume. A `Reset` is collapsed into the next resume, which passes the reply
/// for its own position in the resume sequence. Returns the first `count`
/// yielded integers and the number of resets seen, or the first error.
fn drive_with(
    src: &str,
    count: usize,
    replies: impl Fn(usize) -> i64,
) -> Result<(Vec<i64>, usize), VmError> {
    let module = compile(src);
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(ARENA + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(module, &arena).expect("verifies");
    let mut out = Vec::with_capacity(count);
    let mut resets = 0usize;
    let mut state = vm.call(&[Value::Int(FIRST)])?;
    // Every iteration yields, so `count` yields need at most `count` resets
    // between them. The cap is a backstop that keeps the loop bounded.
    for resumes in 0..(4 * count + 4) {
        match state {
            VmState::Yielded(Value::Int(v)) => {
                out.push(v);
                if out.len() == count {
                    return Ok((out, resets));
                }
            }
            VmState::Reset => resets += 1,
            other => panic!("unexpected state {other:?}"),
        }
        state = vm.resume(Value::Int(replies(resumes)))?;
    }
    panic!("no {count} yields within the cap, got {out:?}");
}

fn drive(src: &str, count: usize) -> Vec<i64> {
    drive_with(src, count, |_| REPLY).expect("runs").0
}

/// Run a body as a delegated child and as `loop main` alone, and require both
/// to produce `expected`. `decls` precedes both forms.
fn delegated_and_alone(decls: &str, body: &str, expected: &[i64]) {
    let delegated = format!(
        "{decls} loop child(a: Word) -> Word {{ {body} }} loop main(t: Word) -> Word {{ child(t) }}"
    );
    let alone = format!("{decls} loop main(a: Word) -> Word {{ {body} }}");
    let got_alone = drive(&alone, expected.len());
    let got_delegated = drive(&delegated, expected.len());
    assert_eq!(got_alone, expected, "loop main alone: {alone}");
    assert_eq!(got_delegated, expected, "delegated: {delegated}");
}

const PRIVATE: &str = "private data st { saved: (Word, Word) }";

// -- Finding 1 --------------------------------------------------------------

#[test]
fn a_tuple_field_of_a_delegated_loop_survives_its_reset() {
    // Before the fix the second element was not reached. The resume after the
    // first reset failed with `flat scalar codec: OutOfBounds`.
    delegated_and_alone("", "let s = (a, 17); yield s.1", &[17, 17, 17, 17, 17, 17]);
}

#[test]
fn a_delegated_loop_parameter_holds_the_resume_value_after_reset() {
    // Before the fix this yielded 5 and then 17 on every later cycle.
    delegated_and_alone("", "let s = (a, 17); yield s.0", &[5, 7, 7, 7, 7, 7]);
}

#[test]
fn the_non_delegated_tuple_controls_are_unchanged() {
    assert_eq!(
        drive(
            "loop main(t: Word) -> Word { let s = (t, 17); yield s.1 }",
            6
        ),
        [17, 17, 17, 17, 17, 17]
    );
    assert_eq!(
        drive(
            "loop main(t: Word) -> Word { let s = (t, 17); yield s.0 }",
            6
        ),
        [5, 7, 7, 7, 7, 7]
    );
}

// -- Finding 2 --------------------------------------------------------------

#[test]
fn a_private_composite_rewritten_from_the_parameter_reads_back_after_reset() {
    // Before the fix this yielded 29, 17, 29, 0, 29, 0.
    delegated_and_alone(
        PRIVATE,
        "st.saved = (5, 29); let old = st.saved; st.saved = (a, 17); let now = st.saved; \
         yield old.1; yield now.1",
        &[29, 17, 29, 17, 29, 17],
    );
}

#[test]
fn a_private_composite_first_field_reads_the_resume_value_after_reset() {
    // Before the fix this yielded 5, 29, 5, 5, 29, 0. In the first iteration
    // `now` is built before any resume, so its first field is the argument 5.
    delegated_and_alone(
        PRIVATE,
        "st.saved = (5, 29); let old = st.saved; st.saved = (a, 17); let now = st.saved; \
         yield old.0; yield old.1; yield now.0",
        &[5, 29, 5, 5, 29, 7, 5, 29, 7],
    );
}

#[test]
fn a_private_composite_self_assignment_survives_reset() {
    // Before the fix this yielded 5 and then 0 on every later cycle.
    delegated_and_alone(
        PRIVATE,
        "st.saved = (a, 17); st.saved = st.saved; yield st.saved.0",
        &[5, 7, 7, 7, 7, 7],
    );
}

// -- Edge cases -------------------------------------------------------------

#[test]
fn each_reset_delivers_the_reply_that_ended_it() {
    // Distinct replies show which resume value each iteration sees. Every
    // iteration is one yield then one reset, so the resumes alternate between
    // completing a yield and ending a reset, and resume `2k + 1` ends reset
    // `k + 1`. Several resets in a row are crossed, each with its own reply.
    let replies = |i: usize| 100 + i as i64;
    let delegated = "loop child(a: Word) -> Word { let s = (a, 17); yield s.0 } \
                     loop main(t: Word) -> Word { child(t) }";
    let alone = "loop main(a: Word) -> Word { let s = (a, 17); yield s.0 }";
    let expected = vec![5, 101, 103, 105];
    let (got, resets) = drive_with(delegated, 4, replies).expect("runs");
    assert_eq!(got, expected);
    assert_eq!(resets, 3);
    assert_eq!(drive_with(alone, 4, replies).expect("runs").0, expected);
}

#[test]
fn a_delegated_parameter_follows_every_resume_like_the_entry_loop() {
    // The mid-iteration half of the semantics. The second `yield a` reads the
    // reply to the first, exactly as it does for `loop main` alone. Before the
    // fix the delegated form yielded 5, 5 and then `Unit`.
    delegated_and_alone("", "yield a; yield a", &[5, 7, 7, 7, 7, 7]);
    delegated_and_alone(
        "",
        "let b = yield a; yield b + 100",
        &[5, 107, 7, 107, 7, 107],
    );
}

#[test]
fn a_nested_delegation_chain_rebinds_the_innermost_loop() {
    // `main` calls `mid` with an offset argument, and `mid` calls `leaf`. Only
    // the innermost loop resets and resumes, so after its first reset the
    // offset is gone and `leaf` sees the reply.
    let src = "loop leaf(c: Word) -> Word { let s = (c, 3); yield s.0 + s.1 } \
               loop mid(b: Word) -> Word { leaf(b + 100) } \
               loop main(t: Word) -> Word { mid(t) }";
    assert_eq!(drive(src, 5), [108, 10, 10, 10, 10]);
}

#[test]
fn a_delegated_loop_with_a_tuple_local_and_a_private_composite() {
    // In the first iteration the resume at the first yield rebinds `a` to 7,
    // but `st.saved` was written from the argument before it, so it still
    // holds 5. From the second iteration on everything sees 7.
    delegated_and_alone(
        PRIVATE,
        "let s = (a, 17); st.saved = (s.1, a); yield s.0; yield st.saved.1",
        &[5, 5, 7, 7, 7, 7],
    );
}

#[test]
fn a_delegated_loop_resumed_through_a_yield_helper() {
    // The suspension happens in a `yield` function called by the delegated
    // loop, so the top frame is not a stream. The innermost stream frame below
    // it is the one rebound.
    let decls = "yield helper(x: Word) -> Word { yield x + 1 }";
    delegated_and_alone(decls, "helper(a)", &[6, 8, 8, 8]);
}

#[test]
fn a_delegated_loop_with_a_different_resume_type_is_refused_at_compile_time() {
    // Before the fix this program yielded 1, reset, and then failed with
    // `condition must be Bool, got Unit`. The type checker now refuses a
    // delegated loop whose parameter type differs from the entry loop's, so
    // the program never reaches the virtual machine. The resume boundary also
    // checks the callee's own parameter type, as a second line of defence for
    // a module that reaches the virtual machine without the type checker.
    let src = "loop child(a: bool) -> Word { if a { yield 1 } else { yield 2 } } \
               loop main(t: Word) -> Word { child(true) }";
    let tokens = keleusma::lexer::tokenize(src).expect("lex");
    let program = keleusma::parser::parse(&tokens).expect("parse");
    let err = keleusma::compiler::compile(&program).expect_err("a Word cannot rebind a bool");
    assert!(
        err.message.contains("resumes with"),
        "expected the delegated resume type refusal, got {}",
        err.message
    );
}

// -- The admitted shape, a delegated `yield` function -------------------------
//
// `docs/spec/GRAMMAR.md` section 6.3 admits a `loop` calling a `yield`
// function that shares its yield contract. Such a callee returns, so its frame
// is rebuilt with the argument on every call and only the entry loop resets.
// These cases run the reported bodies in that shape, and they passed BEFORE
// the fix as well, measured on 2026-10-31, so the defect appears confined to
// a `loop` calling a `loop`. Each body reads its
// parameter only before its first yield, where a `yield` callee and the entry
// loop agree.

/// Run a body as a delegated `yield` function and as `loop main` alone, and
/// require both to produce `expected`.
fn delegated_yield_and_alone(decls: &str, body: &str, expected: &[i64]) {
    let delegated = format!(
        "{decls} yield child(a: Word) -> Word {{ {body} }} loop main(t: Word) -> Word {{ child(t) }}"
    );
    let alone = format!("{decls} loop main(a: Word) -> Word {{ {body} }}");
    assert_eq!(
        drive(&alone, expected.len()),
        expected,
        "loop main alone: {alone}"
    );
    assert_eq!(
        drive(&delegated, expected.len()),
        expected,
        "delegated yield: {delegated}"
    );
}

#[test]
fn the_reported_bodies_in_a_delegated_yield_function() {
    delegated_yield_and_alone("", "let s = (a, 17); yield s.1", &[17, 17, 17, 17]);
    delegated_yield_and_alone("", "let s = (a, 17); yield s.0", &[5, 7, 7, 7]);
    delegated_yield_and_alone(
        PRIVATE,
        "st.saved = (5, 29); let old = st.saved; st.saved = (a, 17); let now = st.saved; \
         yield old.1; yield now.1",
        &[29, 17, 29, 17, 29, 17],
    );
    delegated_yield_and_alone(
        PRIVATE,
        "st.saved = (5, 29); let old = st.saved; st.saved = (a, 17); let now = st.saved; \
         yield old.0; yield old.1; yield now.0",
        &[5, 29, 5, 5, 29, 7, 5, 29, 7],
    );
    delegated_yield_and_alone(
        PRIVATE,
        "st.saved = (a, 17); st.saved = st.saved; yield st.saved.0",
        &[5, 7, 7, 7],
    );
}

#[test]
fn a_delegated_yield_parameter_is_the_argument_for_the_whole_call() {
    // A `yield` callee's parameter is an ordinary argument. A mid-call resume
    // does not rebind it, unlike a loop's parameter, so the second `yield a`
    // still reads the argument. Pinned to keep the fix scoped to stream frames.
    let src = "yield child(a: Word) -> Word { yield a; yield a } \
               loop main(t: Word) -> Word { child(t) }";
    assert_eq!(drive(src, 6), [5, 5, 7, 7, 7, 7]);
}

// -- The arena across a delegated reset ---------------------------------------
//
// Measurements behind `docs/decisions/NESTED_LOOP_ARENAS.md`, taken on
// 2026-11-04. The operator ruled on that day that a `loop` may call a `loop`
// and asked whether nested loops need their own arena. These cases record what
// the virtual machine does today when the inner loop resets while the outer
// frame sits beneath it. They pin present behaviour, not a ruled design, so a
// change made under that record is expected to update them deliberately.

/// The outer loop builds a sixteen-element array, 128 bytes of flat body in the
/// top region, and passes one element to the inner loop, which builds a
/// four-element array of 32 bytes every iteration.
const OUTER_HOLDS_ARRAY: &str = "loop child(a: Word) -> Word { let s = [a, a, a, a]; yield s[1] } \
     loop main(t: Word) -> Word { \
       let b = [t, t, t, t, t, t, t, t, t, t, t, t, t, t, t, t]; child(b[3]) }";

/// One observation per host-visible suspension.
#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    state: String,
    top_used: usize,
    bottom_used: usize,
    epoch: u64,
}

fn snapshots(src: &str, count: usize) -> (Vec<Snapshot>, usize) {
    let module = compile(src);
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(ARENA + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(module, &arena).expect("verifies");
    let mut out = Vec::with_capacity(count);
    let mut state = vm.call(&[Value::Int(FIRST)]).expect("call");
    for _ in 0..count {
        let a = vm.arena();
        out.push(Snapshot {
            state: format!("{state:?}"),
            top_used: a.top_used(),
            bottom_used: a.bottom_used(),
            epoch: a.epoch(),
        });
        state = vm.resume(Value::Int(REPLY)).expect("resume");
    }
    (out, vm.arena().top_peak())
}

#[test]
fn an_inner_reset_reclaims_the_outer_frames_top_region_bodies() {
    let (snaps, top_peak) = snapshots(OUTER_HOLDS_ARRAY, 4);
    // The first yield holds both bodies, 128 bytes for the outer array and 32
    // for the inner one. The inner reset reclaims the whole top region, the
    // outer array included, while the outer frame's local still names it.
    assert_eq!(snaps[0].state, "Yielded(Int(5))");
    assert_eq!(snaps[0].top_used, 160);
    assert_eq!(snaps[1].state, "Reset");
    assert_eq!(snaps[1].top_used, 0);
    // From then on only the inner array occupies the top region.
    assert_eq!(snaps[2].state, "Yielded(Int(7))");
    assert_eq!(snaps[2].top_used, 32);
    assert_eq!(top_peak, 160);
    // The outer frame is retained beneath the inner one. The bottom region
    // does not move across the reset, so the frame stack is not quiescent at
    // an inner reset, and each inner reset advances the single epoch.
    assert!(snaps.iter().all(|s| s.bottom_used == snaps[0].bottom_used));
    assert_eq!(snaps[1].epoch, snaps[0].epoch + 1);
    assert_eq!(snaps[3].epoch, snaps[1].epoch + 1);
}

#[test]
fn the_inner_loop_alone_has_a_smaller_resident_bottom_region() {
    // The control for the retained outer frame. The same inner body as the
    // entry loop holds one frame, so its bottom region is smaller.
    let (alone, _) = snapshots(
        "loop main(a: Word) -> Word { let s = [a, a, a, a]; yield s[1] }",
        2,
    );
    let (nested, _) = snapshots(OUTER_HOLDS_ARRAY, 2);
    assert_eq!(alone[0].top_used, 32);
    assert!(
        nested[0].bottom_used > alone[0].bottom_used,
        "nested {} alone {}",
        nested[0].bottom_used,
        alone[0].bottom_used
    );
}

#[test]
fn the_composed_outer_wcmu_covers_the_measured_top_peak() {
    // `module_wcmu` charges a call site its callee's figure, so the outer
    // stream chunk's heap term is its own 128 bytes plus the inner 32, which
    // is the measured peak. The inner chunk's own figure is only 32. The
    // declared header figure is per chunk and shallow, so it does not carry
    // the composition, which is why the load-time check uses `module_wcmu`.
    let module = compile(OUTER_HOLDS_ARRAY);
    let wcmu = keleusma::verify::module_wcmu(&module, &[]).expect("bounded");
    let child = module
        .chunks
        .iter()
        .position(|c| c.name == "child")
        .unwrap();
    let main = module.chunks.iter().position(|c| c.name == "main").unwrap();
    assert_eq!(wcmu[child].1, 32);
    assert_eq!(wcmu[main].1, 160);
    assert!(wcmu[main].0 >= wcmu[child].0);
    let (_, top_peak) = snapshots(OUTER_HOLDS_ARRAY, 6);
    assert!(wcmu[main].1 as usize >= top_peak);
}

#[test]
fn the_nested_program_runs_in_its_auto_sized_arena() {
    let module = compile(OUTER_HOLDS_ARRAY);
    let capacity = keleusma::vm::auto_arena_capacity_for(&module, &[]).expect("sized");
    let arena = Arena::with_capacity(capacity);
    let mut vm = Vm::new(module, &arena).expect("verifies");
    let mut out = Vec::new();
    let mut state = vm.call(&[Value::Int(FIRST)]).expect("call");
    for _ in 0..8 {
        if let VmState::Yielded(Value::Int(v)) = state {
            out.push(v);
        }
        state = vm.resume(Value::Int(REPLY)).expect("resume");
    }
    assert_eq!(out, [5, 7, 7, 7]);
}

#[test]
fn private_data_written_by_the_outer_prologue_survives_inner_resets() {
    // The persistent region is not reclaimed by any reset, so a composite the
    // outer loop writes into private data before delegating stays readable.
    let src = "private data st { v: (Word, Word) } \
               loop child(a: Word) -> Word { yield st.v.0 + a } \
               loop main(t: Word) -> Word { st.v = (t, t); child(t) }";
    assert_eq!(drive(src, 4), [10, 12, 12, 12]);
}

#[test]
fn a_second_loop_parameter_is_refused_because_reset_clears_it() {
    // `Op::Reset` clears every local of the resetting frame and a resume
    // rebinds only slot 0, so a second parameter would read `Unit` after the
    // first reset. The definite-initialization verifier refuses the read for
    // the entry loop and for a delegated one alike, so no outer-built value can
    // reach an inner loop except through slot 0 or private data.
    for src in [
        "loop main(a: Word, k: Word) -> Word { yield a + k }",
        "loop child(a: Word, k: Word) -> Word { yield a + k } \
         loop main(t: Word) -> Word { child(t, 100) }",
        "loop child(a: Word, p: (Word, Word)) -> Word { yield a + p.1 } \
         loop main(t: Word) -> Word { child(t, (t, 40)) }",
        "loop leaf(c: Word, d: Word) -> Word { yield c + d } \
         loop mid(b: Word, e: Word) -> Word { leaf(b, e + 1) } \
         loop main(t: Word) -> Word { mid(t, 10) }",
    ] {
        let tokens = keleusma::lexer::tokenize(src).expect("lex");
        let program = keleusma::parser::parse(&tokens).expect("parse");
        let err = keleusma::compiler::compile(&program).expect_err(src);
        assert!(
            err.message.contains("definite-initialization"),
            "{src}: {}",
            err.message
        );
    }
}

#[test]
fn a_swap_at_an_inner_reset_restarts_the_outer_prologue() {
    // The outer loop yields once before delegating. Its own `Reset` is never
    // reached, so every reset the host sees is the inner one. A swap there
    // clears every frame, and the next `call` runs the outer prologue again,
    // so the host sees the prologue's yield a second time. Without nesting the
    // same swap restarts the only loop at its own iteration start.
    let src = "loop child(a: Word) -> Word { yield a + 1000 } \
               loop main(t: Word) -> Word { let s = (t, 2); let r = yield s.0 + s.1; child(r) }";
    let arena = Arena::with_capacity(ARENA);
    let mut vm = Vm::new(compile(src), &arena).expect("verifies");
    let mut log = Vec::new();
    log.push(format!(
        "{:?}",
        vm.call(&[Value::Int(FIRST)]).expect("call")
    ));
    for _ in 0..2 {
        log.push(format!(
            "{:?}",
            vm.resume(Value::Int(REPLY)).expect("resume")
        ));
    }
    assert_eq!(log, ["Yielded(Int(7))", "Yielded(Int(1007))", "Reset"]);
    vm.replace_module(compile(src), Vec::new()).expect("swap");
    let after = vm.call(&[Value::Int(FIRST)]).expect("call");
    assert!(
        matches!(after, VmState::Yielded(Value::Int(7))),
        "the prologue yield recurs, got {after:?}"
    );
}

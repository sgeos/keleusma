#![cfg(all(feature = "compile", feature = "verify"))]
//! A `loop` chunk never returns to its caller, which is what closes P5.
//!
//! # The question this answers
//!
//! The proof session asks whether any virtual-machine-internal location
//! survives `Op::Reset` holding a handle into the ephemeral region.
//!
//! `Op::Reset` clears every local of the CURRENT frame to `Unit` and truncates
//! the operand stack to that frame's locals. **It does not touch frames below
//! it.** And a `loop` function MAY call another `loop` function — the type
//! checker's `category_can_call` answers `true` for `Loop -> Loop` — so a
//! caller's frame really can sit beneath the frame that resets, with its own
//! locals holding handles into the region just reclaimed.
//!
//! **What closes the hole is that the callee never comes back.** A `loop`
//! chunk's only exits are `Op::Reset`, which restarts it in place with its
//! frame retained, and `Op::Trap`, which aborts execution. It emits no
//! `Op::Return`, so the caller below is never resumed and its stale locals are
//! never read.
//!
//! # Why this is pinned rather than stated
//!
//! **It is a DYNAMIC property, not a structural one.** Nothing in the verifier
//! forbids a `Return` in a stream chunk; the code generator simply never emits
//! one, closing a `loop` body with `PopN(1); Reset` instead. A future returning
//! stream — or a hand-written module — would reopen the hole silently, and the
//! proof premise resting on it would become false without any test failing.
//! Now one fails.
//!
//! # What this does NOT establish
//!
//! Nothing about hand-written or corrupt bytecode, which the verifier does not
//! reject for containing `Return` in a stream chunk. This is a property of what
//! the compiler EMITS, and the premise it supports should be read that way.

use keleusma::bytecode::{Module, Op};

fn compile(src: &str) -> Module {
    keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(src).expect("lex")).expect("parse"),
    )
    .expect("compile")
}

/// Chunks that carry `Op::Stream` are the `loop`-category ones.
fn stream_chunks(module: &Module) -> Vec<(&str, &[Op])> {
    module
        .chunks
        .iter()
        .filter(|c| c.ops.iter().any(|o| matches!(o, Op::Stream)))
        .map(|c| (c.name.as_str(), c.ops.as_slice()))
        .collect()
}

/// Sources exercising the shapes that could plausibly emit a `Return` inside a
/// stream: a bare stream, one calling another stream, one calling a plain `fn`,
/// one with a conditional, and one whose body ends in a composite.
const CASES: &[(&str, &str)] = &[
    ("bare", "loop main(t: Word) -> Word { let _ = yield t; 0 }"),
    (
        "stream calls stream",
        "loop inner(r: Word) -> Word { let _ = yield 1; 0 }\n\
         loop main(t: Word) -> Word { let _ = inner(0); 0 }",
    ),
    (
        "stream calls fn",
        "fn helper(x: Word) -> Word { x + 1 }\n\
         loop main(t: Word) -> Word { let _ = yield helper(t); 0 }",
    ),
    (
        "conditional body",
        "loop main(t: Word) -> Word { if t > 0 { let _ = yield 1; } else { let _ = yield 2; } 0 }",
    ),
    (
        "composite tail",
        "struct P { a: Word, b: Word }\n\
         loop main(t: Word) -> P { let _ = yield P { a: t, b: t }; P { a: 0, b: 0 } }",
    ),
];

#[test]
fn no_compiled_stream_chunk_emits_return() {
    let mut checked = 0;
    for (label, src) in CASES {
        let module = compile(src);
        let streams = stream_chunks(&module);
        assert!(
            !streams.is_empty(),
            "case {label:?} produced no stream chunk, so it exercises nothing"
        );
        for (name, ops) in streams {
            checked += 1;
            assert!(
                !ops.iter().any(|o| matches!(o, Op::Return)),
                "case {label:?}: stream chunk {name:?} emits Op::Return. The \
                 proof premise that a caller's frame is never resumed after a \
                 callee's Reset rests on this NOT happening, and nothing in the \
                 verifier forbids it — only the code generator's choice to close \
                 a loop body with PopN(1); Reset."
            );
            assert!(
                ops.iter().any(|o| matches!(o, Op::Reset)),
                "case {label:?}: stream chunk {name:?} has no Reset, so the \
                 absence of Return above means nothing"
            );
        }
    }
    assert!(
        checked >= CASES.len(),
        "fewer stream chunks checked ({checked}) than cases supplied"
    );
}

/// The nested shape really is constructible, so the premise is not vacuous.
///
/// If a `loop` could not call a `loop`, the whole question would be moot and
/// this file would be guarding nothing. It can, it compiles, it verifies, and
/// it runs — the hole is closed by the callee never returning, not by the
/// language forbidding the arrangement.
#[test]
fn a_stream_calling_a_stream_compiles_verifies_and_runs() {
    const SRC: &str = "\
struct P { a: Word, b: Word, c: Word }
loop inner(r: Word) -> Word {
    let _ = yield 1;
    0
}
loop main(t: Word) -> Word {
    let kept = P { a: 5, b: 6, c: 7 };
    let _ = inner(0);
    kept.a
}
";
    use keleusma::bytecode::GenericValue as Value;
    use keleusma::vm::{Vm, VmState};

    let module = compile(SRC);
    assert_eq!(
        stream_chunks(&module).len(),
        2,
        "both functions must be streams, or this is not the nested case"
    );
    keleusma::verify::verify(&module).expect("the nested arrangement verifies");

    let arena = keleusma_arena::Arena::with_capacity(1 << 16);
    let mut vm = Vm::new(module, &arena).expect("loads");

    // The callee's Reset surfaces to the host while the caller's frame is still
    // beneath it. The caller is never resumed, which is the whole point.
    let mut saw_reset = false;
    let mut state = vm.call(&[Value::Int(0)]).expect("call");
    for _ in 0..4 {
        if matches!(state, VmState::Reset) {
            saw_reset = true;
        }
        state = vm.resume(Value::Int(0)).expect("resume");
    }
    assert!(
        saw_reset,
        "the nested stream never reset, so this exercises nothing about what \
         survives a reset"
    );
}

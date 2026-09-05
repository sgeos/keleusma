//! The load-time pass validates every index it meets, and one operand RANGE it
//! does not.
//!
//! # What this pins
//!
//! `Op::PushImmediate` carries a small operand with a valid range of `0..=19`.
//! The virtual machine refuses anything outside it with
//! `VmError::InvalidBytecode`, the error meaning "this artefact should never have
//! been produced". **`verify()` does not check that range**, so such a module is
//! admitted, loads, and traps at run time.
//!
//! Measured 2026-09-05, with the mutation proven to have been applied:
//!
//! | step | result |
//! |---|---|
//! | `verify()` | **admits** |
//! | `Vm::new` | **loads** |
//! | call | **`InvalidBytecode`**, naming the reserved operand |
//!
//! # The severity, stated so it is not borrowed from its neighbours
//!
//! **This is defence in depth, not a hole in the load-time guarantee.** The
//! compiler never emits a reserved immediate, so reaching this needs a corrupted
//! or hand-built artefact — and the wire format admits those, which is why the
//! question is worth asking at all.
//!
//! Both outcomes are safe: the runtime refuses either way. What differs is only
//! WHICH layer refuses. Group B of `docs/decisions/INVALID_BYTECODE_CENSUS.md` is
//! a different matter entirely — there, a module the compiler itself produced
//! verifies, loads, and traps. **Reporting these two at the same severity would
//! discredit the one that matters.**
//!
//! # Why it is worth a test rather than only a note
//!
//! The neighbouring checks make the omission look deliberate when it is more
//! likely incidental: the same pass validates a `Const` index against the pool, a
//! `GetData`/`SetData` slot against the data layout, and a `GetLocal` slot against
//! the chunk's local count — each with a precise message. An operand range is the
//! one shape of check it does not perform. If that is ever changed, this test
//! fails and says so.
//!
//! # Not repaired
//!
//! Adding a check costs time on every load, and this project rejects
//! conservatively on purpose. The observation is recorded; adopting it is a
//! separate call.

#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::Arena;
use keleusma::bytecode::{Module, Op, Value};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::verify::verify;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm};

/// A boolean literal compiles to `PushImmediate`; ordinary integer arithmetic
/// does not. **The first revision of this probe used `k + 1` and mutated
/// nothing**, then reported the resulting untouched module as "admitted" — a
/// verdict about a mutation that never happened. The source is chosen here so
/// the mutation has something to bite on, and the count below proves it did.
const SRC: &str = "fn main(k: Word) -> bool { true }";

/// Replace every `PushImmediate` operand with a reserved value, returning how
/// many were changed.
fn poison(module: &mut Module) -> usize {
    let mut applied = 0;
    for chunk in module.chunks.iter_mut() {
        for op in chunk.ops.iter_mut() {
            if let Op::PushImmediate(v) = op {
                *v = 200;
                applied += 1;
            }
        }
    }
    applied
}

/// One control case: a label, the program, and the mutation that corrupts it.
/// Aliased because the tuple is otherwise complex enough to trip the lint, and
/// the lint is right that the shape is easier to read named.
type ControlCase = (&'static str, &'static str, fn(&mut Module) -> usize);

fn compiled() -> Module {
    compile(&parse(&tokenize(SRC).expect("lex")).expect("parse")).expect("compile")
}

/// **The ratchet.** A reserved immediate survives verification and dies at the call.
///
/// If `verify()` starts rejecting it, the gap is closed at the better layer and
/// this test should record that rather than being deleted.
#[test]
fn a_reserved_immediate_is_admitted_at_load_and_trapped_at_run() {
    let mut module = compiled();

    // NON-VACUOUS. Without this, a probe that found no `PushImmediate` would
    // verify an unmutated module and report it as admitted -- which is exactly
    // what the first revision of this test did.
    let applied = poison(&mut module);
    assert_eq!(
        applied, 1,
        "the source no longer compiles to exactly one PushImmediate, so this test is \
         measuring something other than what it claims"
    );

    verify(&module).expect(
        "verify() now REJECTS a reserved immediate. That closes the gap at the load layer, \
         which is the better place; record the new behaviour here instead of deleting this.",
    );

    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena)
        .expect("Vm::new now rejects it, which also closes the gap; record that here");

    let err = vm
        .call(&[Value::Int(1)])
        .expect_err("the reserved immediate now RUNS, so the runtime stopped refusing it");
    let text = alloc::format!("{err:?}");
    assert!(
        text.contains("reserved"),
        "the runtime refuses it for a different reason now: {text}"
    );
}

extern crate alloc;

/// **The controls: every INDEX the same pass meets is validated at load.**
///
/// These are what make the finding specific. Without them, "verify admits a bad
/// operand" could be read as the pass checking nothing, when in fact it checks
/// each of these and reports precisely.
#[test]
fn the_indices_beside_it_are_all_rejected_at_load() {
    let data_src = "private data d { s: Word }\nfn main(k: Word) -> Word { d.s = k; d.s }";
    let plain = "fn main(k: Word) -> Word { k + 1 }";

    let cases: &[ControlCase] = &[
        ("GetData slot", data_src, |m| {
            let mut n = 0;
            for c in m.chunks.iter_mut() {
                for op in c.ops.iter_mut() {
                    if let Op::GetData(s) = op {
                        *s = 60000;
                        n += 1;
                    }
                }
            }
            n
        }),
        ("SetData slot", data_src, |m| {
            let mut n = 0;
            for c in m.chunks.iter_mut() {
                for op in c.ops.iter_mut() {
                    if let Op::SetData(s) = op {
                        *s = 60000;
                        n += 1;
                    }
                }
            }
            n
        }),
        ("GetLocal slot", plain, |m| {
            let mut n = 0;
            for c in m.chunks.iter_mut() {
                for op in c.ops.iter_mut() {
                    if let Op::GetLocal(s) = op {
                        *s = 60000;
                        n += 1;
                    }
                }
            }
            n
        }),
        ("Const index", plain, |m| {
            let mut n = 0;
            for c in m.chunks.iter_mut() {
                for op in c.ops.iter_mut() {
                    if let Op::Const(i) = op {
                        *i = 60000;
                        n += 1;
                    }
                }
            }
            n
        }),
    ];

    for (label, src, mutate) in cases {
        let mut module =
            compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
        let applied = mutate(&mut module);
        assert!(
            applied > 0,
            "the `{label}` case mutated NOTHING, so its verdict would be vacuous"
        );
        assert!(
            verify(&module).is_err(),
            "the `{label}` case is no longer rejected at load. That is a widening of what \
             a corrupt artefact can carry past verification, and it belongs in the \
             InvalidBytecode census."
        );
    }
}

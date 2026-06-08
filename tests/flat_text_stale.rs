#![cfg(all(feature = "compile", feature = "verify"))]
//! Staleness safety for a flat `Text` field that crosses a boundary and
//! outlives its arena epoch (B28 P3, item 1).
//!
//! A flat composite stores a `Text` field as a two-word `(ptr, len)`
//! handle into arena string bytes. When such a composite is yielded to
//! the host and a later `RESET` advances the arena epoch, the referenced
//! string is reclaimed. Reading the field afterward must resolve to a
//! clean stale outcome, never a dereference of reclaimed memory. This is
//! the same safety a bare `KStr` already enjoys through its epoch check;
//! a nested string must enjoy it too.
//!
//! The test pins the use-after-free: it yields the composite, resumes to
//! the `RESET`, overwrites the reclaimed top region with fresh arena
//! allocations, and then decodes the stale yielded value. The decode must
//! fail (or yield a value that does not equal the original content),
//! rather than read the overwritten bytes as if they were the string.

extern crate alloc;

use alloc::string::String;
use keleusma::compiler::compile;
use keleusma::kstring::KString;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, KeleusmaType};

#[derive(KeleusmaType)]
struct Greeting {
    msg: String,
    n: i64,
}

#[test]
fn yielded_flat_text_is_stale_after_reset_not_use_after_free() {
    // The loop builds a struct with a flat `Text` field, yields it, then
    // resumes (reaching the per-iteration RESET). The host holds the
    // yielded struct across the reset.
    let src = "struct Greeting { msg: Text, n: Word }\n\
               loop main(seed: Word) -> Greeting { \
                   let g = Greeting { msg: \"hithere-unique-token\", n: seed }; \
                   yield g \
               }";
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");

    let yielded = match vm.call(&[keleusma::Value::Int(1)]).expect("call") {
        VmState::Yielded(v) => v,
        other => panic!("expected yield, got {:?}", other),
    };

    // While still valid (before the reset), the decode must succeed and
    // recover the content. This anchors that the field decodes at all.
    let live: Greeting = vm.decode(&yielded).expect("decode while live");
    assert_eq!(live.msg, "hithere-unique-token");
    assert_eq!(live.n, 1);

    // Resume to the RESET boundary. The epoch advances and the top region
    // (holding the string) is reclaimed.
    let pre_epoch = vm.arena().epoch();
    match vm.resume(keleusma::Value::Int(2)).expect("resume") {
        VmState::Reset => {}
        other => panic!("expected reset, got {:?}", other),
    }
    assert_eq!(vm.arena().epoch(), pre_epoch + 1);

    // Overwrite the reclaimed top region with fresh bytes so a use-after-
    // free would read these rather than the original string.
    for _ in 0..16 {
        let _ = KString::alloc(vm.arena(), "XXXXXXXXXXXXXXXXXXXX");
    }

    // Decoding the stale yielded value must resolve to a clean stale
    // error, exactly as a bare `KStr` read after a reset does. An `Ok`
    // here means the flat `Text` field dereferenced reclaimed (and now
    // overwritten) arena memory: a use-after-free.
    match vm.decode::<Greeting>(&yielded) {
        Err(_) => { /* clean stale: the desired outcome */ }
        Ok(g) => panic!(
            "stale flat Text decode returned Ok({:?}); it dereferenced \
             reclaimed arena memory (use-after-free) instead of failing stale",
            g.msg
        ),
    }
}

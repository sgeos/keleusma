#![cfg(all(feature = "compile", feature = "verify"))]
//! Static-text composites cross the yield boundary (B28 P3 item 4).
//!
//! A struct or enum carrying a flat `Text` field built from a static string
//! literal points at the immortal bytecode image (rodata), so it may be
//! yielded and decoded by the host. The earlier compile-time rejection of
//! flat-text composites at `yield` is lifted in favour of the read-before-
//! resume contract: the host decodes the yielded value before the next
//! `resume()` (the RESET point). A static field is immortal and reads back
//! correctly even after a RESET; the second test re-yields a static-text
//! composite from a private data slot across a RESET to prove it.
//!
//! Guarded off the narrow-word builds, where `Text` is kept boxed (a host
//! pointer does not fit a narrow word) and the flat-text path does not exist.

#![cfg(not(any(
    feature = "narrow-word-8",
    feature = "narrow-word-16",
    feature = "narrow-word-32"
)))]

extern crate alloc;

use alloc::string::String;
use keleusma::bytecode::Value;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};
use keleusma::{Arena, KeleusmaType};

#[derive(KeleusmaType)]
struct Greeting {
    msg: String,
    n: i64,
}

#[test]
fn static_text_struct_crosses_yield() {
    // A struct with a static-text field is yielded directly. Before item 4 this
    // was a compile error ("a Text field inside a struct ... cannot cross the
    // yield boundary"); now it compiles and the host decodes the rodata text.
    let src = "struct Greeting { msg: Text, n: Word }\n\
               loop main(seed: Word) -> Greeting { \
                   let _ = yield Greeting { msg: \"hi\", n: seed }; \
                   Greeting { msg: \"hi\", n: 0 } \
               }";
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(m, &arena).expect("verify");
    match vm.call(&[Value::Int(5)]).expect("call") {
        VmState::Yielded(v) => {
            // Decode before the next resume (read-before-resume).
            let g: Greeting = vm.decode(&v).expect("decode yielded greeting");
            assert_eq!(g.msg, "hi", "yielded static text");
            assert_eq!(g.n, 5, "yielded word field");
        }
        other => panic!("expected Yielded, got {:?}", other),
    }
}

#[test]
fn static_text_composite_survives_reset_across_yield() {
    // A static-text struct is written to a private data slot on the first
    // iteration, then yielded each iteration. Iteration 2 yields the slot
    // WITHOUT rewriting it, so decoding the same text after the RESET requires
    // the persistent body to survive (item 3a) and the text to be immortal
    // rodata (item 4). The host decodes each yield before resuming.
    let src = "struct Greeting { msg: Text, n: Word }\n\
               private data d { g: Greeting }\n\
               loop main(seed: Word) -> Greeting { \
                   if seed == 0 { d.g = Greeting { msg: \"persist\", n: 7 }; }; \
                   let _ = yield d.g; \
                   Greeting { msg: \"persist\", n: 0 } \
               }";
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize_persistent");
    let mut vm = Vm::new(m, &arena).expect("verify");

    // Iteration 1: write the slot, yield it, decode "persist".
    match vm.call(&[Value::Int(0)]).expect("call") {
        VmState::Yielded(v) => {
            let g: Greeting = vm.decode(&v).expect("decode iter 1");
            assert_eq!(g.msg, "persist", "iter 1 text");
            assert_eq!(g.n, 7, "iter 1 word");
        }
        other => panic!("iter 1 expected Yielded, got {:?}", other),
    }
    // Resume past the yield: the loop body end RESETs the ephemeral arena.
    match vm.resume(Value::Int(1)).expect("resume") {
        VmState::Reset => {}
        other => panic!("expected Reset, got {:?}", other),
    }
    // Iteration 2 (seed 1): no rewrite, yields the slot. The text must have
    // survived the RESET as immortal rodata, so the host decodes "persist"
    // again.
    match vm.resume(Value::Int(1)).expect("resume") {
        VmState::Yielded(v) => {
            let g: Greeting = vm.decode(&v).expect("decode iter 2");
            assert_eq!(g.msg, "persist", "iter 2 text survives RESET as rodata");
            assert_eq!(g.n, 7, "iter 2 word survives RESET");
        }
        other => panic!("iter 2 expected Yielded, got {:?}", other),
    }
}

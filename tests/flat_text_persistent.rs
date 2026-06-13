#![cfg(all(feature = "compile", feature = "verify"))]
// A flat `Text` field built from a static string literal points at the
// immortal bytecode image (rodata), not an ephemeral arena copy (B28 P3 item
// 4). A private `.data` composite slot whose body carries such a field
// therefore reads back correctly after a RESET: the persistent body survives
// the RESET in place (item 3a), and the rodata the text points at is immortal,
// so the rebuilt handle is always live.
//
// Before item 4 the static text was copied into the ephemeral arena top, so
// the persistent body's text pointer dangled after a RESET (the read resolved
// stale, and the length observed below would be 0 on iteration 2). This test
// pins the rodata residence that fixes it.
//
// Survival is observed by passing the slot's text to a host native that
// returns its byte length, then yielding that `Word`; the composite itself is
// never yielded (that path is gated separately) and string `==` is not used
// (it compares by variant, not content, across `StaticStr`/`KStr`). A static
// (rodata) text survives and yields length 7; a dynamic (ephemeral) text does
// not survive and a post-RESET read of it faults cleanly stale, the secure
// failure mode the second test pins.
//
// Guarded off the narrow-word builds, where `Text` is kept boxed (a host
// pointer does not fit a narrow word) and the flat-text path does not exist.
#![cfg(not(any(
    feature = "narrow-word-8",
    feature = "narrow-word-16",
    feature = "narrow-word-32"
)))]

extern crate alloc;

use alloc::string::String;
use keleusma::Arena;
use keleusma::VmError;
use keleusma::bytecode::Value;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

#[test]
fn private_static_text_struct_survives_reset() {
    // Iteration 1 (seed 0) writes a struct with a static-text field into the
    // private composite slot, then yields the byte length of the slot's text.
    // Resuming past the yield reaches the loop body end, which RESETs the
    // ephemeral arena. The restarted stream (seed 1) reads the slot's text
    // WITHOUT rewriting it, so observing the same length again requires both
    // the persistent body to survive the RESET (item 3a) and the text bytes to
    // be immortal rodata (item 4).
    let src = "use host::slen(Text) -> Word\n\
               struct S { msg: Text, n: Word }\n\
               private data d { s: S }\n\
               loop main(seed: Word) -> Word { \
                   if seed == 0 { d.s = S { msg: \"persist\", n: 7 }; }; \
                   let _ = yield host::slen(d.s.msg); \
                   0 \
               }";
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize_persistent");
    let mut vm = Vm::new(m, &arena).expect("verify");
    vm.register_fn("host::slen", |s: String| -> i64 { s.len() as i64 });

    // First call: writes the slot, yields the length of "persist" (7).
    match vm.call(&[Value::Int(0)]).expect("call") {
        VmState::Yielded(Value::Int(n)) => assert_eq!(n, 7, "iteration 1 text length"),
        other => panic!("iter 1 expected Yielded(7), got {:?}", other),
    }
    // Resume past the yield: the body end RESETs the ephemeral arena.
    match vm.resume(Value::Int(1)).expect("resume") {
        VmState::Reset => {}
        other => panic!("expected Reset at loop body end, got {:?}", other),
    }
    // Restart with seed 1: no rewrite, so the slot's text must have survived
    // the RESET as immortal rodata. A correct read still yields length 7; a
    // reclaimed (ephemeral) text would resolve stale and yield 0.
    match vm.resume(Value::Int(1)).expect("resume") {
        VmState::Yielded(Value::Int(n)) => {
            assert_eq!(n, 7, "iteration 2 text survives RESET as rodata")
        }
        other => panic!("iter 2 expected Yielded(7), got {:?}", other),
    }
}

#[test]
fn private_dynamic_text_struct_faults_stale_after_reset() {
    // The complement of the rodata case: a DYNAMIC text field (a host-returned
    // string that matches no module constant) is copied into the ephemeral
    // arena, so it does NOT survive a RESET. The contract this pins is the
    // secure failure mode: a post-RESET read that resolves the string raises a
    // clean stale fault rather than dereferencing reclaimed memory or returning
    // the wrong (or empty) content. The stale ephemeral handle does not match
    // the advanced arena epoch, so resolving it through the native-argument
    // decode errors with a "dynamic string is stale" `TypeError`.
    let src = "use host::dyntext() -> Text\n\
               use host::slen(Text) -> Word\n\
               struct S { msg: Text, n: Word }\n\
               private data d { s: S }\n\
               loop main(seed: Word) -> Word { \
                   if seed == 0 { d.s = S { msg: host::dyntext(), n: 7 }; }; \
                   let _ = yield host::slen(d.s.msg); \
                   0 \
               }";
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize_persistent");
    let mut vm = Vm::new(m, &arena).expect("verify");
    // A 14-byte string that is not a literal anywhere in the script, so it is
    // never resolved to a module constant and stays an ephemeral arena copy.
    vm.register_fn("host::dyntext", || -> String {
        String::from("ephemeral_only")
    });
    vm.register_fn("host::slen", |s: String| -> i64 { s.len() as i64 });

    // First call: writes the dynamic text, yields its length (14).
    match vm.call(&[Value::Int(0)]).expect("call") {
        VmState::Yielded(Value::Int(n)) => assert_eq!(n, 14, "iteration 1 dynamic text length"),
        other => panic!("iter 1 expected Yielded(14), got {:?}", other),
    }
    // Resume past the yield: the body end RESETs the ephemeral arena.
    match vm.resume(Value::Int(1)).expect("resume") {
        VmState::Reset => {}
        other => panic!("expected Reset at loop body end, got {:?}", other),
    }
    // Restart with seed 1: no rewrite. The dynamic text did not survive, so
    // resolving it through the native argument raises a clean stale fault --
    // secure failure, not a dangling read of reclaimed memory.
    match vm.resume(Value::Int(1)) {
        Err(VmError::TypeError(msg)) => {
            assert!(
                msg.contains("stale"),
                "expected a stale fault, got TypeError({msg:?})"
            );
        }
        other => panic!("iter 2 expected a stale TypeError, got {:?}", other),
    }
}

#[test]
fn private_static_text_module_hot_swaps_cleanly() {
    // A module that writes a static-text composite into a private slot is hot
    // swapped, then runs again. This pins that the item-4 rodata residence and
    // the persistent composite body pool survive a swap soundly: the swap drops
    // and re-initialises every private slot (severing any link to the old
    // pool body) and zeros the composite body pool tail, so the old module's
    // bytecode image, freed by the swap, is never dereferenced. The swapped-in
    // module writes its slot fresh and reads back the correct rodata text.
    let src = "use host::slen(Text) -> Word\n\
               struct S { msg: Text, n: Word }\n\
               private data d { s: S }\n\
               loop main(seed: Word) -> Word { \
                   if seed == 0 { d.s = S { msg: \"alpha\", n: 1 }; }; \
                   let _ = yield host::slen(d.s.msg); \
                   0 \
               }";
    let compile_module =
        || compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let m = compile_module();
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize_persistent");
    let mut vm = Vm::new(m, &arena).expect("verify");
    vm.register_fn("host::slen", |s: String| -> i64 { s.len() as i64 });

    // Run the first module: write the static text, yield its length (5).
    match vm.call(&[Value::Int(0)]).expect("call") {
        VmState::Yielded(Value::Int(n)) => assert_eq!(n, 5, "pre-swap text length"),
        other => panic!("pre-swap expected Yielded(5), got {:?}", other),
    }
    match vm.resume(Value::Int(1)).expect("resume") {
        VmState::Reset => {}
        other => panic!("expected Reset, got {:?}", other),
    }

    // Hot swap to a fresh copy of the same module. The one private composite
    // slot is re-initialised to Unit; the swapped-in module writes it fresh
    // before reading. This frees the old bytecode image, so a read of a
    // surviving old pool body would dereference freed memory; the swap-time
    // zeroing and the slot re-initialisation prevent that.
    vm.replace_module(compile_module(), alloc::vec![Value::Unit])
        .expect("hot swap");

    // The swapped-in module runs cleanly: it writes its slot fresh on seed 0
    // and reads back the correct rodata text length (5).
    match vm.call(&[Value::Int(0)]).expect("post-swap call") {
        VmState::Yielded(Value::Int(n)) => assert_eq!(n, 5, "post-swap text length"),
        other => panic!("post-swap expected Yielded(5), got {:?}", other),
    }
}

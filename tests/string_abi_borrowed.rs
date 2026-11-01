//! The agreed string marshalling ABI: a native observes a borrowed byte view.
//!
//! The operator ruled Option B for the string ABI -- make the two embeddings
//! agree -- recorded with its provenance in
//! `docs/decisions/STRING_ABI_OPTION_B.md`. The agreed representation is a
//! `&str` borrowed for the duration of the call, because that is what the
//! ahead-of-time native backend already hands its native: the address of a
//! `{ len, bytes }` block, which is a length and a pointer and nothing else.
//! An owned `String` cannot be that, on either side, without a copy.
//!
//! ## What these tests can and cannot establish
//!
//! The native backend lives on the `v0.3.0` line and cannot be run from here,
//! so no test in this repository observes both embeddings at once. What is
//! pinned instead is the set of OBSERVABLE PROPERTIES the agreed contract
//! consists of, each of which is independently pinned on the other side by
//! `native_codegen/tests/native_calls.rs`. Agreement is then the conjunction
//! of two one-sided pins rather than one two-sided measurement, and a
//! divergence shows up as one side's pin failing. That is weaker than a
//! differential oracle and is stated as such rather than implied away.
//!
//! The four properties, and why each is load-bearing:
//!
//! 1. **Length-delimited, not NUL-delimited.** A Keleusma string is a byte
//!    string and may carry an interior NUL. A `char*` contract would truncate
//!    at it; both sides must report the full length.
//! 2. **Byte length, not character count.** The block's length field counts
//!    bytes, so a multi-byte encoding must not be counted as characters.
//! 3. **Empty is length zero, not absent.** An empty literal is a live block
//!    of length zero, not a null pointer.
//! 4. **The borrowed view and the owned copy carry identical bytes.** The
//!    owned path is retained, so the two must not disagree about content.
//!
//! ## The owned path is retained and is NOT portable
//!
//! `|s: String|` still registers and behaves exactly as before. It is a
//! virtual-machine-only convenience: no ahead-of-time lowering produces an
//! owned `String` without copying, so a native declared against `String` does
//! not carry to the native embedding. That is recorded here rather than
//! deprecated, because deprecation is an embedder-visible call the operator
//! has not made.

#![cfg(all(feature = "compile", feature = "verify"))]

extern crate alloc;

use alloc::string::String;

use keleusma::Arena;
use keleusma::VmError;
use keleusma::bytecode::Value;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};

fn build_vm<'arena>(src: &str, arena: &'arena Arena) -> Vm<'static, 'arena> {
    let tokens = tokenize(src).expect("lex error");
    let program = parse(&tokens).expect("parse error");
    let module = compile(&program).expect("compile error");
    Vm::new(module, arena).expect("verify")
}

fn finished_int(state: VmState) -> i64 {
    match state {
        VmState::Finished(Value::Int(n)) => n,
        other => panic!("expected Finished(Int), got {:?}", other),
    }
}

// -- Property 1: length-delimited, not NUL-delimited --

#[test]
fn an_interior_nul_is_not_truncated() {
    // The pin against a `char*` contract. A NUL-terminated ABI would report 2
    // here. This fails loudly against that reading rather than agreeing with
    // it on a wrong answer, which is the same role this test plays on the
    // native side.
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = build_vm(
        "use host::blen(Text) -> Word\n\
         fn main() -> Word { host::blen(\"ab\\0cd\") }",
        &arena,
    );
    vm.register_fn("host::blen", |s: &str| -> i64 { s.len() as i64 });
    assert_eq!(finished_int(vm.call(&[]).expect("call")), 5);
}

#[test]
fn an_interior_nul_is_carried_through_not_merely_counted() {
    // Counting bytes past a NUL is not the same as being able to READ them.
    // Summing the bytes distinguishes a correct view from one whose length is
    // right but whose data pointer stops early.
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = build_vm(
        "use host::bsum(Text) -> Word\n\
         fn main() -> Word { host::bsum(\"ab\\0cd\") }",
        &arena,
    );
    vm.register_fn("host::bsum", |s: &str| -> i64 {
        s.as_bytes().iter().map(|b| *b as i64).sum()
    });
    // b'a' + b'b' + 0 + b'c' + b'd' = 97 + 98 + 0 + 99 + 100
    assert_eq!(finished_int(vm.call(&[]).expect("call")), 394);
}

// -- Property 2: byte length, not character count --

#[test]
fn multibyte_utf8_is_counted_in_bytes_not_characters() {
    // Three characters, six bytes: 'a' is 1, 'e' with an acute accent is 2,
    // and the CJK ideograph is 3. A character-counting contract reports 3.
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = build_vm(
        "use host::blen(Text) -> Word\n\
         fn main() -> Word { host::blen(\"aé漢\") }",
        &arena,
    );
    vm.register_fn("host::blen", |s: &str| -> i64 { s.len() as i64 });
    assert_eq!(finished_int(vm.call(&[]).expect("call")), 6);
    assert_eq!("aé漢".chars().count(), 3, "the literal is three characters");
}

// -- Property 3: empty is length zero, not absent --

#[test]
fn an_empty_literal_is_a_live_view_of_length_zero() {
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = build_vm(
        "use host::blen(Text) -> Word\n\
         fn main() -> Word { host::blen(\"\") }",
        &arena,
    );
    // Returning a discriminant rather than the length alone: the native must
    // be REACHED with a resolvable view, not fail and be reported as zero.
    vm.register_fn("host::blen", |s: &str| -> i64 {
        if s.is_empty() { 7 } else { s.len() as i64 }
    });
    assert_eq!(finished_int(vm.call(&[]).expect("call")), 7);
}

// -- Property 4: the borrowed view and the owned copy carry identical bytes --

#[test]
fn the_borrowed_view_and_the_owned_copy_observe_the_same_bytes() {
    // The two embeddings of the SAME value inside this line. If the retained
    // owned path and the new borrowed path ever disagree about content, this
    // is the test that says so, and it compares bytes rather than lengths so a
    // same-length divergence cannot slip through.
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = build_vm(
        "use host::bsum_ref(Text) -> Word\n\
         use host::bsum_owned(Text) -> Word\n\
         fn main() -> Word { host::bsum_ref(\"ab\\0cé\") - host::bsum_owned(\"ab\\0cé\") }",
        &arena,
    );
    vm.register_fn("host::bsum_ref", |s: &str| -> i64 {
        s.as_bytes().iter().map(|b| *b as i64).sum()
    });
    vm.register_fn("host::bsum_owned", |s: String| -> i64 {
        s.as_bytes().iter().map(|b| *b as i64).sum()
    });
    assert_eq!(
        finished_int(vm.call(&[]).expect("call")),
        0,
        "the borrowed view and the owned copy disagreed about the bytes"
    );
}

// -- The dynamic (arena-resident) representation resolves the same way --

#[test]
fn a_dynamic_string_resolves_through_the_arena_without_a_copy() {
    // A `StaticStr` borrows the module image; a `KStr` borrows the arena. Both
    // must reach the native as the same kind of view, or the contract would
    // hold only for literals -- which is exactly the "supported where I
    // happened to look" boundary this tree keeps recording.
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = build_vm(
        "use host::dyntext() -> Text\n\
         use host::blen(Text) -> Word\n\
         fn main() -> Word { host::blen(host::dyntext()) }",
        &arena,
    );
    // Not a literal anywhere in the script, so it is never resolved to a
    // module constant and stays an ephemeral arena string.
    vm.register_fn("host::dyntext", || -> String {
        String::from("ephemeral_only")
    });
    vm.register_fn("host::blen", |s: &str| -> i64 { s.len() as i64 });
    assert_eq!(finished_int(vm.call(&[]).expect("call")), 14);
}

// -- Argument position and arity --

#[test]
fn a_borrowed_argument_is_admitted_in_every_position() {
    // The impl family is enumerated by shape, so a shape that was not
    // generated is a compile error at the call site rather than a runtime
    // surprise. Exercising a borrowed slot first, last, and interleaved is
    // what makes that enumeration checked rather than asserted.
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = build_vm(
        "use host::first(Text, Word) -> Word\n\
         use host::last(Word, Text) -> Word\n\
         use host::both(Text, Text) -> Word\n\
         use host::middle(Word, Text, Word) -> Word\n\
         fn main() -> Word { \
             host::first(\"abc\", 10) + host::last(20, \"abcd\") \
             + host::both(\"ab\", \"cde\") + host::middle(1, \"abcdef\", 2) \
         }",
        &arena,
    );
    vm.register_fn("host::first", |s: &str, n: i64| -> i64 {
        s.len() as i64 + n
    });
    vm.register_fn("host::last", |n: i64, s: &str| -> i64 {
        s.len() as i64 + n
    });
    vm.register_fn("host::both", |a: &str, b: &str| -> i64 {
        (a.len() + b.len()) as i64
    });
    vm.register_fn("host::middle", |a: i64, s: &str, b: i64| -> i64 {
        s.len() as i64 + a + b
    });
    // (3 + 10) + (4 + 20) + (2 + 3) + (6 + 1 + 2)
    assert_eq!(finished_int(vm.call(&[]).expect("call")), 13 + 24 + 5 + 9);
}

// -- The retained owned path --

#[test]
fn an_owned_string_argument_still_registers_and_still_works() {
    // The regression guard for the retained path. Adding the borrowed impl
    // family put a second candidate in front of every `register_fn` call site
    // in the tree; this pins that the owned shape still selects the owned
    // impl rather than becoming ambiguous.
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = build_vm(
        "use host::blen(Text) -> Word\n\
         fn main() -> Word { host::blen(\"persist\") }",
        &arena,
    );
    vm.register_fn("host::blen", |s: String| -> i64 { s.len() as i64 });
    assert_eq!(finished_int(vm.call(&[]).expect("call")), 7);
}

// -- Failure modes --

#[test]
fn a_fallible_borrowed_native_surfaces_its_error() {
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = build_vm(
        "use host::checked(Text) -> Word\n\
         fn main() -> Word { host::checked(\"reject\") }",
        &arena,
    );
    vm.register_fn_fallible("host::checked", |s: &str| -> Result<i64, VmError> {
        if s == "reject" {
            Err(VmError::NativeError(String::from("refused by the host")))
        } else {
            Ok(s.len() as i64)
        }
    });
    match vm.call(&[]) {
        Err(VmError::NativeError(msg)) => assert!(
            msg.contains("refused by the host"),
            "expected the host's message, got {msg:?}"
        ),
        other => panic!("expected a NativeError, got {:?}", other),
    }
}

#[test]
fn a_non_text_argument_is_a_clean_type_error() {
    // Default-deny at the boundary: a borrowed slot handed something that is
    // not a string faults rather than reinterpreting the value's bytes.
    // Reached by declaring the native's parameter as `Word` in the script
    // while the host registers a borrowed string slot, which is the shape a
    // mis-declared `use` line produces.
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = build_vm(
        "use host::blen(Word) -> Word\n\
         fn main() -> Word { host::blen(42) }",
        &arena,
    );
    vm.register_fn("host::blen", |s: &str| -> i64 { s.len() as i64 });
    match vm.call(&[]) {
        Err(VmError::TypeError(msg)) => assert!(
            msg.contains("expected Text"),
            "expected a Text type error, got {msg:?}"
        ),
        other => panic!("expected a TypeError, got {:?}", other),
    }
}

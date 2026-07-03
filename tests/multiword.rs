#![cfg(all(feature = "compile", feature = "verify"))]
//! `Multiword<N>` fixed-width multi-word integer, phase 1: the type,
//! construction from a tuple literal, and digit indexing (B19). The
//! value is represented as a flat little-endian array of N words, so a
//! `Multiword<N>` built from `(d0, d1, ..., d_{N-1}) as Multiword<N>`
//! indexes to its digits with `m[i]`, digit 0 being least significant.

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

fn run_to_int(src: &str) -> i64 {
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    match vm.call(&[]).expect("call") {
        VmState::Finished(Value::Int(n)) => n,
        other => panic!("expected a finished integer, got {:?}", other),
    }
}

#[test]
fn multiword_construct_and_index_digit_zero() {
    // Digit 0 is the least significant word.
    assert_eq!(
        run_to_int("fn main() -> Word { let m = (42, 7, 0, 0) as Multiword<4>; m[0] }"),
        42
    );
}

#[test]
fn multiword_index_higher_digits() {
    assert_eq!(
        run_to_int("fn main() -> Word { let m = (42, 7, 3, 9) as Multiword<4>; m[1] }"),
        7
    );
    assert_eq!(
        run_to_int("fn main() -> Word { let m = (42, 7, 3, 9) as Multiword<4>; m[3] }"),
        9
    );
}

#[test]
fn multiword_is_a_first_class_parameter_type() {
    // `Multiword<N>` appears in a function signature and is passed by
    // value like any other type.
    let src = "fn first(m: Multiword<4>) -> Word { m[0] }\n\
               fn main() -> Word { let m = (99, 0, 0, 0) as Multiword<4>; first(m) }";
    assert_eq!(run_to_int(src), 99);
}

#[test]
fn multiword_two_word_digits_sum() {
    assert_eq!(
        run_to_int("fn main() -> Word { let m = (100, 200) as Multiword<2>; m[0] + m[1] }"),
        300
    );
}

#[test]
fn multiword_construct_from_non_literal_tuple() {
    // A tuple bound to a variable also casts to Multiword<N>; the
    // compiler stashes it and extracts each digit.
    let src = "fn main() -> Word { let t = (11, 22, 33, 44); let m = t as Multiword<4>; m[2] }";
    assert_eq!(run_to_int(src), 33);
}

#[test]
fn multiword_turbofish_constructor() {
    // The Multiword::<N>(...) constructor form is equivalent to the
    // tuple cast.
    assert_eq!(
        run_to_int("fn main() -> Word { let m = Multiword::<4>(5, 6, 7, 8); m[1] }"),
        6
    );
}

#[test]
fn multiword_single_word_constructor() {
    // Multiword<1> through the constructor form, which the tuple cast
    // cannot express since a one-element tuple is not surface syntax.
    assert_eq!(
        run_to_int("fn main() -> Word { let m = Multiword::<1>(77); m[0] }"),
        77
    );
}

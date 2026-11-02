//! Trait methods on generic structs and enums.
//!
//! A trait implemented for a generic type is specialized once per
//! concrete instantiation of that type during monomorphization, so the
//! specialized method chunk (`Trait::SpecName::method`) matches the
//! specialized receiver at the call site. Previously a method call on any
//! generic receiver failed: the type-generic case in the first-pass type
//! checker (the impl's receiver type was never instantiated), the
//! const-generic case after monomorphization (the impl was not re-keyed
//! to the specialized struct name).
#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

fn run(src: &str) -> i64 {
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    match vm.call(&[]).expect("call") {
        VmState::Finished(Value::Int(n)) => n,
        other => panic!("expected finished integer, got {:?}", other),
    }
}

#[test]
fn concrete_type_method_still_works() {
    // Regression sentinel: a trait method on a concrete type is unchanged.
    let src = "trait Cap { fn cap(self) -> Word; }\n\
               struct S { c: Word }\n\
               impl Cap for S { fn cap(s: S) -> Word { s.c } }\n\
               fn main() -> Word { let x = S { c: 7 }; x.cap() }";
    assert_eq!(run(src), 7);
}

#[test]
fn type_generic_struct_method() {
    // A trait method on a type-generic struct; the receiver `Cell<Word>`
    // resolves the impl's `T` in the first pass and dispatches to the
    // specialized `Cell__Word` method after monomorphization.
    let src = "trait Get { fn get(self) -> Word; }\n\
               struct Cell<T> { c: T }\n\
               impl<T> Get for Cell<T> { fn get(s: Cell<T>) -> Word { s.c } }\n\
               fn main() -> Word { let x = Cell { c: 9 }; x.get() }";
    assert_eq!(run(src), 9);
}

#[test]
fn const_generic_struct_method() {
    // A trait method on a const-generic struct.
    let src = "trait Cap { fn cap(self) -> Word; }\n\
               struct Buf<const n: Word> { c: Word }\n\
               impl<const n: Word> Cap for Buf<n> { fn cap(s: Buf<n>) -> Word { s.c } }\n\
               fn main() -> Word { let x = Buf::<4> { c: 11 }; x.cap() }";
    assert_eq!(run(src), 11);
}

#[test]
fn distinct_const_specializations_each_get_the_method() {
    // Two const instantiations, each dispatching to its own specialized
    // impl `Buf__c2` and `Buf__c5`.
    let src = "trait Cap { fn cap(self) -> Word; }\n\
               struct Buf<const n: Word> { c: Word }\n\
               impl<const n: Word> Cap for Buf<n> { fn cap(s: Buf<n>) -> Word { s.c } }\n\
               fn main() -> Word { let a = Buf::<2> { c: 3 }; let b = Buf::<5> { c: 8 }; a.cap() + b.cap() }";
    assert_eq!(run(src), 11);
}

#[test]
fn const_generic_method_uses_const_value() {
    // The const parameter is usable as a value in the method body, and is
    // substituted per specialization.
    let src = "trait Size { fn size(self) -> Word; }\n\
               struct Buf<const n: Word> { c: Word }\n\
               impl<const n: Word> Size for Buf<n> { fn size(s: Buf<n>) -> Word { n } }\n\
               fn main() -> Word { let a = Buf::<3> { c: 0 }; let b = Buf::<7> { c: 0 }; a.size() + b.size() }";
    assert_eq!(run(src), 10);
}

#[test]
fn const_generic_enum_method() {
    // A trait method on a const-generic enum, dispatched on a specialized
    // enum receiver `E__c3`.
    let src = "trait Tag { fn tag(self) -> Word; }\n\
               enum E<const n: Word> { A(Word), B }\n\
               impl<const n: Word> Tag for E<n> { fn tag(e: E<n>) -> Word { match e { E::A(x) => x, E::B => 0 } } }\n\
               fn main() -> Word { let x = E::<3>::A(42); x.tag() }";
    assert_eq!(run(src), 42);
}

/// Trait impls on the dimensioned families `Text<N>`, `Fixed<n>` and
/// `Multiword<n>`.
///
/// Until 2026-11-01 the type checker registered such an impl under a key that
/// carried the dimension, such as `Text<4>`, while method lookup, trait-bound
/// checking and the compiler all keyed it by the family name, such as `Text`.
/// The impl was admitted and could never be called, as a method or through a
/// bound. An impl now belongs to the family, as the method lookup already
/// assumed, and a second impl of one trait within one family is refused
/// because both would mangle to the same name.
mod dimensioned_families {
    use super::*;

    fn compile_err(src: &str) -> String {
        let program = parse(&tokenize(src).expect("lex")).expect("parse");
        compile(&program)
            .expect_err("expected a compile error")
            .message
    }

    #[test]
    fn a_method_on_a_text_capacity_is_callable() {
        let src = "trait Shout { fn shout(self) -> Word; }\n\
                   impl Shout for Text<4> { fn shout(t: Text<4>) -> Word { 7 } }\n\
                   fn main() -> Word { let t = \"abcd\" as Text<4>; t.shout() }";
        assert_eq!(run(src), 7);
    }

    #[test]
    fn a_method_on_a_fixed_type_is_callable() {
        let src = "trait Tag { fn tag(self) -> Word; }\n\
                   impl Tag for Fixed { fn tag(x: Fixed) -> Word { 5 } }\n\
                   fn main() -> Word { let a = 3 as Fixed; a.tag() }";
        assert_eq!(run(src), 5);
    }

    #[test]
    fn a_method_on_a_multiword_is_callable() {
        let src = "trait Low { fn low(self) -> Word; }\n\
                   impl Low for Multiword<2> { fn low(m: Multiword<2>) -> Word { m[0] } }\n\
                   fn main() -> Word { let m = (9, 0) as Multiword<2>; m.low() }";
        assert_eq!(run(src), 9);
    }

    #[test]
    fn a_trait_bound_is_satisfied_by_a_text_capacity_impl() {
        let src = "trait Shout { fn shout(self) -> Word; }\n\
                   impl Shout for Text<4> { fn shout(t: Text<4>) -> Word { 8 } }\n\
                   fn call<T: Shout>(x: T) -> Word { x.shout() }\n\
                   fn main() -> Word { call(\"abcd\" as Text<4>) }";
        assert_eq!(run(src), 8);
    }

    #[test]
    fn two_impls_of_one_trait_within_one_family_are_refused() {
        let src = "trait Shout { fn shout(self) -> Word; }\n\
                   impl Shout for Text<4> { fn shout(t: Text<4>) -> Word { 1 } }\n\
                   impl Shout for Text<8> { fn shout(t: Text<8>) -> Word { 2 } }\n\
                   fn main() -> Word { 0 }";
        let msg = compile_err(src);
        assert!(msg.contains("more than once"), "{msg}");
    }

    #[test]
    fn an_impl_for_one_capacity_does_not_accept_another_capacity() {
        let src = "trait Shout { fn shout(self) -> Word; }\n\
                   impl Shout for Text<4> { fn shout(t: Text<4>) -> Word { 1 } }\n\
                   fn main() -> Word { let t = \"abcdefgh\" as Text<8>; t.shout() }";
        // Refused because the receiver's capacity differs from the impl's,
        // not because the method cannot be found, which was the old reason.
        let msg = compile_err(src);
        assert!(!msg.contains("has no method"), "{msg}");
        assert!(msg.contains("Text<4>") && msg.contains("Text<8>"), "{msg}");
    }
}

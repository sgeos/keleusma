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

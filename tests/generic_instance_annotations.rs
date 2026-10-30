//! A `let` annotated with a generic instance type compiles.
//!
//! # The defect
//!
//! Monomorphization renames an instance of a generic struct or enum, `Box<Word>`
//! to `Box__Word`, and rewrote the name in function parameters and returns, but
//! not in `let` annotations. The re-check after monomorphization then compared
//! the two names and refused the binding with "let binding declared as
//! Box<Word> but value has type Box__Word". Measured on 2026-10-30 for a generic
//! enum, a one-parameter struct and a two-parameter struct. A pykeleusma
//! finding.
//!
//! # What these tests do NOT establish
//!
//! They do not cover a host-built value of a generic instance, whose layout the
//! host must name. That part of the finding is open.
#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::Arena;
use keleusma::bytecode::Value;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

fn run(source: &str) -> Value {
    let tokens = keleusma::lexer::tokenize(source).expect("lex");
    let program = keleusma::parser::parse(&tokens).expect("parse");
    let module = keleusma::compiler::compile(&program)
        .unwrap_or_else(|e| panic!("{source}\ncompile: {}", e.message));
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(module, &arena).expect("verifies");
    match vm.call(&[]).expect("runs") {
        VmState::Finished(v) => v,
        other => panic!("{source}: {other:?}"),
    }
}

#[test]
fn annotated_generic_instances_compile_and_run() {
    let cases: [(&str, i64); 5] = [
        (
            "enum Maybe<T> { Just(T), Nothing }\nfn main() -> Word { let m: Maybe<Word> = Maybe::Just(4); match m { Maybe::Just(x) => x, Maybe::Nothing => 0 } }",
            4,
        ),
        (
            "struct Box<T> { v: T }\nfn main() -> Word { let b: Box<Word> = Box { v: 5 }; b.v + 1 }",
            6,
        ),
        (
            "struct Pair<A, B> { a: A, b: B }\nfn main() -> Word { let p: Pair<Word, Byte> = Pair { a: 5, b: 2 as Byte }; p.a }",
            5,
        ),
        // An annotation inside a nested block is rewritten too.
        (
            "struct Box<T> { v: T }\nfn main() -> Word { if true { let b: Box<Word> = Box { v: 7 }; b.v } else { 0 } }",
            7,
        ),
        // The control that already worked, a generic instance in a return type.
        (
            "enum Maybe<T> { Just(T), Nothing }\nfn mk(x: Word) -> Maybe<Word> { Maybe::Just(x) }\nfn main() -> Word { match mk(3) { Maybe::Just(x) => x, Maybe::Nothing => 0 } }",
            3,
        ),
    ];
    for (source, expected) in cases {
        assert_eq!(run(source), Value::Int(expected), "{source}");
    }
}

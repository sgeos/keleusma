#![cfg(all(feature = "compile", feature = "verify"))]
//! General const generics (B40), phase 1: function const parameters used
//! as `Word` values, supplied by an explicit turbofish `f::<8>(...)`, and
//! erased to concrete literals at monomorphization so the verifier sees
//! no symbolic const.

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

fn compile_fails(src: &str) -> bool {
    match parse(&tokenize(src).expect("lex")) {
        Ok(program) => compile(&program).is_err(),
        Err(_) => true,
    }
}

#[test]
fn const_param_used_as_value() {
    // A const parameter is a `Word` value in the body; the turbofish
    // supplies it explicitly and monomorphization substitutes the literal.
    assert_eq!(
        run_to_int("fn val<const n: Word>() -> Word { n }\nfn main() -> Word { val::<5>() }"),
        5
    );
    // Const arithmetic in the body via ordinary `+`.
    assert_eq!(
        run_to_int(
            "fn plus<const n: Word>() -> Word { n + 10 }\nfn main() -> Word { plus::<7>() }"
        ),
        17
    );
}

#[test]
fn distinct_const_args_specialize_independently() {
    // Two instantiations with different const values are distinct
    // specializations, so both concrete values are observed.
    assert_eq!(
        run_to_int(
            "fn val<const n: Word>() -> Word { n }\n\
             fn main() -> Word { val::<3>() + val::<8>() }"
        ),
        11
    );
}

#[test]
fn local_binding_shadows_const_param() {
    // A `let` that reuses the const parameter's name shadows it: the tail
    // `n` is the local `3`, not the const parameter `5`. This is the
    // load-bearing shadowing property of value substitution.
    assert_eq!(
        run_to_int(
            "fn shadow<const n: Word>() -> Word { let n = 3; n }\n\
             fn main() -> Word { shadow::<5>() }"
        ),
        3
    );
    // The const parameter is visible again after the shadowing binding's
    // introduction only within its scope; here a fresh `let` rebinds and
    // the const value is captured before the shadow.
    assert_eq!(
        run_to_int(
            "fn shadow<const n: Word>() -> Word { let a = n; let n = 100; a }\n\
             fn main() -> Word { shadow::<5>() }"
        ),
        5
    );
}

#[test]
fn const_param_bounds_a_verified_loop() {
    // A `for` loop bounded by a const parameter must verify: after
    // substitution the bound is the concrete literal, so the verifier
    // extracts a static iteration count. `Vm::new` runs the verifier, so
    // its success proves the bound became concrete (a symbolic bound
    // would be rejected as not statically boundable).
    let src = "fn loops<const n: Word>() -> Word { for i in 0..n { i } n }\n\
               fn main() -> Word { loops::<5>() }";
    assert_eq!(run_to_int(src), 5);
}

#[test]
fn turbofish_arity_is_checked() {
    // Too many const arguments.
    assert!(compile_fails(
        "fn val<const n: Word>() -> Word { n }\nfn main() -> Word { val::<5, 6>() }"
    ));
    // A const turbofish on a non-const-generic function.
    assert!(compile_fails(
        "fn plain() -> Word { 0 }\nfn main() -> Word { plain::<5>() }"
    ));
    // A const-generic function called without the required turbofish
    // cannot supply the const argument, so the arity check rejects it.
    assert!(compile_fails(
        "fn val<const n: Word>() -> Word { n }\nfn main() -> Word { val() }"
    ));
}

#[test]
fn const_param_type_must_be_word() {
    assert!(compile_fails(
        "fn val<const n: Byte>() -> Word { 0 }\nfn main() -> Word { val::<5>() }"
    ));
}

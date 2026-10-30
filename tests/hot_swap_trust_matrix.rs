//! A registered trust matrix governs every hot swap, not only the bytes path.
//!
//! The pykeleusma line reported that `Vm::replace_module` accepted an
//! unsigned in-memory module after the host had registered a verifying key.
//! `Vm::replace_module_from_bytes` already refused unsigned bytes in that
//! state, V0.2.1 security audit finding 9, so the in-memory entry points were
//! a way around the matrix. The operator ruled on 2026-10-28 that every
//! pykeleusma finding is to be addressed.
//!
//! # What is established
//!
//! With a key registered, `replace_module` and `replace_module_unchecked`
//! refuse with `InvalidSignature` and leave the running module in place. The
//! signed bytes path still swaps. Clearing the matrix restores the in-memory
//! swap, which is the documented way to state that intent.
#![cfg(all(feature = "compile", feature = "signatures"))]

use ed25519_dalek::SigningKey;
use keleusma::Arena;
use keleusma::bytecode::{LoadError, Module, Value};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmError, VmState};
use keleusma::wire_format::module_to_signed_wire_bytes;

fn module(value: i64) -> Module {
    let src = format!("fn main() -> Word {{ {value} }}");
    compile(&parse(&tokenize(&src).unwrap()).unwrap()).unwrap()
}

fn result(vm: &mut Vm) -> Value {
    match vm.call(&[]).unwrap() {
        VmState::Finished(v) => v,
        other => panic!("unexpected {other:?}"),
    }
}

fn refused(r: Result<(), VmError>) -> bool {
    // The error carries the load error's display text.
    let expected = format!("{}", LoadError::InvalidSignature);
    matches!(r, Err(VmError::LoadError(m)) if m == expected)
}

#[test]
fn a_registered_key_refuses_every_in_memory_swap() {
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module(1), &arena).unwrap();
    let signer = SigningKey::from_bytes(&[7u8; 32]);
    vm.register_verifying_key(signer.verifying_key());

    assert!(
        refused(vm.replace_module(module(2), vec![])),
        "replace_module"
    );
    assert!(
        refused(vm.replace_module_unchecked(module(3), vec![])),
        "replace_module_unchecked"
    );
    assert_eq!(result(&mut vm), Value::Int(1), "the running module is kept");

    // The signed bytes path is unaffected.
    let bytes = module_to_signed_wire_bytes(&module(4), &signer).unwrap();
    vm.replace_module_from_bytes(&bytes, vec![]).unwrap();
    assert_eq!(result(&mut vm), Value::Int(4));
}

#[test]
fn clearing_the_matrix_restores_the_in_memory_swap() {
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module(1), &arena).unwrap();
    vm.register_verifying_key(SigningKey::from_bytes(&[7u8; 32]).verifying_key());
    vm.clear_verifying_keys();
    vm.replace_module(module(5), vec![]).unwrap();
    assert_eq!(result(&mut vm), Value::Int(5));
}

/// Negative control. With no key registered, the in-memory swap works as
/// before.
#[test]
fn without_a_key_the_in_memory_swap_is_unchanged() {
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module(1), &arena).unwrap();
    vm.replace_module(module(6), vec![]).unwrap();
    assert_eq!(result(&mut vm), Value::Int(6));
}

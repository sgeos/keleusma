//! A hot swap refuses a module whose working set the arena cannot hold, as
//! `Vm::new` does.
//!
//! # The defect
//!
//! `Vm::new` reserves the module's operand stack, call frames and opaque
//! registry and refuses an arena too small for them. `replace_module`
//! re-reserved them after its reset and fell back to on-demand growth when it
//! could not, so it admitted a module the constructor refuses, which then
//! failed on its first push past the arena. Measured on 2026-10-30 with an
//! arena of 152 bytes and a module needing 560. A pykeleusma finding, that the
//! swap's resource check was looser than the constructor's.
#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::Arena;
use keleusma::vm::{Vm, VmError, VmState, auto_arena_capacity_for};

fn module(src: &str) -> keleusma::bytecode::Module {
    let tokens = keleusma::lexer::tokenize(src).expect("lex");
    keleusma::compiler::compile(&keleusma::parser::parse(&tokens).expect("parse")).expect("compile")
}

const SMALL: &str = "fn main() -> Word { 1 }";
const DEEP: &str = "fn f(a: Word, b: Word, c: Word, d: Word) -> Word { a + b + c + d }\n\
                    fn main() -> Word { f(1, 2, 3, f(4, 5, 6, f(7, 8, 9, f(1, 2, 3, 4)))) }";

#[test]
fn a_swap_the_constructor_would_refuse_is_refused_and_the_old_module_kept() {
    let small = module(SMALL);
    let arena = Arena::with_capacity(auto_arena_capacity_for(&small, &[]).unwrap());
    assert!(
        Vm::new(module(DEEP), &arena).is_err(),
        "the control, the constructor refuses"
    );
    let mut vm = Vm::new(small, &arena).expect("the small module fits");
    match vm.replace_module(module(DEEP), vec![]) {
        Err(VmError::OutOfArena(m)) => assert!(m.contains("working set"), "{m}"),
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert!(matches!(
        vm.call(&[]).expect("the old module runs"),
        VmState::Finished(_)
    ));
}

#[test]
fn a_swap_into_an_arena_sized_for_the_new_module_is_admitted() {
    let deep = module(DEEP);
    let arena = Arena::with_capacity(auto_arena_capacity_for(&deep, &[]).unwrap());
    let mut vm = Vm::new(module(SMALL), &arena).expect("the small module fits");
    vm.replace_module(deep, vec![])
        .expect("the exact size admits the swap");
    assert!(matches!(vm.call(&[]).expect("runs"), VmState::Finished(_)));
}

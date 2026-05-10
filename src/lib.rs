#![no_std]
extern crate alloc;

pub mod ast;
pub mod audio_natives;
pub mod bytecode;
pub mod compiler;
pub mod kstring;
pub mod lexer;
pub mod marshall;
pub mod monomorphize;
pub mod parser;
pub mod target;
pub mod token;
pub mod typecheck;
pub mod utility_natives;
pub mod verify;
pub mod visitor;
pub mod vm;

pub use keleusma_arena::{
    Arena, ArenaHandle, BottomHandle, Budget, EpochSaturated, Stale, TopHandle,
};
pub use kstring::KString;

pub use bytecode::{
    CostModel, Module, NOMINAL_COST_MODEL, VALUE_SLOT_SIZE_BYTES, Value, nominal_op_cycles,
};
pub use keleusma_macros::KeleusmaType;
pub use marshall::{IntoFallibleNativeFn, IntoNativeFn, KeleusmaType};
pub use vm::{NativeCtx, VmError};

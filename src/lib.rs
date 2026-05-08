#![no_std]
extern crate alloc;

pub mod arena;
pub mod ast;
pub mod audio_natives;
pub mod bytecode;
pub mod compiler;
pub mod lexer;
pub mod marshall;
pub mod parser;
pub mod token;
pub mod utility_natives;
pub mod verify;
pub mod vm;

pub use arena::{Arena, HeapHandle, StackHandle};

pub use bytecode::Value;
pub use keleusma_macros::KeleusmaType;
pub use marshall::{IntoFallibleNativeFn, IntoNativeFn, KeleusmaType};
pub use vm::VmError;

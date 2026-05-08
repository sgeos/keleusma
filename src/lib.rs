#![no_std]
extern crate alloc;

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

pub use keleusma_arena::{Arena, BottomHandle, Budget, TopHandle};

pub use bytecode::Value;
pub use keleusma_macros::KeleusmaType;
pub use marshall::{IntoFallibleNativeFn, IntoNativeFn, KeleusmaType};
pub use vm::VmError;

// Backwards-compatible aliases for the previous handle names. These map
// to the conventional `BottomHandle` and `TopHandle` from `keleusma-arena`.
// The Keleusma runtime uses the bottom end for the operand stack region
// and the top end for the dynamic-string heap region.
pub use keleusma_arena::BottomHandle as StackHandle;
pub use keleusma_arena::TopHandle as HeapHandle;

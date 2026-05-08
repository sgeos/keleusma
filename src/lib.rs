#![no_std]
extern crate alloc;

pub mod ast;
pub mod audio_natives;
pub mod bytecode;
pub mod compiler;
pub mod lexer;
pub mod parser;
pub mod token;
pub mod utility_natives;
pub mod verify;
pub mod vm;

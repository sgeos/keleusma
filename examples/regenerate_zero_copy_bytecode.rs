//! Regenerate the binary fixture used by the zero-copy include_bytes
//! example.
//!
//! Reads `examples/zero_copy_demo.kel`, compiles it through the
//! Keleusma frontend, and writes the resulting framed bytecode to
//! `examples/zero_copy_demo.kel.bin`. The binary is committed to the
//! repository so the consuming example can `include_bytes!` it
//! without a build script.
//!
//! Run after a wire format change. The size of the resulting binary
//! becomes the value of the `BYTECODE_LEN` constant in
//! `examples/zero_copy_include_bytes.rs`.
//!
//! Usage:
//!
//!     cargo run --example regenerate_zero_copy_bytecode

use std::fs;
use std::path::PathBuf;

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source_path = manifest_dir.join("examples").join("zero_copy_demo.kel");
    let binary_path = manifest_dir.join("examples").join("zero_copy_demo.kel.bin");

    let source = fs::read_to_string(&source_path).expect("read source");
    let tokens = tokenize(&source).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    let bytes = module.to_bytes().expect("encode");

    fs::write(&binary_path, &bytes).expect("write binary");
    println!("wrote {} ({} bytes)", binary_path.display(), bytes.len());
}

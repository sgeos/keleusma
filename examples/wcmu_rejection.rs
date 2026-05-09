//! Demonstrate WCMU rejection.
//!
//! Construct a Vm with a deliberately undersized arena. The
//! verification rejects the module before any execution can begin.
//!
//! Run with: `cargo run --example wcmu_rejection`

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{Vm, VmError};

const SCRIPT: &str = "
loop main(input: i64) -> i64 {
    let doubled = input * 2;
    let _ignored = yield doubled;
    doubled
}
";

fn main() {
    let tokens = tokenize(SCRIPT).expect("lex error");
    let program = parse(&tokens).expect("parse error");
    let module = compile(&program).expect("compile error");

    // Try to construct a Vm with an arena too small for the program.
    // The verification fails at Vm construction, before any code runs.
    match Vm::new_with_arena_capacity(module, 16) {
        Ok(_) => unreachable!("expected verification to fail"),
        Err(VmError::VerifyError(msg)) => {
            println!("verification rejected the module:");
            println!("  {}", msg);
        }
        Err(other) => panic!("expected VerifyError, got {:?}", other),
    }

    println!();
    println!("the host can either size the arena adequately via Vm::new_with_arena_capacity");
    println!("or use Vm::new_auto to compute a suitable capacity from the module");
}

//! Demonstrate the full Keleusma WCMU pipeline.
//!
//! Compile a script. Compute its worst-case memory usage budget.
//! Construct a Vm with auto-sized arena. Run it.
//!
//! Run with: `cargo run --example wcmu_basic`

use keleusma::Value;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::verify;
use keleusma::vm::{Vm, VmState};

const SCRIPT: &str = "
loop main(input: i64) -> i64 {
    let doubled = input * 2;
    let _ignored = yield doubled;
    doubled
}
";

fn main() {
    // Compile.
    let tokens = tokenize(SCRIPT).expect("lex error");
    let program = parse(&tokens).expect("parse error");
    let module = compile(&program).expect("compile error");

    // Inspect the WCMU budget for each Stream chunk in the module.
    let chunk_wcmu = verify::module_wcmu(&module, &[]).expect("module wcmu");
    for (idx, chunk) in module.chunks.iter().enumerate() {
        if matches!(chunk.block_type, keleusma::bytecode::BlockType::Stream) {
            let (stack_bytes, heap_bytes) = chunk_wcmu[idx];
            println!("chunk `{}`:", chunk.name);
            println!("  stack WCMU: {} bytes", stack_bytes);
            println!("  heap  WCMU: {} bytes", heap_bytes);
            println!("  total:      {} bytes", stack_bytes + heap_bytes);
        }
    }

    // Construct an auto-sized arena and a Vm that borrows it.
    let cap = keleusma::vm::auto_arena_capacity_for(&module, &[]).expect("auto capacity");
    let arena = keleusma::Arena::with_capacity(cap);
    let mut vm = Vm::new(module, &arena).expect("vm construction");
    println!();
    println!("auto-sized arena capacity: {} bytes", vm.arena().capacity());

    // Drive the coroutine through one yield.
    match vm.call(&[Value::Int(21)]).expect("vm call") {
        VmState::Yielded(v) => println!("yielded: {:?}", v),
        other => panic!("expected yield, got {:?}", other),
    }
}

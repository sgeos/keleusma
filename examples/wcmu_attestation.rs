//! Demonstrate native function WCMU attestation.
//!
//! Register a native function. Declare its WCET and WCMU bounds.
//! Re-verify the program with the attestations included. Show the
//! difference between the default attestation (zero heap) and the
//! declared one.
//!
//! Run with: `cargo run --example wcmu_attestation`

use keleusma::Value;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::verify;
use keleusma::vm::{Vm, VmState};

const SCRIPT: &str = "
use host::compute_value
loop main(input: i64) -> i64 {
    let v = host::compute_value(input);
    let _ignored = yield v;
    v
}
";

fn main() {
    // Compile.
    let tokens = tokenize(SCRIPT).expect("lex error");
    let program = parse(&tokens).expect("parse error");
    let module = compile(&program).expect("compile error");

    // Inspect the WCMU budget with no native attestation. Heap is zero.
    let chunk_wcmu_default = verify::module_wcmu(&module, &[]).expect("module wcmu");
    println!("Default attestation (no host declaration):");
    for (idx, chunk) in module.chunks.iter().enumerate() {
        if matches!(chunk.block_type, keleusma::bytecode::BlockType::Stream) {
            let (s, h) = chunk_wcmu_default[idx];
            println!("  chunk `{}`: stack {} heap {}", chunk.name, s, h);
        }
    }

    // Inspect with the host's declared attestation. Heap reflects the
    // bound the host promises the native will not exceed.
    let attested_native_wcmu = [256u32];
    let chunk_wcmu_attested =
        verify::module_wcmu(&module, &attested_native_wcmu).expect("module wcmu");
    println!();
    println!("Attested host::compute_value with 256 bytes:");
    for (idx, chunk) in module.chunks.iter().enumerate() {
        if matches!(chunk.block_type, keleusma::bytecode::BlockType::Stream) {
            let (s, h) = chunk_wcmu_attested[idx];
            println!("  chunk `{}`: stack {} heap {}", chunk.name, s, h);
        }
    }

    // Construct a Vm with adequate capacity.
    let mut vm = Vm::new_with_arena_capacity(module, 4096).expect("vm construction");

    // Register the native and declare its WCET and WCMU bounds.
    vm.register_fn("host::compute_value", |x: i64| -> i64 { x * 3 });
    vm.set_native_bounds("host::compute_value", 25, 256)
        .expect("set bounds");

    // Re-verify resources with the declared attestations now in place.
    vm.verify_resources().expect("verify_resources");
    println!();
    println!(
        "verify_resources succeeded with arena {} bytes",
        vm.arena().capacity()
    );

    // Drive the coroutine through one yield.
    match vm.call(&[Value::Int(7)]).expect("vm call") {
        VmState::Yielded(v) => println!("yielded: {:?}", v),
        other => panic!("expected yield, got {:?}", other),
    }
}

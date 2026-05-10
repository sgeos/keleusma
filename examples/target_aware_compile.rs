//! End-to-end demonstration of target-aware compilation.
//!
//! The compiler accepts a `Target` descriptor through
//! `compile_with_target`. The target's word, address, and float
//! widths are baked into the bytecode wire format, and the compiler
//! rejects programs that use features unsupported by the target
//! (such as floating-point operations on a no-float target).
//!
//! The current 64-bit runtime accepts bytecode with widths at most
//! its own. Emitting for a narrower target produces bytecode the
//! runtime can still load, with integer arithmetic masked to the
//! declared width. A future runtime build for a narrower native
//! target could refuse 64-bit bytecode and admit only its own width.
//!
//! Run with: `cargo run --example target_aware_compile`

use keleusma::compiler::compile_with_target;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::target::Target;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

fn main() {
    let int_only = r#"
        fn main() -> i64 {
            let x: i64 = 7;
            x * 6
        }
    "#;
    let with_float = r#"
        fn main() -> f64 {
            let f: f64 = 1.5;
            f
        }
    "#;

    println!("=== host target (64-bit, all features) ===");
    compile_and_run(int_only, &Target::host());
    compile_and_run(with_float, &Target::host());

    println!();
    println!("=== embedded_32 target (32-bit, all features) ===");
    compile_with_target_show_widths(int_only, &Target::embedded_32());
    compile_and_run(int_only, &Target::embedded_32());
    compile_and_run(with_float, &Target::embedded_32());

    println!();
    println!("=== embedded_16 target (16-bit, no floats) ===");
    compile_with_target_show_widths(int_only, &Target::embedded_16());
    compile_and_run(int_only, &Target::embedded_16());
    let result = compile_with_target(
        &parse(&tokenize(with_float).expect("lex")).expect("parse"),
        &Target::embedded_16(),
    );
    match result {
        Ok(_) => panic!("expected float rejection"),
        Err(e) => println!("float program rejected: {}", e.message),
    }

    println!();
    println!("=== embedded_8 target (8-bit, no floats, no strings) ===");
    compile_with_target_show_widths(int_only, &Target::embedded_8());
    compile_and_run(int_only, &Target::embedded_8());
    let with_string = "fn main() -> i64 { let s = \"hi\"; 0 }";
    let result = compile_with_target(
        &parse(&tokenize(with_string).expect("lex")).expect("parse"),
        &Target::embedded_8(),
    );
    match result {
        Ok(_) => panic!("expected string rejection"),
        Err(e) => println!("string program rejected: {}", e.message),
    }
}

fn compile_with_target_show_widths(src: &str, target: &Target) {
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile_with_target(&program, target).expect("compile");
    println!(
        "  declared widths: word={} addr={} float={}",
        module.word_bits_log2, module.addr_bits_log2, module.float_bits_log2,
    );
}

fn compile_and_run(src: &str, target: &Target) {
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile_with_target(&program, target).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    match vm.call(&[]).expect("call") {
        VmState::Finished(v) => println!("  result: {:?}", v),
        other => panic!("unexpected: {:?}", other),
    }
    let _ = Value::Unit;
}

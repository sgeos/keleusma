//! End-to-end demonstration of string concatenation, slicing, and
//! f-string interpolation.
//!
//! `concat(a, b)` and `slice(s, start, end)` are utility natives.
//! f-strings `f"hello {name}"` desugar at lex time to a chain of
//! `concat` and `to_string` calls. The desugaring is performed by
//! the lexer, which queues the synthesized tokens; the parser sees
//! a regular function-call AST and emits the corresponding bytecode.
//!
//! WCET note. String concatenation and slicing produce dynamic
//! strings whose worst-case output length is the sum of operand
//! lengths (concat) or `end - start` (slice). The current verifier
//! treats native function allocations as the per-native attestation
//! supplied through `Vm::set_native_bounds`. Hosts that rely on
//! `verify_resource_bounds` for real-time embedding must declare
//! heap bounds for `concat` and `slice` before constructing the VM
//! through the safe constructor.
//!
//! Run with: `cargo run --example string_ops`

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::utility_natives::register_utility_natives;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

fn main() {
    let src = r#"
        use to_string
        use concat
        use slice
        fn main() -> String {
            let name = "Keleusma";
            let n: i64 = 42;
            let greeting = f"hello, {name}! n = {n}";
            let head = slice(greeting, 0, 5);
            concat(head, "...")
        }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    register_utility_natives(&mut vm);
    match vm.call(&[]) {
        Ok(VmState::Finished(Value::DynStr(s))) => {
            println!("result: {}", s);
            assert_eq!(s, "hello...");
            println!("string ops executed end to end");
        }
        other => panic!("unexpected: {:?}", other),
    }
}

//! Demonstrator for the parametric `GenericVm<W, A, F>` shape (B16).
//!
//! The bundled `Vm<'a, 'arena>` is a type alias for
//! `GenericVm<'a, 'arena, i64, u64, f64>`. Hosts targeting narrower
//! native runtimes instantiate the generic shape directly with their
//! preferred trait parameters. This example shows the recipe with a
//! 16-bit signed word, a 16-bit unsigned address, and a 32-bit float
//! (the `embedded_16` Target rejects floating-point opcodes at
//! compile time, so the float parameter is a no-op for this program;
//! the parameter is carried anyway because the trait is required by
//! the runtime's shape).
//!
//! The recipe is documented in `docs/guide/COOKBOOK.md`. The same
//! narrow-runtime pattern appears in the integration test
//! `tests/narrow_vm.rs`.
//!
//! Run with: `cargo run --example narrow_runtime`.

use keleusma::Arena;
use keleusma::GenericValue;
use keleusma::compiler::compile_with_target;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::target::Target;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, GenericVm, GenericVmState};

/// Host-defined narrow-runtime alias. Hosts that target a single
/// width define this once near the top of their crate and use the
/// alias for every register-call site.
type NarrowVm<'a, 'arena> = GenericVm<'a, 'arena, i16, u16, f32>;

fn main() {
    println!("=== narrow runtime: Vm<i16, u16, f32> on embedded_16 bytecode ===");

    plain_arithmetic();
    wrapping_at_word_boundary();
    host_function_truncation();
}

fn plain_arithmetic() {
    let src = "fn main() -> Word { 1 + 2 }";
    let module = {
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        compile_with_target(&program, &Target::embedded_16()).expect("compile")
    };

    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm: NarrowVm<'_, '_> = NarrowVm::new(module, &arena).expect("verify");

    match vm.call(&[]).expect("call") {
        GenericVmState::Finished(GenericValue::Int(n)) => {
            assert_eq!(n, 3_i16);
            println!("  plain: 1 + 2 = {}", n);
        }
        other => panic!("unexpected: {:?}", other),
    }
}

fn wrapping_at_word_boundary() {
    // 30_000 + 10_000 = 40_000 exceeds i16::MAX (32_767); the Word
    // trait's wrapping_add discipline wraps to -25_536.
    let src = "fn main() -> Word { 30000 + 10000 }";
    let module = {
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        compile_with_target(&program, &Target::embedded_16()).expect("compile")
    };

    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm: NarrowVm<'_, '_> = NarrowVm::new(module, &arena).expect("verify");

    match vm.call(&[]).expect("call") {
        GenericVmState::Finished(GenericValue::Int(n)) => {
            assert_eq!(n, -25_536_i16);
            println!("  wrap : 30000 + 10000 = {} (wrapped at i16 boundary)", n);
        }
        other => panic!("unexpected: {:?}", other),
    }
}

fn host_function_truncation() {
    // Host closures speak Rust's natural `i64`; the marshall layer
    // truncates through `Word::from_i64_wrap` at the boundary. The
    // host author writes ergonomic Rust without worrying about the
    // narrower script word.
    let src = "use host::triple\nfn main() -> Word { host::triple(7) }";
    let module = {
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        compile_with_target(&program, &Target::embedded_16()).expect("compile")
    };

    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm: NarrowVm<'_, '_> = NarrowVm::new(module, &arena).expect("verify");
    vm.register_fn("host::triple", |x: i64| -> i64 { x * 3 });

    match vm.call(&[]).expect("call") {
        GenericVmState::Finished(GenericValue::Int(n)) => {
            assert_eq!(n, 21_i16);
            println!("  host: triple(7) = {} (via i64 host closure)", n);
        }
        other => panic!("unexpected: {:?}", other),
    }
}

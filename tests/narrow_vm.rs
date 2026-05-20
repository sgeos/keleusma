//! Narrow-runtime demonstrator for B16 step 7.
//!
//! Exercises a `GenericVm<i16, u16, f32>` against bytecode compiled
//! with `Target::embedded_16()`. The 16-bit target rejects
//! floating-point features at compile time, so the script side is
//! purely integer; the `f32` float-trait parameter on the runtime is
//! a no-op for this program. See `examples/narrow_runtime.rs` for the
//! standalone example and `docs/guide/COOKBOOK.md` for the type-alias
//! recipe.

#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::Arena;
use keleusma::GenericValue;
use keleusma::compiler::compile_with_target;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::target::Target;
use keleusma::vm::{GenericVm, GenericVmState};

/// Host-defined narrow-runtime alias following the recipe in
/// `docs/guide/COOKBOOK.md` (16-bit signed word, 16-bit unsigned
/// address, 32-bit float kept for future float opcodes even though
/// the `embedded_16` target has `has_floats = false`).
type NarrowVm<'a, 'arena> = GenericVm<'a, 'arena, i16, u16, f32>;

#[test]
fn narrow_runtime_runs_embedded_16_bytecode() {
    let src = "fn main() -> Word { 1 + 2 }";
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile_with_target(&program, &Target::embedded_16()).expect("compile");

    let arena = Arena::with_capacity(4096);
    let mut vm: NarrowVm<'_, '_> = NarrowVm::new(module, &arena).expect("new");
    match vm.call(&[]).expect("call") {
        GenericVmState::Finished(GenericValue::Int(n)) => assert_eq!(n, 3_i16),
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn narrow_runtime_arithmetic_wraps_at_i16_boundary() {
    // 30_000 + 10_000 = 40_000 exceeds i16::MAX (32_767) and wraps
    // to 40_000 - 65_536 = -25_536 under the Word trait's
    // wrapping_add discipline.
    let src = "fn main() -> Word { 30000 + 10000 }";
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile_with_target(&program, &Target::embedded_16()).expect("compile");

    let arena = Arena::with_capacity(4096);
    let mut vm: NarrowVm<'_, '_> = NarrowVm::new(module, &arena).expect("new");
    match vm.call(&[]).expect("call") {
        GenericVmState::Finished(GenericValue::Int(n)) => assert_eq!(n, -25_536_i16),
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn narrow_runtime_register_fn_marshall_truncates_through_word() {
    // The host writes the natural Rust signature `i64`; the
    // marshall layer truncates to `i16` through Word::from_i64_wrap.
    let src = "use host::triple\nfn main() -> Word { host::triple(7) }";
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile_with_target(&program, &Target::embedded_16()).expect("compile");

    let arena = Arena::with_capacity(4096);
    let mut vm: NarrowVm<'_, '_> = NarrowVm::new(module, &arena).expect("new");
    vm.register_fn("host::triple", |x: i64| -> i64 { x * 3 });
    match vm.call(&[]).expect("call") {
        GenericVmState::Finished(GenericValue::Int(n)) => assert_eq!(n, 21_i16),
        other => panic!("unexpected: {:?}", other),
    }
}

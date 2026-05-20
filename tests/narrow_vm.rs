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

/// Narrow-Word but f64-Float alias. Used to exercise the lifted
/// `Library<W, A, f64>` impls of `stddsl::Math` and `stddsl::Audio`
/// on a runtime whose word type is narrower than the default i64.
#[cfg(feature = "floats")]
type NarrowWordF64Vm<'a, 'arena> = GenericVm<'a, 'arena, i16, u16, f64>;

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
fn narrow_runtime_rejects_wider_word_bytecode() {
    // Bytecode declared for the default 64-bit host runtime (Target::host)
    // must be rejected by the i16 narrow VM. Without the load-time width
    // check the narrow VM would silently truncate constants through
    // Word::from_i64_wrap.
    let src = "fn main() -> Word { 0 }";
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile_with_target(&program, &Target::host()).expect("compile");

    let arena = Arena::with_capacity(4096);
    let err = match NarrowVm::new(module, &arena) {
        Ok(_) => panic!("must reject wider bytecode"),
        Err(e) => e,
    };
    let msg = format!("{:?}", err);
    assert!(
        msg.contains("word_bits_log2"),
        "expected width-mismatch error, got: {}",
        msg
    );
}

#[cfg(feature = "floats")]
#[test]
fn narrow_float_runtime_runs_f32_bytecode() {
    // Exercise a runtime whose Float type is f32. The bytecode is
    // compiled with a custom Target that declares the matching
    // float width. The host closure's f64 parameters truncate to
    // f32 through Float::from_f64 / Float::to_f64.
    let target = Target {
        word_bits_log2: 6,
        addr_bits_log2: 6,
        float_bits_log2: 5,
        has_floats: true,
        has_strings: false,
    };
    let src = "use host::halve\nfn main() -> Float { host::halve(8.0) }";
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile_with_target(&program, &target).expect("compile");

    let arena = Arena::with_capacity(4096);
    type F32Vm<'a, 'arena> = GenericVm<'a, 'arena, i64, u64, f32>;
    let mut vm: F32Vm<'_, '_> = F32Vm::new(module, &arena).expect("new");
    vm.register_fn("host::halve", |x: f64| -> f64 { x / 2.0 });
    match vm.call(&[]).expect("call") {
        GenericVmState::Finished(GenericValue::Float(f)) => {
            assert_eq!(f, 4.0_f32);
        }
        other => panic!("unexpected: {:?}", other),
    }
}

#[cfg(feature = "floats")]
#[test]
fn wider_float_bytecode_rejected_by_f32_runtime() {
    // Bytecode declaring float_bits_log2 = 6 (f64) must be rejected
    // by a runtime whose F = f32. Otherwise the load-time width
    // check would silently narrow constants from f64 to f32.
    let target = Target {
        word_bits_log2: 6,
        addr_bits_log2: 6,
        float_bits_log2: 6,
        has_floats: true,
        has_strings: false,
    };
    let src = "fn main() -> Float { 1.5 }";
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile_with_target(&program, &target).expect("compile");

    let arena = Arena::with_capacity(4096);
    type F32Vm<'a, 'arena> = GenericVm<'a, 'arena, i64, u64, f32>;
    let err = match F32Vm::new(module, &arena) {
        Ok(_) => panic!("must reject wider float bytecode"),
        Err(e) => e,
    };
    let msg = format!("{:?}", err);
    assert!(
        msg.contains("float_bits_log2"),
        "expected width-mismatch error, got: {}",
        msg
    );
}

#[cfg(feature = "floats")]
#[test]
fn narrow_runtime_can_register_math_library_via_lifted_impl() {
    // The stddsl::Math bundle was lifted to `Library<W, A, f64>` so
    // a runtime with W = i16 (and F kept at f64 because the bundle's
    // closures pin f64) can register it. Without the lift the
    // narrow runtime would not satisfy the bundle's trait bound.
    let target = Target {
        word_bits_log2: 4,
        addr_bits_log2: 4,
        float_bits_log2: 6,
        has_floats: true,
        has_strings: false,
    };
    let src = "use math::sqrt\nfn main() -> Float { math::sqrt(9.0) }";
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile_with_target(&program, &target).expect("compile");

    let arena = Arena::with_capacity(4096);
    let mut vm: NarrowWordF64Vm<'_, '_> = NarrowWordF64Vm::new(module, &arena).expect("new");
    vm.register_library(keleusma::stddsl::Math);
    match vm.call(&[]).expect("call") {
        GenericVmState::Finished(GenericValue::Float(f)) => assert!((f - 3.0_f64).abs() < 1e-9),
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

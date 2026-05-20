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

extern crate alloc;

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

#[test]
fn narrow_runtime_checked_arithmetic_exercises_word_widen() {
    // Pattern-arm checked multiplication. On the narrow i16 runtime,
    // 200 * 200 = 40_000 exceeds i16::MAX (32_767); the runtime
    // computes the true product via W::widen / WideWord::wide_mul
    // (i16 -> i32) and surfaces the (high, low) intermediate
    // through the overflow arm. The example verifies that
    // narrow-word checked arithmetic uses the right widened type:
    // high = 40000 >> 16 = 0; low = 40000 - 32768 - 32768 = wraps
    // (40000 as i16 = -25_536).
    let src = "
        fn product_overflow_low() -> Word {
            200 * 200 {
                ok(v) => v,
                overflow(_, l) => l,
                underflow(_, l) => l,
            }
        }
        fn main() -> Word { product_overflow_low() }
    ";
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
fn narrow_runtime_rejects_hot_swap_to_wider_bytecode() {
    // Construct a narrow Vm against narrow bytecode (admitted), then
    // attempt to hot-swap to wider bytecode (must be rejected by the
    // load-time width check inside replace_module_inner, not silently
    // installed). Without the check the post-swap Vm would silently
    // truncate i64 constants through Word::from_i64_wrap.
    let narrow_src = "fn main() -> Word { 0 }";
    let narrow_module = {
        let tokens = tokenize(narrow_src).expect("lex");
        let program = parse(&tokens).expect("parse");
        compile_with_target(&program, &Target::embedded_16()).expect("compile")
    };
    let wider_src = "fn main() -> Word { 0 }";
    let wider_module = {
        let tokens = tokenize(wider_src).expect("lex");
        let program = parse(&tokens).expect("parse");
        compile_with_target(&program, &Target::host()).expect("compile")
    };

    let arena = Arena::with_capacity(4096);
    let mut vm: NarrowVm<'_, '_> = NarrowVm::new(narrow_module, &arena).expect("new");
    let err = match vm.replace_module_unchecked(wider_module, alloc::vec::Vec::new()) {
        Ok(_) => panic!("must reject wider hot-swap"),
        Err(e) => e,
    };
    let msg = format!("{:?}", err);
    assert!(
        msg.contains("word_bits_log2"),
        "expected width-mismatch error, got: {}",
        msg
    );
}

#[cfg(all(feature = "floats", feature = "text"))]
#[test]
fn narrow_runtime_can_register_text_library_via_lifted_impl() {
    // The stddsl::Text bundle now lifts to Library<W, A, F> for any
    // (W, A, F). A narrow Vm whose W = i16 can register it; the
    // utility natives use Word::from_i64_wrap for length values and
    // Word::to_i64 for index arguments so the script-visible word
    // type drives the boundary semantics.
    let target = Target {
        word_bits_log2: 4,
        addr_bits_log2: 4,
        float_bits_log2: 6,
        has_floats: true,
        has_strings: true,
    };
    let src = "use length\nfn main() -> Word { length(\"hello\") }";
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile_with_target(&program, &target).expect("compile");

    let arena = Arena::with_capacity(4096);
    let mut vm: NarrowWordF64Vm<'_, '_> = NarrowWordF64Vm::new(module, &arena).expect("new");
    vm.register_library(keleusma::stddsl::Text);
    match vm.call(&[]).expect("call") {
        GenericVmState::Finished(GenericValue::Int(n)) => assert_eq!(n, 5_i16),
        other => panic!("unexpected: {:?}", other),
    }
}

#[cfg(feature = "floats")]
#[test]
fn narrow_runtime_can_register_audio_library_via_lifted_impl() {
    // Item 6 follow-up: explicitly exercise the Audio bundle on a
    // narrow-Word runtime. Mirrors the Math test but registers Audio
    // and calls audio::midi_to_freq.
    let target = Target {
        word_bits_log2: 4,
        addr_bits_log2: 4,
        float_bits_log2: 6,
        has_floats: true,
        has_strings: false,
    };
    let src = "use audio::midi_to_freq\nfn main() -> Float { audio::midi_to_freq(69) }";
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile_with_target(&program, &target).expect("compile");

    let arena = Arena::with_capacity(4096);
    let mut vm: NarrowWordF64Vm<'_, '_> = NarrowWordF64Vm::new(module, &arena).expect("new");
    vm.register_library(keleusma::stddsl::Audio);
    match vm.call(&[]).expect("call") {
        GenericVmState::Finished(GenericValue::Float(f)) => {
            // MIDI 69 = A4 = 440 Hz.
            assert!((f - 440.0_f64).abs() < 1e-9);
        }
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn narrow_runtime_view_bytes_zero_copy_runs_embedded_16_bytecode() {
    // Item 7: regression test that view_bytes_zero_copy threads the
    // load-time width check correctly on a narrow runtime. The zero-
    // copy path reads widths from framing-header bytes 10..12 rather
    // than materialising a Module; the path is exercised by reading
    // a precompiled byte slice produced via Module::to_bytes.
    let src = "fn main() -> Word { 1 + 2 }";
    let module = {
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        compile_with_target(&program, &Target::embedded_16()).expect("compile")
    };
    let bytes = module.to_bytes().expect("serialize");
    // Align the bytes to 8-byte boundary as view_bytes_zero_copy requires.
    let mut aligned: rkyv::util::AlignedVec<8> = rkyv::util::AlignedVec::with_capacity(bytes.len());
    aligned.extend_from_slice(&bytes);

    let arena = Arena::with_capacity(4096);
    let mut vm: NarrowVm<'_, '_> =
        unsafe { NarrowVm::view_bytes_zero_copy(aligned.as_slice(), &arena) }.expect("view");
    match vm.call(&[]).expect("call") {
        GenericVmState::Finished(GenericValue::Int(n)) => assert_eq!(n, 3_i16),
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn narrow_runtime_view_bytes_zero_copy_rejects_wider_bytecode() {
    // Item 7 paired regression: the zero-copy path must reject a
    // mismatched width just as Vm::new does.
    let src = "fn main() -> Word { 0 }";
    let module = {
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        compile_with_target(&program, &Target::host()).expect("compile")
    };
    let bytes = module.to_bytes().expect("serialize");
    let mut aligned: rkyv::util::AlignedVec<8> = rkyv::util::AlignedVec::with_capacity(bytes.len());
    aligned.extend_from_slice(&bytes);

    let arena = Arena::with_capacity(4096);
    let err = match unsafe { NarrowVm::view_bytes_zero_copy(aligned.as_slice(), &arena) } {
        Ok(_) => panic!("must reject wider bytecode on zero-copy path"),
        Err(e) => e,
    };
    let msg = format!("{:?}", err);
    assert!(
        msg.contains("word_bits_log2"),
        "expected width-mismatch error, got: {}",
        msg
    );
}

#[test]
fn i8_narrow_runtime_runs_embedded_8_bytecode() {
    // Item 8: end-to-end smoke test for Vm<i8>. The embedded_8 target
    // has no floats and no strings, so the script is integer-only.
    // 100 + 27 = 127 fits i8::MAX exactly. 100 + 28 wraps via
    // Word::wrapping_add to -128 (i8 boundary).
    let src = "fn main() -> Word { 100 + 27 }";
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile_with_target(&program, &Target::embedded_8()).expect("compile");

    let arena = Arena::with_capacity(4096);
    type RetroVm<'a, 'arena> = GenericVm<'a, 'arena, i8, u16, f32>;
    let mut vm: RetroVm<'_, '_> = RetroVm::new(module, &arena).expect("new");
    match vm.call(&[]).expect("call") {
        GenericVmState::Finished(GenericValue::Int(n)) => assert_eq!(n, 127_i8),
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn i8_narrow_runtime_handles_aggregate_tuple() {
    // i8 runtime against an aggregate type (tuple). Each tuple element
    // is an i8-valued Word; the runtime stores Value::Tuple([Int(W); 2])
    // and returns the second element as Word.
    let src = "fn pair() -> (Word, Word) { (10, 20) }\n\
               fn main() -> Word {\n\
                   let p = pair();\n\
                   p.1\n\
               }";
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile_with_target(&program, &Target::embedded_8()).expect("compile");

    let arena = Arena::with_capacity(4096);
    type RetroVm<'a, 'arena> = GenericVm<'a, 'arena, i8, u16, f32>;
    let mut vm: RetroVm<'_, '_> = RetroVm::new(module, &arena).expect("new");
    match vm.call(&[]).expect("call") {
        GenericVmState::Finished(GenericValue::Int(n)) => assert_eq!(n, 20_i8),
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn i8_narrow_runtime_view_bytes_zero_copy_round_trips() {
    // Belt-and-suspenders: the zero-copy load path threads the load-time
    // width check through Vm<i8> as well as Vm<i16>.
    let src = "fn main() -> Word { 100 + 20 }";
    let module = {
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        compile_with_target(&program, &Target::embedded_8()).expect("compile")
    };
    let bytes = module.to_bytes().expect("serialize");
    let mut aligned: rkyv::util::AlignedVec<8> = rkyv::util::AlignedVec::with_capacity(bytes.len());
    aligned.extend_from_slice(&bytes);

    let arena = Arena::with_capacity(4096);
    type RetroVm<'a, 'arena> = GenericVm<'a, 'arena, i8, u16, f32>;
    let mut vm: RetroVm<'_, '_> =
        unsafe { RetroVm::view_bytes_zero_copy(aligned.as_slice(), &arena) }.expect("view");
    match vm.call(&[]).expect("call") {
        GenericVmState::Finished(GenericValue::Int(n)) => assert_eq!(n, 120_i8),
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn i8_narrow_runtime_wraps_at_i8_boundary() {
    // Item 8 paired regression: 100 + 28 = 128 exceeds i8::MAX and
    // wraps to -128 via Word::wrapping_add.
    let src = "fn main() -> Word { 100 + 28 }";
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile_with_target(&program, &Target::embedded_8()).expect("compile");

    let arena = Arena::with_capacity(4096);
    type RetroVm<'a, 'arena> = GenericVm<'a, 'arena, i8, u16, f32>;
    let mut vm: RetroVm<'_, '_> = RetroVm::new(module, &arena).expect("new");
    match vm.call(&[]).expect("call") {
        GenericVmState::Finished(GenericValue::Int(n)) => assert_eq!(n, -128_i8),
        other => panic!("unexpected: {:?}", other),
    }
}

#[cfg(feature = "floats")]
#[test]
fn f32_narrow_runtime_can_register_math_library_via_lifted_impl() {
    // After step 10, Math lifts to Library<W, A, F> for any (W, A, F).
    // Register Math on a runtime whose Float type is f32; the host
    // closures' f64 arguments and returns truncate at the marshall
    // boundary through Float::from_f64 / Float::to_f64. The numeric
    // result (sqrt(9.0) = 3.0) survives the narrowing because 3.0
    // is exactly representable in f32.
    let target = Target {
        word_bits_log2: 6,
        addr_bits_log2: 6,
        float_bits_log2: 5,
        has_floats: true,
        has_strings: false,
    };
    let src = "use math::sqrt\nfn main() -> Float { math::sqrt(9.0) }";
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile_with_target(&program, &target).expect("compile");

    let arena = Arena::with_capacity(4096);
    type F32Vm<'a, 'arena> = GenericVm<'a, 'arena, i64, u64, f32>;
    let mut vm: F32Vm<'_, '_> = F32Vm::new(module, &arena).expect("new");
    vm.register_library(keleusma::stddsl::Math);
    match vm.call(&[]).expect("call") {
        GenericVmState::Finished(GenericValue::Float(f)) => {
            assert_eq!(f, 3.0_f32);
        }
        other => panic!("unexpected: {:?}", other),
    }
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

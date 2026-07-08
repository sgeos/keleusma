//! Narrow-runtime demonstrator for B16 step 7.
//!
//! Exercises a `GenericVm<i16, u16, f32>` against bytecode compiled
//! with `Target::embedded_16()`. The 16-bit target rejects
//! floating-point features at compile time, so the script side is
//! purely integer; the `f32` float-trait parameter on the runtime is
//! a no-op for this program. See `examples/narrow_runtime.rs` for the
//! standalone example and `docs/guide/COOKBOOK.md` for the type-alias
//! recipe.

// The tests instantiate parametric runtimes such as
// `GenericVm<i16, u16, f32>` and compile against `Target::embedded_16`
// or `Target::embedded_8`. `compile_with_target` validates the
// target against the build-wide `RUNTIME_*_BITS_LOG2` ceiling, not
// against the parametric runtime instantiated in the test. Under
// `narrow-word-8` the build's ceiling is 3 bits and the
// `embedded_16` target's 4 bits exceeds it; the compile rejects
// before the runtime widening has a chance to run. Gate the entire
// file on a build wide enough to admit `embedded_16`. Hosts that
// build with the narrowest runtime do not exercise these
// demonstrators.
#![cfg(all(
    feature = "compile",
    feature = "verify",
    not(any(feature = "narrow-word-8", feature = "narrow-address-8"))
))]

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
    // high = 40000 lsr 16 = 0; low = 40000 - 32768 - 32768 = wraps
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

// `narrow_runtime_can_register_text_library_via_lifted_impl` was
// removed in V0.2.0 with the deletion of `stddsl::Text` and the
// `length`/`to_string`/`concat`/`slice` utility natives.

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

// --- Multiword<N> on the narrow i16 runtime (B19). The multi-word
// carry, borrow, and comparison lowerings compute their sign-bit shift
// and sign constant from the target word width, so they must be correct
// at a 16-bit word, not only at the default 64-bit word. These tests
// lock that in: a signed sign constant of 1 lsl 15 narrows through
// Word::from_i64_wrap to the i16 sign pattern 0x8000, and the top-bit
// extraction shifts by word_bits - 1 = 15. ---

/// Compile a Multiword source for the 16-bit target and run it on the
/// i16 narrow runtime, returning the finished integer.
fn run_i16(src: &str) -> i16 {
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile_with_target(&program, &Target::embedded_16()).expect("compile");
    let arena = Arena::with_capacity(4096);
    let mut vm: NarrowVm<'_, '_> = NarrowVm::new(module, &arena).expect("new");
    match vm.call(&[]).expect("call") {
        GenericVmState::Finished(GenericValue::Int(n)) => n,
        other => panic!("unexpected: {:?}", other),
    }
}

/// As `run_i16`, for a bool-returning entry point.
fn run_bool_i16(src: &str) -> bool {
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile_with_target(&program, &Target::embedded_16()).expect("compile");
    let arena = Arena::with_capacity(4096);
    let mut vm: NarrowVm<'_, '_> = NarrowVm::new(module, &arena).expect("new");
    match vm.call(&[]).expect("call") {
        GenericVmState::Finished(GenericValue::Bool(b)) => b,
        other => panic!("unexpected: {:?}", other),
    }
}

/// Compile a Multiword source for the 16-bit target but run it on the DEFAULT
/// 64-bit runtime. This is the declared-word-width-narrower-than-runtime
/// widening path: the bytecode declares 16-bit words while the runtime is
/// 64-bit, so the runtime must mask each limb to the declared width. The
/// return is the finished i64.
fn run_decl16_on_wide(src: &str) -> i64 {
    type WideVm<'a, 'arena> = GenericVm<'a, 'arena, i64, u64, f64>;
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile_with_target(&program, &Target::embedded_16()).expect("compile");
    let arena = Arena::with_capacity(4096);
    let mut vm: WideVm<'_, '_> = WideVm::new(module, &arena).expect("new");
    match vm.call(&[]).expect("call") {
        GenericVmState::Finished(GenericValue::Int(n)) => n,
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn narrow_multiword_construct_and_index() {
    // A two-word value at i16 is a 32-bit magnitude; the words index
    // back out unchanged.
    assert_eq!(
        run_i16("fn main() -> Word { let m = (42, 7) as Multiword<2>; m[0] }"),
        42_i16
    );
    assert_eq!(
        run_i16("fn main() -> Word { let m = (42, 7) as Multiword<2>; m[1] }"),
        7_i16
    );
}

#[test]
fn narrow_multiword_add_unsigned_carry() {
    // Low word -1 is 0xFFFF, unsigned 65_535 at i16; adding 1 carries
    // into the high word, giving (0, 1). This exercises the word_bits-1
    // = 15 top-bit shift at the narrow width.
    assert_eq!(
        run_i16(
            "fn main() -> Word { let a = (-1, 0) as Multiword<2>; let b = (1, 0) as Multiword<2>; let s = a + b; s[0] }"
        ),
        0_i16
    );
    assert_eq!(
        run_i16(
            "fn main() -> Word { let a = (-1, 0) as Multiword<2>; let b = (1, 0) as Multiword<2>; let s = a + b; s[1] }"
        ),
        1_i16
    );
}

#[test]
fn narrow_multiword_add_no_spurious_signed_carry() {
    // i16::MAX = 32_767. Adding 1 sets the low word's sign bit, turning
    // it into i16::MIN, but no bit carries out of the low word, so the
    // high word stays 0. A signed-flag cascade would wrongly carry.
    assert_eq!(
        run_i16(
            "fn main() -> Word { let a = (32767, 0) as Multiword<2>; let b = (1, 0) as Multiword<2>; let s = a + b; s[1] }"
        ),
        0_i16
    );
    assert_eq!(
        run_i16(
            "fn main() -> Word { let a = (32767, 0) as Multiword<2>; let b = (1, 0) as Multiword<2>; let s = a + b; s[0] }"
        ),
        i16::MIN
    );
}

#[test]
fn narrow_multiword_compare_high_word_signed_low_word_unsigned() {
    // Signed top word: (0, -1) is a negative value, below zero.
    assert!(run_bool_i16(
        "fn main() -> bool { let a = (0, -1) as Multiword<2>; let b = (0, 0) as Multiword<2>; a < b }"
    ));
    // Unsigned low word: -1 is 65_535 unsigned, so (-1, 0) is the
    // larger value when the high words are equal.
    assert!(run_bool_i16(
        "fn main() -> bool { let a = (-1, 0) as Multiword<2>; let b = (1, 0) as Multiword<2>; a > b }"
    ));
}

#[test]
fn narrow_multiword_mul_unsigned_high_correction() {
    // At i16, low word -1 is 0xFFFF = 65_535 unsigned. (-1, 0) * (2, 0)
    // is the unsigned product 131_070, whose low two 16-bit words are
    // (-2, 1). Exercises the signed-to-unsigned high-word correction and
    // the i16 widening path of Op::CheckedMul.
    assert_eq!(
        run_i16(
            "fn main() -> Word { let a = (-1, 0) as Multiword<2>; let b = (2, 0) as Multiword<2>; let s = a * b; s[0] }"
        ),
        -2_i16
    );
    assert_eq!(
        run_i16(
            "fn main() -> Word { let a = (-1, 0) as Multiword<2>; let b = (2, 0) as Multiword<2>; let s = a * b; s[1] }"
        ),
        1_i16
    );
}

#[test]
fn narrow_multiword_mul_carry_into_high_word() {
    // 300 * 300 = 90_000 exceeds 2^16, so the low digit product carries
    // 1 into result word 1; the low word is 90_000 mod 2^16 = 24_464.
    assert_eq!(
        run_i16(
            "fn main() -> Word { let a = (300, 0) as Multiword<2>; let b = (300, 0) as Multiword<2>; let s = a * b; s[0] }"
        ),
        24_464_i16
    );
    assert_eq!(
        run_i16(
            "fn main() -> Word { let a = (300, 0) as Multiword<2>; let b = (300, 0) as Multiword<2>; let s = a * b; s[1] }"
        ),
        1_i16
    );
}

#[test]
fn narrow_multiword_mul_rejects_word_count_that_overflows_accumulator() {
    // The two-word Comba accumulator of the multiply stays exact only
    // while 2N + 1 < 2^word_bits. On the 16-bit target that bound is
    // N < 32768, so a Multiword<32768> multiply must be rejected at
    // compile time rather than lowered to a silently wrong product. The
    // operands are declared through a parameter type, so no 32768-element
    // tuple is constructed; the guard fires before any unrolling. Every
    // top-level function is compiled, so the uncalled `wide` is enough.
    let src = "fn wide(a: Multiword<32768>, b: Multiword<32768>) -> Word { let s = a * b; s[0] }\n\
               fn main() -> Word { 0 }";
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    assert!(
        compile_with_target(&program, &Target::embedded_16()).is_err(),
        "Multiword<32768> multiply must be rejected on a 16-bit word"
    );
    // The same word count is admitted when it does not overflow: on the
    // default 64-bit host word, 2N + 1 is far below 2^64, so the multiply
    // compiles (verified indirectly by a small N here to keep the test
    // cheap; the 64-bit N=32768 case would unroll a billion products).
    let ok_src = "fn wide(a: Multiword<2>, b: Multiword<2>) -> Word { let s = a * b; s[0] }\n\
                  fn main() -> Word { 0 }";
    let ok_tokens = tokenize(ok_src).expect("lex");
    let ok_program = parse(&ok_tokens).expect("parse");
    assert!(compile_with_target(&ok_program, &Target::embedded_16()).is_ok());
}

#[test]
fn narrow_multiword_fixed_mul_positive() {
    // Q8.8 at i16: 1.5 = 384, 2.0 = 512, product 3.0 = 768. The raw
    // product 3 * 2^16 is shifted right by F = 8 to 768. Confirms the
    // sub-word shift and its logical-shift mask at a 16-bit word.
    assert_eq!(
        run_i16(
            "fn main() -> Word { let a = (384, 0) as Multiword<2, 8>; let b = (512, 0) as Multiword<2, 8>; let s = a * b; s[0] }"
        ),
        768_i16
    );
}

#[test]
fn narrow_multiword_fixed_mul_negative() {
    // Q8.8 at i16: -1.5 = (-384, -1), times 2.0 = 512, product -3.0 =
    // (-768, -1). Exercises the product-level signed correction and the
    // arithmetic shift at the narrow width.
    assert_eq!(
        run_i16(
            "fn main() -> Word { let a = (-384, -1) as Multiword<2, 8>; let b = (512, 0) as Multiword<2, 8>; let s = a * b; s[0] }"
        ),
        -768_i16
    );
    assert_eq!(
        run_i16(
            "fn main() -> Word { let a = (-384, -1) as Multiword<2, 8>; let b = (512, 0) as Multiword<2, 8>; let s = a * b; s[1] }"
        ),
        -1_i16
    );
}

#[test]
fn narrow_multiword_div_and_mod() {
    // 100 / 7 = 14, 100 % 7 = 2 at the i16 word width.
    assert_eq!(
        run_i16(
            "fn main() -> Word { let a = (100, 0) as Multiword<2>; let b = (7, 0) as Multiword<2>; let s = a / b; s[0] }"
        ),
        14_i16
    );
    assert_eq!(
        run_i16(
            "fn main() -> Word { let a = (100, 0) as Multiword<2>; let b = (7, 0) as Multiword<2>; let s = a % b; s[0] }"
        ),
        2_i16
    );
    // -100 / 7 = -14, sign handling at the narrow width.
    assert_eq!(
        run_i16(
            "fn main() -> Word { let a = (-100, -1) as Multiword<2>; let b = (7, 0) as Multiword<2>; let s = a / b; s[0] }"
        ),
        -14_i16
    );
}

#[test]
fn narrow_multiword_div_spans_two_words() {
    // 2^16 / 2 = 2^15, the bit pattern i16::MIN in the low word with a
    // zero high word.
    assert_eq!(
        run_i16(
            "fn main() -> Word { let a = (0, 1) as Multiword<2>; let b = (2, 0) as Multiword<2>; let s = a / b; s[1] }"
        ),
        0_i16
    );
    assert_eq!(
        run_i16(
            "fn main() -> Word { let a = (0, 1) as Multiword<2>; let b = (2, 0) as Multiword<2>; let s = a / b; s[0] }"
        ),
        i16::MIN
    );
}

#[test]
fn narrow_multiword_div_min_over_minus_one_wraps() {
    // The most negative two-word value at i16 is MIN = (0, -32768) =
    // -2^31. MIN / -1 is mathematically 2^31, which overflows the
    // two-word range and wraps back to MIN, matching the scalar
    // wrapping division; MIN % -1 is 0. The magnitude of MIN is not
    // representable, so this is the one division edge where the result
    // wraps.
    assert_eq!(
        run_i16(
            "fn main() -> Word { let a = (0, -32768) as Multiword<2>; let b = (-1, -1) as Multiword<2>; let s = a / b; s[1] }"
        ),
        i16::MIN
    );
    assert_eq!(
        run_i16(
            "fn main() -> Word { let a = (0, -32768) as Multiword<2>; let b = (-1, -1) as Multiword<2>; let s = a / b; s[0] }"
        ),
        0_i16
    );
    assert_eq!(
        run_i16(
            "fn main() -> Word { let a = (0, -32768) as Multiword<2>; let b = (-1, -1) as Multiword<2>; let s = a % b; s[0] }"
        ),
        0_i16
    );
    assert_eq!(
        run_i16(
            "fn main() -> Word { let a = (0, -32768) as Multiword<2>; let b = (-1, -1) as Multiword<2>; let s = a % b; s[1] }"
        ),
        0_i16
    );
}

#[test]
fn narrow_declared_multiword_long_division_on_wide_runtime() {
    // Audit follow-up: the multiword long-division inner loop under a declared
    // word width narrower than the runtime width was flagged unconfirmed. Here
    // the bytecode declares a 16-bit word (Target::embedded_16) but runs on the
    // 64-bit runtime. The long division must respect the declared width, not
    // the runtime width. The matching-width results (`run_i16` above) are the
    // reference.
    //
    // 100 / 7 = 14, 100 % 7 = 2 at the declared 16-bit width.
    assert_eq!(
        run_decl16_on_wide(
            "fn main() -> Word { let a = (100, 0) as Multiword<2>; let b = (7, 0) as Multiword<2>; let s = a / b; s[0] }"
        ),
        14
    );
    assert_eq!(
        run_decl16_on_wide(
            "fn main() -> Word { let a = (100, 0) as Multiword<2>; let b = (7, 0) as Multiword<2>; let s = a % b; s[0] }"
        ),
        2
    );
    // Sign handling at the narrow declared width: -100 / 7 = -14.
    assert_eq!(
        run_decl16_on_wide(
            "fn main() -> Word { let a = (-100, -1) as Multiword<2>; let b = (7, 0) as Multiword<2>; let s = a / b; s[0] }"
        ),
        -14
    );
    // Spans a word boundary: 2^16 / 2 = 2^15. The quotient's low limb is the
    // 16-bit sign pattern i16::MIN (=-32768) after masking to the declared
    // width, NOT the naive 64-bit 32768, and the high limb is zero.
    assert_eq!(
        run_decl16_on_wide(
            "fn main() -> Word { let a = (0, 1) as Multiword<2>; let b = (2, 0) as Multiword<2>; let s = a / b; s[0] }"
        ),
        i16::MIN as i64
    );
    assert_eq!(
        run_decl16_on_wide(
            "fn main() -> Word { let a = (0, 1) as Multiword<2>; let b = (2, 0) as Multiword<2>; let s = a / b; s[1] }"
        ),
        0
    );
    // A genuinely multi-limb divisor (nonzero high limb) drives the inner
    // loop: (6*2^16 + 5) / (3*2^16) = 2 remainder 5.
    assert_eq!(
        run_decl16_on_wide(
            "fn main() -> Word { let a = (5, 6) as Multiword<2>; let b = (0, 3) as Multiword<2>; let s = a / b; s[0] }"
        ),
        2
    );
    assert_eq!(
        run_decl16_on_wide(
            "fn main() -> Word { let a = (5, 6) as Multiword<2>; let b = (0, 3) as Multiword<2>; let s = a % b; s[0] }"
        ),
        5
    );
}

#[test]
fn narrow_declared_long_division_masks_remainder_on_wide_runtime() {
    // Audit D6: the bit-serial long division shifts the running remainder left
    // by one on every step, and on the 64-bit runtime a declared-16-bit limb is
    // not physically masked (unlike the i16 narrow runtime, where the limb type
    // wraps at 16 bits). If a shifted remainder limb kept bits above the
    // declared width, the wide-runtime result would diverge from the narrow
    // runtime. Cross-check every result limb of several large-quotient and
    // high-limb-divisor divisions against the physically-16-bit i16 runtime,
    // the correct 16-bit oracle. Agreement demonstrates the lowering keeps the
    // limbs masked across iterations.
    let cases = [
        // Large quotient (~30-bit dividend), many remainder shifts.
        "fn main() -> Word { let a = (0, 12345) as Multiword<2>; let b = (7, 0) as Multiword<2>; let s = a / b; s[0] }",
        "fn main() -> Word { let a = (0, 12345) as Multiword<2>; let b = (7, 0) as Multiword<2>; let s = a / b; s[1] }",
        "fn main() -> Word { let a = (0, 12345) as Multiword<2>; let b = (7, 0) as Multiword<2>; let s = a % b; s[0] }",
        // High-limb divisor near the sign bit, driving a large remainder that
        // shifts its top bit repeatedly.
        "fn main() -> Word { let a = (65535, 32766) as Multiword<2>; let b = (0, 32767) as Multiword<2>; let s = a / b; s[0] }",
        "fn main() -> Word { let a = (65535, 32766) as Multiword<2>; let b = (0, 32767) as Multiword<2>; let s = a % b; s[0] }",
        "fn main() -> Word { let a = (65535, 32766) as Multiword<2>; let b = (0, 32767) as Multiword<2>; let s = a % b; s[1] }",
        // A three-word division at the narrow width.
        "fn main() -> Word { let a = (1, 2, 40000) as Multiword<3>; let b = (0, 0, 7) as Multiword<3>; let s = a / b; s[0] }",
        "fn main() -> Word { let a = (1, 2, 40000) as Multiword<3>; let b = (0, 0, 7) as Multiword<3>; let s = a % b; s[2] }",
    ];
    for src in cases {
        assert_eq!(
            run_decl16_on_wide(src),
            run_i16(src) as i64,
            "wide-runtime long division diverged from the 16-bit oracle for: {src}"
        );
    }
}

#[test]
fn narrow_multiword_fixed_div() {
    // Q8.8 at i16: 6.0 / 2.0 = 3.0. 6.0 = 1536, 2.0 = 512, and the
    // pre-shifted division (1536 lsl 8) / 512 = 768 = 3.0. Confirms the
    // dividend pre-shift at the narrow width.
    assert_eq!(
        run_i16(
            "fn main() -> Word { let a = (1536, 0) as Multiword<2, 8>; let b = (512, 0) as Multiword<2, 8>; let s = a / b; s[0] }"
        ),
        768_i16
    );
    // Fixed-point modulo keeps the scale: 5.5 % 2.0 = 1.5 = 384.
    assert_eq!(
        run_i16(
            "fn main() -> Word { let a = (1408, 0) as Multiword<2, 8>; let b = (512, 0) as Multiword<2, 8>; let s = a % b; s[0] }"
        ),
        384_i16
    );
}

#[test]
fn narrow_scalar_word_shifts() {
    // Word shifts at the i16 width, assembly-mnemonic keyword shifts.
    assert_eq!(run_i16("fn main() -> Word { 5 lsl 2 }"), 20_i16);
    // Arithmetic right shift `asr` preserves the sign.
    assert_eq!(
        run_i16("fn main() -> Word { let x = 0 - 8; x asr 1 }"),
        -4_i16
    );
    // Logical right shift `lsr` zero-fills: -8 = 0xFFF8, lsr 1 = 0x7FFC = 32764.
    assert_eq!(
        run_i16("fn main() -> Word { let x = 0 - 8; x lsr 1 }"),
        32764_i16
    );
}

#[test]
fn narrow_multiword_shift_arithmetic_vs_logical() {
    // (0, -1) is -2^16 at i16. Arithmetic `asr` 1 gives -2^15 = (i16::MIN,
    // -1); logical `lsr` 1 gives (i16::MIN, i16::MAX).
    assert_eq!(
        run_i16("fn main() -> Word { let m = (0, -1) as Multiword<2>; let s = m asr 1; s[1] }"),
        -1_i16
    );
    assert_eq!(
        run_i16("fn main() -> Word { let m = (0, -1) as Multiword<2>; let s = m lsr 1; s[1] }"),
        i16::MAX
    );
    assert_eq!(
        run_i16("fn main() -> Word { let m = (0, -1) as Multiword<2>; let s = m lsr 1; s[0] }"),
        i16::MIN
    );
    // Cross-word left shift: (5, 0) lsl 16 = (0, 5).
    assert_eq!(
        run_i16("fn main() -> Word { let m = (5, 0) as Multiword<2>; let s = m lsl 16; s[1] }"),
        5_i16
    );
}

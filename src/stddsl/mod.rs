//! Standard DSL libraries packaged as host-registerable bundles.
//!
//! Each bundle is a unit struct implementing the [`Library`]
//! trait. Hosts register a bundle through
//! [`crate::vm::Vm::register_library`]:
//!
//! ```ignore
//! use keleusma::stddsl;
//! vm.register_library(stddsl::Math);
//! vm.register_library(stddsl::Audio);
//! vm.register_library(stddsl::Text);
//! ```
//!
//! ## Available libraries
//!
//! - [`Math`] - pure floating-point math routines and named
//!   constants under the `math::` namespace.
//! - [`Audio`] - digital signal processing helpers under the
//!   `audio::` namespace. The Audio bundle does not register
//!   `math::` entries; a host script that needs both should
//!   register Math and Audio.
//! - [`Text`] - text utilities (`to_string`, `length`, `concat`,
//!   `slice`, `println`). Requires the `text` cargo feature on the
//!   library; the script-side surface for text is also gated on
//!   the same feature.
//! - [`Shell`] - shell-script utilities (`shell::getenv`,
//!   `shell::run`, `shell::run_checked`, `shell::exit`). Requires
//!   the `shell` cargo feature, which adds a `std` dependency.
//!   `shell` is incompatible with the no_std build profile.
//!
//! ## Single-file scripts
//!
//! Keleusma scripts are necessarily single-file. There is no
//! `import` or `mod` mechanism inside the language; cross-script
//! reuse is intentionally outside the scope of the V0.2 surface.
//! If your application's needs grow to where you find yourself
//! wishing for modularisation, the recommended path is to roll a
//! custom DSL library: implement [`Library`] on a host-side unit
//! struct that registers the natives your scripts call, and let
//! every script consume the same vocabulary through `use`
//! declarations. The host-side library is the unit of reuse,
//! not the script.

extern crate alloc;

use crate::address::Address;
use crate::float::Float;
use crate::vm::{GenericVm, Vm};
use crate::word::Word;

/// Host-registerable bundle of native functions.
///
/// A `Library` registers a related set of native functions on a
/// VM. Implementors are typically unit structs in the `stddsl`
/// module or in host crates that want to ship their own bundles.
/// The trait method takes `self` by value so unit structs can be
/// dropped after registration.
///
/// Hosts call [`GenericVm::register_library`] which delegates to
/// [`Library::register`]; the trait is the extensibility surface
/// for third-party bundles.
///
/// The trait is parametric over the runtime's word, address, and
/// float types so library authors can opt their bundles into
/// narrow-runtime support. The standard bundles ([`Math`],
/// [`Audio`], [`Text`], [`Shell`]) are presently implemented only
/// for the default `(i64, u64, f64)` shape.
pub trait Library<W: Word, A: Address, F: Float> {
    /// Register every native function in this bundle on `vm`.
    fn register<'a, 'arena>(self, vm: &mut GenericVm<'a, 'arena, W, A, F>);
}

/// Pure floating-point math routines and named constants.
///
/// Registers the following entries under the `math::` namespace.
///
/// Algebraic and rounding routines: `sqrt`, `pow`, `abs`, `sign`,
/// `floor`, `ceil`, `round`, `trunc`, `fmod`, `hypot`, `min`,
/// `max`, `clamp`, `lerp`.
///
/// Trigonometric routines: `sin`, `cos`, `tan`, `asin`, `acos`,
/// `atan`, `atan2`, `tanh`.
///
/// Exponential and logarithmic routines: `exp`, `ln`, `log10`,
/// `log2`.
///
/// Zero-argument constant accessors: `pi`, `tau`, `e`, `sqrt_2`,
/// `ln_2`, `ln_10`.
///
/// Backed by `libm` and `core::f64::consts` so the bundle works
/// in `no_std` builds.
pub struct Math;

/// Digital signal processing helpers for audio applications.
///
/// Registers the following entries under the `audio::` namespace.
///
/// Pitch conversion: `midi_to_freq`, `freq_to_midi`,
/// `cents_to_ratio`, `ratio_to_cents`, `semitones_to_ratio`,
/// `ratio_to_semitones`.
///
/// Amplitude conversion: `db_to_linear`, `linear_to_db`.
///
/// Time conversion: `ms_to_samples`, `samples_to_ms`.
///
/// Filter coefficient helpers: `onepole_lpf_alpha`,
/// `onepole_hpf_alpha`.
///
/// Spatial helper: `pan_law` returning an equal-power
/// `(left, right)` gain pair.
///
/// The Audio bundle does not register entries under the `math::`
/// namespace. A host script that needs both audio and math
/// helpers should register the [`Math`] bundle as well. Backed by
/// `libm`.
pub struct Audio;

/// Text utilities. Requires the `text` cargo feature.
///
/// Registers `to_string`, `length`, `concat`, `slice`, `println`.
/// The Text bundle is a no-op when the `text` feature is
/// disabled; scripts that compile without the feature cannot
/// produce text values for these natives to operate on.
pub struct Text;

/// Shell-script utilities. Requires the `shell` cargo feature.
///
/// Registers `shell::getenv` (returns `Option<Text>`),
/// `shell::run` (returns `(Word, Text)`), `shell::run_checked`
/// (returns `Text`, traps on non-zero exit), and `shell::exit`
/// (terminates the host process).
///
/// The Shell bundle is unavailable when the `shell` feature is
/// disabled because it depends on `std::process` and `std::env`.
/// The Vm cannot be constructed in `no_std` mode with the
/// `shell` feature enabled.
pub struct Shell;

impl<W: Word, A: Address> Library<W, A, f64> for Math {
    fn register<'a, 'arena>(self, vm: &mut GenericVm<'a, 'arena, W, A, f64>) {
        math::register(vm);
    }
}

impl<W: Word, A: Address> Library<W, A, f64> for Audio {
    fn register<'a, 'arena>(self, vm: &mut GenericVm<'a, 'arena, W, A, f64>) {
        crate::audio_natives::register_audio_natives(vm);
    }
}

impl Library<i64, u64, f64> for Text {
    fn register<'a, 'arena>(self, vm: &mut Vm<'a, 'arena>) {
        text::register(vm);
    }
}

#[cfg(feature = "shell")]
impl Library<i64, u64, f64> for Shell {
    fn register<'a, 'arena>(self, vm: &mut Vm<'a, 'arena>) {
        shell::register(vm);
    }
}

mod math {
    extern crate alloc;
    use alloc::string::String;
    use core::f64::consts;

    use crate::address::Address;
    use crate::vm::{GenericVm, VmError};
    use crate::word::Word;

    pub fn register<'a, 'arena, W: Word, A: Address>(vm: &mut GenericVm<'a, 'arena, W, A, f64>) {
        // Algebraic and rounding routines.
        vm.register_fn("math::sqrt", |x: f64| -> f64 { libm::sqrt(x) });
        vm.register_fn("math::pow", |base: f64, exp: f64| -> f64 {
            libm::pow(base, exp)
        });
        vm.register_fn("math::abs", |x: f64| -> f64 { libm::fabs(x) });
        vm.register_fn("math::sign", |x: f64| -> f64 {
            // Branchless on the not-NaN path. Preserves -0.0 as 0.0
            // and propagates NaN.
            if x.is_nan() {
                f64::NAN
            } else if x > 0.0 {
                1.0
            } else if x < 0.0 {
                -1.0
            } else {
                0.0
            }
        });
        vm.register_fn("math::floor", |x: f64| -> f64 { libm::floor(x) });
        vm.register_fn("math::ceil", |x: f64| -> f64 { libm::ceil(x) });
        vm.register_fn("math::round", |x: f64| -> f64 { libm::round(x) });
        vm.register_fn("math::trunc", |x: f64| -> f64 { libm::trunc(x) });
        vm.register_fn_fallible("math::fmod", |x: f64, y: f64| -> Result<f64, VmError> {
            if y == 0.0 {
                return Err(VmError::NativeError(String::from(
                    "math::fmod: divisor must be non-zero",
                )));
            }
            Ok(libm::fmod(x, y))
        });
        vm.register_fn("math::hypot", |x: f64, y: f64| -> f64 { libm::hypot(x, y) });
        vm.register_fn("math::min", |a: f64, b: f64| -> f64 { libm::fmin(a, b) });
        vm.register_fn("math::max", |a: f64, b: f64| -> f64 { libm::fmax(a, b) });
        vm.register_fn("math::clamp", |val: f64, min: f64, max: f64| -> f64 {
            if val < min {
                min
            } else if val > max {
                max
            } else {
                val
            }
        });
        vm.register_fn("math::lerp", |a: f64, b: f64, t: f64| -> f64 {
            a + (b - a) * t
        });

        // Trigonometric routines. `tanh` is grouped with the
        // trigonometric block because it is the standard
        // hyperbolic shaping primitive used alongside sine and
        // cosine in audio waveshapers.
        vm.register_fn("math::sin", |x: f64| -> f64 { libm::sin(x) });
        vm.register_fn("math::cos", |x: f64| -> f64 { libm::cos(x) });
        vm.register_fn("math::tan", |x: f64| -> f64 { libm::tan(x) });
        vm.register_fn_fallible("math::asin", |x: f64| -> Result<f64, VmError> {
            if !(-1.0..=1.0).contains(&x) {
                return Err(VmError::NativeError(String::from(
                    "math::asin: argument must lie in [-1, 1]",
                )));
            }
            Ok(libm::asin(x))
        });
        vm.register_fn_fallible("math::acos", |x: f64| -> Result<f64, VmError> {
            if !(-1.0..=1.0).contains(&x) {
                return Err(VmError::NativeError(String::from(
                    "math::acos: argument must lie in [-1, 1]",
                )));
            }
            Ok(libm::acos(x))
        });
        vm.register_fn("math::atan", |x: f64| -> f64 { libm::atan(x) });
        vm.register_fn("math::atan2", |y: f64, x: f64| -> f64 { libm::atan2(y, x) });
        vm.register_fn("math::tanh", |x: f64| -> f64 { libm::tanh(x) });

        // Exponential and logarithmic routines.
        vm.register_fn("math::exp", |x: f64| -> f64 { libm::exp(x) });
        vm.register_fn_fallible("math::ln", |x: f64| -> Result<f64, VmError> {
            if x <= 0.0 {
                return Err(VmError::NativeError(String::from(
                    "math::ln: argument must be strictly positive",
                )));
            }
            Ok(libm::log(x))
        });
        vm.register_fn_fallible("math::log10", |x: f64| -> Result<f64, VmError> {
            if x <= 0.0 {
                return Err(VmError::NativeError(String::from(
                    "math::log10: argument must be strictly positive",
                )));
            }
            Ok(libm::log10(x))
        });
        vm.register_fn_fallible("math::log2", |x: f64| -> Result<f64, VmError> {
            if x <= 0.0 {
                return Err(VmError::NativeError(String::from(
                    "math::log2: argument must be strictly positive",
                )));
            }
            Ok(libm::log2(x))
        });

        // Named constants, exposed as zero-argument functions.
        // The pi, tau, and e constants are universally agreed.
        // The sqrt_2 entry serves the common diagonal-norm idiom;
        // ln_2 and ln_10 support manual change-of-base operations
        // against the script-side natural log.
        vm.register_fn("math::pi", || -> f64 { consts::PI });
        vm.register_fn("math::tau", || -> f64 { consts::TAU });
        vm.register_fn("math::e", || -> f64 { consts::E });
        vm.register_fn("math::sqrt_2", || -> f64 { consts::SQRT_2 });
        vm.register_fn("math::ln_2", || -> f64 { consts::LN_2 });
        vm.register_fn("math::ln_10", || -> f64 { consts::LN_10 });
    }
}

mod text {
    use crate::vm::Vm;
    pub fn register<'a, 'arena>(vm: &mut Vm<'a, 'arena>) {
        // Delegate to the existing utility_natives bundle which
        // registers the arena-aware `to_string`, `concat`,
        // `slice`, `length`, and `println`. The math::* entries
        // formerly registered alongside these have been moved to
        // the Math bundle; this delegate now installs only the
        // text-shaped utilities.
        crate::utility_natives::register_utility_natives(vm);
    }
}

#[cfg(feature = "shell")]
pub mod shell;

#[cfg(all(test, feature = "compile", feature = "verify"))]
mod tests {
    use super::*;
    use crate::bytecode::Value;
    use crate::compiler::compile;
    use crate::lexer::tokenize;
    use crate::parser::parse;
    use crate::vm::{DEFAULT_ARENA_CAPACITY, VmState};

    /// Run a Keleusma program with the Math bundle registered and
    /// return the result. Test helper local to the Math bundle
    /// tests below.
    fn run_with_math(src: &str) -> Value {
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        vm.register_library(Math);
        match vm.call(&[]).unwrap() {
            VmState::Finished(v) => v,
            VmState::Yielded(v) => panic!("unexpected yield: {:?}", v),
            VmState::Reset => panic!("unexpected reset"),
        }
    }

    fn assert_close(val: Value, expected: f64, tol: f64) {
        match val {
            Value::Float(f) => assert!(
                (f - expected).abs() < tol,
                "expected ~{}, got {}",
                expected,
                f
            ),
            other => panic!("expected Float, got {:?}", other),
        }
    }

    // -- Algebraic and rounding routines --

    #[test]
    fn math_sqrt() {
        assert_close(
            run_with_math("use math::sqrt\nfn main() -> Float { math::sqrt(9.0) }"),
            3.0,
            1e-9,
        );
    }

    #[test]
    fn math_pow() {
        assert_close(
            run_with_math("use math::pow\nfn main() -> Float { math::pow(2.0, 10.0) }"),
            1024.0,
            1e-9,
        );
    }

    #[test]
    fn math_abs() {
        assert_close(
            run_with_math("use math::abs\nfn main() -> Float { math::abs(-3.25) }"),
            3.25,
            1e-9,
        );
    }

    #[test]
    fn math_sign_positive() {
        assert_close(
            run_with_math("use math::sign\nfn main() -> Float { math::sign(7.5) }"),
            1.0,
            1e-9,
        );
    }

    #[test]
    fn math_sign_negative() {
        assert_close(
            run_with_math("use math::sign\nfn main() -> Float { math::sign(-0.001) }"),
            -1.0,
            1e-9,
        );
    }

    #[test]
    fn math_sign_zero() {
        assert_close(
            run_with_math("use math::sign\nfn main() -> Float { math::sign(0.0) }"),
            0.0,
            1e-12,
        );
    }

    #[test]
    fn math_floor() {
        assert_close(
            run_with_math("use math::floor\nfn main() -> Float { math::floor(3.7) }"),
            3.0,
            1e-9,
        );
    }

    #[test]
    fn math_ceil() {
        assert_close(
            run_with_math("use math::ceil\nfn main() -> Float { math::ceil(3.2) }"),
            4.0,
            1e-9,
        );
    }

    #[test]
    fn math_round() {
        assert_close(
            run_with_math("use math::round\nfn main() -> Float { math::round(3.5) }"),
            4.0,
            1e-9,
        );
    }

    #[test]
    fn math_trunc_positive() {
        assert_close(
            run_with_math("use math::trunc\nfn main() -> Float { math::trunc(3.7) }"),
            3.0,
            1e-9,
        );
    }

    #[test]
    fn math_trunc_negative() {
        assert_close(
            run_with_math("use math::trunc\nfn main() -> Float { math::trunc(-3.7) }"),
            -3.0,
            1e-9,
        );
    }

    #[test]
    fn math_fmod() {
        assert_close(
            run_with_math("use math::fmod\nfn main() -> Float { math::fmod(7.5, 2.0) }"),
            1.5,
            1e-9,
        );
    }

    #[test]
    fn math_hypot() {
        assert_close(
            run_with_math("use math::hypot\nfn main() -> Float { math::hypot(3.0, 4.0) }"),
            5.0,
            1e-9,
        );
    }

    #[test]
    fn math_min_max_clamp_lerp() {
        assert_close(
            run_with_math(
                "use math::min\nuse math::max\nfn main() -> Float { math::min(10.0, math::max(3.0, 5.0)) }",
            ),
            5.0,
            1e-9,
        );
        assert_close(
            run_with_math("use math::clamp\nfn main() -> Float { math::clamp(5.0, 0.0, 1.0) }"),
            1.0,
            1e-9,
        );
        assert_close(
            run_with_math("use math::lerp\nfn main() -> Float { math::lerp(0.0, 100.0, 0.25) }"),
            25.0,
            1e-9,
        );
    }

    // -- Trigonometric routines --

    #[test]
    fn math_sin_cos_tan() {
        assert_close(
            run_with_math("use math::sin\nfn main() -> Float { math::sin(0.0) }"),
            0.0,
            1e-9,
        );
        assert_close(
            run_with_math("use math::cos\nfn main() -> Float { math::cos(0.0) }"),
            1.0,
            1e-9,
        );
        assert_close(
            run_with_math("use math::tan\nfn main() -> Float { math::tan(0.0) }"),
            0.0,
            1e-9,
        );
    }

    #[test]
    fn math_asin_acos_atan() {
        assert_close(
            run_with_math("use math::asin\nfn main() -> Float { math::asin(1.0) }"),
            core::f64::consts::FRAC_PI_2,
            1e-9,
        );
        assert_close(
            run_with_math("use math::acos\nfn main() -> Float { math::acos(0.0) }"),
            core::f64::consts::FRAC_PI_2,
            1e-9,
        );
        assert_close(
            run_with_math("use math::atan\nfn main() -> Float { math::atan(1.0) }"),
            core::f64::consts::FRAC_PI_4,
            1e-9,
        );
    }

    #[test]
    fn math_atan2_quadrants() {
        assert_close(
            run_with_math("use math::atan2\nfn main() -> Float { math::atan2(1.0, 1.0) }"),
            core::f64::consts::FRAC_PI_4,
            1e-9,
        );
        assert_close(
            run_with_math("use math::atan2\nfn main() -> Float { math::atan2(1.0, -1.0) }"),
            3.0 * core::f64::consts::FRAC_PI_4,
            1e-9,
        );
    }

    #[test]
    fn math_tanh_zero_and_large() {
        assert_close(
            run_with_math("use math::tanh\nfn main() -> Float { math::tanh(0.0) }"),
            0.0,
            1e-12,
        );
        assert_close(
            run_with_math("use math::tanh\nfn main() -> Float { math::tanh(100.0) }"),
            1.0,
            1e-9,
        );
    }

    // -- Exponential and logarithmic routines --

    #[test]
    fn math_exp_zero_and_one() {
        assert_close(
            run_with_math("use math::exp\nfn main() -> Float { math::exp(0.0) }"),
            1.0,
            1e-9,
        );
        assert_close(
            run_with_math("use math::exp\nfn main() -> Float { math::exp(1.0) }"),
            core::f64::consts::E,
            1e-9,
        );
    }

    #[test]
    fn math_ln_e() {
        assert_close(
            run_with_math("use math::ln\nfn main() -> Float { math::ln(2.718281828459045) }"),
            1.0,
            1e-12,
        );
    }

    #[test]
    fn math_log10_and_log2() {
        assert_close(
            run_with_math("use math::log10\nfn main() -> Float { math::log10(1000.0) }"),
            3.0,
            1e-9,
        );
        assert_close(
            run_with_math("use math::log2\nfn main() -> Float { math::log2(8.0) }"),
            3.0,
            1e-9,
        );
    }

    // -- Domain errors --

    #[test]
    fn math_asin_domain_error() {
        let tokens = tokenize("use math::asin\nfn main() -> Float { math::asin(2.0) }").unwrap();
        let program = parse(&tokens).unwrap();
        let module = compile(&program).unwrap();
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        vm.register_library(Math);
        assert!(vm.call(&[]).is_err());
    }

    #[test]
    fn math_ln_nonpositive_error() {
        let tokens = tokenize("use math::ln\nfn main() -> Float { math::ln(0.0) }").unwrap();
        let program = parse(&tokens).unwrap();
        let module = compile(&program).unwrap();
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        vm.register_library(Math);
        assert!(vm.call(&[]).is_err());
    }

    #[test]
    fn math_fmod_zero_divisor_error() {
        let tokens =
            tokenize("use math::fmod\nfn main() -> Float { math::fmod(1.0, 0.0) }").unwrap();
        let program = parse(&tokens).unwrap();
        let module = compile(&program).unwrap();
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        vm.register_library(Math);
        assert!(vm.call(&[]).is_err());
    }

    // -- Constants --

    #[test]
    fn math_pi() {
        assert_close(
            run_with_math("use math::pi\nfn main() -> Float { math::pi() }"),
            core::f64::consts::PI,
            1e-15,
        );
    }

    #[test]
    fn math_tau() {
        assert_close(
            run_with_math("use math::tau\nfn main() -> Float { math::tau() }"),
            core::f64::consts::TAU,
            1e-15,
        );
    }

    #[test]
    fn math_e_constant() {
        assert_close(
            run_with_math("use math::e\nfn main() -> Float { math::e() }"),
            core::f64::consts::E,
            1e-15,
        );
    }

    #[test]
    fn math_sqrt_2_constant() {
        assert_close(
            run_with_math("use math::sqrt_2\nfn main() -> Float { math::sqrt_2() }"),
            core::f64::consts::SQRT_2,
            1e-15,
        );
    }

    #[test]
    fn math_ln_2_constant() {
        assert_close(
            run_with_math("use math::ln_2\nfn main() -> Float { math::ln_2() }"),
            core::f64::consts::LN_2,
            1e-15,
        );
    }

    #[test]
    fn math_ln_10_constant() {
        assert_close(
            run_with_math("use math::ln_10\nfn main() -> Float { math::ln_10() }"),
            core::f64::consts::LN_10,
            1e-15,
        );
    }
}

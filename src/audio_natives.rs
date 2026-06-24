//! Built-in audio native functions for digital signal processing.
//!
//! All functions are registered through the ergonomic `register_fn`
//! marshalling layer documented in `marshall.rs`. Argument and return
//! types are ordinary Rust primitives. Fallible functions return
//! `Result<R, VmError>` and are registered with `register_fn_fallible`.
//!
//! All entries live under the `audio::` namespace. The bundle does
//! not register entries under the `math::` namespace; hosts that
//! need math helpers alongside audio helpers should register
//! [`crate::stddsl::Math`] in addition to [`crate::stddsl::Audio`].

extern crate alloc;
use alloc::string::String;
use core::f64::consts;

use crate::address::Address;
use crate::float::Float;
use crate::vm::{GenericVm, VmError};
use crate::word::Word;

/// Register all audio native functions on a VM instance.
///
/// These are pure functions that do not require engine access.
/// They are available under the `audio::` namespace.
///
/// Parametric over the full `(W, A, F)` triple. The closures use
/// the universal `KeleusmaType<W, F> for i64` and `for f64` impls
/// to bridge the host's Rust signatures to the runtime's script
/// word and float types. When `F = f32`, every `f64`-typed
/// closure argument and return value passes through
/// `Float::from_f64` and `Float::to_f64` at the boundary; this
/// narrows constants and intermediates and is documented as a
/// known precision tradeoff for narrow-float runtimes.
pub fn register_audio_natives<'a, 'arena, W: Word, A: Address, F: Float>(
    vm: &mut GenericVm<'a, 'arena, W, A, F>,
) {
    // -- Pitch conversion --

    // MIDI note to frequency. Standard formula 440 * 2^((note - 69) / 12).
    vm.register_fn("audio::midi_to_freq", |note: i64| -> f64 {
        440.0 * libm::pow(2.0, (note - 69) as f64 / 12.0)
    });

    // Frequency to MIDI. Fallible because frequency must be positive.
    vm.register_fn_fallible("audio::freq_to_midi", |freq: f64| -> Result<i64, VmError> {
        if freq <= 0.0 {
            return Err(VmError::NativeError(String::from(
                "audio::freq_to_midi: frequency must be positive",
            )));
        }
        let note = 69.0 + 12.0 * libm::log2(freq / 440.0);
        Ok(libm::round(note) as i64)
    });

    // Cents to frequency ratio: 2^(cents / 1200).
    vm.register_fn("audio::cents_to_ratio", |cents: f64| -> f64 {
        libm::pow(2.0, cents / 1200.0)
    });

    // Frequency ratio to cents. Fallible because the ratio must be
    // strictly positive for the logarithm.
    vm.register_fn_fallible(
        "audio::ratio_to_cents",
        |ratio: f64| -> Result<f64, VmError> {
            if ratio <= 0.0 {
                return Err(VmError::NativeError(String::from(
                    "audio::ratio_to_cents: ratio must be strictly positive",
                )));
            }
            Ok(1200.0 * libm::log2(ratio))
        },
    );

    // Semitones to frequency ratio: 2^(semitones / 12).
    vm.register_fn("audio::semitones_to_ratio", |semitones: f64| -> f64 {
        libm::pow(2.0, semitones / 12.0)
    });

    // Frequency ratio to semitones. Fallible because the ratio
    // must be strictly positive for the logarithm.
    vm.register_fn_fallible(
        "audio::ratio_to_semitones",
        |ratio: f64| -> Result<f64, VmError> {
            if ratio <= 0.0 {
                return Err(VmError::NativeError(String::from(
                    "audio::ratio_to_semitones: ratio must be strictly positive",
                )));
            }
            Ok(12.0 * libm::log2(ratio))
        },
    );

    // -- Amplitude conversion --

    // Decibels to linear amplitude. 10^(db / 20).
    vm.register_fn("audio::db_to_linear", |db: f64| -> f64 {
        libm::pow(10.0, db / 20.0)
    });

    // Linear amplitude to decibels. Fallible because amplitude
    // must be positive.
    vm.register_fn_fallible(
        "audio::linear_to_db",
        |linear: f64| -> Result<f64, VmError> {
            if linear <= 0.0 {
                return Err(VmError::NativeError(String::from(
                    "audio::linear_to_db: amplitude must be positive",
                )));
            }
            Ok(20.0 * libm::log10(linear))
        },
    );

    // -- Time conversion --

    // Milliseconds to samples at the given sample rate. Fallible
    // because the sample rate must be strictly positive.
    vm.register_fn_fallible(
        "audio::ms_to_samples",
        |ms: f64, sample_rate: f64| -> Result<f64, VmError> {
            if sample_rate <= 0.0 {
                return Err(VmError::NativeError(String::from(
                    "audio::ms_to_samples: sample_rate must be strictly positive",
                )));
            }
            Ok(ms * sample_rate / 1000.0)
        },
    );

    // Samples at the given sample rate to milliseconds. Fallible
    // because the sample rate must be strictly positive.
    vm.register_fn_fallible(
        "audio::samples_to_ms",
        |samples: f64, sample_rate: f64| -> Result<f64, VmError> {
            if sample_rate <= 0.0 {
                return Err(VmError::NativeError(String::from(
                    "audio::samples_to_ms: sample_rate must be strictly positive",
                )));
            }
            Ok(samples * 1000.0 / sample_rate)
        },
    );

    // -- Filter coefficient helpers --

    // One-pole low-pass filter coefficient.
    //
    // Returns `alpha` in the difference equation
    //   y[n] = y[n-1] + alpha * (x[n] - y[n-1])
    // such that the filter has a -3 dB point at `cutoff_hz`.
    // Derived from the standard RC analogy
    //   alpha = 1 - exp(-2*pi*cutoff / sample_rate).
    //
    // The script side multiplies the previous output by
    // `(1 - alpha)` and the current input by `alpha`. Fallible
    // because the sample rate must be strictly positive and the
    // cutoff must be non-negative.
    vm.register_fn_fallible(
        "audio::onepole_lpf_alpha",
        |cutoff_hz: f64, sample_rate: f64| -> Result<f64, VmError> {
            if sample_rate <= 0.0 {
                return Err(VmError::NativeError(String::from(
                    "audio::onepole_lpf_alpha: sample_rate must be strictly positive",
                )));
            }
            if cutoff_hz < 0.0 {
                return Err(VmError::NativeError(String::from(
                    "audio::onepole_lpf_alpha: cutoff_hz must be non-negative",
                )));
            }
            Ok(1.0 - libm::exp(-2.0 * consts::PI * cutoff_hz / sample_rate))
        },
    );

    // One-pole high-pass filter coefficient.
    //
    // Returns `alpha` for the complement-style high-pass
    //   lp[n] = lp[n-1] + (1 - alpha) * (x[n] - lp[n-1])
    //   hp[n] = x[n] - lp[n]
    // where `alpha = exp(-2*pi*cutoff / sample_rate)` is the
    // decay coefficient of the underlying one-pole. Fallible on
    // the same grounds as the LPF helper.
    vm.register_fn_fallible(
        "audio::onepole_hpf_alpha",
        |cutoff_hz: f64, sample_rate: f64| -> Result<f64, VmError> {
            if sample_rate <= 0.0 {
                return Err(VmError::NativeError(String::from(
                    "audio::onepole_hpf_alpha: sample_rate must be strictly positive",
                )));
            }
            if cutoff_hz < 0.0 {
                return Err(VmError::NativeError(String::from(
                    "audio::onepole_hpf_alpha: cutoff_hz must be non-negative",
                )));
            }
            Ok(libm::exp(-2.0 * consts::PI * cutoff_hz / sample_rate))
        },
    );

    // -- Spatial helper --

    // Equal-power pan law.
    //
    // Position is in [-1.0, 1.0] where -1.0 is full left, 0.0 is
    // centre, and +1.0 is full right. Values outside the range
    // are clamped. Returns a (left, right) gain pair drawn from
    //   theta = (pos + 1) * pi / 4
    //   left  = cos(theta)
    //   right = sin(theta)
    // Sum-of-squares of the returned gains is unity, giving the
    // textbook constant-power pan.
    vm.register_fn("audio::pan_law", |pos: f64| -> (f64, f64) {
        let clamped = pos.clamp(-1.0, 1.0);
        let theta = (clamped + 1.0) * consts::FRAC_PI_4;
        (libm::cos(theta), libm::sin(theta))
    });
}

#[cfg(all(test, feature = "compile", feature = "verify"))]
mod tests {
    use super::*;
    use crate::bytecode::Value;
    use crate::compiler::compile;
    use crate::lexer::tokenize;
    use crate::parser::parse;
    use crate::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};

    /// Run a Keleusma program with the Audio bundle registered
    /// and return the result. Tests that need math helpers should
    /// register the Math bundle alongside; the Audio bundle no
    /// longer pulls math into the namespace.
    fn run_with_audio(src: &str) -> Value {
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        register_audio_natives(&mut vm);
        match vm.call(&[]).unwrap() {
            // This helper serves tests whose program returns a scalar, for which
            // the value carries no arena reference and is safe to return after
            // this scope's `arena` drops. A program returning a composite (an
            // arena-resident flat body since B28 item 2 step 6B) must decode it
            // while the VM is alive through `run_with_audio_pair`/`Vm::decode`.
            VmState::Finished(v) => v,
            VmState::Yielded(v) => panic!("unexpected yield: {:?}", v),
            VmState::Reset => panic!("unexpected reset"),
            VmState::BreakpointHit { chunk, op } => {
                panic!("unexpected breakpoint at chunk {} op {}", chunk, op)
            }
        }
    }

    /// Run a program returning a `(Float, Float)` tuple and decode it through
    /// the context-aware `Vm::decode` while the VM is still alive.
    ///
    /// `Vm::decode` reads the flat body at the module-declared widths, which is
    /// the canonical layout a native composite result is packed with (B28 item
    /// 2 / B36). The arena-less, runtime-width `from_value` would misread the
    /// body on a narrow-float build, where the module float is four bytes but
    /// the host runtime float is eight; `Vm::decode` reads at the module width
    /// and widens to the runtime `f64`, so it is correct on every build.
    fn run_with_audio_pair(src: &str) -> (f64, f64) {
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        register_audio_natives(&mut vm);
        match vm.call(&[]).unwrap() {
            VmState::Finished(v) => vm
                .decode::<(f64, f64)>(&v)
                .expect("expected a (Float, Float) tuple"),
            other => panic!("expected a finished tuple, got {:?}", other),
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

    // -- Pitch conversion --

    #[test]
    fn midi_to_freq_a4() {
        assert_close(
            run_with_audio(
                "use audio::midi_to_freq\nfn main() -> Float { audio::midi_to_freq(69) }",
            ),
            440.0,
            0.01,
        );
    }

    #[test]
    fn midi_to_freq_c4() {
        assert_close(
            run_with_audio(
                "use audio::midi_to_freq\nfn main() -> Float { audio::midi_to_freq(60) }",
            ),
            261.6256,
            0.01,
        );
    }

    #[test]
    fn freq_to_midi_440() {
        let val = run_with_audio(
            "use audio::freq_to_midi\nfn main() -> Word { audio::freq_to_midi(440.0) }",
        );
        assert_eq!(val, Value::Int(69));
    }

    #[test]
    fn cents_to_ratio_one_octave() {
        assert_close(
            run_with_audio(
                "use audio::cents_to_ratio\nfn main() -> Float { audio::cents_to_ratio(1200.0) }",
            ),
            2.0,
            1e-9,
        );
    }

    #[test]
    fn ratio_to_cents_octave() {
        assert_close(
            run_with_audio(
                "use audio::ratio_to_cents\nfn main() -> Float { audio::ratio_to_cents(2.0) }",
            ),
            1200.0,
            1e-9,
        );
    }

    #[test]
    fn semitones_to_ratio_octave() {
        assert_close(
            run_with_audio(
                "use audio::semitones_to_ratio\nfn main() -> Float { audio::semitones_to_ratio(12.0) }",
            ),
            2.0,
            1e-9,
        );
    }

    #[test]
    fn ratio_to_semitones_octave() {
        assert_close(
            run_with_audio(
                "use audio::ratio_to_semitones\nfn main() -> Float { audio::ratio_to_semitones(2.0) }",
            ),
            12.0,
            1e-9,
        );
    }

    // -- Amplitude conversion --

    #[test]
    fn db_to_linear_zero() {
        assert_close(
            run_with_audio(
                "use audio::db_to_linear\nfn main() -> Float { audio::db_to_linear(0.0) }",
            ),
            1.0,
            1e-9,
        );
    }

    #[test]
    fn db_to_linear_minus6() {
        assert_close(
            run_with_audio(
                "use audio::db_to_linear\nfn main() -> Float { audio::db_to_linear(-6.0) }",
            ),
            0.501187,
            1e-4,
        );
    }

    #[test]
    fn linear_to_db_one() {
        assert_close(
            run_with_audio(
                "use audio::linear_to_db\nfn main() -> Float { audio::linear_to_db(1.0) }",
            ),
            0.0,
            1e-9,
        );
    }

    // -- Time conversion --

    #[test]
    fn ms_to_samples_round_trip() {
        // 1000 ms at 48000 Hz is 48000 samples.
        assert_close(
            run_with_audio(
                "use audio::ms_to_samples\nfn main() -> Float { audio::ms_to_samples(1000.0, 48000.0) }",
            ),
            48000.0,
            1e-9,
        );
    }

    #[test]
    fn samples_to_ms_round_trip() {
        // 48000 samples at 48000 Hz is 1000 ms.
        assert_close(
            run_with_audio(
                "use audio::samples_to_ms\nfn main() -> Float { audio::samples_to_ms(48000.0, 48000.0) }",
            ),
            1000.0,
            1e-9,
        );
    }

    // -- Filter coefficient helpers --

    #[test]
    fn onepole_lpf_alpha_at_nyquist_approaches_one() {
        // Sanity check: at cutoff equal to a substantial fraction
        // of sample rate, alpha approaches 1 (pass-through).
        let val = run_with_audio(
            "use audio::onepole_lpf_alpha\nfn main() -> Float { audio::onepole_lpf_alpha(10000.0, 48000.0) }",
        );
        match val {
            Value::Float(f) => {
                assert!(f > 0.0 && f < 1.0, "alpha out of range: {}", f);
            }
            other => panic!("expected Float, got {:?}", other),
        }
    }

    #[test]
    fn onepole_lpf_alpha_zero_cutoff_is_zero() {
        // alpha = 1 - exp(0) = 0.
        assert_close(
            run_with_audio(
                "use audio::onepole_lpf_alpha\nfn main() -> Float { audio::onepole_lpf_alpha(0.0, 48000.0) }",
            ),
            0.0,
            1e-12,
        );
    }

    #[test]
    fn onepole_hpf_alpha_zero_cutoff_is_one() {
        // alpha = exp(0) = 1.
        assert_close(
            run_with_audio(
                "use audio::onepole_hpf_alpha\nfn main() -> Float { audio::onepole_hpf_alpha(0.0, 48000.0) }",
            ),
            1.0,
            1e-12,
        );
    }

    // -- Spatial helper --

    #[test]
    fn pan_law_centre() {
        let (l, r) = run_with_audio_pair(
            "use audio::pan_law\nfn main() -> (Float, Float) { audio::pan_law(0.0) }",
        );
        let target = core::f64::consts::FRAC_1_SQRT_2;
        // The center gain is an irrational `1/sqrt(2)`. On a `narrow-float-32`
        // build the module float is `f32`, so the native's `f64` result is cast
        // to `f32` precision when packed into the tuple body and decoded back at
        // the module width, which differs from the `f64` target by about
        // `1e-8`; the tolerance loosens to an `f32`-appropriate bound there. On
        // the bundled `f64` runtime the value is exact to `f64` precision and
        // the tight bound holds (B36).
        #[cfg(feature = "narrow-float-32")]
        let tol = 1e-6;
        #[cfg(not(feature = "narrow-float-32"))]
        let tol = 1e-9;
        assert!(
            (l - target).abs() < tol,
            "left expected ~{}, got {}",
            target,
            l
        );
        assert!(
            (r - target).abs() < tol,
            "right expected ~{}, got {}",
            target,
            r
        );
    }

    #[test]
    fn pan_law_full_left() {
        let (l, r) = run_with_audio_pair(
            "use audio::pan_law\nfn main() -> (Float, Float) { audio::pan_law(-1.0) }",
        );
        assert!((l - 1.0).abs() < 1e-9, "left expected 1.0, got {}", l);
        assert!(r.abs() < 1e-9, "right expected 0.0, got {}", r);
    }

    #[test]
    fn pan_law_full_right() {
        let (l, r) = run_with_audio_pair(
            "use audio::pan_law\nfn main() -> (Float, Float) { audio::pan_law(1.0) }",
        );
        assert!(l.abs() < 1e-9, "left expected 0.0, got {}", l);
        assert!((r - 1.0).abs() < 1e-9, "right expected 1.0, got {}", r);
    }

    #[test]
    fn pan_law_clamps_out_of_range() {
        // pos = 2.0 clamps to 1.0, equivalent to full right.
        let (l, r) = run_with_audio_pair(
            "use audio::pan_law\nfn main() -> (Float, Float) { audio::pan_law(2.0) }",
        );
        assert!(l.abs() < 1e-9, "left expected 0.0, got {}", l);
        assert!((r - 1.0).abs() < 1e-9, "right expected 1.0, got {}", r);
    }

    // -- Error paths --

    #[test]
    fn freq_to_midi_nonpositive_error() {
        let tokens =
            tokenize("use audio::freq_to_midi\nfn main() -> Word { audio::freq_to_midi(0.0) }")
                .unwrap();
        let program = parse(&tokens).unwrap();
        let module = compile(&program).unwrap();
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        register_audio_natives(&mut vm);
        assert!(vm.call(&[]).is_err());
    }

    #[test]
    fn ms_to_samples_bad_sample_rate_error() {
        let tokens = tokenize(
            "use audio::ms_to_samples\nfn main() -> Float { audio::ms_to_samples(10.0, 0.0) }",
        )
        .unwrap();
        let program = parse(&tokens).unwrap();
        let module = compile(&program).unwrap();
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        register_audio_natives(&mut vm);
        assert!(vm.call(&[]).is_err());
    }

    #[test]
    fn onepole_lpf_alpha_negative_cutoff_error() {
        let tokens = tokenize(
            "use audio::onepole_lpf_alpha\nfn main() -> Float { audio::onepole_lpf_alpha(-1.0, 48000.0) }",
        )
        .unwrap();
        let program = parse(&tokens).unwrap();
        let module = compile(&program).unwrap();
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        register_audio_natives(&mut vm);
        assert!(vm.call(&[]).is_err());
    }
}

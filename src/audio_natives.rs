//! Built-in audio and math native functions.
//!
//! All functions are registered through the ergonomic `register_fn`
//! marshalling layer documented in `marshall.rs`. Argument and return
//! types are ordinary Rust primitives. Fallible functions return
//! `Result<R, VmError>` and are registered with `register_fn_fallible`.

extern crate alloc;
use alloc::string::String;

use crate::vm::{Vm, VmError};

/// Register all audio utility native functions on a VM instance.
///
/// These are pure functions that do not require engine access.
/// They are available under the `audio` and `math` namespaces.
pub fn register_audio_natives<'a, 'arena>(vm: &mut Vm<'a, 'arena>) {
    // MIDI note to frequency. Standard formula 440 * 2^((note - 69) / 12).
    vm.register_fn("audio::midi_to_freq", |note: i64| -> f64 {
        440.0 * libm::pow(2.0, (note - 69) as f64 / 12.0)
    });

    // Frequency to MIDI. Fallible because frequency must be positive.
    vm.register_fn_fallible("audio::freq_to_midi", |freq: f64| -> Result<i64, VmError> {
        if freq <= 0.0 {
            return Err(VmError::NativeError(String::from(
                "freq_to_midi: frequency must be positive",
            )));
        }
        let note = 69.0 + 12.0 * libm::log2(freq / 440.0);
        Ok(libm::round(note) as i64)
    });

    // Decibels to linear amplitude. 10^(db / 20).
    vm.register_fn("audio::db_to_linear", |db: f64| -> f64 {
        libm::pow(10.0, db / 20.0)
    });

    // Linear amplitude to decibels. Fallible because amplitude must be positive.
    vm.register_fn_fallible(
        "audio::linear_to_db",
        |linear: f64| -> Result<f64, VmError> {
            if linear <= 0.0 {
                return Err(VmError::NativeError(String::from(
                    "linear_to_db: amplitude must be positive",
                )));
            }
            Ok(20.0 * libm::log10(linear))
        },
    );

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

    vm.register_fn("math::sin", |x: f64| -> f64 { libm::sin(x) });
    vm.register_fn("math::cos", |x: f64| -> f64 { libm::cos(x) });
    vm.register_fn("math::pow", |base: f64, exp: f64| -> f64 {
        libm::pow(base, exp)
    });
    vm.register_fn("math::abs", |x: f64| -> f64 { libm::fabs(x) });
    vm.register_fn("math::min", |a: f64, b: f64| -> f64 { libm::fmin(a, b) });
    vm.register_fn("math::max", |a: f64, b: f64| -> f64 { libm::fmax(a, b) });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::Value;
    use crate::compiler::compile;
    use crate::lexer::tokenize;
    use crate::parser::parse;
    use crate::vm::DEFAULT_ARENA_CAPACITY;

    fn run_with_natives(src: &str) -> Value {
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        register_audio_natives(&mut vm);
        match vm.call(&[]).unwrap() {
            crate::vm::VmState::Finished(v) => v,
            crate::vm::VmState::Yielded(v) => panic!("unexpected yield: {:?}", v),
            crate::vm::VmState::Reset => panic!("unexpected reset"),
        }
    }

    #[test]
    fn midi_to_freq_a4() {
        let val = run_with_natives(
            "use audio::midi_to_freq\nfn main() -> f64 { audio::midi_to_freq(69) }",
        );
        match val {
            Value::Float(f) => assert!((f - 440.0).abs() < 0.01),
            other => panic!("expected Float, got {:?}", other),
        }
    }

    #[test]
    fn midi_to_freq_c4() {
        let val = run_with_natives(
            "use audio::midi_to_freq\nfn main() -> f64 { audio::midi_to_freq(60) }",
        );
        match val {
            Value::Float(f) => assert!((f - 261.63).abs() < 0.01),
            other => panic!("expected Float, got {:?}", other),
        }
    }

    #[test]
    fn freq_to_midi_440() {
        let val = run_with_natives(
            "use audio::freq_to_midi\nfn main() -> i64 { audio::freq_to_midi(440.0) }",
        );
        assert_eq!(val, Value::Int(69));
    }

    #[test]
    fn db_to_linear_zero() {
        let val = run_with_natives(
            "use audio::db_to_linear\nfn main() -> f64 { audio::db_to_linear(0.0) }",
        );
        match val {
            Value::Float(f) => assert!((f - 1.0).abs() < 0.001),
            other => panic!("expected Float, got {:?}", other),
        }
    }

    #[test]
    fn db_to_linear_minus6() {
        let val = run_with_natives(
            "use audio::db_to_linear\nfn main() -> f64 { audio::db_to_linear(-6.0) }",
        );
        match val {
            Value::Float(f) => assert!((f - 0.501).abs() < 0.01),
            other => panic!("expected Float, got {:?}", other),
        }
    }

    #[test]
    fn linear_to_db_one() {
        let val = run_with_natives(
            "use audio::linear_to_db\nfn main() -> f64 { audio::linear_to_db(1.0) }",
        );
        match val {
            Value::Float(f) => assert!(f.abs() < 0.001),
            other => panic!("expected Float, got {:?}", other),
        }
    }

    #[test]
    fn clamp_within_range() {
        let val =
            run_with_natives("use math::clamp\nfn main() -> f64 { math::clamp(0.5, 0.0, 1.0) }");
        match val {
            Value::Float(f) => assert!((f - 0.5).abs() < 0.001),
            other => panic!("expected Float, got {:?}", other),
        }
    }

    #[test]
    fn clamp_below_min() {
        let val =
            run_with_natives("use math::clamp\nfn main() -> f64 { math::clamp(-1.0, 0.0, 1.0) }");
        match val {
            Value::Float(f) => assert!(f.abs() < 0.001),
            other => panic!("expected Float, got {:?}", other),
        }
    }

    #[test]
    fn clamp_above_max() {
        let val =
            run_with_natives("use math::clamp\nfn main() -> f64 { math::clamp(5.0, 0.0, 1.0) }");
        match val {
            Value::Float(f) => assert!((f - 1.0).abs() < 0.001),
            other => panic!("expected Float, got {:?}", other),
        }
    }

    #[test]
    fn lerp_midpoint() {
        let val =
            run_with_natives("use math::lerp\nfn main() -> f64 { math::lerp(0.0, 100.0, 0.5) }");
        match val {
            Value::Float(f) => assert!((f - 50.0).abs() < 0.001),
            other => panic!("expected Float, got {:?}", other),
        }
    }

    #[test]
    fn sin_zero() {
        let val = run_with_natives("use math::sin\nfn main() -> f64 { math::sin(0.0) }");
        match val {
            Value::Float(f) => assert!(f.abs() < 0.001),
            other => panic!("expected Float, got {:?}", other),
        }
    }

    #[test]
    fn pow_two_cubed() {
        let val = run_with_natives("use math::pow\nfn main() -> f64 { math::pow(2.0, 3.0) }");
        match val {
            Value::Float(f) => assert!((f - 8.0).abs() < 0.001),
            other => panic!("expected Float, got {:?}", other),
        }
    }

    #[test]
    fn abs_negative() {
        let val = run_with_natives("use math::abs\nfn main() -> f64 { math::abs(-42.5) }");
        match val {
            Value::Float(f) => assert!((f - 42.5).abs() < 0.001),
            other => panic!("expected Float, got {:?}", other),
        }
    }

    #[test]
    fn min_max() {
        let val = run_with_natives(
            "use math::min\nuse math::max\nfn main() -> f64 { math::min(10.0, math::max(3.0, 5.0)) }",
        );
        match val {
            Value::Float(f) => assert!((f - 5.0).abs() < 0.001),
            other => panic!("expected Float, got {:?}", other),
        }
    }
}

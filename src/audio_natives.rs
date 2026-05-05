extern crate alloc;
use alloc::format;
use alloc::string::String;

use crate::bytecode::Value;
use crate::vm::{Vm, VmError};

/// Extract an f64 from a Value, accepting both Float and Int (with cast).
fn extract_f64(val: &Value) -> Result<f64, VmError> {
    match val {
        Value::Float(f) => Ok(*f),
        Value::Int(i) => Ok(*i as f64),
        other => Err(VmError::TypeError(format!(
            "expected f64 or i64, got {}",
            other.type_name()
        ))),
    }
}

/// Extract an i64 from a Value.
fn extract_i64(val: &Value) -> Result<i64, VmError> {
    match val {
        Value::Int(i) => Ok(*i),
        other => Err(VmError::TypeError(format!(
            "expected i64, got {}",
            other.type_name()
        ))),
    }
}

/// Convert a MIDI note number (0-127) to frequency in Hz.
///
/// Uses the standard formula: 440 * 2^((note - 69) / 12).
fn native_midi_to_freq(args: &[Value]) -> Result<Value, VmError> {
    if args.len() != 1 {
        return Err(VmError::NativeError(format!(
            "midi_to_freq: expected 1 argument, got {}",
            args.len()
        )));
    }
    let note = extract_i64(&args[0])?;
    let freq = 440.0 * libm::pow(2.0, (note - 69) as f64 / 12.0);
    Ok(Value::Float(freq))
}

/// Convert a frequency in Hz to the nearest MIDI note number.
///
/// Uses the inverse formula: 69 + 12 * log2(freq / 440).
fn native_freq_to_midi(args: &[Value]) -> Result<Value, VmError> {
    if args.len() != 1 {
        return Err(VmError::NativeError(format!(
            "freq_to_midi: expected 1 argument, got {}",
            args.len()
        )));
    }
    let freq = extract_f64(&args[0])?;
    if freq <= 0.0 {
        return Err(VmError::NativeError(String::from(
            "freq_to_midi: frequency must be positive",
        )));
    }
    let note = 69.0 + 12.0 * libm::log2(freq / 440.0);
    Ok(Value::Int(libm::round(note) as i64))
}

/// Convert decibels to linear amplitude.
///
/// Uses the formula: 10^(db / 20).
fn native_db_to_linear(args: &[Value]) -> Result<Value, VmError> {
    if args.len() != 1 {
        return Err(VmError::NativeError(format!(
            "db_to_linear: expected 1 argument, got {}",
            args.len()
        )));
    }
    let db = extract_f64(&args[0])?;
    let linear = libm::pow(10.0, db / 20.0);
    Ok(Value::Float(linear))
}

/// Convert linear amplitude to decibels.
///
/// Uses the formula: 20 * log10(linear).
fn native_linear_to_db(args: &[Value]) -> Result<Value, VmError> {
    if args.len() != 1 {
        return Err(VmError::NativeError(format!(
            "linear_to_db: expected 1 argument, got {}",
            args.len()
        )));
    }
    let linear = extract_f64(&args[0])?;
    if linear <= 0.0 {
        return Err(VmError::NativeError(String::from(
            "linear_to_db: amplitude must be positive",
        )));
    }
    let db = 20.0 * libm::log10(linear);
    Ok(Value::Float(db))
}

/// Clamp a value between min and max.
fn native_clamp(args: &[Value]) -> Result<Value, VmError> {
    if args.len() != 3 {
        return Err(VmError::NativeError(format!(
            "clamp: expected 3 arguments, got {}",
            args.len()
        )));
    }
    let val = extract_f64(&args[0])?;
    let min = extract_f64(&args[1])?;
    let max = extract_f64(&args[2])?;
    let clamped = if val < min {
        min
    } else if val > max {
        max
    } else {
        val
    };
    Ok(Value::Float(clamped))
}

/// Linear interpolation between two values.
///
/// lerp(a, b, t) = a + (b - a) * t
fn native_lerp(args: &[Value]) -> Result<Value, VmError> {
    if args.len() != 3 {
        return Err(VmError::NativeError(format!(
            "lerp: expected 3 arguments, got {}",
            args.len()
        )));
    }
    let a = extract_f64(&args[0])?;
    let b = extract_f64(&args[1])?;
    let t = extract_f64(&args[2])?;
    let result = a + (b - a) * t;
    Ok(Value::Float(result))
}

/// Compute sine of a value in radians.
fn native_sin(args: &[Value]) -> Result<Value, VmError> {
    if args.len() != 1 {
        return Err(VmError::NativeError(format!(
            "sin: expected 1 argument, got {}",
            args.len()
        )));
    }
    let x = extract_f64(&args[0])?;
    Ok(Value::Float(libm::sin(x)))
}

/// Compute cosine of a value in radians.
fn native_cos(args: &[Value]) -> Result<Value, VmError> {
    if args.len() != 1 {
        return Err(VmError::NativeError(format!(
            "cos: expected 1 argument, got {}",
            args.len()
        )));
    }
    let x = extract_f64(&args[0])?;
    Ok(Value::Float(libm::cos(x)))
}

/// Raise base to the power of exponent.
fn native_pow(args: &[Value]) -> Result<Value, VmError> {
    if args.len() != 2 {
        return Err(VmError::NativeError(format!(
            "pow: expected 2 arguments, got {}",
            args.len()
        )));
    }
    let base = extract_f64(&args[0])?;
    let exp = extract_f64(&args[1])?;
    Ok(Value::Float(libm::pow(base, exp)))
}

/// Compute the absolute value of a float.
fn native_abs(args: &[Value]) -> Result<Value, VmError> {
    if args.len() != 1 {
        return Err(VmError::NativeError(format!(
            "abs: expected 1 argument, got {}",
            args.len()
        )));
    }
    let x = extract_f64(&args[0])?;
    Ok(Value::Float(libm::fabs(x)))
}

/// Compute the minimum of two floats.
fn native_min(args: &[Value]) -> Result<Value, VmError> {
    if args.len() != 2 {
        return Err(VmError::NativeError(format!(
            "min: expected 2 arguments, got {}",
            args.len()
        )));
    }
    let a = extract_f64(&args[0])?;
    let b = extract_f64(&args[1])?;
    Ok(Value::Float(libm::fmin(a, b)))
}

/// Compute the maximum of two floats.
fn native_max(args: &[Value]) -> Result<Value, VmError> {
    if args.len() != 2 {
        return Err(VmError::NativeError(format!(
            "max: expected 2 arguments, got {}",
            args.len()
        )));
    }
    let a = extract_f64(&args[0])?;
    let b = extract_f64(&args[1])?;
    Ok(Value::Float(libm::fmax(a, b)))
}

/// Register all audio utility native functions on a VM instance.
///
/// These are pure functions that do not require engine access.
/// They are available under the `audio` and `math` namespaces.
pub fn register_audio_natives(vm: &mut Vm) {
    vm.register_native("audio::midi_to_freq", native_midi_to_freq);
    vm.register_native("audio::freq_to_midi", native_freq_to_midi);
    vm.register_native("audio::db_to_linear", native_db_to_linear);
    vm.register_native("audio::linear_to_db", native_linear_to_db);
    vm.register_native("math::clamp", native_clamp);
    vm.register_native("math::lerp", native_lerp);
    vm.register_native("math::sin", native_sin);
    vm.register_native("math::cos", native_cos);
    vm.register_native("math::pow", native_pow);
    vm.register_native("math::abs", native_abs);
    vm.register_native("math::min", native_min);
    vm.register_native("math::max", native_max);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::compile;
    use crate::lexer::tokenize;
    use crate::parser::parse;

    fn run_with_natives(src: &str) -> Value {
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let mut vm = Vm::new(module).unwrap();
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

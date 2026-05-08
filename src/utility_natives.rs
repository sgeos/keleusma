extern crate alloc;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::bytecode::Value;
use crate::vm::{Vm, VmError};

/// Convert any value to its string representation.
fn native_to_string(args: &[Value]) -> Result<Value, VmError> {
    if args.len() != 1 {
        return Err(VmError::NativeError(format!(
            "to_string: expected 1 argument, got {}",
            args.len()
        )));
    }
    let s = match &args[0] {
        Value::Int(n) => format!("{}", n),
        Value::Float(f) => format!("{}", f),
        Value::Bool(b) => format!("{}", b),
        Value::Str(s) => s.clone(),
        Value::Unit => String::from("()"),
        Value::None => String::from("None"),
        Value::Tuple(elems) => {
            let parts: Vec<String> = elems
                .iter()
                .map(|e| match native_to_string(core::slice::from_ref(e)) {
                    Ok(Value::Str(s)) => s,
                    _ => String::from("?"),
                })
                .collect();
            format!("({})", parts.join(", "))
        }
        Value::Array(elems) => {
            let parts: Vec<String> = elems
                .iter()
                .map(|e| match native_to_string(core::slice::from_ref(e)) {
                    Ok(Value::Str(s)) => s,
                    _ => String::from("?"),
                })
                .collect();
            format!("[{}]", parts.join(", "))
        }
        Value::Struct {
            type_name, fields, ..
        } => {
            let parts: Vec<String> = fields
                .iter()
                .map(|(name, val)| {
                    let vs = match native_to_string(core::slice::from_ref(val)) {
                        Ok(Value::Str(s)) => s,
                        _ => String::from("?"),
                    };
                    format!("{}: {}", name, vs)
                })
                .collect();
            format!("{} {{ {} }}", type_name, parts.join(", "))
        }
        Value::Enum {
            type_name,
            variant,
            fields,
        } => {
            if fields.is_empty() {
                format!("{}::{}", type_name, variant)
            } else {
                let parts: Vec<String> = fields
                    .iter()
                    .map(|e| match native_to_string(core::slice::from_ref(e)) {
                        Ok(Value::Str(s)) => s,
                        _ => String::from("?"),
                    })
                    .collect();
                format!("{}::{}({})", type_name, variant, parts.join(", "))
            }
        }
    };
    Ok(Value::Str(s))
}

/// Get the length of an array, string, or tuple.
fn native_length(args: &[Value]) -> Result<Value, VmError> {
    if args.len() != 1 {
        return Err(VmError::NativeError(format!(
            "length: expected 1 argument, got {}",
            args.len()
        )));
    }
    match &args[0] {
        Value::Array(arr) => Ok(Value::Int(arr.len() as i64)),
        Value::Str(s) => Ok(Value::Int(s.chars().count() as i64)),
        Value::Tuple(t) => Ok(Value::Int(t.len() as i64)),
        other => Err(VmError::TypeError(format!(
            "length: unsupported type {}",
            other.type_name()
        ))),
    }
}

/// Debug print a value. Returns Unit. In no_std this is a no-op; the host
/// can override with a closure using `register_native_closure` if output
/// is desired.
fn native_println(args: &[Value]) -> Result<Value, VmError> {
    if args.len() != 1 {
        return Err(VmError::NativeError(format!(
            "println: expected 1 argument, got {}",
            args.len()
        )));
    }
    // No-op in no_std. The value is consumed but not printed.
    Ok(Value::Unit)
}

/// Register all utility native functions on the VM.
///
/// Registers: `to_string`, `length`, `println`, `math::sqrt`, `math::floor`,
/// `math::ceil`, `math::round`, `math::log2`.
///
/// `to_string`, `length`, and `println` accept any `Value` variant and so
/// remain registered through `register_native`. The math functions take
/// fixed primitive types and use the ergonomic `register_fn` API.
pub fn register_utility_natives(vm: &mut Vm) {
    vm.register_native("to_string", native_to_string);
    vm.register_native("length", native_length);
    vm.register_native("println", native_println);

    vm.register_fn("math::sqrt", |x: f64| -> f64 { libm::sqrt(x) });
    vm.register_fn("math::floor", |x: f64| -> f64 { libm::floor(x) });
    vm.register_fn("math::ceil", |x: f64| -> f64 { libm::ceil(x) });
    vm.register_fn("math::round", |x: f64| -> f64 { libm::round(x) });
    vm.register_fn("math::log2", |x: f64| -> f64 { libm::log2(x) });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::compile;
    use crate::lexer::tokenize;
    use crate::parser::parse;
    use crate::vm::VmState;

    fn run_with_utilities(src: &str) -> Value {
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let mut vm = Vm::new(module).unwrap();
        register_utility_natives(&mut vm);
        match vm.call(&[]).unwrap() {
            VmState::Finished(v) => v,
            VmState::Yielded(v) => panic!("unexpected yield: {:?}", v),
            VmState::Reset => panic!("unexpected reset"),
        }
    }

    #[test]
    fn to_string_int() {
        let val = run_with_utilities("use to_string\nfn main() -> String { to_string(42) }");
        assert_eq!(val, Value::Str(String::from("42")));
    }

    #[test]
    fn to_string_float() {
        let val = run_with_utilities("use to_string\nfn main() -> String { to_string(3.14) }");
        if let Value::Str(s) = val {
            assert!(s.starts_with("3.14"));
        } else {
            panic!("expected Str");
        }
    }

    #[test]
    fn to_string_bool() {
        let val = run_with_utilities("use to_string\nfn main() -> String { to_string(true) }");
        assert_eq!(val, Value::Str(String::from("true")));
    }

    #[test]
    fn to_string_string() {
        let val = run_with_utilities("use to_string\nfn main() -> String { to_string(\"hello\") }");
        assert_eq!(val, Value::Str(String::from("hello")));
    }

    #[test]
    fn length_array() {
        let val = run_with_utilities("use length\nfn main() -> i64 { length([10, 20, 30]) }");
        assert_eq!(val, Value::Int(3));
    }

    #[test]
    fn length_string() {
        let val = run_with_utilities("use length\nfn main() -> i64 { length(\"hello\") }");
        assert_eq!(val, Value::Int(5));
    }

    #[test]
    fn length_tuple() {
        let val = run_with_utilities("use length\nfn main() -> i64 { length((1, 2, 3)) }");
        assert_eq!(val, Value::Int(3));
    }

    #[test]
    fn sqrt_value() {
        let val = run_with_utilities("use math::sqrt\nfn main() -> f64 { math::sqrt(9.0) }");
        assert_eq!(val, Value::Float(3.0));
    }

    #[test]
    fn floor_value() {
        let val = run_with_utilities("use math::floor\nfn main() -> f64 { math::floor(3.7) }");
        assert_eq!(val, Value::Float(3.0));
    }

    #[test]
    fn ceil_value() {
        let val = run_with_utilities("use math::ceil\nfn main() -> f64 { math::ceil(3.2) }");
        assert_eq!(val, Value::Float(4.0));
    }

    #[test]
    fn round_value() {
        let val = run_with_utilities("use math::round\nfn main() -> f64 { math::round(3.5) }");
        assert_eq!(val, Value::Float(4.0));
    }

    #[test]
    fn log2_value() {
        let val = run_with_utilities("use math::log2\nfn main() -> f64 { math::log2(8.0) }");
        assert_eq!(val, Value::Float(3.0));
    }
}

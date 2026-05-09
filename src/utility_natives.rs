extern crate alloc;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use keleusma_arena::{Arena, KString};

use crate::bytecode::Value;
use crate::vm::{NativeCtx, Vm, VmError};

/// Render a value to its string representation, optionally resolving
/// arena-backed `KStr` handles when an arena is supplied.
///
/// Helper shared by [`native_to_string`] and
/// [`native_to_string_with_ctx`]. With an arena, `KStr` handles
/// resolve to their UTF-8 contents through
/// [`keleusma_arena::KString::get`]; without an arena, `KStr` renders
/// as a placeholder marker.
fn render_value_to_string(arena: Option<&Arena>, val: &Value) -> String {
    match val {
        Value::Int(n) => format!("{}", n),
        Value::Float(f) => format!("{}", f),
        Value::Bool(b) => format!("{}", b),
        Value::StaticStr(s) | Value::DynStr(s) => s.clone(),
        Value::KStr(h) => match arena {
            Some(a) => match h.get(a) {
                Ok(s) => String::from(s),
                Err(_) => String::from("<stale>"),
            },
            None => String::from("<arena-string>"),
        },
        Value::Unit => String::from("()"),
        Value::None => String::from("None"),
        Value::Func {
            chunk_idx,
            env,
            recursive,
        } => {
            let kind = if *recursive { "rec" } else { "closure" };
            if env.is_empty() && !*recursive {
                format!("<fn:{}>", chunk_idx)
            } else {
                format!("<{}:{}/{}>", kind, chunk_idx, env.len())
            }
        }
        Value::Tuple(elems) => {
            let parts: Vec<String> = elems
                .iter()
                .map(|e| render_value_to_string(arena, e))
                .collect();
            format!("({})", parts.join(", "))
        }
        Value::Array(elems) => {
            let parts: Vec<String> = elems
                .iter()
                .map(|e| render_value_to_string(arena, e))
                .collect();
            format!("[{}]", parts.join(", "))
        }
        Value::Struct {
            type_name, fields, ..
        } => {
            let parts: Vec<String> = fields
                .iter()
                .map(|(name, v)| format!("{}: {}", name, render_value_to_string(arena, v)))
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
                    .map(|e| render_value_to_string(arena, e))
                    .collect();
                format!("{}::{}({})", type_name, variant, parts.join(", "))
            }
        }
    }
}

/// Convert any value to its string representation.
///
/// Returns a `Value::DynStr` allocated through the global allocator.
/// Subject to the cross-yield prohibition on dynamic strings. The
/// arena-aware variant [`native_to_string_with_ctx`] returns
/// `Value::KStr` with bounded-memory accounting and stale-pointer
/// detection.
fn native_to_string(args: &[Value]) -> Result<Value, VmError> {
    if args.len() != 1 {
        return Err(VmError::NativeError(format!(
            "to_string: expected 1 argument, got {}",
            args.len()
        )));
    }
    Ok(Value::DynStr(render_value_to_string(None, &args[0])))
}

/// Convert any value to its string representation with arena context.
///
/// Returns a [`Value::KStr`] backed by the host-owned arena's top
/// region. The result becomes [`keleusma_arena::Stale`] on the next
/// reset. Use this variant for the bounded-memory path.
fn native_to_string_with_ctx(ctx: &NativeCtx<'_>, args: &[Value]) -> Result<Value, VmError> {
    if args.len() != 1 {
        return Err(VmError::NativeError(format!(
            "to_string: expected 1 argument, got {}",
            args.len()
        )));
    }
    let s = render_value_to_string(Some(ctx.arena), &args[0]);
    let handle = KString::alloc(ctx.arena, &s).map_err(|_| {
        VmError::NativeError(String::from(
            "to_string: arena allocation failed (out of memory)",
        ))
    })?;
    Ok(Value::KStr(handle))
}

/// Get the length of an array, string, or tuple.
///
/// For arena-backed strings produced via [`Value::KStr`], the
/// arena-aware variant [`native_length_with_ctx`] resolves the handle
/// before counting characters. The non-arena variant returns an error
/// for `KStr` because resolution requires arena context.
fn native_length(args: &[Value]) -> Result<Value, VmError> {
    if args.len() != 1 {
        return Err(VmError::NativeError(format!(
            "length: expected 1 argument, got {}",
            args.len()
        )));
    }
    match &args[0] {
        Value::Array(arr) => Ok(Value::Int(arr.len() as i64)),
        Value::StaticStr(s) | Value::DynStr(s) => Ok(Value::Int(s.chars().count() as i64)),
        Value::KStr(_) => Err(VmError::NativeError(String::from(
            "length: arena-backed KStr requires arena context; register length through register_native_with_ctx",
        ))),
        Value::Tuple(t) => Ok(Value::Int(t.len() as i64)),
        other => Err(VmError::TypeError(format!(
            "length: unsupported type {}",
            other.type_name()
        ))),
    }
}

/// Get the length of an array, string, or tuple with arena context.
///
/// Resolves [`Value::KStr`] handles through the arena. Otherwise
/// identical to [`native_length`].
fn native_length_with_ctx(ctx: &NativeCtx<'_>, args: &[Value]) -> Result<Value, VmError> {
    if args.len() != 1 {
        return Err(VmError::NativeError(format!(
            "length: expected 1 argument, got {}",
            args.len()
        )));
    }
    match &args[0] {
        Value::Array(arr) => Ok(Value::Int(arr.len() as i64)),
        Value::StaticStr(s) | Value::DynStr(s) => Ok(Value::Int(s.chars().count() as i64)),
        Value::KStr(h) => match h.get(ctx.arena) {
            Ok(s) => Ok(Value::Int(s.chars().count() as i64)),
            Err(_) => Err(VmError::NativeError(String::from(
                "length: KStr is stale (arena reset since allocation)",
            ))),
        },
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

/// Register utility native functions in the legacy non-arena form.
///
/// `to_string` returns [`Value::DynStr`] from the global allocator.
/// `length` errors on [`Value::KStr`] arguments. Use
/// [`register_utility_natives_with_ctx`] for the arena-aware
/// variants that produce `KStr` and that resolve `KStr` arguments.
///
/// Registers: `to_string`, `length`, `println`, `math::sqrt`, `math::floor`,
/// `math::ceil`, `math::round`, `math::log2`.
pub fn register_utility_natives<'a, 'arena>(vm: &mut Vm<'a, 'arena>) {
    vm.register_native("to_string", native_to_string);
    vm.register_native("length", native_length);
    vm.register_native("println", native_println);

    vm.register_fn("math::sqrt", |x: f64| -> f64 { libm::sqrt(x) });
    vm.register_fn("math::floor", |x: f64| -> f64 { libm::floor(x) });
    vm.register_fn("math::ceil", |x: f64| -> f64 { libm::ceil(x) });
    vm.register_fn("math::round", |x: f64| -> f64 { libm::round(x) });
    vm.register_fn("math::log2", |x: f64| -> f64 { libm::log2(x) });
}

/// Register the arena-aware utility native functions on the VM.
///
/// `to_string` returns [`Value::KStr`] backed by the host-owned
/// arena's top region. The result becomes
/// [`keleusma_arena::Stale`] on the next reset. `length` resolves
/// `Value::KStr` arguments through the arena before counting
/// characters. The bounded-memory path for native-produced strings.
///
/// Registers: `to_string`, `length`, `println`, `math::sqrt`,
/// `math::floor`, `math::ceil`, `math::round`, `math::log2`. Math
/// functions are arity-typed and identical between the two
/// registrations.
pub fn register_utility_natives_with_ctx<'a, 'arena>(vm: &mut Vm<'a, 'arena>) {
    vm.register_native_with_ctx("to_string", native_to_string_with_ctx);
    vm.register_native_with_ctx("length", native_length_with_ctx);
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
    use crate::vm::{DEFAULT_ARENA_CAPACITY, VmState};

    fn run_with_utilities(src: &str) -> Value {
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
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
        assert_eq!(val, Value::DynStr(String::from("42")));
    }

    #[test]
    fn to_string_float() {
        let val = run_with_utilities("use to_string\nfn main() -> String { to_string(2.5) }");
        if let Value::DynStr(s) = val {
            assert!(s.starts_with("2.5"));
        } else {
            panic!("expected DynStr");
        }
    }

    #[test]
    fn to_string_bool() {
        let val = run_with_utilities("use to_string\nfn main() -> String { to_string(true) }");
        assert_eq!(val, Value::DynStr(String::from("true")));
    }

    #[test]
    fn to_string_string() {
        let val = run_with_utilities("use to_string\nfn main() -> String { to_string(\"hello\") }");
        assert_eq!(val, Value::DynStr(String::from("hello")));
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

    // -- Arena-aware utility variants --

    fn run_with_utilities_with_ctx(
        src: &str,
        arena: &keleusma_arena::Arena,
    ) -> (Value, Option<String>) {
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let mut vm = Vm::new(module, arena).unwrap();
        register_utility_natives_with_ctx(&mut vm);
        let result = match vm.call(&[]).unwrap() {
            VmState::Finished(v) => v,
            VmState::Yielded(v) => panic!("unexpected yield: {:?}", v),
            VmState::Reset => panic!("unexpected reset"),
        };
        // Resolve KStr eagerly while the arena is still valid so the
        // assertion can use a copied String.
        let resolved = match &result {
            Value::KStr(h) => h.get(arena).ok().map(String::from),
            _ => None,
        };
        (result, resolved)
    }

    #[test]
    fn to_string_with_ctx_int_returns_kstr() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (val, resolved) = run_with_utilities_with_ctx(
            "use to_string\nfn main() -> String { to_string(42) }",
            &arena,
        );
        assert!(matches!(val, Value::KStr(_)));
        assert_eq!(resolved.as_deref(), Some("42"));
    }

    #[test]
    fn to_string_with_ctx_string_returns_kstr() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (val, resolved) = run_with_utilities_with_ctx(
            "use to_string\nfn main() -> String { to_string(\"hello\") }",
            &arena,
        );
        assert!(matches!(val, Value::KStr(_)));
        assert_eq!(resolved.as_deref(), Some("hello"));
    }

    #[test]
    fn length_with_ctx_string_counts_chars() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (val, _) = run_with_utilities_with_ctx(
            "use length\nfn main() -> i64 { length(\"hello\") }",
            &arena,
        );
        assert_eq!(val, Value::Int(5));
    }
}

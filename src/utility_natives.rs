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

/// Concatenate two strings.
///
/// Resolves either operand, including arena-backed `Value::KStr`,
/// when an arena is supplied. Returns `Value::DynStr` containing the
/// catenation. Subject to the cross-yield prohibition on dynamic
/// strings. The arena-aware variant [`native_concat_with_ctx`]
/// returns `Value::KStr` for the bounded-memory path.
///
/// Worst-case output length is the sum of the operand lengths, so
/// hosts that rely on `verify_resource_bounds` for real-time
/// embedding must declare a heap bound through `set_native_bounds`
/// before invoking the VM. Without an attestation, the analysis
/// treats `concat` as zero-cost, which is unsound for unbounded
/// inputs.
fn native_concat(args: &[Value]) -> Result<Value, VmError> {
    if args.len() != 2 {
        return Err(VmError::NativeError(format!(
            "concat: expected 2 arguments, got {}",
            args.len()
        )));
    }
    let a = string_view_no_arena(&args[0])?;
    let b = string_view_no_arena(&args[1])?;
    let mut out = String::with_capacity(a.len() + b.len());
    out.push_str(a);
    out.push_str(b);
    Ok(Value::DynStr(out))
}

/// Concatenate two strings with arena context.
///
/// Resolves arena-backed `Value::KStr` handles before catenation and
/// allocates the result through the host-owned arena's top region as
/// `Value::KStr`. The result becomes [`keleusma_arena::Stale`] on the
/// next reset.
fn native_concat_with_ctx(ctx: &NativeCtx<'_>, args: &[Value]) -> Result<Value, VmError> {
    if args.len() != 2 {
        return Err(VmError::NativeError(format!(
            "concat: expected 2 arguments, got {}",
            args.len()
        )));
    }
    let a = string_view_with_arena(ctx.arena, &args[0])?;
    let b = string_view_with_arena(ctx.arena, &args[1])?;
    let mut out = String::with_capacity(a.len() + b.len());
    out.push_str(&a);
    out.push_str(&b);
    let handle = KString::alloc(ctx.arena, &out).map_err(|_| {
        VmError::NativeError(String::from(
            "concat: arena allocation failed (out of memory)",
        ))
    })?;
    Ok(Value::KStr(handle))
}

/// Substring slice from `start` (inclusive) to `end` (exclusive).
///
/// Bounds: `0 <= start <= end <= length`. Out-of-range indices return
/// a `NativeError`. Indexes are character counts measured in Unicode
/// code points (matching `length`'s semantics), not byte offsets, so
/// multi-byte characters are not split.
///
/// Returns `Value::DynStr`. The arena-aware variant
/// [`native_slice_with_ctx`] returns `Value::KStr`.
///
/// Worst-case output length is `end - start`. The same WCMU
/// attestation guidance as `concat` applies.
fn native_slice(args: &[Value]) -> Result<Value, VmError> {
    if args.len() != 3 {
        return Err(VmError::NativeError(format!(
            "slice: expected 3 arguments, got {}",
            args.len()
        )));
    }
    let s = string_view_no_arena(&args[0])?;
    let start = match &args[1] {
        Value::Int(i) => *i,
        other => {
            return Err(VmError::TypeError(format!(
                "slice: start must be i64, got {}",
                other.type_name()
            )));
        }
    };
    let end = match &args[2] {
        Value::Int(i) => *i,
        other => {
            return Err(VmError::TypeError(format!(
                "slice: end must be i64, got {}",
                other.type_name()
            )));
        }
    };
    let out = slice_chars(s, start, end)?;
    Ok(Value::DynStr(out))
}

/// Substring slice with arena context.
fn native_slice_with_ctx(ctx: &NativeCtx<'_>, args: &[Value]) -> Result<Value, VmError> {
    if args.len() != 3 {
        return Err(VmError::NativeError(format!(
            "slice: expected 3 arguments, got {}",
            args.len()
        )));
    }
    let s = string_view_with_arena(ctx.arena, &args[0])?;
    let start = match &args[1] {
        Value::Int(i) => *i,
        other => {
            return Err(VmError::TypeError(format!(
                "slice: start must be i64, got {}",
                other.type_name()
            )));
        }
    };
    let end = match &args[2] {
        Value::Int(i) => *i,
        other => {
            return Err(VmError::TypeError(format!(
                "slice: end must be i64, got {}",
                other.type_name()
            )));
        }
    };
    let out = slice_chars(&s, start, end)?;
    let handle = KString::alloc(ctx.arena, &out).map_err(|_| {
        VmError::NativeError(String::from(
            "slice: arena allocation failed (out of memory)",
        ))
    })?;
    Ok(Value::KStr(handle))
}

/// Helper: read a string Value as a `&str`, rejecting `Value::KStr`
/// because no arena is available for resolution. Used by the
/// non-context-aware native variants.
fn string_view_no_arena(v: &Value) -> Result<&str, VmError> {
    match v {
        Value::StaticStr(s) | Value::DynStr(s) => Ok(s.as_str()),
        Value::KStr(_) => Err(VmError::NativeError(String::from(
            "expected String argument; arena-backed KStr requires arena context",
        ))),
        other => Err(VmError::TypeError(format!(
            "expected String, got {}",
            other.type_name()
        ))),
    }
}

/// Helper: read a string Value as an owned `String`, resolving
/// `Value::KStr` through the supplied arena. Used by the
/// context-aware native variants.
fn string_view_with_arena(arena: &Arena, v: &Value) -> Result<String, VmError> {
    match v {
        Value::StaticStr(s) | Value::DynStr(s) => Ok(s.clone()),
        Value::KStr(h) => match h.get(arena) {
            Ok(s) => Ok(String::from(s)),
            Err(_) => Err(VmError::NativeError(String::from(
                "KStr is stale (arena reset since allocation)",
            ))),
        },
        other => Err(VmError::TypeError(format!(
            "expected String, got {}",
            other.type_name()
        ))),
    }
}

/// Helper: extract a substring by character indices. Returns a new
/// owned `String`. Bounds-checks both indices and rejects out-of-range
/// or inverted ranges.
fn slice_chars(s: &str, start: i64, end: i64) -> Result<String, VmError> {
    if start < 0 {
        return Err(VmError::NativeError(format!(
            "slice: start index {} is negative",
            start
        )));
    }
    if end < start {
        return Err(VmError::NativeError(format!(
            "slice: end index {} less than start {}",
            end, start
        )));
    }
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len() as i64;
    if end > len {
        return Err(VmError::NativeError(format!(
            "slice: end index {} exceeds length {}",
            end, len
        )));
    }
    let start_u = start as usize;
    let end_u = end as usize;
    Ok(chars[start_u..end_u].iter().collect())
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
/// Registers: `to_string`, `length`, `concat`, `slice`, `println`,
/// `math::sqrt`, `math::floor`, `math::ceil`, `math::round`,
/// `math::log2`.
pub fn register_utility_natives<'a, 'arena>(vm: &mut Vm<'a, 'arena>) {
    vm.register_native("to_string", native_to_string);
    vm.register_native("length", native_length);
    vm.register_native("concat", native_concat);
    vm.register_native("slice", native_slice);
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
/// Registers: `to_string`, `length`, `concat`, `slice`, `println`,
/// `math::sqrt`, `math::floor`, `math::ceil`, `math::round`,
/// `math::log2`. Math functions are arity-typed and identical
/// between the two registrations.
pub fn register_utility_natives_with_ctx<'a, 'arena>(vm: &mut Vm<'a, 'arena>) {
    vm.register_native_with_ctx("to_string", native_to_string_with_ctx);
    vm.register_native_with_ctx("length", native_length_with_ctx);
    vm.register_native_with_ctx("concat", native_concat_with_ctx);
    vm.register_native_with_ctx("slice", native_slice_with_ctx);
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

    // -- f-string interpolation --

    #[test]
    fn fstring_no_interpolation() {
        let val = run_with_utilities("fn main() -> String { f\"hello\" }");
        assert_eq!(val, Value::StaticStr(String::from("hello")));
    }

    #[test]
    fn fstring_single_interp() {
        let val =
            run_with_utilities("use to_string\nfn main() -> String { let n: i64 = 42; f\"{n}\" }");
        assert_eq!(val, Value::DynStr(String::from("42")));
    }

    #[test]
    fn fstring_mixed_interp() {
        let val = run_with_utilities(
            "use to_string\nuse concat\n\
             fn main() -> String { let n: i64 = 42; f\"x = {n}!\" }",
        );
        assert_eq!(val, Value::DynStr(String::from("x = 42!")));
    }

    #[test]
    fn fstring_multiple_interps() {
        let val = run_with_utilities(
            "use to_string\nuse concat\n\
             fn main() -> String {\n\
                let a: i64 = 1;\n\
                let b: i64 = 2;\n\
                f\"({a}, {b})\"\n\
             }",
        );
        assert_eq!(val, Value::DynStr(String::from("(1, 2)")));
    }

    #[test]
    fn fstring_escaped_braces() {
        let val = run_with_utilities("fn main() -> String { f\"\\{key\\}\" }");
        assert_eq!(val, Value::StaticStr(String::from("{key}")));
    }

    // -- Concat and slice --

    #[test]
    fn concat_two_static_strings() {
        let val = run_with_utilities(
            "use concat\nfn main() -> String { concat(\"hello, \", \"world\") }",
        );
        assert_eq!(val, Value::DynStr(String::from("hello, world")));
    }

    #[test]
    fn concat_static_with_dynamic() {
        let val = run_with_utilities(
            "use concat\nuse to_string\nfn main() -> String { concat(\"x = \", to_string(42)) }",
        );
        assert_eq!(val, Value::DynStr(String::from("x = 42")));
    }

    #[test]
    fn slice_basic() {
        let val = run_with_utilities("use slice\nfn main() -> String { slice(\"hello\", 1, 4) }");
        assert_eq!(val, Value::DynStr(String::from("ell")));
    }

    #[test]
    fn slice_full_range() {
        let val = run_with_utilities("use slice\nfn main() -> String { slice(\"hello\", 0, 5) }");
        assert_eq!(val, Value::DynStr(String::from("hello")));
    }

    #[test]
    fn slice_empty_range() {
        let val = run_with_utilities("use slice\nfn main() -> String { slice(\"hello\", 2, 2) }");
        assert_eq!(val, Value::DynStr(String::from("")));
    }

    #[test]
    fn concat_with_ctx_returns_kstr() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (val, resolved) = run_with_utilities_with_ctx(
            "use concat\nfn main() -> String { concat(\"foo\", \"bar\") }",
            &arena,
        );
        assert!(matches!(val, Value::KStr(_)));
        assert_eq!(resolved.as_deref(), Some("foobar"));
    }

    #[test]
    fn slice_with_ctx_returns_kstr() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (val, resolved) = run_with_utilities_with_ctx(
            "use slice\nfn main() -> String { slice(\"hello world\", 6, 11) }",
            &arena,
        );
        assert!(matches!(val, Value::KStr(_)));
        assert_eq!(resolved.as_deref(), Some("world"));
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

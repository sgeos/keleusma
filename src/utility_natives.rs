extern crate alloc;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::address::Address;
use crate::bytecode::GenericValue;
use crate::float::Float;
use crate::kstring::KString;
use crate::vm::{GenericVm, NativeCtx, Vm, VmError};
use crate::word::Word;
use keleusma_arena::Arena;

/// Render a value to its string representation, resolving
/// arena-backed `KStr` handles through the supplied arena.
///
/// Stale `KStr` handles render as the literal `<stale>` placeholder.
///
/// Parametric over `(W, F)`. Integer payloads are formatted through
/// `Word::to_i64` so any narrow word type produces the same numeric
/// rendering as the default i64. Float payloads are formatted
/// through `Float::to_f64` for the same reason.
fn render_value_to_string<W: Word, F: Float>(arena: &Arena, val: &GenericValue<W, F>) -> String {
    match val {
        GenericValue::Int(n) => format!("{}", W::to_i64(*n)),
        GenericValue::Byte(b) => format!("{}", b),
        GenericValue::Fixed(bits) => format!("Fixed({})", W::to_i64(*bits)),
        #[cfg(feature = "floats")]
        GenericValue::Float(f) => format!("{}", F::to_f64(*f)),
        GenericValue::Bool(b) => format!("{}", b),
        GenericValue::StaticStr(s) => s.clone(),
        GenericValue::KStr(h) => match h.get(arena) {
            Ok(s) => String::from(s),
            Err(_) => String::from("<stale>"),
        },
        GenericValue::Unit => String::from("()"),
        GenericValue::None => String::from("None"),
        GenericValue::Func {
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
        GenericValue::Tuple(elems) => {
            let parts: Vec<String> = elems
                .iter()
                .map(|e| render_value_to_string(arena, e))
                .collect();
            format!("({})", parts.join(", "))
        }
        GenericValue::Array(elems) => {
            let parts: Vec<String> = elems
                .iter()
                .map(|e| render_value_to_string(arena, e))
                .collect();
            format!("[{}]", parts.join(", "))
        }
        GenericValue::Struct {
            type_name, fields, ..
        } => {
            let parts: Vec<String> = fields
                .iter()
                .map(|(name, v)| format!("{}: {}", name, render_value_to_string(arena, v)))
                .collect();
            format!("{} {{ {} }}", type_name, parts.join(", "))
        }
        GenericValue::Enum {
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
        GenericValue::Opaque(o) => format!("<opaque {}>", o.type_name()),
        #[cfg(not(feature = "floats"))]
        GenericValue::_PhantomFloat(_) => unreachable!("_PhantomFloat is never constructed"),
    }
}

/// Convert any value to its string representation.
///
/// Returns a [`GenericValue::KStr`] backed by the host-owned arena's
/// top region. The result becomes [`keleusma_arena::Stale`] on the
/// next reset.
fn native_to_string_with_ctx<W: Word, F: Float>(
    ctx: &NativeCtx<'_>,
    args: &[GenericValue<W, F>],
) -> Result<GenericValue<W, F>, VmError> {
    check_arity("to_string", 1, args)?;
    let s = render_value_to_string(ctx.arena, &args[0]);
    finalize_string_result("to_string", ctx.arena, s)
}

/// Get the length of an array, string, or tuple with arena context.
///
/// Resolves [`GenericValue::KStr`] handles through the arena. The
/// returned integer is wrapped through `Word::from_i64_wrap` so the
/// length fits the runtime's word width. Lengths that exceed `W`'s
/// range silently truncate; hosts running narrow runtimes against
/// long arrays should bound input sizes elsewhere.
fn native_length_with_ctx<W: Word, F: Float>(
    ctx: &NativeCtx<'_>,
    args: &[GenericValue<W, F>],
) -> Result<GenericValue<W, F>, VmError> {
    check_arity("length", 1, args)?;
    match &args[0] {
        GenericValue::Array(arr) => Ok(GenericValue::Int(W::from_i64_wrap(arr.len() as i64))),
        GenericValue::StaticStr(s) => Ok(GenericValue::Int(W::from_i64_wrap(
            s.chars().count() as i64
        ))),
        GenericValue::KStr(h) => match h.get(ctx.arena) {
            Ok(s) => Ok(GenericValue::Int(
                W::from_i64_wrap(s.chars().count() as i64),
            )),
            Err(_) => Err(VmError::NativeError(String::from(
                "length: KStr is stale (arena reset since allocation)",
            ))),
        },
        GenericValue::Tuple(t) => Ok(GenericValue::Int(W::from_i64_wrap(t.len() as i64))),
        other => Err(VmError::TypeError(format!(
            "length: unsupported type {}",
            other.type_name()
        ))),
    }
}

/// Helper: read a string Value as an owned `String`, resolving
/// arena-backed `GenericValue::KStr` through the supplied arena.
fn read_string_arg<W: Word, F: Float>(
    arena: &Arena,
    v: &GenericValue<W, F>,
) -> Result<String, VmError> {
    match v {
        GenericValue::StaticStr(s) => Ok(s.clone()),
        GenericValue::KStr(h) => match h.get(arena) {
            Ok(s) => Ok(String::from(s)),
            Err(_) => Err(VmError::NativeError(String::from(
                "KStr is stale (arena reset since allocation)",
            ))),
        },
        other => Err(VmError::TypeError(format!(
            "expected Text, got {}",
            other.type_name()
        ))),
    }
}

/// Helper: validate that the argument count matches `expected` and
/// produce a uniform error message otherwise. Used by every native
/// in this module that has a fixed arity.
fn check_arity<W: Word, F: Float>(
    name: &str,
    expected: usize,
    args: &[GenericValue<W, F>],
) -> Result<(), VmError> {
    if args.len() != expected {
        return Err(VmError::NativeError(format!(
            "{}: expected {} argument{}, got {}",
            name,
            expected,
            if expected == 1 { "" } else { "s" },
            args.len()
        )));
    }
    Ok(())
}

/// Helper: read an i64-valued argument or produce a typed error.
/// Sign-extends the runtime's word type via `Word::to_i64` so callers
/// can treat the value as an `i64` regardless of `W`'s width.
fn read_i64_arg<W: Word, F: Float>(
    name: &str,
    arg_label: &str,
    v: &GenericValue<W, F>,
) -> Result<i64, VmError> {
    match v {
        GenericValue::Int(i) => Ok(W::to_i64(*i)),
        other => Err(VmError::TypeError(format!(
            "{}: {} must be Word, got {}",
            name,
            arg_label,
            other.type_name()
        ))),
    }
}

/// Helper: copy a produced `String` into the arena and wrap the
/// resulting handle in `GenericValue::KStr`. The error path produces
/// a clear allocation-failure message that mentions the originating
/// native name.
fn finalize_string_result<W: Word, F: Float>(
    name: &str,
    arena: &Arena,
    out: String,
) -> Result<GenericValue<W, F>, VmError> {
    let handle = KString::alloc(arena, &out).map_err(|_| {
        VmError::NativeError(format!("{}: arena allocation failed (out of memory)", name))
    })?;
    Ok(GenericValue::KStr(handle))
}

/// Concatenate two strings with arena context.
fn native_concat_with_ctx<W: Word, F: Float>(
    ctx: &NativeCtx<'_>,
    args: &[GenericValue<W, F>],
) -> Result<GenericValue<W, F>, VmError> {
    check_arity("concat", 2, args)?;
    let a = read_string_arg(ctx.arena, &args[0])?;
    let b = read_string_arg(ctx.arena, &args[1])?;
    let mut out = String::with_capacity(a.len() + b.len());
    out.push_str(&a);
    out.push_str(&b);
    finalize_string_result("concat", ctx.arena, out)
}

/// Substring slice from `start` (inclusive) to `end` (exclusive) with
/// arena context.
fn native_slice_with_ctx<W: Word, F: Float>(
    ctx: &NativeCtx<'_>,
    args: &[GenericValue<W, F>],
) -> Result<GenericValue<W, F>, VmError> {
    check_arity("slice", 3, args)?;
    let s = read_string_arg(ctx.arena, &args[0])?;
    let start = read_i64_arg("slice", "start", &args[1])?;
    let end = read_i64_arg("slice", "end", &args[2])?;
    let out = slice_chars(&s, start, end)?;
    finalize_string_result("slice", ctx.arena, out)
}

/// Helper: extract a substring by character indices.
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
fn native_println<W: Word, F: Float>(
    args: &[GenericValue<W, F>],
) -> Result<GenericValue<W, F>, VmError> {
    check_arity("println", 1, args)?;
    // No-op in no_std. The value is consumed but not printed.
    Ok(GenericValue::Unit)
}

/// Register utility native functions on the VM.
///
/// `to_string` returns [`GenericValue::KStr`] backed by the
/// host-owned arena's top region. The result becomes
/// [`keleusma_arena::Stale`] on the next reset. `length` resolves
/// `GenericValue::KStr` arguments through the arena before counting
/// characters. `concat` and `slice` produce arena-allocated
/// `GenericValue::KStr` results from their `StaticStr` or `KStr`
/// inputs.
///
/// Registers: `to_string`, `length`, `concat`, `slice`, `println`.
///
/// Parametric over `(W, A, F)` so narrow runtimes can register the
/// bundle without writing their own utility-natives equivalents.
/// Lengths returned by `length` and indices accepted by `slice` are
/// bridged through `Word::to_i64` and `Word::from_i64_wrap`; lengths
/// that exceed `W`'s range silently truncate.
pub fn register_utility_natives<'a, 'arena, W: Word, A: Address, F: Float>(
    vm: &mut GenericVm<'a, 'arena, W, A, F>,
) {
    vm.register_native_with_ctx("to_string", native_to_string_with_ctx::<W, F>);
    vm.register_native_with_ctx("length", native_length_with_ctx::<W, F>);
    vm.register_native_with_ctx("concat", native_concat_with_ctx::<W, F>);
    vm.register_native_with_ctx("slice", native_slice_with_ctx::<W, F>);
    vm.register_native("println", native_println::<W, F>);
}

/// Deprecated alias for [`register_utility_natives`]. Retained for
/// API compatibility; both registration entry points now produce
/// arena-allocated `GenericValue::KStr` results since
/// `GenericValue::DynStr` was removed in V0.2.0.
#[deprecated(
    since = "0.2.0",
    note = "Value::DynStr was removed in V0.2.0; use register_utility_natives, which is now arena-aware by default."
)]
pub fn register_utility_natives_with_ctx<'a, 'arena>(vm: &mut Vm<'a, 'arena>) {
    register_utility_natives(vm);
}

#[cfg(all(test, feature = "compile", feature = "verify", feature = "floats"))]
mod tests {
    use super::*;
    use crate::bytecode::Value;
    use crate::compiler::compile;
    use crate::lexer::tokenize;
    use crate::parser::parse;
    use crate::vm::{DEFAULT_ARENA_CAPACITY, VmState};

    /// Run a Keleusma program with the bundled utility natives
    /// registered and return the result together with a string
    /// rendering resolved eagerly through the arena so callers can
    /// assert on string contents after the arena outlives the call.
    ///
    /// The Math bundle is also registered so that tests covering
    /// f-string interpolation against numeric values, and any
    /// historical tests that exercised `math::*` routines, can
    /// resolve their `use math::*` declarations.
    fn run_with_utilities(src: &str, arena: &keleusma_arena::Arena) -> (Value, Option<String>) {
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let mut vm = Vm::new(module, arena).unwrap();
        register_utility_natives(&mut vm);
        vm.register_library(crate::stddsl::Math);
        let result = match vm.call(&[]).unwrap() {
            VmState::Finished(v) => v,
            VmState::Yielded(v) => panic!("unexpected yield: {:?}", v),
            VmState::Reset => panic!("unexpected reset"),
        };
        let resolved = result
            .as_str_with_arena(arena)
            .ok()
            .flatten()
            .map(String::from);
        (result, resolved)
    }

    #[test]
    fn to_string_int() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (val, resolved) =
            run_with_utilities("use to_string\nfn main() -> Text { to_string(42) }", &arena);
        assert!(matches!(val, Value::KStr(_)));
        assert_eq!(resolved.as_deref(), Some("42"));
    }

    #[test]
    fn to_string_float() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (val, resolved) = run_with_utilities(
            "use to_string\nfn main() -> Text { to_string(2.5) }",
            &arena,
        );
        assert!(matches!(val, Value::KStr(_)));
        assert!(resolved.unwrap().starts_with("2.5"));
    }

    #[test]
    fn to_string_bool() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (val, resolved) = run_with_utilities(
            "use to_string\nfn main() -> Text { to_string(true) }",
            &arena,
        );
        assert!(matches!(val, Value::KStr(_)));
        assert_eq!(resolved.as_deref(), Some("true"));
    }

    #[cfg(feature = "text")]
    #[test]
    fn to_string_string() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (val, resolved) = run_with_utilities(
            "use to_string\nfn main() -> Text { to_string(\"hello\") }",
            &arena,
        );
        assert!(matches!(val, Value::KStr(_)));
        assert_eq!(resolved.as_deref(), Some("hello"));
    }

    #[test]
    fn length_array() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (val, _) = run_with_utilities(
            "use length\nfn main() -> Word { length([10, 20, 30]) }",
            &arena,
        );
        assert_eq!(val, Value::Int(3));
    }

    #[cfg(feature = "text")]
    #[test]
    fn length_string() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (val, _) = run_with_utilities(
            "use length\nfn main() -> Word { length(\"hello\") }",
            &arena,
        );
        assert_eq!(val, Value::Int(5));
    }

    #[test]
    fn length_tuple() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (val, _) = run_with_utilities(
            "use length\nfn main() -> Word { length((1, 2, 3)) }",
            &arena,
        );
        assert_eq!(val, Value::Int(3));
    }

    #[test]
    fn sqrt_value() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (val, _) = run_with_utilities(
            "use math::sqrt\nfn main() -> Float { math::sqrt(9.0) }",
            &arena,
        );
        assert_eq!(val, Value::Float(3.0));
    }

    #[test]
    fn floor_value() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (val, _) = run_with_utilities(
            "use math::floor\nfn main() -> Float { math::floor(3.7) }",
            &arena,
        );
        assert_eq!(val, Value::Float(3.0));
    }

    #[test]
    fn ceil_value() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (val, _) = run_with_utilities(
            "use math::ceil\nfn main() -> Float { math::ceil(3.2) }",
            &arena,
        );
        assert_eq!(val, Value::Float(4.0));
    }

    #[test]
    fn round_value() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (val, _) = run_with_utilities(
            "use math::round\nfn main() -> Float { math::round(3.5) }",
            &arena,
        );
        assert_eq!(val, Value::Float(4.0));
    }

    #[test]
    fn log2_value() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (val, _) = run_with_utilities(
            "use math::log2\nfn main() -> Float { math::log2(8.0) }",
            &arena,
        );
        assert_eq!(val, Value::Float(3.0));
    }

    // -- f-string interpolation --

    #[cfg(feature = "text")]
    #[test]
    fn fstring_no_interpolation() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (val, _) = run_with_utilities("fn main() -> Text { f\"hello\" }", &arena);
        assert_eq!(val, Value::StaticStr(String::from("hello")));
    }

    #[cfg(feature = "text")]
    #[test]
    fn fstring_single_interp() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (val, resolved) = run_with_utilities(
            "use to_string\nfn main() -> Text { let n: Word = 42; f\"{n}\" }",
            &arena,
        );
        assert!(matches!(val, Value::KStr(_)));
        assert_eq!(resolved.as_deref(), Some("42"));
    }

    #[cfg(feature = "text")]
    #[test]
    fn fstring_mixed_interp() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (val, resolved) = run_with_utilities(
            "use to_string\nuse concat\n\
             fn main() -> Text { let n: Word = 42; f\"x = {n}!\" }",
            &arena,
        );
        assert!(matches!(val, Value::KStr(_)));
        assert_eq!(resolved.as_deref(), Some("x = 42!"));
    }

    #[cfg(feature = "text")]
    #[test]
    fn fstring_multiple_interps() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (val, resolved) = run_with_utilities(
            "use to_string\nuse concat\n\
             fn main() -> Text {\n\
                let a: Word = 1;\n\
                let b: Word = 2;\n\
                f\"({a}, {b})\"\n\
             }",
            &arena,
        );
        assert!(matches!(val, Value::KStr(_)));
        assert_eq!(resolved.as_deref(), Some("(1, 2)"));
    }

    #[cfg(feature = "text")]
    #[test]
    fn fstring_escaped_braces() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (val, _) = run_with_utilities("fn main() -> Text { f\"\\{key\\}\" }", &arena);
        assert_eq!(val, Value::StaticStr(String::from("{key}")));
    }

    // -- Concat and slice --

    #[cfg(feature = "text")]
    #[test]
    fn concat_two_static_strings() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (val, resolved) = run_with_utilities(
            "use concat\nfn main() -> Text { concat(\"hello, \", \"world\") }",
            &arena,
        );
        assert!(matches!(val, Value::KStr(_)));
        assert_eq!(resolved.as_deref(), Some("hello, world"));
    }

    #[cfg(feature = "text")]
    #[test]
    fn concat_static_with_dynamic() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (val, resolved) = run_with_utilities(
            "use concat\nuse to_string\nfn main() -> Text { concat(\"x = \", to_string(42)) }",
            &arena,
        );
        assert!(matches!(val, Value::KStr(_)));
        assert_eq!(resolved.as_deref(), Some("x = 42"));
    }

    #[cfg(feature = "text")]
    #[test]
    fn slice_basic() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (val, resolved) = run_with_utilities(
            "use slice\nfn main() -> Text { slice(\"hello\", 1, 4) }",
            &arena,
        );
        assert!(matches!(val, Value::KStr(_)));
        assert_eq!(resolved.as_deref(), Some("ell"));
    }

    #[cfg(feature = "text")]
    #[test]
    fn slice_full_range() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (val, resolved) = run_with_utilities(
            "use slice\nfn main() -> Text { slice(\"hello\", 0, 5) }",
            &arena,
        );
        assert!(matches!(val, Value::KStr(_)));
        assert_eq!(resolved.as_deref(), Some("hello"));
    }

    #[cfg(feature = "text")]
    #[test]
    fn slice_empty_range() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (val, resolved) = run_with_utilities(
            "use slice\nfn main() -> Text { slice(\"hello\", 2, 2) }",
            &arena,
        );
        assert!(matches!(val, Value::KStr(_)));
        assert_eq!(resolved.as_deref(), Some(""));
    }
}

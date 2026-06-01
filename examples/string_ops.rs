//! End-to-end demonstration of host-registered string operations.
//!
//! V0.2.0 ships no string utilities in the runtime; all domain
//! functionality is host-registered. This example registers `concat`
//! and `slice` as host natives and a script composes them to build a
//! result. The V0.1.x `to_string`/`length` utilities and f-string
//! interpolation were retired in V0.2.0; formatting is now the
//! responsibility of a host-registered native.
//!
//! WCET note. Text concatenation and slicing produce dynamic strings
//! whose worst-case output length is the sum of operand lengths
//! (concat) or `end - start` (slice). The verifier treats native
//! allocations through the per-native attestation supplied by
//! `Vm::set_native_bounds`. Hosts that rely on `verify_resource_bounds`
//! for real-time embedding must declare heap bounds for `concat` and
//! `slice` before constructing the VM through the safe constructor.
//!
//! Run with: `cargo run --example string_ops`

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value, VmError};

fn main() {
    // The script imports two host-registered string natives and
    // composes them. The natives carry typed `use` signatures so the
    // type checker validates the call sites; the bodies are supplied
    // in Rust below, after compilation.
    let src = r#"
        use concat(Text, Text) -> Text
        use slice(Text, Word, Word) -> Text
        fn main() -> Text {
            let greeting = concat("hello, ", "Keleusma");
            let head = slice(greeting, 0, 5);
            concat(head, "...")
        }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");

    // concat(a: Text, b: Text) -> Text. Each operand may be a static
    // string literal (`StaticStr`) or an arena-backed dynamic string
    // (`KStr`); `as_str_with_arena` covers both.
    vm.register_native_with_ctx("concat", |ctx, args| {
        if args.len() != 2 {
            return Err(VmError::NativeError(
                "concat: expected exactly two arguments".into(),
            ));
        }
        let a = args[0]
            .as_str_with_arena(ctx.arena)
            .map_err(|_| VmError::NativeError("concat: stale KStr".into()))?
            .ok_or_else(|| {
                VmError::TypeError(format!(
                    "concat: expected Text, got {}",
                    args[0].type_name()
                ))
            })?;
        let b = args[1]
            .as_str_with_arena(ctx.arena)
            .map_err(|_| VmError::NativeError("concat: stale KStr".into()))?
            .ok_or_else(|| {
                VmError::TypeError(format!(
                    "concat: expected Text, got {}",
                    args[1].type_name()
                ))
            })?;
        let mut out = String::with_capacity(a.len() + b.len());
        out.push_str(a);
        out.push_str(b);
        Ok(Value::StaticStr(out))
    });

    // slice(s: Text, start: Word, end: Word) -> Text. A byte-range
    // slice with bounds and UTF-8 char-boundary checks.
    vm.register_native_with_ctx("slice", |ctx, args| {
        if args.len() != 3 {
            return Err(VmError::NativeError(
                "slice: expected exactly three arguments".into(),
            ));
        }
        let s = args[0]
            .as_str_with_arena(ctx.arena)
            .map_err(|_| VmError::NativeError("slice: stale KStr".into()))?
            .ok_or_else(|| {
                VmError::TypeError(format!("slice: expected Text, got {}", args[0].type_name()))
            })?;
        let read_word = |v: &Value| -> Result<i64, VmError> {
            match v {
                Value::Int(n) => Ok(*n),
                other => Err(VmError::TypeError(format!(
                    "slice: expected Word, got {}",
                    other.type_name()
                ))),
            }
        };
        let start = read_word(&args[1])?;
        let end = read_word(&args[2])?;
        if start < 0 || end < start || end as usize > s.len() {
            return Err(VmError::NativeError(format!(
                "slice: range [{}, {}) out of bounds for length {}",
                start,
                end,
                s.len()
            )));
        }
        let (lo, hi) = (start as usize, end as usize);
        if !s.is_char_boundary(lo) || !s.is_char_boundary(hi) {
            return Err(VmError::NativeError(
                "slice: range does not fall on a UTF-8 character boundary".into(),
            ));
        }
        Ok(Value::StaticStr(s[lo..hi].to_owned()))
    });

    match vm.call(&[]).expect("vm call") {
        VmState::Finished(v) => {
            let s = v
                .as_str_with_arena(&arena)
                .expect("Text result resolves against the live arena")
                .expect("Text result has string contents");
            println!("result: {}", s);
            assert_eq!(s, "hello...");
            println!("string ops executed end to end");
        }
        other => panic!("unexpected: {:?}", other),
    }
}

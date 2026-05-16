//! Register Rust's owned `String` as an opaque type usable from
//! Keleusma scripts.
//!
//! This example demonstrates the recommended pattern for text-heavy
//! work in Keleusma. The cheap arena-resident `Text` surface
//! covers literals, concatenation through `+`, and the bundled
//! `to_string` and `concat` natives. Heavy text manipulation
//! (regex, casing, Unicode normalisation, splitting, parsing) is
//! delegated to Rust's standard library via host-registered native
//! functions that operate on an opaque `RustString` value.
//!
//! The `RustString` opaque type wraps Rust's owned `String`. It
//! crosses the yield boundary, persists across arena resets, and
//! has a lifetime managed by `Arc`. Scripts treat it as a handle
//! and route every operation through registered natives.
//!
//! Run with: `cargo run --example opaque_rust_string --features text`
//!
//! Expected output:
//! ```text
//! script returned: HELLO, KELEUSMA!
//! byte length on host: 16
//! ```
//!
//! ## Why an opaque type rather than `Text`
//!
//! Keleusma's `Text` is designed for cheap arena-resident
//! manipulation under the WCMU bound. Programs that allocate
//! growing text values pay against the per-iteration arena
//! budget. For text work whose size is determined by the host's
//! data rather than the script's literal constants, the WCMU
//! bound becomes the host's responsibility: the host validates
//! the input before constructing the opaque and treats the
//! operation as host-attested.
//!
//! The opaque approach also lets scripts reference text values
//! that outlive a Stream iteration. `Text` is arena-resident and
//! cleared at each reset; `RustString` is reference-counted and
//! persists for the lifetime of the `Arc`.

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, HostOpaque, Value, host_arc};

/// A Rust `String` exposed to scripts as the opaque type
/// `RustString`. The newtype wrapper is required because foreign
/// `impl HostOpaque for String` would violate Rust's orphan rule.
/// Hosts inside their own crate that own both types can implement
/// `HostOpaque` directly on their concrete type.
struct RustString(String);

impl HostOpaque for RustString {
    fn type_name(&self) -> &'static str {
        "RustString"
    }
}

/// Helper for native functions that extract the `&RustString`
/// reference from a `Value::Opaque` argument with a typed error
/// on mismatch.
fn read_rust_string<'a>(
    native: &'static str,
    v: &'a Value,
) -> Result<&'a RustString, keleusma::VmError> {
    let opaque = match v {
        Value::Opaque(o) => o,
        other => {
            return Err(keleusma::VmError::TypeError(format!(
                "{}: expected RustString, got {}",
                native,
                other.type_name()
            )));
        }
    };
    opaque.as_ref().downcast_ref::<RustString>().ok_or_else(|| {
        keleusma::VmError::TypeError(format!(
            "{}: expected RustString, got opaque {}",
            native,
            opaque.type_name()
        ))
    })
}

fn main() {
    // The script imports three host-registered natives and uses
    // them to manipulate an opaque RustString end to end. Every
    // operation goes through a registered native that performs
    // the work in Rust. The script never inspects the opaque's
    // internal structure.
    let src = r#"
        use make_string
        use upper_case
        use append_exclamation

        fn main() -> RustString {
            let s = make_string("hello, keleusma");
            let s = upper_case(s);
            append_exclamation(s)
        }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");

    // make_string(text: Text) -> RustString
    //
    // Constructs a new opaque `RustString` from a Keleusma text
    // literal. The `Text` operand may be a `StaticStr` or a
    // `KStr`; `as_str_with_arena` covers both.
    vm.register_native_with_ctx("make_string", |ctx, args| {
        if args.len() != 1 {
            return Err(keleusma::VmError::NativeError(
                "make_string: expected exactly one argument".into(),
            ));
        }
        let s: &str = args[0]
            .as_str_with_arena(ctx.arena)
            .map_err(|_| {
                keleusma::VmError::NativeError(
                    "make_string: stale KStr (arena reset since allocation)".into(),
                )
            })?
            .ok_or_else(|| {
                keleusma::VmError::TypeError(format!(
                    "make_string: expected Text, got {}",
                    args[0].type_name()
                ))
            })?;
        Ok(Value::Opaque(host_arc(RustString(s.to_owned()))))
    });

    // upper_case(s: RustString) -> RustString
    //
    // Returns a new opaque `RustString` whose contents are the
    // input uppercased. Operates entirely in Rust through the
    // standard `str::to_uppercase` implementation.
    vm.register_native("upper_case", |args| {
        if args.len() != 1 {
            return Err(keleusma::VmError::NativeError(
                "upper_case: expected exactly one argument".into(),
            ));
        }
        let s = read_rust_string("upper_case", &args[0])?;
        Ok(Value::Opaque(host_arc(RustString(s.0.to_uppercase()))))
    });

    // append_exclamation(s: RustString) -> RustString
    //
    // Returns a new opaque `RustString` with a trailing `!`.
    vm.register_native("append_exclamation", |args| {
        if args.len() != 1 {
            return Err(keleusma::VmError::NativeError(
                "append_exclamation: expected exactly one argument".into(),
            ));
        }
        let s = read_rust_string("append_exclamation", &args[0])?;
        let mut out = s.0.clone();
        out.push('!');
        Ok(Value::Opaque(host_arc(RustString(out))))
    });

    // Run the script. It returns an opaque `RustString` that the
    // host receives as `Value::Opaque`.
    let result = match vm.call(&[]).expect("vm call") {
        VmState::Finished(v) => v,
        other => panic!("expected finished, got {:?}", other),
    };

    // Extract the typed `RustString` and read its content.
    let opaque = match result {
        Value::Opaque(o) => o,
        other => panic!("expected opaque, got {:?}", other),
    };
    let typed = opaque
        .as_ref()
        .downcast_ref::<RustString>()
        .expect("downcast RustString");
    println!("script returned: {}", typed.0);
    println!("byte length on host: {}", typed.0.len());

    assert_eq!(typed.0, "HELLO, KELEUSMA!");
    assert_eq!(typed.0.len(), 16);
}

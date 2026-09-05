//! Why the composite operand-form refusals are not reachable through a native.
//!
//! # The question this answers
//!
//! The runtime carries seven refusals of the form "operand form does not match
//! ... body". They fire when the access form the compiler baked disagrees with
//! the representation the value actually has. `VmError::InvalidBytecode` means
//! the artefact should never have been produced, so a module that verifies,
//! loads, and reaches one of these is a hole in the load-time guarantee.
//!
//! Group D of `docs/decisions/INVALID_BYTECODE_CENSUS.md`. It was named as the
//! next pass because the shape looked dangerous: **the compiler bakes a FLAT
//! access for a scalar-fielded struct, and a host-built composite is BOXED.**
//! `GenericValue::struct_with_widths` says so in as many words -- "no arena
//! here, so the no-arena path is boxed" -- and the `GetField` dispatch has arms
//! for flat-with-flat and boxed-with-boxed, with everything else falling to the
//! refusal.
//!
//! **Measured, the pairing does not occur, and the reason is a real defence
//! rather than luck.** A host-supplied value is put through
//! `into_arena_canonical` at the call boundary, which rewrites a boxed composite
//! into the arena's flat canonical form, after `correct_native_enum_hints` has
//! restored a boxed enum's discriminant and padding from the module's recorded
//! layouts. **The boxed body never reaches an access site**, so the baked flat
//! access meets a flat body.
//!
//! # What this file is for
//!
//! That defence is load-bearing and is not obvious from either side: the
//! compiler's baking and the runtime's dispatch each look locally correct, and
//! the thing that reconciles them is a third piece of code at the boundary. If
//! it regresses, seven refusals become reachable at once and the failure is an
//! `InvalidBytecode` on a legitimate program.
//!
//! The tests below drive a boxed composite of each kind through a native and
//! assert the access WORKS. They are a regression guard on the canonicalization,
//! not a demonstration that the refusals are dead code.
//!
//! # What is deliberately NOT claimed
//!
//! Not that group D is unreachable. Seven shapes were tried and none reached a
//! refusal; the shapes are named below so a later reader knows what has been
//! ruled out rather than repeating it. The other route into these sites -- a
//! composite-typed re-entrant `yield` reply -- is refused earlier, by the typed
//! operand-stack pass at compile time, and is pinned separately below.

#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::Arena;
use keleusma::bytecode::{ArrayBody, Value};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmError};

/// A host native, as `register_native` takes it: a plain `fn` pointer.
type Native = fn(&[Value]) -> Result<Value, VmError>;

/// One probe: a label, the program, the native backing it, and the word the
/// program must return.
type Case = (&'static str, &'static str, Native, i64);

fn boxed_struct(a: i64, b: i64) -> Value {
    Value::struct_with_widths(
        "S".into(),
        vec![("a".into(), Value::Int(a)), ("b".into(), Value::Int(b))],
        8,
        8,
    )
}

fn mk_struct(_args: &[Value]) -> Result<Value, VmError> {
    Ok(boxed_struct(7, 9))
}

fn mk_tuple(_args: &[Value]) -> Result<Value, VmError> {
    Ok(Value::tuple_with_widths(
        vec![Value::Int(7), Value::Int(9)],
        8,
        8,
    ))
}

fn mk_struct_array(_args: &[Value]) -> Result<Value, VmError> {
    Ok(Value::Array(ArrayBody::boxed(vec![
        boxed_struct(7, 9),
        boxed_struct(11, 13),
    ])))
}

fn mk_enum(_args: &[Value]) -> Result<Value, VmError> {
    Ok(Value::enum_value(
        "E".into(),
        "B".into(),
        1,
        vec![Value::Int(7)],
    ))
}

/// Compile, verify, load, register the native, run. Every step is checked
/// separately, because the defect class this file guards produces a value that
/// passes the first three and fails inside the call.
fn run(src: &str, native: Native) -> Result<i64, String> {
    let tokens = tokenize(src).map_err(|e| format!("lex: {e:?}"))?;
    let program = parse(&tokens).map_err(|e| format!("parse: {e:?}"))?;
    let module = compile(&program).map_err(|e| format!("compile: {}", e.message))?;
    keleusma::verify::verify(&module).map_err(|e| format!("verify: {e:?}"))?;
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).map_err(|e| format!("load: {e:?}"))?;
    vm.register_native("host::mk", native);
    let mut shared = alloc::vec![0u8; vm.shared_data_bytes()];
    match vm.call_with_shared(&mut shared, &[Value::Int(0)]) {
        Ok(keleusma::vm::VmState::Finished(Value::Int(v))) => Ok(v),
        Ok(other) => Err(format!("unexpected state: {other:?}")),
        Err(e) => Err(format!("run: {e:?}")),
    }
}

extern crate alloc;

/// **The regression guard.** Each shape is a boxed composite crossing the host
/// boundary and then meeting a compiler-baked flat access.
///
/// A failure here reporting `InvalidBytecode` means the canonicalization at the
/// boundary has regressed and group D has opened: a legitimate program now
/// verifies, loads, and dies mid-call.
#[test]
fn a_boxed_composite_from_a_native_meets_a_baked_flat_access_and_works() {
    let cases: &[Case] = &[
        (
            "struct field",
            "use host::mk() -> S\nstruct S { a: Word, b: Word }\n\
             fn main(k: Word) -> Word { let s = host::mk(); s.a }",
            mk_struct,
            7,
        ),
        (
            "struct rebound then accessed",
            "use host::mk() -> S\nstruct S { a: Word, b: Word }\n\
             fn main(k: Word) -> Word { let s = host::mk(); let u = s; u.b }",
            mk_struct,
            9,
        ),
        (
            "tuple index",
            "use host::mk() -> (Word, Word)\n\
             fn main(k: Word) -> Word { let t = host::mk(); t.0 }",
            mk_tuple,
            7,
        ),
        (
            "index into an array of structs, then a field",
            "use host::mk() -> [S; 2]\nstruct S { a: Word, b: Word }\n\
             fn main(k: Word) -> Word { let xs = host::mk(); xs[1].a }",
            mk_struct_array,
            11,
        ),
        (
            "enum payload through a match",
            "use host::mk() -> E\nenum E { A, B(Word) }\n\
             fn main(k: Word) -> Word { let e = host::mk(); match e { E::B(v) => v, _ => 0 } }",
            mk_enum,
            7,
        ),
    ];

    // NON-VACUOUS: a corpus that shrank, or cases that stopped reaching the
    // runtime, would satisfy an all-pass loop while establishing nothing.
    assert!(
        cases.len() >= 5,
        "the corpus shrank to {} cases",
        cases.len()
    );

    for (name, src, native, expected) in cases {
        match run(src, *native) {
            Ok(got) => assert_eq!(
                got, *expected,
                "the `{name}` case ran but returned the wrong value, so the boundary \
                 conversion is producing a body that reads at the wrong offsets -- which is \
                 worse than the refusal this file guards, because it is silent"
            ),
            Err(e) => panic!(
                "the `{name}` case failed: {e}. If this is an InvalidBytecode about an operand \
                 form not matching a body, the canonicalization at the host boundary has \
                 regressed and group D of the InvalidBytecode census has opened."
            ),
        }
    }
}

/// **The other route into group D is closed earlier, at compile time.**
///
/// A composite-typed re-entrant `yield` reply is one of the two shapes the typed
/// operand-stack pass is documented to defer on. It does not reach a runtime
/// refusal, because the pass rejects the module outright.
///
/// Pinned because "the typed pass defers here" reads like an open door, and this
/// records that for a composite reply it is a closed one. If these begin to
/// compile, the deferred operand meets the baked access at run time and group D
/// needs re-examining.
#[test]
fn a_composite_typed_yield_reply_is_refused_at_compile_time() {
    let cases: &[(&str, &str)] = &[
        (
            "struct reply",
            "struct S { a: Word, b: Word }\n\
             loop main(k: Word) -> Word { let s: S = yield 0; s.a }",
        ),
        (
            "tuple reply",
            "loop main(k: Word) -> Word { let t: (Word, Word) = yield 0; t.0 }",
        ),
        (
            "array reply",
            "loop main(k: Word) -> Word { let a: [Word; 2] = yield 0; a[1] }",
        ),
    ];

    for (name, src) in cases {
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let err = compile(&program)
            .err()
            .map(|e| e.message)
            .unwrap_or_else(|| {
                panic!(
                    "the `{name}` case now COMPILES. A composite yield reply is host-supplied and \
                 its shape is deferred by the typed pass, so it now meets a compiler-baked \
                 access at run time. Re-examine group D of the InvalidBytecode census: this \
                 was the route that was closed."
                )
            });
        assert!(
            err.contains("typed operand-stack") || err.contains("ExpectedComposite"),
            "the `{name}` case is refused for a different reason now ({err}), so this test no \
             longer measures the typed pass and the route may have reopened elsewhere"
        );
    }
}

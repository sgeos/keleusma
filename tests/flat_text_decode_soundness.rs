#![cfg(all(feature = "compile", feature = "verify"))]
//! Soundness of decoding a flat `Text` field, reported by the pykeleusma line
//! against keleusma 0.2.2 and reproduced on v0.2.3 on 2026-10-25.
//!
//! Two defects combined into undefined behaviour reached through safe APIs.
//! The type checker did not unify a `yield` expression with the declared
//! return type, so a same-size body of another layout was admitted. And
//! `Vm::decode` rebuilt a `Text` field's two words as a pointer without
//! checking them, so any layout mismatch between the body and the host's
//! chosen type read a scalar as an address. Before the fix both cases below
//! ended the test process with SIGSEGV. The controls show that legitimate
//! constant and host-passed strings still decode.
//!
//! The controls avoid placing a resume value inside a composite literal. On
//! 2026-10-25 `let n = yield (..); ("x", n)` was refused at load time with
//! `NewCompositeSizeMismatch`, before and after this fix. That is a separate,
//! conservative defect recorded in the V0.2.X process records.

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{Vm, VmState};
use keleusma::{Arena, Value, VmError};

fn compile_src(src: &str) -> Result<keleusma::bytecode::Module, String> {
    let tokens = tokenize(src).map_err(|e| format!("{e:?}"))?;
    let program = parse(&tokens).map_err(|e| format!("{e:?}"))?;
    compile(&program).map_err(|e| e.message)
}

fn first_yield<'a>(src: &str, arena: &'a Arena, arg: Value) -> (Vm<'static, 'a>, Value) {
    let module = compile_src(src).expect("compiles");
    let mut vm = Vm::new(module, arena).expect("verifies");
    match vm.call(&[arg]).expect("call") {
        VmState::Yielded(v) => (vm, v),
        other => panic!("expected a yield, got {other:?}"),
    }
}

#[test]
fn a_yield_of_another_type_is_rejected() {
    for src in [
        // The reported program. (Word, Text) has the size of (Text, Word).
        "loop main(e: Word) -> (Text, Word) { if e > 0 { yield (\"a\", 1) } else { yield (3, \"wake\") } }",
        "loop main(e: Word) -> (Text, Word) { let n = yield (3, \"wake\"); n }",
        "loop main(e: Word) -> Word { let n = yield true; n }",
        "yield g() -> Word { yield \"x\" }\nfn main() -> Word { 0 }",
    ] {
        let err = compile_src(src).expect_err(src);
        assert!(err.contains("yielded value has type"), "{src}: {err}");
    }
}

#[test]
fn a_yield_of_the_declared_type_is_accepted() {
    for src in [
        "loop main(e: Word) -> (Text, Word) { if e > 0 { yield (\"a\", 1) } else { yield (\"b\", 2) } }",
        "loop main(e: Word) -> Word { let n = yield e + 1; n }",
        "yield g() -> Word { yield 7 }\nloop main(r: Word) -> Word { let x = g(); yield x }",
    ] {
        compile_src(src).unwrap_or_else(|e| panic!("{src}: {e}"));
    }
}

#[test]
fn decoding_with_a_mismatched_host_type_is_an_error_not_a_crash() {
    let arena = Arena::with_capacity(65536);
    let (vm, v) = first_yield(
        "loop main(e: Word) -> (Word, Text) { let n = yield (3, \"wake\"); n }",
        &arena,
        Value::Int(0),
    );
    // The body is (Word, Text). Reading it as (Text, Word) takes the Word 3
    // as a string's data pointer.
    match vm.decode::<(String, i64)>(&v) {
        Err(VmError::TypeError(m)) => assert!(m.contains("does not reference memory"), "{m}"),
        other => panic!("expected a TypeError, got {other:?}"),
    }
    // The correct type still decodes the same body.
    assert_eq!(
        vm.decode::<(i64, String)>(&v).expect("decode"),
        (3, "wake".into())
    );
}

#[test]
fn a_constant_string_in_a_yielded_body_decodes() {
    let arena = Arena::with_capacity(65536);
    let (vm, v) = first_yield(
        "loop main(e: Word) -> (Text, Word) { let n = yield (\"send\", e); (\"x\", 0) }",
        &arena,
        Value::Int(5),
    );
    assert_eq!(
        vm.decode::<(String, i64)>(&v).expect("decode"),
        ("send".into(), 5)
    );
}

#[test]
fn a_host_passed_string_in_a_yielded_body_decodes() {
    let arena = Arena::with_capacity(65536);
    let (vm, v) = first_yield(
        "loop main(e: Text) -> (Text, Text) { yield (\"echo\", e) }",
        &arena,
        Value::StaticStr("from the host".into()),
    );
    assert_eq!(
        vm.decode::<(String, String)>(&v).expect("decode"),
        ("echo".into(), "from the host".into())
    );
}

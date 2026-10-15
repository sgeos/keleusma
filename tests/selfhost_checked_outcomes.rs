//! Source-level controls for checked arithmetic and its scoped outcome bindings.
#![cfg(feature = "self-host")]

use keleusma::Arena;
use keleusma::bytecode::{Module, Value};
use keleusma::selfhost::{compile_src, self_hosted_compile, try_parse_functions};
use keleusma::target::Target;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

fn execute(module: Module) -> Result<(u8, i64), keleusma::vm::VmError> {
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).unwrap();
    let mut vm = Vm::new(module, &arena).unwrap();
    match vm.call(&[])? {
        VmState::Finished(Value::Int(n)) => Ok((3, n)),
        VmState::Finished(Value::Byte(n)) => Ok((2, i64::from(n))),
        other => panic!("unexpected execution {other:?}"),
    }
}

#[test]
fn source_outcomes_match_reference_artifacts_and_execution() {
    let cases = [
        "fn main() -> Word { 7 / 0 { ok(v) => v, zero_divisor(n) => n } }",
        "fn main() -> Word { 7 / 0 { ok(v) => v, zero_divisor(n) when n == 9 => n } }",
        "fn main() -> Word { (bnot 0 lsr 1) + 1 { overflow(h, l) => h + l, ok(v) => v } }",
        "fn main() -> Byte { (255 as Byte) + (1 as Byte) { ok(v) => v, overflow(n) => 7 as Byte } }",
        "fn main() -> Word { 7 % 0 { ok(v) => v, zero_divisor(7) => 9 } }",
        "fn main() -> Word { 8 % 0 { ok(v) => v, zero_divisor(7) => 9 } }",
        "fn main() -> Word { (bnot 0 lsr 1) * 2 { overflow(h, l) when h == 0 => l, overflow(_, l) => l, ok(v) => v } }",
        "fn main() -> Word { (1 lsl 63) - 1 { underflow(h, l) => h + l, ok(v) => v } }",
        "fn main() -> Word { (1 lsl 63) / -1 { overflow(0, l) => l, overflow(h, l) => h + l, ok(v) => v } }",
        "fn main() -> Byte { (0 as Byte) - (1 as Byte) { underflow(v) => v, ok(v) => v } }",
        "fn main() -> Word { -7 { ok(v) => v, overflow(_, l) => l } }",
        "fn main() -> Word { let n = 99; let x = 7 / 0 { ok(n) => n, zero_divisor(n) => n }; n + x }",
        "fn main() -> Word { 7 / 0 { ok(v) => v, zero_divisor(n) => 9 / 0 { ok(v) => v, zero_divisor(v) => n + v } } }",
        "fn main() -> Word { 7 / 0 { ok(v) => if true { v } else { 1 }, zero_divisor(n) when if true { n == 7 } else { false } => if n == 7 { let x = n + 1; x } else { 0 } } }",
        "fn f(x: Word, y: Word) -> Word { x + y } fn main() -> Word { f(7 / 0 { ok(v) => v, zero_divisor(n) => f(n, 2) }, 3) }",
        "fn main() -> Word { if (7 / 0 { ok(v) => v, zero_divisor(n) => n }) == 7 { 1 } else { 0 } }",
        "fn main() -> Word { 7 / 0 { ok(v) => v, zero_divisor(n) => match n { 7 => 9, _ => 0 } } }",
        "fn f(n: Word) -> Word when (n / 1 { ok(v) => v }) > 0 { n } fn f(n: Word) -> Word { 0 } fn main() -> Word { f(7) }",
        "fn main() -> Word { -7 % 0 { ok(v) => v, zero_divisor(-7) => 9, zero_divisor(_) => 1 } }",
    ];
    let mut failures = Vec::new();
    for source in cases {
        let reference = compile_src(source);
        match self_hosted_compile(source, &Target::host()) {
            Ok(actual) => {
                assert_eq!(
                    actual.to_bytes().unwrap(),
                    reference.to_bytes().unwrap(),
                    "{source}"
                );
                assert_eq!(
                    format!("{:?}", execute(actual)),
                    format!("{:?}", execute(reference)),
                    "{source}"
                );
            }
            Err(error) => {
                failures.push(format!("{source}\n{error}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn parser_independently_rejects_invalid_arithmetic_outcomes() {
    for source in [
        "fn main() -> Word { 7 / 0 { ok(v) => v, ok(w) => w } }",
        "fn main() -> Word { 7 / 0 { ok(v) => v, zero_divisor(n) => n, zero_divisor(_) when true => 0 } }",
        "fn main() -> Word { 7 / 0 { overflow(h, l) => l } }",
        "fn main() -> Word { 7 / 0 { ok(v) when v > 0 => v } }",
        "fn main() -> Word { 7 / 0 { ok(1) => 1 } }",
        "fn main() -> Word { 7 % 0 { ok(v) => v, overflow(h, l) => l } }",
        "fn main() -> Word { 1 + 2 { ok(v) => v, overflow(v) => v } }",
        "fn main() -> Byte { (1 as Byte) + (2 as Byte) { ok(v) => v, overflow(h, l) => l } }",
        "fn main() -> Word { 7 / 0 { ok(v) => v, zero_divisor(n) => n }; n }",
        "fn main() -> Word { 7 { ok(v) => v } }",
        "fn main() -> Word { 7 / 0 { ok(v) => v",
        "fn main() -> Word { 7 / 0 { ok(v) => v,",
        "fn main() -> Word { 7 / 0 { ok(v) => v, zero_divisor(, n) => n } }",
    ] {
        assert!(
            try_parse_functions(source).is_err(),
            "parser admitted {source}"
        );
    }
}

#[test]
fn checked_context_capacity_is_guarded_before_writing() {
    let nested = |depth: usize| {
        let mut source = String::from("fn main() -> Word { ");
        for _ in 0..depth {
            source.push_str("1 + 2 { ok(_) => ");
        }
        source.push('0');
        for _ in 0..depth {
            source.push('}');
        }
        source.push('}');
        source
    };
    assert!(try_parse_functions(&nested(32)).is_ok());
    let error = try_parse_functions(&nested(33)).unwrap_err();
    assert!(
        error.to_string().contains("nesting 33 exceeds 32"),
        "{error}"
    );
}

#[test]
fn outcome_guards_and_results_reach_the_type_stage() {
    for source in [
        "fn main() -> Word { 7 / 0 { ok(v) => v, zero_divisor(n) when n => n } }",
        "fn main() -> Word { 7 / 0 { ok(v) => v, zero_divisor(n) => true } }",
    ] {
        assert!(try_parse_functions(source).is_ok(), "syntax {source}");
        assert!(
            self_hosted_compile(source, &Target::host()).is_err(),
            "types {source}"
        );
    }
}

#[test]
fn unsupported_checked_families_are_not_source_rejections() {
    use keleusma::selfhost_host::{PARSE_DIAG_TAG_BASE, ParseRecordError};
    for code in [41, 42] {
        assert!(
            !ParseRecordError::Diagnostic {
                code: PARSE_DIAG_TAG_BASE - code,
                detail: 0
            }
            .is_source_rejection()
        );
    }
    assert!(
        ParseRecordError::Diagnostic {
            code: PARSE_DIAG_TAG_BASE - 40,
            detail: 0
        }
        .is_source_rejection()
    );
    let error =
        try_parse_functions("fn main() -> Word { 1 lsl 2 { ok(v) => v, overflow(h, l) => l } }")
            .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("outside the implemented arithmetic families"),
        "{error}"
    );
}

#[test]
fn lexer_source_capacity_accepts_the_boundary_and_refuses_the_next_byte() {
    let cap = keleusma::selfhost_host::LEX_SOURCE_CAP;
    let exact = " ".repeat(cap);
    assert_eq!(keleusma::selfhost::lex_token_count(&exact), 0);
    let oversized = " ".repeat(cap + 1);
    let error = try_parse_functions(&oversized).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("exceeding lexer source capacity"),
        "{error}"
    );
}

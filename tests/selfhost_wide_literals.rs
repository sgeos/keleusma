//! Full-width literal values must survive the packed token and record transports.
#![cfg(feature = "self-host")]

use keleusma::selfhost::{compile_src, lex_token_trace, self_hosted_compile};
use keleusma::target::Target;

#[test]
fn wide_literals_preserve_complete_arithmetic_artifacts() {
    for source in [
        "fn main() -> Word { 9223372036854775807 }",
        "fn main() -> Word { 36028797018963967 + 1 }",
        "fn main() -> Word { 36028797018963968 + 1 }",
        "fn main() -> Word { 9223372036854775807 + 1 { overflow(h, l) => h + l, ok(v) => v } }",
        "fn main() -> Word { -9223372036854775807 - 2 { underflow(-1, l) => l, ok(v) => v } }",
        "fn main() -> Word { 9223372036854775807 / 0 { ok(v) => v, zero_divisor(9223372036854775807) => 7, zero_divisor(_) => 9 } }",
        "fn main() -> Word { -9223372036854775807 / 0 { ok(v) => v, zero_divisor(-9223372036854775807) => 7, zero_divisor(_) => 9 } }",
        "fn main() -> Word { 0x7fffffffffffffff + 1 { overflow(_, l) => l, ok(v) => v } }",
        "fn main() -> Word { let n = 9223372036854775807; n - 9223372036854775807 }",
        "fn main() -> Word { let text = \"9223372036854775807\"; 9223372036854775807 }",
        "const data d { n: Word = 9223372036854775807 } fn main() -> Word { d.n }",
        "const data d { n: Word = -1 } fn main() -> Word { d.n }",
        "const data d { n: Word = -9223372036854775807 } fn main() -> Word { d.n }",
        "fn main() -> Word { match 9223372036854775807 { 9223372036854775807 => 1, _ => 0 } }",
    ] {
        let reference = compile_src(source);
        let actual = self_hosted_compile(source, &Target::host())
            .unwrap_or_else(|error| panic!("{source}\n{error}"));
        assert_eq!(
            actual.to_bytes().unwrap(),
            reference.to_bytes().unwrap(),
            "{source}"
        );
    }
}

#[test]
fn full_width_integer_tokens_flush_at_end_of_input() {
    for source in [
        "9223372036854775807",
        "0x7fffffffffffffff",
        "0b111111111111111111111111111111111111111111111111111111111111111",
    ] {
        let (_, tokens) = lex_token_trace(source);
        assert_eq!(tokens, [(12, i64::MAX)], "{source}");
    }
}

#[test]
fn overflowing_integer_tokens_are_refused_before_parsing() {
    for source in [
        "9223372036854775808",
        "18446744073709551616",
        "0x8000000000000000",
        "0xffffffffffffffff",
        "0b1000000000000000000000000000000000000000000000000000000000000000",
        "0x",
        "0b",
    ] {
        assert!(
            keleusma::lexer::tokenize(source).is_err(),
            "reference {source}"
        );
        assert!(
            std::panic::catch_unwind(|| lex_token_trace(source)).is_err(),
            "pipeline {source}"
        );
    }
}

#[test]
fn wide_integer_table_shares_the_intern_capacity() {
    let cap = keleusma::selfhost_host::LEX_INTERN_CAP;
    let mut source = (0..cap - 1).map(|i| format!("n{i} ")).collect::<String>();
    source.push_str("9223372036854775807");
    let (names, tokens) = lex_token_trace(&source);
    assert_eq!(names.len(), cap);
    assert_eq!(tokens.last(), Some(&(12, i64::MAX)));
    source.push_str(" 9223372036854775807");
    let (names, tokens) = lex_token_trace(&source);
    assert_eq!(names.len(), cap);
    assert_eq!(tokens.last(), Some(&(12, i64::MAX)));
    source.push_str(" 9223372036854775806");
    assert!(std::panic::catch_unwind(|| lex_token_trace(&source)).is_err());
}

#[test]
fn collected_and_fused_feeds_transport_the_same_full_width_value() {
    let source =
        "fn main() -> Word { 9223372036854775807 + 1 { overflow(_, l) => l, ok(v) => v } }";
    let collected = keleusma::selfhost::parse_record_trace(source);
    let fused = keleusma::selfhost::parse_record_trace_fused(source);
    assert_eq!(collected, fused);
    assert!(
        fused
            .1
            .iter()
            .any(|&(kind, value, _)| kind == 1 && value == i64::MAX)
    );
    assert!(!keleusma::selfhost::parse_cursor_trace(source).is_empty());
}

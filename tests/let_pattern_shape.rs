//! A `let` destructuring pattern is checked against the value's type.
//!
//! The operator ruled on 2026-10-31 that a destructuring pattern whose shape
//! does not match the value's type is refused at compile time, in the type
//! checker. Before the repair the type checker bound such a pattern without
//! checking it unless the value carried a `Text<N>`, so `let (a, b) = 5`
//! compiled and faulted at run time with `cannot tuple-index Int`, and
//! `let (a, b) = (1, 2, 3)` compiled and faulted on the tuple body.
//!
//! The controls show that a matching pattern still compiles and runs, so the
//! refusals are not a blanket refusal of destructuring.
#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::Arena;
use keleusma::bytecode::Value;
use keleusma::vm::{Vm, VmState};

fn parse(source: &str) -> keleusma::ast::Program {
    let tokens = keleusma::lexer::tokenize(source).expect("lex");
    keleusma::parser::parse(&tokens).expect("parse")
}

/// The type checker's verdict alone, so a refusal is attributed to it and not
/// to a later stage.
fn type_check(source: &str) -> Result<(), String> {
    keleusma::typecheck::check(&mut parse(source)).map_err(|e| e.message)
}

fn run_word(source: &str) -> i64 {
    let module = keleusma::compiler::compile(&parse(source))
        .unwrap_or_else(|e| panic!("{source}: {}", e.message));
    let need = keleusma::vm::required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(1 << 20);
    arena.resize_persistent(need).unwrap();
    let mut vm = Vm::new(module, &arena).unwrap_or_else(|e| panic!("{source}: {e:?}"));
    match vm.call(&[]) {
        Ok(VmState::Finished(Value::Int(n))) => n,
        other => panic!("{source}: {other:?}"),
    }
}

#[test]
fn a_tuple_pattern_against_a_non_tuple_is_refused_by_the_type_checker() {
    let cases = [
        ("fn main() -> Word { let (a, b) = 5; a }", "Word"),
        (
            "fn main() -> Word { let (a, b) = Option::Some(1); 0 }",
            "Option",
        ),
        ("fn main() -> Word { let (a, b) = true; 0 }", "bool"),
        // A nested tuple pattern against a scalar element.
        (
            "fn main() -> Word { let ((a, b), c) = (1, 2); a + b + c }",
            "Word",
        ),
    ];
    for (source, named) in cases {
        let err = type_check(source).expect_err(source);
        assert!(
            err.contains("tuple pattern does not match") && err.contains(named),
            "{source}: {err}"
        );
        assert!(
            keleusma::compiler::compile(&parse(source)).is_err(),
            "{source}"
        );
    }
}

#[test]
fn a_tuple_pattern_of_the_wrong_arity_is_refused_by_the_type_checker() {
    for (source, pattern, value) in [
        ("fn main() -> Word { let (a, b) = (1, 2, 3); a + b }", 2, 3),
        (
            "fn main() -> Word { let (a, b, c) = (1, 2); a + b + c }",
            3,
            2,
        ),
        (
            "fn main() -> Word { let (a, (b, c)) = (1, (2, 3, 4)); a + b + c }",
            2,
            3,
        ),
    ] {
        let err = type_check(source).expect_err(source);
        assert!(
            err.contains(&format!("tuple pattern of {pattern} elements"))
                && err.contains(&format!("of {value} elements")),
            "{source}: {err}"
        );
    }
}

#[test]
fn a_struct_or_enum_pattern_against_another_type_is_refused_by_the_type_checker() {
    for (source, expected) in [
        (
            "struct P { x: Word, y: Word }\nfn main() -> Word { let P { x, y } = 5; x + y }",
            "struct pattern `P` does not match",
        ),
        (
            "struct P { x: Word, y: Word }\nstruct Q { x: Word, y: Word }\n\
             fn main() -> Word { let P { x, y } = Q { x: 1, y: 2 }; x + y }",
            "struct pattern `P` does not match",
        ),
        (
            "enum E { A(Word), B }\nfn main() -> Word { let E::A(x) = (1, 2); x }",
            "enum pattern `E::A` does not match",
        ),
        (
            "fn main() -> Word { let (a, 3) = (1, true); a }",
            "literal pattern of type Word does not match",
        ),
    ] {
        let err = type_check(source).expect_err(source);
        assert!(err.contains(expected), "{source}: {err}");
    }
}

#[test]
fn a_matching_destructuring_pattern_still_compiles_and_runs() {
    for (source, expected) in [
        ("fn main() -> Word { let (a, b) = (1, 2); a + b }", 3),
        (
            "fn main() -> Word { let (a, (b, c)) = (1, (2, 3)); a * 100 + b * 10 + c }",
            123,
        ),
        (
            "fn pair() -> (Word, Word) { (4, 5) }\nfn main() -> Word { let (a, _) = pair(); a }",
            4,
        ),
        (
            "fn main() -> Word { let (a, b): (Word, Word) = (6, 7); a * 10 + b }",
            67,
        ),
    ] {
        assert_eq!(type_check(source), Ok(()), "{source}");
        assert_eq!(run_word(source), expected, "{source}");
    }
}

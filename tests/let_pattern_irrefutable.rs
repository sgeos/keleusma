//! A `let` pattern must be irrefutable for the type of its value, and a struct
//! pattern destructures in a `let` as a tuple pattern does.
//!
//! The operator ruled on 2026-10-31 on two follow-ups to the pattern shape
//! check in `let_pattern_shape.rs`. First, a refutable `let` pattern is refused
//! at compile time with a message that points to `match`. Before the repair
//! `let 3 = 5` compiled and matched nothing, because the compiler bound any
//! pattern other than a name, a wildcard or a tuple as one hidden local. A
//! literal pattern, an enum variant pattern of an enum with more than one
//! variant, `Option::Some` and `Option::None`, and any pattern holding one of
//! these are refutable. A name, a wildcard, a tuple or struct pattern of
//! irrefutable parts, and a variant pattern of a single-variant enum are not.
//!
//! Second, `let P { x, y } = p` binds the fields. Before the repair it was
//! refused with `undefined variable: x`, because the compiler bound the struct
//! pattern as that same hidden local. Field renaming, wildcard fields, a
//! subset of the fields and nesting follow the struct pattern a `match` arm
//! accepts, which `docs/spec/GRAMMAR.md` section 8 defines for every pattern
//! position. The controls run each admitted program and check its value, so a
//! refusal of everything or a binding of the wrong field would fail here.
#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::Arena;
use keleusma::bytecode::Value;
use keleusma::vm::{Vm, VmState};

fn parse(source: &str) -> keleusma::ast::Program {
    let tokens = keleusma::lexer::tokenize(source).expect("lex");
    keleusma::parser::parse(&tokens).expect("parse")
}

/// The type checker's verdict alone, so a refusal is attributed to it.
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

const P: &str = "struct P { x: Word, y: Word }\n";
const E: &str = "enum E { A(Word), B }\n";
const W: &str = "enum W { A(Word, Word) }\n";

#[test]
fn a_refutable_let_pattern_is_refused_and_points_to_match() {
    let cases = [
        String::from("fn main() -> Word { let 3 = 5; 7 }"),
        String::from("fn main() -> Word { let true = true; 7 }"),
        String::from("fn main() -> Word { let (a, 3) = (1, 2); a }"),
        String::from("fn main() -> Word { let (a, (b, 0)) = (1, (2, 3)); a + b }"),
        format!("{E}fn main() -> Word {{ let E::A(x) = E::A(4); x }}"),
        format!("{E}fn main() -> Word {{ let E::B = E::B; 7 }}"),
        format!("{E}fn main() -> Word {{ let (a, E::B) = (1, E::B); a }}"),
        String::from("fn main() -> Word { let Option::Some(x) = Option::Some(4); x }"),
        String::from(
            "fn main() -> Word { let o: Option<Word> = Option::None; let Option::None = o; 7 }",
        ),
        format!("{P}fn main() -> Word {{ let P {{ x: 0, y }} = P {{ x: 0, y: 2 }}; y }}"),
        // A literal inside a single-variant enum payload is still refutable.
        format!("{W}fn main() -> Word {{ let W::A(x, 0) = W::A(3, 0); x }}"),
        // A unit variant of a payload-free enum matched against a `Word`, the
        // discriminant form the type checker admits in a match arm.
        String::from("enum K { A }\nfn main() -> Word { let K::A = 0; 7 }"),
    ];
    for source in &cases {
        let err = type_check(source).expect_err(source);
        assert!(
            err.contains("refutable") && err.contains("match"),
            "{source}: {err}"
        );
        let compiled = keleusma::compiler::compile(&parse(source));
        assert!(compiled.is_err(), "{source}");
    }
}

#[test]
fn a_refutable_pattern_in_a_match_arm_is_still_admitted() {
    for (source, expected) in [
        (
            String::from("fn main() -> Word { match 5 { 3 => 1, _ => 2 } }"),
            2,
        ),
        (
            format!("{E}fn main() -> Word {{ match E::A(4) {{ E::A(x) => x, E::B => 0 }} }}"),
            4,
        ),
    ] {
        assert_eq!(type_check(&source), Ok(()), "{source}");
        assert_eq!(run_word(&source), expected, "{source}");
    }
}

#[test]
fn an_irrefutable_let_pattern_is_admitted_and_binds() {
    for (source, expected) in [
        (String::from("fn main() -> Word { let x = 5; x }"), 5),
        (String::from("fn main() -> Word { let _ = 5; 7 }"), 7),
        (
            String::from("fn main() -> Word { let (a, (b, _)) = (1, (2, 3)); a * 10 + b }"),
            12,
        ),
        (
            format!("{W}fn main() -> Word {{ let W::A(x, y) = W::A(3, 4); x * 10 + y }}"),
            34,
        ),
        (
            format!("{W}fn main() -> Word {{ let W::A(_, y) = W::A(3, 4); y }}"),
            4,
        ),
        (
            format!(
                "{W}fn mk(n: Word) -> W {{ W::A(n, n + 1) }}\nfn main() -> Word {{ let (W::A(a, b), c) = (mk(5), 9); a * 100 + b * 10 + c }}"
            ),
            569,
        ),
    ] {
        assert_eq!(type_check(&source), Ok(()), "{source}");
        assert_eq!(run_word(&source), expected, "{source}");
    }
}

#[test]
fn a_struct_pattern_destructures_in_a_let() {
    for (source, expected) in [
        // The shorthand form, the form the defect report named.
        (
            format!(
                "{P}fn main() -> Word {{ let p = P {{ x: 1, y: 2 }}; let P {{ x, y }} = p; x * 10 + y }}"
            ),
            12,
        ),
        // Field renaming, and a pattern written out of declaration order.
        (
            format!(
                "{P}fn main() -> Word {{ let P {{ y: b, x: a }} = P {{ x: 1, y: 2 }}; a * 10 + b }}"
            ),
            12,
        ),
        // A wildcard field and a subset of the fields.
        (
            format!("{P}fn main() -> Word {{ let P {{ x: _, y }} = P {{ x: 1, y: 2 }}; y }}"),
            2,
        ),
        (
            format!("{P}fn main() -> Word {{ let P {{ y }} = P {{ x: 1, y: 2 }}; y }}"),
            2,
        ),
        // The value from a call and from a parameter.
        (
            format!(
                "{P}fn mk(n: Word) -> P {{ P {{ x: n, y: n + 1 }} }}\nfn main() -> Word {{ let P {{ x, y }} = mk(4); x * 10 + y }}"
            ),
            45,
        ),
        (
            format!(
                "{P}fn sum(p: P) -> Word {{ let P {{ x, y }} = p; x + y }}\nfn main() -> Word {{ sum(P {{ x: 3, y: 4 }}) }}"
            ),
            7,
        ),
        // A struct nested in a struct, a tuple in a struct and a struct in a tuple.
        (
            format!(
                "{P}struct Q {{ p: P, z: Word }}\nfn main() -> Word {{ let Q {{ p: P {{ x, y }}, z }} = Q {{ p: P {{ x: 1, y: 2 }}, z: 3 }}; x * 100 + y * 10 + z }}"
            ),
            123,
        ),
        (
            String::from(
                "struct T { pair: (Word, Word), z: Word }\nfn main() -> Word { let T { pair: (a, b), z } = T { pair: (1, 2), z: 3 }; a * 100 + b * 10 + z }",
            ),
            123,
        ),
        (
            format!(
                "{P}fn main() -> Word {{ let (P {{ x, y }}, z) = (P {{ x: 1, y: 2 }}, 3); x * 100 + y * 10 + z }}"
            ),
            123,
        ),
        // Mixed scalar fields, which the flat layout packs at different widths.
        (
            String::from(
                "struct M { b: Byte, f: bool, w: Word }\nfn main() -> Word { let M { b, f, w } = M { b: 7 as Byte, f: true, w: 9 }; if f { (b as Word) * 10 + w } else { 0 } }",
            ),
            79,
        ),
    ] {
        assert_eq!(type_check(&source), Ok(()), "{source}");
        assert_eq!(run_word(&source), expected, "{source}");
    }
}

#[test]
fn a_struct_let_pattern_with_an_unknown_field_is_refused() {
    let source = format!("{P}fn main() -> Word {{ let P {{ x, z }} = P {{ x: 1, y: 2 }}; x }}");
    let err = type_check(&source).expect_err(&source);
    assert!(err.contains("has no field `z`"), "{source}: {err}");
}

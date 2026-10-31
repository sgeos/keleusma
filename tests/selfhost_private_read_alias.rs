//! The self-hosted compiler copies the same private composite reads as the
//! reference, or refuses the program.
//!
//! The reference decides which reads to copy over its copy-free ops, and
//! `codegen.kel` runs the same rule over its own op buffer, which holds the
//! same ops for every program both compile. This file checks that the two
//! agree byte for byte on the programs of `tests/private_read_alias.rs` that
//! the self-hosted subset admits, that the rest are refused rather than
//! compiled differently, and that the copy pass refuses past its capacity.
#![cfg(feature = "self-host")]

use keleusma::bytecode::{ArrayElem, Module, NewCompositeOperand, Op};
use keleusma::value_layout::CompositeKind;
use std::panic::{AssertUnwindSafe, catch_unwind};

const P: &str = "struct P { x: Word, y: Word }\n";

fn reference(source: &str) -> Module {
    let tokens = keleusma::lexer::tokenize(source).expect("lex");
    let program = keleusma::parser::parse(&tokens).expect("parse");
    keleusma::compiler::compile(&program).unwrap_or_else(|e| panic!("{source}: {}", e.message))
}

fn copies(ops: &[Op]) -> usize {
    ops.windows(4)
        .filter(|w| {
            matches!(
                w,
                [
                    Op::NewComposite(NewCompositeOperand::Flat {
                        kind: CompositeKind::Array,
                        count: 1,
                        ..
                    }),
                    Op::PushImmediate(4),
                    Op::GetIndex(ArrayElem::FlatNested { .. }),
                    Op::PopN(6),
                ]
            )
        })
        .count()
}

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    /// Every chunk's ops, pool and frame size equal the reference's.
    Identical,
    /// The self-hosted pipeline declined the program.
    Refused,
    /// The self-hosted pipeline compiled the program to other bytes.
    Diverges,
}

fn outcome(source: &str) -> Outcome {
    let owned = source.to_string();
    let Ok(module) = catch_unwind(AssertUnwindSafe(|| {
        keleusma::selfhost::self_host_compile(&owned)
    })) else {
        return Outcome::Refused;
    };
    let reference = reference(source);
    let same = module.chunks.len() == reference.chunks.len()
        && module.chunks.iter().zip(&reference.chunks).all(|(m, r)| {
            m.name == r.name
                && m.ops == r.ops
                && m.constants == r.constants
                && m.local_count == r.local_count
        });
    if same {
        Outcome::Identical
    } else {
        Outcome::Diverges
    }
}

/// The programs and the outcome each must have, with the number of copies the
/// reference emits across the module.
fn cases() -> Vec<(&'static str, String, Outcome, usize)> {
    let s = format!("{P}private data d {{ s: P }} ");
    let st = format!("{P}private data d {{ s: P, t: Word }} ");
    vec![
        (
            "whole-slot reproducer",
            format!(
                "{s}fn main() -> Word {{ d.s = P {{ x: 3, y: 4 }}; let o = d.s; \
                 d.s = P {{ x: 1, y: 2 }}; o.x * 10 + o.y }}"
            ),
            Outcome::Identical,
            1,
        ),
        (
            "nested view",
            "struct I { a: Word }\nstruct Q { i: I, y: Word }\nprivate data d { s: Q } \
             fn main() -> Word { d.s = Q { i: I { a: 3 }, y: 4 }; let o = d.s.i; \
             d.s = Q { i: I { a: 1 }, y: 2 }; o.a }"
                .to_string(),
            Outcome::Identical,
            1,
        ),
        (
            "self assignment",
            format!("{s}fn main() -> Word {{ d.s = P {{ x: 3, y: 4 }}; d.s = d.s; d.s.x }}"),
            Outcome::Identical,
            1,
        ),
        (
            "getter return",
            format!(
                "{s}fn get() -> P {{ d.s }} fn main() -> Word {{ d.s = P {{ x: 3, y: 4 }}; \
                 let o = get(); d.s = P {{ x: 1, y: 2 }}; o.x * 10 + o.y }}"
            ),
            Outcome::Identical,
            1,
        ),
        (
            "call argument",
            format!(
                "{s}fn f(o: P) -> Word {{ d.s = P {{ x: 1, y: 2 }}; o.x * 10 + o.y }} \
                 fn main() -> Word {{ d.s = P {{ x: 3, y: 4 }}; f(d.s) }}"
            ),
            Outcome::Identical,
            1,
        ),
        (
            "callee write",
            format!(
                "{s}fn w() -> Word {{ d.s = P {{ x: 1, y: 2 }}; 0 }} fn main() -> Word {{ \
                 d.s = P {{ x: 3, y: 4 }}; let o = d.s; let z = w(); o.x * 10 + o.y + z }}"
            ),
            Outcome::Identical,
            1,
        ),
        (
            "loop after the binding",
            format!(
                "{s}fn main() -> Word {{ d.s = P {{ x: 3, y: 4 }}; let o = d.s; \
                 for i in 0..3 limit 3 {{ d.s = P {{ x: i, y: i }}; }} o.x * 10 + o.y }}"
            ),
            Outcome::Identical,
            1,
        ),
        (
            "use inside a later loop",
            format!(
                "{st}fn main() -> Word {{ d.s = P {{ x: 3, y: 4 }}; d.t = 0; let o = d.s; \
                 for i in 0..2 limit 2 {{ d.t = d.t + o.x * 10 + o.y; d.s = P {{ x: 1, y: 2 }}; }} d.t }}"
            ),
            Outcome::Identical,
            1,
        ),
        (
            "binding inside the loop",
            format!(
                "{st}fn main() -> Word {{ d.s = P {{ x: 3, y: 4 }}; d.t = 0; \
                 for i in 0..2 limit 2 {{ let o = d.s; d.t = d.t + o.x * 10 + o.y; \
                 d.s = P {{ x: 1, y: 2 }}; }} d.t }}"
            ),
            Outcome::Identical,
            0,
        ),
        (
            "write in a branch",
            format!(
                "{s}fn main() -> Word {{ d.s = P {{ x: 3, y: 4 }}; let o = d.s; \
                 if o.x > 0 {{ d.s = P {{ x: 1, y: 2 }}; 0 }} else {{ 0 }}; o.x * 10 + o.y }}"
            ),
            Outcome::Identical,
            1,
        ),
        (
            "projection before the write",
            format!(
                "{s}fn main() -> Word {{ d.s = P {{ x: 3, y: 4 }}; let v = d.s.x; \
                 d.s = P {{ x: 1, y: 2 }}; v }}"
            ),
            Outcome::Identical,
            0,
        ),
        (
            "no write",
            format!(
                "{s}fn main() -> Word {{ d.s = P {{ x: 3, y: 4 }}; let o = d.s; o.x * 10 + o.y }}"
            ),
            Outcome::Identical,
            0,
        ),
        (
            "dead before the write",
            format!(
                "{s}fn main() -> Word {{ d.s = P {{ x: 3, y: 4 }}; let o = d.s; \
                 let v = o.x * 10 + o.y; d.s = P {{ x: 1, y: 2 }}; v }}"
            ),
            Outcome::Identical,
            0,
        ),
        (
            "used only in the replacing value",
            format!(
                "{s}fn main() -> Word {{ d.s = P {{ x: 3, y: 4 }}; let p = d.s; \
                 d.s = P {{ x: p.x + 1, y: p.y }}; d.s.x }}"
            ),
            Outcome::Identical,
            0,
        ),
        (
            "packed into a flat tuple first",
            format!(
                "{s}fn main() -> Word {{ d.s = P {{ x: 3, y: 4 }}; let t = (1, d.s); \
                 d.s = P {{ x: 1, y: 2 }}; t.1.x }}"
            ),
            Outcome::Identical,
            0,
        ),
        (
            "indexed element, outside the subset",
            format!(
                "{P}private data d {{ x: [P; 2] }} fn main() -> Word {{ d.x[1] = P {{ x: 3, y: 4 }}; \
                 let o = d.x[1]; d.x[1] = P {{ x: 1, y: 2 }}; o.x * 10 + o.y }}"
            ),
            Outcome::Refused,
            1,
        ),
        (
            "tuple slot, outside the subset",
            "private data d { s: (Word, Word) } fn main() -> Word { d.s = (3, 4); let o = d.s; \
             d.s = (1, 2); o.0 * 10 + o.1 }"
                .to_string(),
            Outcome::Refused,
            1,
        ),
    ]
}

#[test]
fn the_self_hosted_compiler_copies_the_same_reads_or_refuses() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let results: Vec<_> = cases()
        .into_iter()
        .map(|(label, source, expected, expected_copies)| {
            let got = outcome(&source);
            let module = reference(&source);
            let n: usize = module.chunks.iter().map(|c| copies(&c.ops)).sum();
            (label, got, expected, n, expected_copies)
        })
        .collect();
    std::panic::set_hook(prev);
    let mut mismatches = Vec::new();
    for (label, got, expected, n, expected_copies) in &results {
        if got != expected {
            mismatches.push(format!("{label}: expected {expected:?}, got {got:?}"));
        }
        if n != expected_copies {
            mismatches.push(format!(
                "{label}: the reference emits {n} copies, expected {expected_copies}"
            ));
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
    // Non-vacuity in both directions. Byte identity has to be shown on programs
    // that copy and on programs that do not, or agreement could mean that
    // neither side copies anything.
    let identical_with = results
        .iter()
        .filter(|r| r.1 == Outcome::Identical && r.3 > 0)
        .count();
    let identical_without = results
        .iter()
        .filter(|r| r.1 == Outcome::Identical && r.3 == 0)
        .count();
    assert!(
        identical_with >= 8,
        "identical copying programs: {identical_with}"
    );
    assert!(
        identical_without >= 5,
        "identical copy-free programs: {identical_without}"
    );
}

/// The copy pass holds 64 copies per function, and a function that needs more
/// is refused, not compiled to unlike bytes.
#[test]
fn a_function_past_the_copy_capacity_is_refused() {
    let calls = |n: usize| {
        let mut body = String::new();
        for i in 0..n {
            body.push_str(&format!("let z{i} = f(d.s); "));
        }
        format!(
            "{P}private data d {{ s: P }} fn f(o: P) -> Word {{ o.x }} \
             fn main() -> Word {{ d.s = P {{ x: 3, y: 4 }}; {body}0 }}"
        )
    };
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let at_cap = outcome(&calls(64));
    let past_cap = catch_unwind(AssertUnwindSafe(|| {
        keleusma::selfhost::self_host_compile(&calls(65))
    }));
    std::panic::set_hook(prev);
    assert_eq!(copies(chunk_main(&reference(&calls(64)))), 64);
    assert_eq!(at_cap, Outcome::Identical, "sixty-four copies fit");
    let payload = past_cap.expect_err("sixty-five copies are refused");
    let message = payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_default();
    assert!(
        message.contains("copy pass"),
        "refused by the copy pass, not elsewhere: {message}"
    );
}

fn chunk_main(module: &Module) -> &[Op] {
    &module
        .chunks
        .iter()
        .find(|c| c.name == "main")
        .expect("main")
        .ops
}

//! A combinatorial trap-free corpus for the two trap analyses.
//!
//! Strengthening item 12 observed that every stage classifies as possibly
//! trapping, so agreement on a trap-free verdict rested on about a dozen small
//! modules. This corpus composes every fully handled first-pass form with every
//! other, in function, loop and stream positions, and requires both the Rust
//! and the Keleusma analysis to report each module trap-free. Each composed
//! module has a control in which one handler loses its coverage, which both
//! analyses must report as possibly trapping at the same instruction.
//!
//! The Keleusma analysis exists only under the `self-host` feature, so the
//! comparison runs only there. Without it, the Rust verdicts are still pinned.

#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::bytecode::Module;
use keleusma::trap_analysis::{TrapAnalysisError, TrapVerdict, classify_traps};

/// Fully handled forms of type `Word`, over the parameters `a`, `b`, `i` and
/// the bytes `p`, `q`, and the declarations in `PRELUDE`.
const HANDLED: &[&str] = &[
    "(a + b { ok(v) => v, overflow(h, l) => l, underflow(h, l) => h })",
    "(a - b { ok(v) => v, overflow(h, l) => l, underflow(h, l) => h })",
    "(a * b { ok(v) => v, overflow(h, l) => l, underflow(h, l) => h })",
    "(a / b { ok(v) => v, overflow(h, l) => l, zero_divisor(n) => n })",
    "(a % b { ok(v) => v, zero_divisor(n) => n })",
    "(-a { ok(v) => v, overflow(h, l) => l })",
    "((p + q { ok(v) => v, overflow(w) => w }) as Word)",
    "((p - q { ok(v) => v, underflow(w) => w }) as Word)",
    "((p / q { ok(v) => v, zero_divisor(n) => n }) as Word)",
    "((p % q { ok(v) => v, zero_divisor(n) => n }) as Word)",
    "(ws[i] { ok(v) => v, invalid_index(j) => j })",
    "((bs[i] { ok(v) => v, invalid_index(_) => 0 as Byte }) as Word)",
    "(ps[i] { ok(r) => r.x, invalid_index(j) => j })",
    "(d.w[i] { ok(v) => v, invalid_index(j) => j })",
    // A handled write is an expression only as a whole `let` initializer.
    "d.w[i] = a { ok(o, n) => o + n, invalid_index(j) => j }",
    "(host::f(a) { ok(v) => v, host_error(c) => c })",
    "(host::f(b) { ok(v) => v, host_error => 0 })",
    "(a + b)",
    "(if a < b { a } else { b })",
];

/// The same forms with coverage removed, so a default trap remains.
const UNCOVERED: &[&str] = &[
    "(a / b)",
    "(a % b { ok(v) => v, zero_divisor(n) when n > 0 => n })",
    "(ws[i])",
    "(ps[i].x)",
    "(d.w[i])",
    "(host::f(a))",
    "(ws[i] { ok(v) => v, invalid_index(0) => 0 })",
];

const PRELUDE: &str = "use host::f\n\
    struct R { x: Word, y: Word }\n\
    private data d { w: [Word; 4] }\n\
    fn touch(v: Word) -> Word { d.w[0] = v { ok(_, n) => n, invalid_index(j) => j } }\n";

fn locals() -> &'static str {
    "let ws = [1, 2, 3, 4]; let bs = [1 as Byte, 2 as Byte]; \
     let ps = [R { x: 1, y: 2 }, R { x: 3, y: 4 }];"
}

fn function(e1: &str, e2: &str) -> String {
    format!(
        "{PRELUDE}fn main(a: Word, b: Word, i: Word, p: Byte, q: Byte) -> Word {{ {} let x = {e1}; let y = {e2}; x + y }}",
        locals()
    )
}

fn looped(e1: &str, e2: &str) -> String {
    format!(
        "{PRELUDE}fn main(a: Word, b: Word, i: Word, p: Byte, q: Byte) -> Word {{ {} let s = 0; \
         for k in 0..3 {{ let x = {e1}; let y = {e2}; }} s }}",
        locals()
    )
}

fn stream(e1: &str, e2: &str) -> String {
    format!(
        "{PRELUDE}loop main(a: Word) -> Word {{ let b = a; let i = a; let p = 1 as Byte; let q = 2 as Byte; {} \
         let x = {e1}; let y = {e2}; yield x + y }}",
        locals()
    )
}

fn compile(source: &str) -> Module {
    let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap())
        .unwrap_or_else(|e| panic!("{source}: {}", e.message));
    keleusma::compiler::compile(&ast).unwrap_or_else(|e| panic!("{source}: {}", e.message))
}

fn classify(module: &Module) -> Result<TrapVerdict, TrapAnalysisError> {
    let reference = classify_traps(module);
    #[cfg(feature = "self-host")]
    assert_eq!(
        keleusma::selfhost::classify_traps_via_kel(module),
        reference
    );
    reference
}

#[test]
fn every_composed_handled_module_is_trap_free_in_both_analyses() {
    let mut reached = 0usize;
    for (n, e1) in HANDLED.iter().enumerate() {
        for e2 in HANDLED.iter().skip(n) {
            for source in [function(e1, e2), looped(e1, e2), stream(e1, e2)] {
                assert_eq!(
                    classify(&compile(&source)),
                    Ok(TrapVerdict::TrapFree),
                    "{source}"
                );
                reached += 1;
            }
        }
    }
    let pairs = HANDLED.len() * (HANDLED.len() + 1) / 2;
    assert_eq!(reached, 3 * pairs);
}

#[test]
fn every_uncovered_form_is_possibly_trapping_in_both_analyses() {
    let mut reached = 0usize;
    for bad in UNCOVERED {
        for good in HANDLED {
            for source in [function(bad, good), function(good, bad), stream(good, bad)] {
                assert!(
                    matches!(classify(&compile(&source)), Ok(TrapVerdict::MayTrap { .. })),
                    "{source}"
                );
                reached += 1;
            }
        }
    }
    assert_eq!(reached, 3 * UNCOVERED.len() * HANDLED.len());
}

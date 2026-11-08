//! **ORDER 6: DOES THE WHOLE LANGUAGE LOWER, CONSTRUCT BY CONSTRUCT?**
//!
//! The roadmap's Order 6 gate is that the full language lowers to native code.
//! The corpus censuses answer a narrower question, whether the programs in the
//! tree lower, and a construct no corpus program uses is invisible to them.
//! This census walks the grammar in `docs/spec/GRAMMAR.md` section by section
//! instead. Each construct has a small subject, which is compiled by the
//! reference compiler, lowered, and executed natively and on the virtual
//! machine on several inputs, with and without the optimiser.
//!
//! Every subject ends in one of three dispositions, and the census prints the
//! table and pins it.
//!
//! - **Agrees.** It lowers, and every input gives the virtual machine's result.
//! - **Refused.** The backend refuses it, with the reason it names. A refusal
//!   is not a defect, but it is a construct the native shape does not yet
//!   carry, and Order 6 counts it.
//! - **Not this driver's.** The construct needs a host interface this scalar
//!   driver does not provide, such as a native, a `Text` result or a stream,
//!   and another file covers it. The row names that file.
//!
//! A subject the reference compiler rejects is a defect of this file, never a
//! disposition, so the census fails on it.
mod common;

use keleusma_native::{LowerOptions, module_refusals};

/// How a construct is driven.
enum Drive {
    /// `fn main(a: Word, b: Word) -> Word`, at these inputs.
    Scalar(&'static [(i64, i64)]),
    /// Another file drives it.
    Elsewhere(&'static str),
}

struct Construct {
    section: &'static str,
    what: &'static str,
    src: &'static str,
    drive: Drive,
}

const INPUTS: &[(i64, i64)] = &[(7, 3), (-5, 2), (0, 9), (1000, -17)];

const CONSTRUCTS: &[Construct] = &[
    Construct {
        section: "4 arithmetic",
        what: "Word + - * / %",
        src: "fn main(a: Word, b: Word) -> Word { (a + b) * (a - b) + (a / (b * b + 1)) - (a % (b * b + 1)) }",
        drive: Drive::Scalar(INPUTS),
    },
    Construct {
        section: "4 arithmetic",
        what: "unary negation",
        src: "fn main(a: Word, b: Word) -> Word { -a + -(b - 1) }",
        drive: Drive::Scalar(INPUTS),
    },
    Construct {
        section: "4 shift and bitwise",
        what: "band bor bxor bnot lsl asr lsr",
        src: "fn main(a: Word, b: Word) -> Word { ((a band 255) bor (b bxor 3)) + ((a lsl 2) asr 1) + ((bnot b) lsr 60) }",
        drive: Drive::Scalar(INPUTS),
    },
    Construct {
        section: "4 comparison and logic",
        what: "comparisons, and, or, not",
        src: "fn main(a: Word, b: Word) -> Word { if (a < b and a != 0) or not (a >= b) { 1 } else { if a == b or a <= b or a > b { 2 } else { 3 } } }",
        drive: Drive::Scalar(INPUTS),
    },
    Construct {
        section: "4 function calls",
        what: "calls between functions",
        src: "fn sq(x: Word) -> Word { x * x } fn add(x: Word, y: Word) -> Word { x + y } fn main(a: Word, b: Word) -> Word { add(sq(a), sq(b)) }",
        drive: Drive::Scalar(INPUTS),
    },
    Construct {
        section: "4 pipelines",
        what: "the pipe operator",
        src: "fn inc(x: Word) -> Word { x + 1 } fn add(x: Word, y: Word) -> Word { x + y } fn main(a: Word, b: Word) -> Word { let v = a |> inc() |> add(b); v }",
        drive: Drive::Scalar(INPUTS),
    },
    Construct {
        section: "4 if/else",
        what: "nested conditionals as values",
        src: "fn main(a: Word, b: Word) -> Word { let c = if a > b { a } else { b }; if c > 10 { c - 10 } else { c + 10 } }",
        drive: Drive::Scalar(INPUTS),
    },
    Construct {
        section: "4 match",
        what: "a match on Word literals with a wildcard",
        src: "fn main(a: Word, b: Word) -> Word { match a { 0 => b, 7 => b * 2, _ => a + b } }",
        drive: Drive::Scalar(INPUTS),
    },
    Construct {
        section: "4 structs",
        what: "construction and field access",
        src: "struct P { x: Word, y: Word } fn main(a: Word, b: Word) -> Word { let p = P { x: a, y: b }; p.x * 10 + p.y }",
        drive: Drive::Scalar(INPUTS),
    },
    Construct {
        section: "4 structs",
        what: "a struct passed and returned",
        src: "struct P { x: Word, y: Word } fn swap(p: P) -> P { P { x: p.y, y: p.x } } fn main(a: Word, b: Word) -> Word { let q = swap(P { x: a, y: b }); q.x - q.y }",
        drive: Drive::Scalar(INPUTS),
    },
    Construct {
        section: "3 tuples",
        what: "tuple construction and projection",
        src: "fn main(a: Word, b: Word) -> Word { let t = (a, b, a + b); t.0 + t.1 * t.2 }",
        drive: Drive::Scalar(INPUTS),
    },
    Construct {
        section: "4 array indexing",
        what: "arrays indexed at run time",
        src: "fn main(a: Word, b: Word) -> Word { let xs = [a, b, a + b, a - b]; xs[(b * b) % 4] + xs[0] }",
        drive: Drive::Scalar(INPUTS),
    },
    Construct {
        section: "3 enums",
        what: "enums with payloads and match",
        src: "enum S { A(Word), B(Word, Word), C } fn pick(n: Word, a: Word) -> S { if n > 5 { S::A(a) } else { if n > 0 { S::B(a, n) } else { S::C } } } fn main(a: Word, b: Word) -> Word { match pick(b, a) { S::A(x) => x, S::B(x, y) => x * y, S::C => 0 - 1 } }",
        drive: Drive::Scalar(INPUTS),
    },
    Construct {
        section: "3 enums",
        what: "whole-body enum equality across variants",
        src: "enum S { A(Word), B(Word, Word), C } fn pick(n: Word, a: Word) -> S { if n > 5 { S::A(a) } else { if n > 0 { S::B(a, n) } else { S::C } } } fn main(a: Word, b: Word) -> Word { let x = if pick(b, a) == S::A(a) { 1 } else { 0 }; let y = if pick(b, a) == pick(b + 1, a) { 10 } else { 0 }; x + y }",
        drive: Drive::Scalar(&[(7, 9), (7, 3), (7, 0), (4, 6)]),
    },
    Construct {
        section: "3 Byte",
        what: "Byte arithmetic and conversion",
        src: "fn main(a: Word, b: Word) -> Word { let x = (a as Byte) + (b as Byte); (x as Word) * 2 }",
        drive: Drive::Scalar(INPUTS),
    },
    Construct {
        section: "3 Bool",
        what: "Bool values and locals",
        src: "fn main(a: Word, b: Word) -> Word { let p = a > b; let q = p and b > 0; if q { 1 } else { if p { 2 } else { 3 } } }",
        drive: Drive::Scalar(INPUTS),
    },
    Construct {
        section: "3 Fixed",
        what: "fixed-point arithmetic",
        src: "fn main(a: Word, b: Word) -> Word { let x = (a as Fixed<16>) * (b as Fixed<16>); let y = x + (a as Fixed<16>); y as Word }",
        drive: Drive::Scalar(INPUTS),
    },
    Construct {
        section: "3 Float",
        what: "float arithmetic and conversion",
        src: "fn main(a: Word, b: Word) -> Word { let x = (a as Float) * 1.5 + (b as Float); x as Word }",
        drive: Drive::Scalar(INPUTS),
    },
    Construct {
        section: "5 variable binding",
        what: "let shadowing",
        src: "fn main(a: Word, b: Word) -> Word { let x = a; let x = x + b; let x = x * 2; x }",
        drive: Drive::Scalar(INPUTS),
    },
    Construct {
        section: "5 for loop",
        what: "a counted for loop over private data",
        src: "private data d { s: Word } fn main(a: Word, b: Word) -> Word { d.s = 0; for i in 0..5 { d.s = d.s + i * a; } d.s + b }",
        drive: Drive::Scalar(INPUTS),
    },
    Construct {
        section: "5 bounded repetition",
        what: "for with a limit and a handled outcome",
        src: "private data d { s: Word, r: Word } fn main(a: Word, b: Word) -> Word { d.s = 0; d.r = 0; for i in 0..b limit 8 { d.s = d.s + i * a; } on { ok(n) => { d.r = n * 100; }, limit(i) => { d.r = 0 - i; } } d.r + d.s }",
        drive: Drive::Scalar(&[(1, 3), (1, 8), (1, 20), (1, 0)]),
    },
    Construct {
        section: "5 break",
        what: "break out of a for loop",
        src: "private data d { s: Word } fn main(a: Word, b: Word) -> Word { d.s = 0; for i in 0..10 { if i > b { break; } d.s = d.s + a; } d.s }",
        drive: Drive::Scalar(INPUTS),
    },
    Construct {
        section: "6.5 multiheaded functions",
        what: "heads on literal arguments",
        src: "fn f(0) -> Word { 100 } fn f(n: Word) -> Word { n * 2 } fn main(a: Word, b: Word) -> Word { f(a) + f(0) + b }",
        drive: Drive::Scalar(INPUTS),
    },
    Construct {
        section: "6.6 guard clauses",
        what: "a guarded head",
        src: "fn g(n: Word) -> Word when n > 0 { n } fn g(n: Word) -> Word { 0 - n } fn main(a: Word, b: Word) -> Word { g(a) + g(b) }",
        drive: Drive::Scalar(INPUTS),
    },
    Construct {
        section: "7.5 newtype",
        what: "newtype construction and use",
        src: "fn pos(x: Word) -> bool { x >= 0 } newtype N = Word where pos; fn main(a: Word, b: Word) -> Word { let n = N(a * a); let m = N(b * b); (n as Word) + (m as Word) }",
        drive: Drive::Scalar(INPUTS),
    },
    Construct {
        section: "7.5 numeric overflow construct",
        what: "handled overflow",
        src: "fn main(a: Word, b: Word) -> Word { a * b { ok(v) => v, overflow(h, l) => h, underflow(h, l) => h } }",
        drive: Drive::Scalar(INPUTS),
    },
    Construct {
        section: "7.5 array indexing construct",
        what: "handled index",
        src: "fn main(a: Word, b: Word) -> Word { let xs = [1, 2, 3]; xs[a] { ok(v) => v + b, invalid_index(j) => j * 100 } }",
        drive: Drive::Scalar(INPUTS),
    },
    Construct {
        section: "7.5 indexed write construct",
        what: "handled indexed write",
        src: "private data d { x: [Word; 4] } fn main(a: Word, b: Word) -> Word { d.x[a] = b { ok(o, n) => o + n, invalid_index(j) => j } }",
        drive: Drive::Scalar(INPUTS),
    },
    Construct {
        section: "7.5 discriminant-to-enum",
        what: "handled discriminant conversion",
        src: "enum E { X = 0, Y = 1 } fn main(a: Word, b: Word) -> Word { let e = a as E { ok(v) => v, invalid_discriminant(d) => E::X }; match e { E::X => b, E::Y => b * 2 } }",
        drive: Drive::Scalar(&[(0, 3), (1, 3), (9, 3)]),
    },
    Construct {
        section: "8 pattern matching",
        what: "tuple and struct patterns",
        src: "struct P { x: Word, y: Word } fn sel(p: P) -> Word { match p { P { x, y } => x * 3 - y, _ => 0 } } fn main(a: Word, b: Word) -> Word { let t = (a, b); let u = match t { (x, y) => x + y, _ => 0 }; u + sel(P { x: a, y: b }) }",
        drive: Drive::Scalar(INPUTS),
    },
    Construct {
        section: "5 assert",
        what: "assert, compiled out of a release build",
        src: "fn main(a: Word, b: Word) -> Word { assert a == a; a + b }",
        drive: Drive::Scalar(INPUTS),
    },
    Construct {
        section: "data segments",
        what: "const data",
        src: "const data k { base: Word = 40, step: Word = 2 } fn main(a: Word, b: Word) -> Word { k.base + a * k.step + b }",
        drive: Drive::Scalar(INPUTS),
    },
    Construct {
        section: "data segments",
        what: "shared data",
        src: "",
        drive: Drive::Elsewhere("shared_data.rs, static_library_linkage.rs"),
    },
    Construct {
        section: "6.2 yield functions",
        what: "a reentrant entry",
        src: "",
        drive: Drive::Elsewhere("native_trap_outcomes.rs, retcon_bytecode.rs"),
    },
    Construct {
        section: "6.3 loop functions",
        what: "a stream entry",
        src: "",
        drive: Drive::Elsewhere("general_stream_sequence.rs, stage_differential.rs"),
    },
    Construct {
        section: "7 impure functions",
        what: "host natives",
        src: "",
        drive: Drive::Elsewhere("native_calls.rs, native_host_failure.rs"),
    },
    Construct {
        section: "3 Text",
        what: "text literals, locals and returns",
        src: "",
        drive: Drive::Elsewhere("text_surface.rs, text_comparison.rs"),
    },
    Construct {
        section: "3 opaque types",
        what: "opaque host handles",
        src: "",
        drive: Drive::Elsewhere("native_opaque.rs"),
    },
];

#[test]
fn the_language_construct_census() {
    let mut agrees = 0;
    let mut refused = Vec::new();
    let mut elsewhere = 0;
    let mut rejected = Vec::new();
    for c in CONSTRUCTS {
        let inputs = match &c.drive {
            Drive::Elsewhere(file) => {
                elsewhere += 1;
                eprintln!("  {:<28} {:<46} covered by {file}", c.section, c.what);
                continue;
            }
            Drive::Scalar(inputs) => inputs,
        };
        let Some(m) = common::try_build(c.src) else {
            rejected.push(c.what);
            eprintln!(
                "  {:<28} {:<46} REJECTED BY THE REFERENCE",
                c.section, c.what
            );
            continue;
        };
        let refusals = module_refusals(&m, LowerOptions::default());
        if let Some((_, why)) = refusals.first() {
            let why = why.to_string();
            eprintln!(
                "  {:<28} {:<46} refused: {}",
                c.section,
                c.what,
                &why[..why.len().min(90)]
            );
            refused.push((c.what, why));
            continue;
        }
        for &(a, b) in *inputs {
            for optimise in [false, true] {
                let (vm, native) = common::vm_and_native_two_arg_at(c.src, a, b, optimise);
                assert_eq!(native, vm, "{} on ({a}, {b}), optimised {optimise}", c.what);
            }
        }
        agrees += 1;
        eprintln!(
            "  {:<28} {:<46} agrees on {} inputs",
            c.section,
            c.what,
            inputs.len()
        );
    }
    eprintln!(
        "  constructs {}: agree {agrees}, refused {}, elsewhere {elsewhere}, rejected {}",
        CONSTRUCTS.len(),
        refused.len(),
        rejected.len()
    );
    assert!(
        rejected.is_empty(),
        "subjects the reference rejects: {rejected:?}"
    );
}

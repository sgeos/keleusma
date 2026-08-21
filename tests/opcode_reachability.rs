//! Which opcodes a SOURCE PROGRAM can actually produce.
//!
//! # Why this file exists
//!
//! The `v0.3.0` line maintains an opcode census: for each of the 66 opcodes, a
//! source program that emits it. Two resisted after eight construct attempts, and
//! they framed the question better than "which construct" — **the question is
//! whether one exists at all.**
//!
//! That distinction is the point. For an instruction set whose opcode count is a
//! stated rad-hard design constraint, an opcode no program can produce is a more
//! valuable finding than one more witnessed opcode. So this file records a
//! verdict for each, and is careful never to report "I could not find one" as
//! "none exists".
//!
//! # The technique, which generalises
//!
//! Both opcodes are emitted only as a FALLBACK when a static type is unknown. So
//! the target is not an unusual shape — it is **making inference fail**. Reading
//! the guard's own match arms for the kinds it does NOT handle is what cracked the
//! first one; guessing at constructs is what failed eight times before that.

#![cfg(feature = "compile")]

use keleusma::bytecode::Op;
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

fn ops_of(src: &str) -> Vec<Op> {
    let ast = parse(&tokenize(src).expect("lex")).expect("parse");
    let module = compile(&ast).expect("the reference must accept the probe");
    module.chunks.iter().flat_map(|c| c.ops.clone()).collect()
}

/// **`Op::Len` IS REACHABLE. The construct is an `if` EXPRESSION as a `for`-in
/// source.**
///
/// `Op::Len` is emitted only when `static_for_in_length` returns `None`. That
/// function matches on `ArrayLiteral`, `Call`, `FieldAccess`, `Ident`,
/// `ArrayIndex` and `Match`, then falls through to `_ => None`. **`Expr::If` is
/// not among them**, so an `if` expression as the iteration source takes the
/// fallback.
///
/// Found by reading the guard's match arms for what it omits, after six probes
/// that varied the SHAPE of the source rather than its expression KIND.
///
/// The controls matter as much as the case: every other source kind is checked to
/// take the static path, so a change that made `static_for_in_length` return
/// `None` for everything would fail here rather than look like a win.
#[test]
fn a_for_in_over_an_if_expression_reaches_op_len() {
    let reached = ops_of(
        "fn f(c: bool) -> Word { let a = [1, 2]; let b = [3, 4]; \
         for x in if c { a } else { b } { let _d = x; } 0 }\n\
         fn main() -> Word { f(true) }",
    );
    assert!(
        reached.iter().any(|o| matches!(o, Op::Len)),
        "an `if`-expression `for`-in source no longer reaches `Op::Len`; if \
         `static_for_in_length` gained an `Expr::If` arm, this opcode may have no \
         remaining producer and that is worth knowing"
    );

    // THE CONTROLS. Each of these IS handled by `static_for_in_length`, so each
    // must take the static path. Without them a blanket regression would pass.
    const STATIC_SOURCES: &[(&str, &str)] = &[
        (
            "ident",
            "fn main() -> Word { let xs = [10, 20]; for x in xs { let _d = x; } 0 }",
        ),
        (
            "literal",
            "fn main() -> Word { for x in [10, 20] { let _d = x; } 0 }",
        ),
        (
            "index",
            "fn main() -> Word { let m = [[1, 2], [3, 4]]; for x in m[0] { let _d = x; } 0 }",
        ),
        (
            "call",
            "fn g() -> [Word; 2] { [1, 2] }\nfn main() -> Word { for x in g() { let _d = x; } 0 }",
        ),
        (
            "field",
            "struct S { xs: [Word; 2] }\nfn main() -> Word { let s = S { xs: [1, 2] }; \
             for x in s.xs { let _d = x; } 0 }",
        ),
        (
            "match",
            "fn main() -> Word { let a = [1, 2]; for x in match 1 { _ => a } { let _d = x; } 0 }",
        ),
    ];
    let mut checked = 0;
    for (label, src) in STATIC_SOURCES {
        assert!(
            !ops_of(src).iter().any(|o| matches!(o, Op::Len)),
            "{label}: a statically-sized source now emits `Op::Len`, so the \
             fallback is being taken where the static path should apply"
        );
        checked += 1;
    }
    assert_eq!(checked, STATIC_SOURCES.len(), "not every control ran");
}

/// **`Op::IsStruct` IS NOT REACHED BY ANY CONSTRUCT TRIED. THAT IS NOT THE SAME AS
/// UNREACHABLE, AND THIS TEST DOES NOT CLAIM IT IS.**
///
/// The guard is `named_type_name(ty) != Some(type_name)` for a struct pattern —
/// the scrutinee must NOT be statically that struct. `ty` comes from
/// `infer_expr_type`, which has **no `Expr::If` arm**, so an `if`-expression
/// scrutinee should make it `None` and fire the opcode.
///
/// # It does not, and falsifying my own hypothesis is the result worth recording
///
/// The same `if`-expression trick that reaches `Op::Len` does NOT reach
/// `Op::IsStruct`, with either a constant or a runtime condition. **Making
/// inference fail is necessary but not sufficient**, so something further along
/// the struct-pattern path suppresses the test. What that is has not been
/// established.
///
/// # What was tried, so the next attempt does not repeat it
///
/// Struct-pattern matches whose scrutinee is: a plain local, an `if` expression
/// (constant and runtime condition), a call result, an array index, a nested
/// `match`, and a struct field. All compile; none emits `Op::IsStruct`.
///
/// `src/compiler.rs` already asserts the fold-out for the ordinary case, so the
/// opcode is deliberately avoided when the type IS known. The open question is
/// whether any source makes it unknown at this particular site.
///
/// **If it turns out no source can, that is the finding** — an opcode carried by
/// an instruction set whose count is a rad-hard constraint, with no producer.
#[test]
fn op_is_struct_resists_every_construct_tried_so_far() {
    const TRIED: &[(&str, &str)] = &[
        (
            "plain local",
            "struct P { a: Word }\nfn main() -> Word { let p = P { a: 1 }; \
                         match p { P { a } => a, _ => 0 } }",
        ),
        (
            "if runtime",
            "struct P { a: Word }\nfn f(c: bool) -> Word { let p = P { a: 1 }; \
                        let q = P { a: 2 }; match if c { p } else { q } { P { a } => a, _ => 0 } }\n\
                        fn main() -> Word { f(true) }",
        ),
        (
            "call result",
            "struct P { a: Word }\nfn g() -> P { P { a: 1 } }\n\
                         fn main() -> Word { match g() { P { a } => a, _ => 0 } }",
        ),
        (
            "array index",
            "struct P { a: Word }\nfn main() -> Word { \
                         let ps = [P { a: 1 }, P { a: 2 }]; match ps[0] { P { a } => a, _ => 0 } }",
        ),
        (
            "nested match",
            "struct P { a: Word }\nfn main() -> Word { let p = P { a: 1 }; \
                          match match 1 { _ => p } { P { a } => a, _ => 0 } }",
        ),
        (
            "struct field",
            "struct I { a: Word }\nstruct O { i: I }\n\
                          fn main() -> Word { let o = O { i: I { a: 1 } }; \
                          match o.i { I { a } => a, _ => 0 } }",
        ),
    ];

    let mut tried = 0;
    for (label, src) in TRIED {
        assert!(
            !ops_of(src).iter().any(|o| matches!(o, Op::IsStruct(_))),
            "{label} now reaches `Op::IsStruct`. That CLOSES an open question: \
             record the construct in the opcode census and replace this test with \
             one that pins it, the way `Op::Len` is pinned above"
        );
        tried += 1;
    }
    assert_eq!(tried, TRIED.len(), "not every attempted construct ran");

    // MUST-FIRE on the probes being meaningful. If the reference stopped accepting
    // struct patterns entirely, every assertion above would pass while testing
    // nothing about the opcode.
    assert!(
        ops_of(TRIED[0].1)
            .iter()
            .any(|o| matches!(o, Op::GetLocal(_))),
        "the probe corpus no longer compiles to anything recognisable, so the \
         absence of `Op::IsStruct` says nothing"
    );
}

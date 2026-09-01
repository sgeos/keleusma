//! What the byte-identity oracle's corpus actually exercises at a function boundary.
//!
//! # The finding this pins
//!
//! The oracle compares whole compiled stages, which makes it the project's strongest correctness
//! signal for the self-hosted compiler. Its reach, however, is a property of what the twelve stage
//! sources happen to contain, not of the pipeline. Measured 2026-08-31:
//!
//! **All 861 declared functions return `Word`, and all 733 declared parameters are `Word`.**
//!
//! No other type crosses a function boundary anywhere in the corpus. That is the general form of
//! the string-literal gap `tests/lexical_divergence_census.rs` records: the corpus contains no
//! string literal at all, and it is equally silent about `bool`, `Byte`, `Text`, `Float`, `Fixed`,
//! tuples, arrays, structs and enums in a signature.
//!
//! # What this does NOT say
//!
//! It does not say non-`Word` types are untested. They are — through short synthetic snippets,
//! principally the construct-support boundary table. A first draft of this claim said the boundary
//! table was the ONLY such coverage and that was wrong: fourteen test files drive the self-hosted
//! compiler and two of them carry substantial non-`Word` material.
//!
//! The distinction is synthetic-versus-SCALE, not synthetic-versus-absent. A 200-kilobyte stage
//! exercises interactions between constructs that a three-line snippet cannot reach, and those
//! interactions are what a byte-identity oracle exists to catch. For `Word` the project has both
//! instruments; for every other type it has only the snippets.
//!
//! # The instrument
//!
//! Signatures are read through the project's own lexer and parser, not matched out of the source
//! text. Five instrument errors were made in one session by pattern-matching data that was
//! reachable through its real reader; a regular expression here would be the sixth.

#![cfg(all(feature = "compile", feature = "verify"))]

extern crate alloc;

use alloc::collections::BTreeSet;
use alloc::string::{String, ToString};

use keleusma::ast::TypeExpr;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;

/// The type's surface name, enough to tell `Word` from anything else.
fn type_name(t: &TypeExpr) -> String {
    match t {
        TypeExpr::Prim(p, _) => alloc::format!("{:?}", p),
        TypeExpr::Named(n, _, _, _) => n.clone(),
        TypeExpr::Tuple(_, _) => "<tuple>".to_string(),
        TypeExpr::Array(inner, _, _) => alloc::format!("[{}]", type_name(inner)),
        other => alloc::format!("{:?}", other)
            .split('(')
            .next()
            .unwrap_or("<unknown>")
            .to_string(),
    }
}

#[test]
fn the_corpus_exercises_exactly_one_type_at_every_function_boundary() {
    let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src/selfhost/kel"));
    let mut stages = 0usize;
    let mut functions = 0usize;
    let mut params = 0usize;
    let mut returns: BTreeSet<String> = BTreeSet::new();
    let mut parameters: BTreeSet<String> = BTreeSet::new();

    let mut paths: alloc::vec::Vec<_> = std::fs::read_dir(dir)
        .expect("the stage source directory")
        .map(|e| e.expect("a directory entry").path())
        .collect();
    paths.sort();
    for path in paths {
        if path.extension().is_some_and(|x| x == "kel") {
            stages += 1;
            let src = std::fs::read_to_string(&path).expect("read a stage source");
            let tokens = tokenize(&src).expect("a stage source lexes");
            let program = parse(&tokens).expect("a stage source parses");
            for f in &program.functions {
                functions += 1;
                returns.insert(type_name(&f.return_type));
                for p in &f.params {
                    params += 1;
                    parameters.insert(
                        p.type_expr
                            .as_ref()
                            .map(type_name)
                            .unwrap_or_else(|| "<inferred>".to_string()),
                    );
                }
            }
        }
    }

    // NON-VACUITY on the instrument. A directory read that found nothing, or a parser that
    // returned no functions, would make both set comparisons below trivially satisfiable.
    assert!(
        stages >= 10,
        "found {stages} stage sources, so this check has broken rather than the corpus having \
         shrunk"
    );
    assert!(
        functions > 500 && params > 400,
        "read {functions} functions and {params} parameters, which is far below the corpus's \
         size; the extraction has broken rather than the corpus having shrunk"
    );

    let word: BTreeSet<String> = ["Word".to_string()].into_iter().collect();
    assert_eq!(
        returns, word,
        "the corpus's return types are no longer exactly {{Word}}. That is a real WIDENING of the \
         byte-identity oracle's reach and is good news, but the reasoning recorded above and in \
         the design journal assumes the single-type shape. Update it rather than deleting this."
    );
    assert_eq!(
        parameters, word,
        "the corpus's parameter types are no longer exactly {{Word}}. As above: this is the \
         oracle getting stronger, and the claims that rest on the old shape need revisiting."
    );
}

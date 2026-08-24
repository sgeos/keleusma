#![cfg(all(feature = "compile", feature = "verify"))]
//! The example corpus contains a composite built inside an ITERATING loop
//! body, in each of the three ways such a value can be disposed of.
//!
//! # Why this is pinned
//!
//! Measured on 2026-08-23, the corpus held **79** composite construction sites
//! and **not one** was built inside an iterating loop body. All 30 that sat
//! inside a `Loop` region were immediately followed by `Break` — arm results of
//! a `match` dispatch, since `Op::Loop` encodes dispatch as well as iteration.
//! Sixty-three of the 79 were `if`/`match` arm results altogether.
//!
//! That mattered because the per-iteration composite is the shape the
//! composite-region-reuse work is entirely about, and **a corpus that does not
//! contain it cannot exercise any of it**. The same defect this repository
//! keeps recording, in the corpus rather than in a test: coverage that is a
//! property of the case list, mistaken for a property of the thing under test.
//!
//! The corpus was working validation that the language is useful. It was never
//! chosen to exercise the memory model, and it did not.
//!
//! # The three dispositions, and why all three are needed
//!
//! | disposition | script | why it differs |
//! |---|---|---|
//! | consumed within the iteration | `12_sensor_window.kel` | the confined site a planner may give one reused slot |
//! | handed to the host | `13_telemetry_stream.kel` | escapes; a reused slot would serve the host the next iteration's bytes |
//! | copied to a data slot | `14_frame_log.kel` | the bytes are copied to the persistent region, so nothing aliases the ephemeral body |
//!
//! # What this test does NOT establish
//!
//! It classifies by the instruction that immediately follows the construction,
//! not by dataflow. That is an approximation, and it is exactly what a real
//! confinement analysis would replace. It is strong enough to show the shapes
//! are PRESENT, which is all it claims.

use keleusma::bytecode::{Module, Op};
use std::collections::BTreeSet;
use std::path::PathBuf;

fn scripts_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/scripts")
}

fn compile_file(path: &PathBuf) -> Option<Module> {
    let src = std::fs::read_to_string(path).ok()?;
    let tokens = keleusma::lexer::tokenize(&src).ok()?;
    let mut ast = keleusma::parser::parse(&tokens).ok()?;
    keleusma::typecheck::check(&mut ast).ok()?;
    let ast = keleusma::monomorphize::monomorphize(ast);
    keleusma::compiler::compile(&ast).ok()
}

/// The instruction immediately consuming each composite built inside a `Loop`
/// region, across the whole corpus.
fn in_loop_dispositions() -> (BTreeSet<String>, usize) {
    let mut kinds = BTreeSet::new();
    let mut count = 0usize;

    let mut paths: Vec<PathBuf> = std::fs::read_dir(scripts_dir())
        .expect("examples/scripts is readable")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "kel"))
        .collect();
    paths.sort();

    for path in &paths {
        let Some(module) = compile_file(path) else {
            continue;
        };
        for chunk in &module.chunks {
            // ITERATING loops only. `Op::Loop` is a break-scope marker, not an
            // iteration marker: the compiler emits it for `match` and for
            // multi-clause dispatch as well as for `for`. A dispatch body runs
            // ONCE, so a composite built there is never reused and confinement
            // is irrelevant to it.
            //
            // **This test counted dispatch scopes as loops in its first
            // draft**, which is the same error the `v0.3.0` line made twice
            // while measuring the same question. Their discriminator: a scope
            // containing an UNCONDITIONAL `Break` targeting its own exit leaves
            // in one pass. A `for` range test is a `BreakIf` and does not
            // count.
            let mut inside = vec![false; chunk.ops.len()];
            for (i, op) in chunk.ops.iter().enumerate() {
                if let Op::Loop(exit) = op {
                    let end = (*exit as usize).min(chunk.ops.len());
                    let dispatch = chunk.ops[i + 1..end]
                        .iter()
                        .any(|o| matches!(o, Op::Break(t) if *t == *exit));
                    if dispatch {
                        continue;
                    }
                    for slot in inside.iter_mut().take(end).skip(i + 1) {
                        *slot = true;
                    }
                }
            }
            for (i, op) in chunk.ops.iter().enumerate() {
                if !matches!(op, Op::NewComposite(_)) || !inside[i] {
                    continue;
                }
                count += 1;
                let name = match chunk.ops.get(i + 1) {
                    Some(Op::SetLocal(_)) => "SetLocal",
                    Some(Op::Yield) => "Yield",
                    Some(Op::SetData(_) | Op::SetDataIndexed(_, _)) => "SetData",
                    Some(Op::Break(_)) => "Break",
                    Some(Op::Return) => "Return",
                    _ => "other",
                };
                kinds.insert(name.to_string());
            }
        }
    }
    (kinds, count)
}

#[test]
fn the_corpus_builds_composites_inside_loops_and_disposes_of_them_three_ways() {
    let (kinds, count) = in_loop_dispositions();

    assert!(
        count > 0,
        "no composite is built inside any loop region in the corpus, so this \
         test is measuring nothing"
    );

    for required in ["SetLocal", "Yield", "SetData"] {
        assert!(
            kinds.contains(required),
            "the corpus no longer builds a composite inside a loop and disposes \
             of it via {required}. Found only {kinds:?}. Before 2026-08-23 the \
             corpus had NONE of these three and every in-loop composite was an \
             arm result followed by Break, which is why they were added. \
             Removing one silently returns the corpus to being unable to \
             exercise the memory model it is cited for."
        );
    }

    // No `Break` disposition is expected here any more, and its absence is the
    // point rather than a gap: the 30 arm-result composites the corpus had
    // before the extension all sit in DISPATCH scopes, which this walk now
    // excludes. If one appears, the discriminator has stopped discriminating.
    assert!(
        !kinds.contains("Break"),
        "a composite inside an ITERATING loop is immediately followed by an \
         unconditional Break, which should not happen: {kinds:?}. The likeliest \
         cause is that the dispatch filter has stopped excluding match scopes."
    );
}

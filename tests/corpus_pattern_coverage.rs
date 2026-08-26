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

/// Every top-level script is indexed in the directory's README, and every
/// indexed name exists.
///
/// # Why
///
/// `examples/scripts/README.md` carries a table of the scripts and a sentence
/// stating an invariant about them. **Adding three scripts made that sentence
/// false and left them out of the table**, and neither showed up in any test —
/// the corpus walkers read the directory, not the index.
///
/// A README that silently stops describing its directory is worse than none,
/// because a reader takes the table as the roster.
/// At least one confined site is reachable WITHOUT a callee summary.
///
/// # Why this is separate from the three dispositions
///
/// The `v0.3.0` line measured the corpus after `12` through `14` landed and
/// found **all three composite sites disqualified** by a crude escape test:
/// one by `Yield`, three by `SetLocal`, three by `Call`. Every subject needed
/// two analysis features at once, so a confinement predicate with only its
/// local-store handling would admit **nothing** even with subjects present.
///
/// `12_sensor_window.kel` calls a helper to compute a field, which is realistic
/// and is exactly why it is not the right first subject. This pins that the
/// corpus also carries a site whose only obstacle is the local store, so the
/// predicate has something to admit on day one.
///
/// # What it does not claim
///
/// Nothing about whether such a site IS confined — that is the analysis's job.
/// Only that the corpus contains one whose loop body makes no call, so a
/// verdict is reachable without a callee summary.
#[test]
fn a_confined_candidate_exists_with_no_call_in_its_loop_body() {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(scripts_dir())
        .expect("readable")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "kel"))
        .collect();
    paths.sort();

    let mut candidates = 0usize;
    for path in &paths {
        let Some(module) = compile_file(path) else {
            continue;
        };
        for chunk in &module.chunks {
            for (i, op) in chunk.ops.iter().enumerate() {
                let Op::Loop(exit) = op else { continue };
                let end = (*exit as usize).min(chunk.ops.len());
                let body = &chunk.ops[i + 1..end];
                if body
                    .iter()
                    .any(|o| matches!(o, Op::Break(t) if *t == *exit))
                {
                    continue; // dispatch, not iteration
                }
                let builds = body.iter().any(|o| matches!(o, Op::NewComposite(_)));
                let calls = body.iter().any(|o| {
                    matches!(
                        o,
                        Op::Call(..) | Op::CallVerifiedNative(..) | Op::CallExternalNative(..)
                    )
                });
                let escapes = body
                    .iter()
                    .any(|o| matches!(o, Op::Yield | Op::SetData(_) | Op::SetDataIndexed(_, _)));
                if builds && !calls && !escapes {
                    candidates += 1;
                }
            }
        }
    }

    assert!(
        candidates > 0,
        "no iterating loop in the corpus builds a composite without also making \
         a call or escaping it. A confinement predicate would then need a callee \
         summary before it could admit ANYTHING, which is the state the corpus \
         was in before 15_pixel_blend.kel."
    );
}

#[test]
fn the_readme_indexes_every_top_level_script() {
    let dir = scripts_dir();
    let readme = std::fs::read_to_string(dir.join("README.md")).expect("README.md is present");

    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .expect("readable")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "kel"))
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();
    names.sort();

    assert!(
        names.len() >= 12,
        "expected the script roster, found {}; this test would pass vacuously",
        names.len()
    );

    // Names taken from the TABLE ROWS, not from anywhere in the file.
    //
    // **The first version of this test asked whether the README merely
    // CONTAINED each name, and it could not fail.** Deleting a script's table
    // row left it green, because the prose below the table mentions the same
    // file. A check satisfied by a different part of the document than the one
    // it is about is not a check, and this is the third instance of that shape
    // in one session, after a translation clause and an evidence citation.
    // Mutation caught all three; reading caught none of them.
    let indexed: Vec<String> = readme
        .lines()
        .filter(|l| l.trim_start().starts_with("| [`"))
        .filter_map(|l| {
            let start = l.find("[`")? + 2;
            let end = l[start..].find('`')? + start;
            Some(l[start..end].to_string())
        })
        .filter(|n| n.ends_with(".kel"))
        .collect();

    assert!(
        !indexed.is_empty(),
        "no table row in examples/scripts/README.md names a script, so the \
         index was checked against nothing"
    );

    let missing: Vec<&String> = names.iter().filter(|n| !indexed.contains(n)).collect();
    assert!(
        missing.is_empty(),
        "these scripts have no row in the examples/scripts/README.md table: \
         {missing:?}. The corpus walkers read the DIRECTORY and would not have \
         noticed, which is exactly how three scripts were added unindexed."
    );

    // The other direction: a row naming a file that no longer exists sends its
    // reader to a dead link, the same defect pointing the other way.
    for named in &indexed {
        assert!(
            names.iter().any(|n| n == named),
            "examples/scripts/README.md indexes {named:?}, which is not present"
        );
    }
}

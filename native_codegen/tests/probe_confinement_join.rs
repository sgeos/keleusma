//! **DOES THE EXISTING CONFINEMENT VERDICT ANSWER THE PLANNER'S QUESTION?**
//!
//! The operator ruled that the region planner's soundness obligation is
//! discharged BY ANALYSIS, that an inconclusive verdict declines, and that the
//! `V0.2.X` line owns the analysis while this line consumes it. Before adapting
//! any procedure, the cheapest possibility is that `keleusma::confine` already
//! answers the question — and nobody had checked.
//!
//! This probe reports; it asserts only what would make its report meaningless.

mod common;

use keleusma::confine::{Confinement, Reason, Scope, module_confinement};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

#[test]
fn does_the_confinement_verdict_answer_the_planners_question() {
    let mut modules = 0usize;
    let mut sites = 0usize;
    let mut iter_confined = 0usize;
    let mut iter_declines = 0usize;
    let mut invoc_confined = 0usize;
    let mut invoc_declines = 0usize;
    let mut yielded = 0usize;
    // Does every site the PLANNER places have a verdict at the same key?
    let mut planned = 0usize;
    let mut planned_with_verdict = 0usize;

    for p in common::corpus_sources() {
        let Ok(src) = std::fs::read_to_string(&p) else {
            continue;
        };
        let Some(m) = tokenize(&src)
            .ok()
            .and_then(|t| parse(&t).ok())
            .and_then(|a| compile(&a).ok())
        else {
            continue;
        };
        modules += 1;
        let per_chunk = module_confinement(&m);
        for (ci, chunk) in m.chunks.iter().enumerate() {
            let verdicts = &per_chunk[ci];
            for v in verdicts {
                sites += 1;
                let confined = matches!(v.verdict, Confinement::Confined);
                match v.scope {
                    Scope::Iteration { .. } => {
                        if confined {
                            iter_confined += 1
                        } else {
                            iter_declines += 1
                        }
                    }
                    Scope::Invocation => {
                        if confined {
                            invoc_confined += 1
                        } else {
                            invoc_declines += 1
                        }
                    }
                }
                if matches!(v.reason, Reason::Yielded { .. }) {
                    yielded += 1;
                }
            }
            // The JOIN. The planner keys a placement by the op index of the
            // `NewComposite`; a verdict keys by the same address. If these do
            // not line up one-for-one there is no consuming the verdict at all.
            let plan = keleusma_native::region::plan_chunk_region(chunk);
            for s in &plan.sites {
                planned += 1;
                if verdicts.iter().any(|v| v.ip == s.op_index) {
                    planned_with_verdict += 1;
                }
            }
        }
    }

    println!("\n================ CONFINEMENT VERDICT vs THE PLANNER'S QUESTION");
    println!("  population: {modules} modules");
    println!("  verdict sites            : {sites}");
    println!("  planner placements       : {planned}");
    println!("  placements WITH a verdict: {planned_with_verdict}");
    println!("  ---- by scope, the planner's two reuse hazards ----");
    println!("  Iteration  confined {iter_confined}   declines {iter_declines}");
    println!("  Invocation confined {invoc_confined}   declines {invoc_declines}");
    println!("  sites whose reason is Yielded: {yielded}");
    println!(
        "\n  A `Confined` verdict LICENSES reuse for that scope. `CannotEstablish`\n  \
         and `Escapes` both decline, per the operator's ruling and per the\n  \
         analysis's own documented contract."
    );
    println!("================\n");

    // Non-vacuity: a sweep that saw no sites, or no placements, would report a
    // perfect join for the wrong reason.
    assert!(
        sites > 0 && planned > 0,
        "the sweep found {sites} verdict sites and {planned} placements, so the \
         join figure says nothing"
    );
}

/// **THE SITE THE OBLIGATION IS ABOUT.** `13_telemetry_stream.kel` was written
/// to carry the escaping shape and says so in its header. If the analysis does
/// not name that site, it does not answer the question that matters however good
/// its aggregate numbers are.
#[test]
fn the_known_escaping_site_is_named_by_the_analysis() {
    let path = common::corpus_sources().into_iter().find(|p| {
        p.file_name()
            .is_some_and(|n| n == "13_telemetry_stream.kel")
    });
    let Some(path) = path else {
        println!("  13_telemetry_stream.kel is not in this corpus; nothing to check");
        return;
    };
    let src = std::fs::read_to_string(&path).expect("read");
    let m = compile(&parse(&tokenize(&src).expect("lex")).expect("parse")).expect("compile");
    let per_chunk = module_confinement(&m);

    let mut found: Vec<String> = Vec::new();
    for (ci, chunk) in m.chunks.iter().enumerate() {
        for v in &per_chunk[ci] {
            if !matches!(v.verdict, Confinement::Confined) {
                found.push(format!(
                    "{}::{} site at op {} -> {:?} because {:?} (scope {:?})",
                    path.file_name().unwrap().to_string_lossy(),
                    chunk.name,
                    v.ip,
                    v.verdict,
                    v.reason,
                    v.scope
                ));
            }
        }
    }

    println!("\n================ THE KNOWN ESCAPING SITE");
    for f in &found {
        println!("  {f}");
    }
    println!("================\n");

    assert!(
        !found.is_empty(),
        "every site in the module written to CARRY the escaping shape is \
         reported confined. Either the module no longer carries it or the \
         analysis does not see it; both are findings and neither is a pass"
    );
}

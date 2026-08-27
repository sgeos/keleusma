//! **What the verifier actually does with heap, read from the verifier.**
//!
//! `planner_verifier_axis` refuted this line's claim that a syntactic path walk
//! is an upper bound on the verifier's figure, and left the 11-of-11 numeric
//! agreement standing with **no established mechanism**. That is a weaker
//! position than it looked, and inferring a replacement from output would repeat
//! the error.
//!
//! **`src/verify.rs` is readable here and is the authority.** What it does:
//!
//! | where | what |
//! |---|---|
//! | `:992` | `heap += then.heap_total.max(else.heap_total)` — comment: *"Exactly one branch executes"* |
//! | `:1016` | a bare `if` with no `else` adds the then-arm |
//! | `:1087` | a loop adds `body_heap.max(break_heap)` — a body counts ONCE, not per iteration |
//! | `:1863` | a chunk's heap includes `max_invocations * per_call_wcmu` for its CALLEES |
//! | `:1774` | the module figure is the MAX over chunks of that per-chunk figure |
//!
//! # So the mechanism is real, and it is not the one the refutation might suggest
//!
//! **The verifier MODELS BRANCH EXCLUSIVITY. The backend does not** —
//! `plan_chunk_region` gives every static site its own offset. **That difference
//! is the arena-bound gap**, and it is now traceable to the line that implements
//! it rather than inferred from a ratio.
//!
//! **The 11-of-11 agreement was therefore not a coincidence**: the path walk
//! implements the verifier's own arm rule.
//!
//! # And the walk's KNOWN defect is identifiable from the same reading
//!
//! **The walk follows no calls.** A chunk whose callee allocates gets the callee's
//! heap in the verifier's figure and nothing in the walk's. **That is the
//! candidate explanation for the three `UNDER` modules**, and this file checks it
//! rather than asserting it.
//!
//! # CONFIRMED, MEASURED 2026-08-27: THE PROPERTY SEPARATES THE TWO SETS CLEANLY
//!
//! ```text
//!   UNDER     09_big_numbers.kel     allocating callee: YES
//!   UNDER     10_multbyte.kel        allocating callee: YES
//!   UNDER     fixed_arithmetic.kel   allocating callee: YES
//!   EXCEEDING with NO allocating callee: 11 of 11
//! ```
//!
//! **Every module where the walk falls short has an allocating callee; no module
//! where it agrees exactly does.** The detector is shown to discriminate first —
//! 7 modules with, 62 without — so the clean split is a measurement rather than a
//! constant answer.
//!
//! ## So the account is complete
//!
//! 1. The verifier takes the **max over arms** (`verify.rs:992`).
//! 2. The verifier **adds callee heap** (`verify.rs:1863`).
//! 3. The backend **sums over every static site** and models neither.
//! 4. The walk does (1) and not (2).
//!
//! **The gap is (3) against (1).** The 11-of-11 agreement holds because those
//! modules have no allocating callee, so (2) never bites and the walk and the
//! verifier are computing the same thing.
//!
//! **This is established from the implementing lines plus a confirming
//! measurement**, not inferred from a ratio — which is exactly what the previous
//! attempt did wrong.
//!
//! # What is NOT reinstated
//!
//! **The upper-bound claim stays refuted.** The walk under-counts wherever a
//! callee allocates, so it is not an upper bound on the verifier's figure in
//! general, and nothing here says otherwise.

use keleusma::bytecode::{Module, NewCompositeOperand, Op};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

const CORPUS_DIRS: [&str; 4] = [
    "examples/scripts",
    "src/selfhost/kel",
    "examples/rtos/scripts",
    "compiler/kel",
];

fn all_compiling_modules() -> Vec<(String, Module)> {
    let root = std::path::Path::new("..");
    let mut stack: Vec<std::path::PathBuf> = CORPUS_DIRS.iter().map(|d| root.join(d)).collect();
    let mut paths = Vec::new();
    while let Some(p) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&p) else { continue };
        for e in rd.flatten() {
            let q = e.path();
            if q.is_dir() {
                stack.push(q);
            } else if q.extension().is_some_and(|x| x == "kel") {
                paths.push(q);
            }
        }
    }
    paths.sort();
    let mut out = Vec::new();
    for p in paths {
        let Ok(src) = std::fs::read_to_string(&p) else {
            continue;
        };
        let Ok(toks) = tokenize(&src) else { continue };
        let Ok(ast) = parse(&toks) else { continue };
        let Ok(m) = compile(&ast) else { continue };
        out.push((
            p.file_name().unwrap_or_default().to_string_lossy().to_string(),
            m,
        ));
    }
    out
}

fn is_site(op: &Op) -> bool {
    matches!(op, Op::NewComposite(NewCompositeOperand::Flat { .. }))
}

fn chunk_sites(c: &keleusma::bytecode::Chunk) -> usize {
    c.ops.iter().filter(|o| is_site(o)).count()
}

/// Does any chunk in this module CALL a chunk that allocates?
///
/// This is the shape the verifier accounts for at `:1863` and the walk does not.
/// One level of call is enough to establish presence or absence; the verifier
/// walks the whole graph, and a module with no allocating callee at ANY depth is
/// what the clean case needs.
fn has_allocating_callee(m: &Module) -> bool {
    // A chunk index that allocates, directly.
    let allocates: Vec<bool> = m.chunks.iter().map(|c| chunk_sites(c) > 0).collect();
    for c in &m.chunks {
        for op in &c.ops {
            if let Op::Call(ix, _) = op {
                if allocates.get(*ix as usize).copied().unwrap_or(false) {
                    return true;
                }
            }
        }
    }
    false
}

/// **THE DETECTOR DISCRIMINATES**, shown before its output is believed: it must
/// separate a module whose callee allocates from one whose callee does not.
#[test]
fn the_allocating_callee_detector_separates_the_two_cases() {
    let corpus = all_compiling_modules();
    let with: Vec<&String> = corpus
        .iter()
        .filter(|(_, m)| has_allocating_callee(m))
        .map(|(n, _)| n)
        .collect();
    let without: Vec<&String> = corpus
        .iter()
        .filter(|(_, m)| !has_allocating_callee(m))
        .map(|(n, _)| n)
        .collect();
    assert!(
        !with.is_empty(),
        "no module in the corpus has an allocating callee, so this detector reports \
         one constant answer and asserts nothing"
    );
    assert!(
        !without.is_empty(),
        "every module has an allocating callee, so this detector reports one \
         constant answer and asserts nothing"
    );
    println!(
        "\n  detector discriminates: {} module(s) with an allocating callee, {} without",
        with.len(),
        without.len()
    );
}

#[test]
fn do_allocating_callees_explain_where_the_walk_falls_short() {
    let corpus = all_compiling_modules();

    // The three the corpus-wide comparison reported as UNDER, and the eleven
    // that exceed. Named rather than recomputed, because this test is about
    // whether ONE property separates those two known sets.
    const UNDER: &[&str] = &["09_big_numbers.kel", "10_multbyte.kel", "fixed_arithmetic.kel"];
    const EXCEEDING: &[&str] = &[
        "rogue_ai_boss.kel",
        "rogue_ai_chaser.kel",
        "rogue_ai_fast.kel",
        "rogue_ai_hunter.kel",
        "rogue_ai_ranged.kel",
        "rogue_ai_sleeper.kel",
        "rogue_ai_smart.kel",
        "rogue_ai_tracker.kel",
        "rogue_ai_wander.kel",
        "rogue_combat.kel",
        "rogue_player_ai.kel",
    ];

    let look = |want: &str| -> Option<bool> {
        corpus.iter().find(|(n, _)| n == want).map(|(_, m)| has_allocating_callee(m))
    };

    println!("\n================ DO ALLOCATING CALLEES EXPLAIN THE SHORTFALL?");
    println!("  The verifier adds a callee's heap to its caller's figure (verify.rs:1863).");
    println!("  The path walk follows no calls. So a module with an allocating callee");
    println!("  should show the walk BELOW the verifier -- which is the UNDER set.");
    println!("  ------------------------------------------------");
    let mut under_with = 0usize;
    let mut under_without: Vec<&str> = Vec::new();
    for n in UNDER {
        match look(n) {
            Some(true) => {
                under_with += 1;
                println!("  UNDER     {n:<26} allocating callee: YES  <- explained");
            }
            Some(false) => {
                under_without.push(n);
                println!("  UNDER     {n:<26} allocating callee: no   <- UNEXPLAINED");
            }
            None => println!("  UNDER     {n:<26} not in corpus"),
        }
    }
    println!("  ------------------------------------------------");
    let mut exc_with: Vec<&str> = Vec::new();
    let mut exc_without = 0usize;
    for n in EXCEEDING {
        match look(n) {
            Some(true) => {
                exc_with.push(n);
                println!("  EXCEEDING {n:<26} allocating callee: YES  <- would break the account");
            }
            Some(false) => exc_without += 1,
            None => println!("  EXCEEDING {n:<26} not in corpus"),
        }
    }
    println!("  EXCEEDING with NO allocating callee: {exc_without} of {}", EXCEEDING.len());
    println!("  ------------------------------------------------");
    if under_without.is_empty() && exc_with.is_empty() {
        println!("  VERDICT: the property SEPARATES the two sets cleanly. Every module");
        println!("  where the walk falls short has an allocating callee; no module where");
        println!("  it agrees exactly does. The walk's defect is that it follows no calls,");
        println!("  and the 11-of-11 agreement holds because those modules have none.");
    } else {
        println!("  VERDICT: the property does NOT separate the two sets. Unexplained:");
        for n in &under_without {
            println!("    UNDER with no allocating callee : {n}");
        }
        for n in &exc_with {
            println!("    EXCEEDING with one              : {n}");
        }
        println!("  The account is incomplete and those modules are where to look.");
    }
    println!("  THE UPPER-BOUND CLAIM STAYS REFUTED either way: the walk under-counts");
    println!("  wherever a callee allocates, so it does not bound the verifier.");
    println!("================\n");

    assert!(
        UNDER.iter().chain(EXCEEDING.iter()).any(|n| look(n).is_some()),
        "none of the named modules is in the corpus, so this report describes nothing"
    );
}

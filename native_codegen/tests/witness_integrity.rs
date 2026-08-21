//! **DOES EACH WITNESS PROGRAM STILL WITNESS WHAT IT CLAIMS?**
//!
//! The corpus carries programs whose only purpose is to emit opcodes no real
//! application emits. Their headers say which. **A header is prose, and prose
//! does not fail** — a construct that stops producing its opcode drops a census
//! figure silently, with nothing going red.
//!
//! # This is not hypothetical, and the instance is an hour old
//!
//! `Op::IsStruct` was reachable only through an un-annotated struct-parameter
//! pattern. The `v0.2.3` line repaired the load-time hole behind it **in the
//! compiler**, by folding the irrefutable type test away — so the construct this
//! corpus had just adopted stopped emitting the opcode. Nothing in this tree
//! would have failed. The coverage census would have quietly reported 65 of 66
//! and a reader would have had to notice the number moved.
//!
//! **The repair was right**, and that is what makes this the right guard: the
//! corpus must notice when a witness expires, not argue with the repair.
//!
//! # The claim is machine-checked, not prose
//!
//! Each witness file carries a `WITNESSES:` line naming the opcodes it asserts
//! it emits. This compiles the file and checks every one. **The claim lives
//! beside the program it describes**, so moving a function between files moves
//! the obligation with it.
//!
//! # What this does NOT check
//!
//! **Not that the backend lowers them**, and not that anything executes. Those
//! are `isa_lowering_census` and `corpus_differential`. This asks only whether
//! the program still produces the opcodes its own header promises — the weakest
//! of the three claims and the one nothing else makes.
use keleusma::bytecode::Module;
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use std::collections::BTreeSet;

const SCRIPTS: &str = "../examples/scripts";

fn build(src: &str) -> Option<Module> {
    tokenize(src)
        .ok()
        .and_then(|t| parse(&t).ok())
        .and_then(|a| compile(&a).ok())
}

fn emitted(m: &Module) -> BTreeSet<String> {
    m.chunks
        .iter()
        .flat_map(|c| c.ops.iter())
        .map(|o| {
            let d = format!("{o:?}");
            d.split('(').next().unwrap_or(&d).to_string()
        })
        .collect()
}

/// `(file name, claimed opcodes)` for every script declaring a claim.
fn claims() -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    let mut paths: Vec<_> = std::fs::read_dir(SCRIPTS)
        .expect("read the scripts directory")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "kel"))
        .collect();
    paths.sort();
    for p in paths {
        let src = std::fs::read_to_string(&p).expect("read a script");
        // **The claim form is EXACT, not a substring match.** A first attempt
        // used `contains("WITNESSES:")` and matched the prose line "WHAT IT
        // WITNESSES:" in a header, parsing English into opcode names. The guard
        // caught itself, which is the behaviour wanted, but the lesson is that a
        // machine-checked claim needs a form prose cannot accidentally take.
        let Some(line) = src
            .lines()
            .map(str::trim)
            .find(|l| l.starts_with("// WITNESSES:"))
        else {
            continue;
        };
        let list = line
            .split("// WITNESSES:")
            .nth(1)
            .unwrap_or("")
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        out.push((p.file_name().unwrap().to_string_lossy().to_string(), list));
    }
    out
}

/// **EVERY CLAIM IS CHECKED.**
///
/// A file that stops emitting an opcode it advertises fails here, naming the
/// opcode and the file, rather than moving a census figure quietly.
#[test]
fn every_witness_program_emits_what_it_claims() {
    let claims = claims();
    println!("\n================ WITNESS INTEGRITY");
    println!("  files declaring a claim : {}", claims.len());

    let mut broken: Vec<String> = Vec::new();
    let mut checked = 0usize;
    for (name, wanted) in &claims {
        let src = std::fs::read_to_string(format!("{SCRIPTS}/{name}")).expect("read");
        let Some(m) = build(&src) else {
            broken.push(format!("{name}: DOES NOT COMPILE, so it witnesses nothing"));
            continue;
        };
        let have = emitted(&m);
        let missing: Vec<&String> = wanted.iter().filter(|w| !have.contains(*w)).collect();
        checked += wanted.len();
        println!(
            "  {name:30} claims {:2}, missing {}",
            wanted.len(),
            missing.len()
        );
        for m in missing {
            broken.push(format!("{name}: claims `{m}` and does not emit it"));
        }
    }
    println!("  opcode claims checked   : {checked}");
    println!(
        "\n  THIS CHECKS EMISSION ONLY. Not that the backend lowers them, not that\n  \
         anything executes -- those are `isa_lowering_census` and\n  \
         `corpus_differential`. This is the weakest of the three claims and the\n  \
         one nothing else makes."
    );
    println!("================\n");

    // Non-vacuity: a walk that found no claims would pass while checking nothing.
    assert!(
        claims.len() >= 4,
        "only {} witness files declare a claim, so this guard is reading the \
         wrong tree or the claims have been removed",
        claims.len()
    );
    assert!(
        checked >= 15,
        "only {checked} opcode claims were checked, which is too few for the \
         witness corpus and suggests the claim lines have been emptied"
    );

    assert!(
        broken.is_empty(),
        "A WITNESS PROGRAM NO LONGER EMITS WHAT IT CLAIMS. This is usually NEWS \
         rather than a defect in the program: a construct can be folded away by \
         a compiler repair, which is exactly what happened to the `Op::IsStruct` \
         witness. FIND ANOTHER CONSTRUCT, or amend the claim and re-measure the \
         coverage census -- do NOT delete the claim to restore green.\n\n{}",
        broken.join("\n")
    );
}

/// **THE MUST-FIRE CONTROL.** A claim checker that never fails is decoration.
///
/// Runs the identical comparison against a program claiming an opcode it plainly
/// does not emit, and asserts the mismatch is detected.
#[test]
fn the_claim_check_detects_an_unmet_claim() {
    let m = build("fn main() -> Word { 1 }\n").expect("compiles");
    let have = emitted(&m);
    assert!(
        !have.contains("FixedDiv"),
        "this trivial program emits FixedDiv, so it is the wrong control"
    );
    // The same predicate the census above applies, exercised on a known miss.
    let wanted = ["Return".to_string(), "FixedDiv".to_string()];
    let missing: Vec<&String> = wanted.iter().filter(|w| !have.contains(*w)).collect();
    assert_eq!(
        missing.len(),
        1,
        "the claim predicate did not detect exactly the one unmet claim, so a \
         real unmet claim above could pass unnoticed. Emitted: {have:?}"
    );
    assert_eq!(missing[0], "FixedDiv");
}

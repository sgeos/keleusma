//! **EVERY PLACE THE EMITTER PUSHES AN UNKNOWN WIDTH, AND WHY.**
//!
//! # Why a census rather than another fix
//!
//! Three separate increments on 2026-09-17 each found one arm dropping a width it
//! could have preserved — `Div`/`Mod`, then the three bitwise operators, then all
//! four shifts. **Each was found by a generated subject stumbling on it**, and
//! each time the repair was the same shape.
//!
//! Chasing instances one at a time is how a family gets half-closed. This file
//! enumerates the population instead: **every bare `push` in the emitter must
//! carry a disposition here.** A new one fails closed rather than joining
//! silently, which is the idiom this package already applies to opcodes, corpus
//! modules and handoff figures.
//!
//! # What an unknown width costs
//!
//! Not a wrong answer. `NewComposite` refuses an operand of unknown packed width,
//! so the cost is capability: **the result cannot be stored in a composite
//! field.** Every gap in this family was a silent narrowing, never a mispack.
//!
//! # ⚠ A DISPOSITION IS NOT A CLAIM THAT THE SITE IS FINE
//!
//! Two dispositions are `unmeasured` — recorded as such rather than asserted to
//! be correct. **A third was `Op::Not`, and answering it found a fourth gap**: a
//! negated comparison could not fill a `bool` field while an un-negated one could,
//! because the comparison arm pushed `Scalar(1)` and `Op::Not` pushed nothing.
//! `Op::Dup` and `Op::PushImmediate` were measured at the same time and neither
//! narrows anything — their results do not reach a composite. **Saying "not measured" is the honest half of a census**, and a
//! reader deciding where to look next needs to tell it apart from "measured and
//! sound".

use std::collections::BTreeSet;

/// `(enclosing opcode arm, disposition)`.
///
/// - `fallback` — the arm preserves a width when it can and reaches this line
///   only when the operands themselves carry none. Correct by construction.
/// - `sound` — the pushed value genuinely has no statically known packed width.
/// - `unmeasured` — nobody has checked whether a width is determinable here.
const DISPOSITIONS: &[(&str, &str)] = &[
    ("Op::Div | Op::Mod => {", "fallback"),
    ("Op::BitAnd | Op::BitOr | Op::BitXor => {", "fallback"),
    ("Op::Shl | Op::Shr => {", "fallback"),
    (
        "Op::Dup => {",
        "unmeasured: a duplicate could copy the width of what it duplicates, and \
         nothing has measured whether losing it narrows anything",
    ),
    (
        "Op::PushImmediate(imm) => {",
        "unmeasured: an immediate has a knowable shape, but the compiler emits \
         Op::Const for a literal field value, so no subject reaches this",
    ),
    (
        "Op::GetData(_)",
        "sound: a data slot's shape comes from the module's layout tables, which \
         this arm does not consult; the typed verifier covers the slot side",
    ),
    (
        "Op::Yield => {",
        "sound: a per-yield reentrant reply has no static shape, which the handoff \
         records as the reason the typed verifier defers here too",
    ),
];

fn source() -> String {
    std::fs::read_to_string("src/lib.rs").expect("the emitter is readable")
}

/// Bare `push` sites, mapped to the opcode arm that encloses them.
fn bare_push_arms() -> Vec<(usize, String)> {
    let src = source();
    let mut arm = String::new();
    let mut out = Vec::new();
    for (i, line) in src.lines().enumerate() {
        let t = line.trim_start();
        if line.starts_with("            Op::") {
            arm = t.to_string();
        }
        // **KEYED ON `contains`, NOT `starts_with`.** The first version of this
        // scan required the call to begin the line, and missed every
        // `None => st.push(v),` arm — which is the exact form the three repairs
        // this census exists to generalise had just introduced. **A census whose
        // matcher cannot see the sites it was written for reports a clean
        // population and means nothing**, and this package catalogues that class
        // three times over. Found because the stale half of the check fired.
        if line.contains("st.push(") {
            out.push((i + 1, arm.clone()));
        }
    }
    out
}

/// **NON-VACUOUS.** A scan finding nothing would pass while checking nothing, and
/// two derivations in this repository have already done exactly that.
#[test]
fn the_scan_finds_the_push_sites_it_claims_to_classify() {
    let sites = bare_push_arms();
    assert!(
        sites.len() >= 8,
        "the scan found only {} bare push site(s). The emitter had ten when this \
         census was written; a collapse to nothing means the matcher broke, not \
         that the sites went away.",
        sites.len()
    );
}

/// **Every bare push carries a disposition, or this fails.**
#[test]
fn every_unknown_width_push_is_dispositioned() {
    let known: BTreeSet<&str> = DISPOSITIONS.iter().map(|(a, _)| *a).collect();
    let found: BTreeSet<String> = bare_push_arms().into_iter().map(|(_, a)| a).collect();

    let undispositioned: Vec<&String> = found
        .iter()
        .filter(|a| {
            !known
                .iter()
                .any(|k| a.starts_with(k) || k.starts_with(a.as_str()))
        })
        .collect();
    assert!(
        undispositioned.is_empty(),
        "{} opcode arm(s) push an unknown width with no disposition recorded: \
         {undispositioned:?}.\n\nSay whether the width is genuinely unavailable, \
         whether the arm preserves it when it can and this is the fallback, or \
         that nobody has measured it. **An unknown width is not a defect, but an \
         unexplained one is an unclosed question** — three arms in this family \
         were found to be narrowing capability silently, each by a generated \
         subject stumbling on it.",
        undispositioned.len()
    );

    let stale: Vec<&&str> = known
        .iter()
        .filter(|k| {
            !found
                .iter()
                .any(|a| a.starts_with(**k) || k.starts_with(a.as_str()))
        })
        .collect();
    assert!(
        stale.is_empty(),
        "{} disposition(s) name an arm that no longer pushes an unknown width: \
         {stale:?}. Delete the rows, so the table does not keep explaining \
         something that stopped happening.",
        stale.len()
    );
}

/// **The `unmeasured` count is stated, so it cannot quietly grow.**
#[test]
fn the_unmeasured_population_is_pinned() {
    const PINNED_UNMEASURED: usize = 2;
    let n = DISPOSITIONS
        .iter()
        .filter(|(_, d)| d.starts_with("unmeasured"))
        .count();
    assert_eq!(
        n, PINNED_UNMEASURED,
        "the unmeasured population moved to {n}. Growing it means a new arm was \
         admitted without being looked at; shrinking it means a question was \
         answered and the disposition should say what the answer was."
    );
}

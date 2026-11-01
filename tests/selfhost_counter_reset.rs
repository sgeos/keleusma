#![cfg(feature = "self-host")]
//! Every stage counter that indexes a record must be reset somewhere.
//!
//! # The defect this exists to catch, which cost a four-cause diagnosis
//!
//! `wire.kel` failed to self-compile byte-identically, and the last of four causes was one line.
//! `forin_count`, the bare `for` form's program-order counter, was never added to the per-function
//! reset that already cleared its own documented analogue `forlimit_count`. It indexes an emitted
//! record's argument as `7 * forin_count`, so the SECOND and every later function containing a bare
//! `for` emitted a record pointing past its own parts.
//!
//! **The symptom was FEWER operations rather than different ones**, which is what eventually
//! identified it — after prefix bisection, a rebuilt dependency chain, delta debugging, and a
//! five-line synthetic. Two of the four causes were first diagnosed WRONGLY along the way.
//!
//! **The shape is mechanical, so it does not need a diagnosis a second time.** A field that
//! accumulates and is then multiplied into a record index is a counter; a counter that is never
//! assigned zero anywhere carries across whatever scope it was meant to be local to.
//!
//! # What this checks, and what it deliberately does not
//!
//! It does **not** check that the reset DOMINATES the use — that needs control-flow analysis this
//! guard has no business doing, and the sound resets in these stages sit at two different scopes.
//! `forlimit_count` and `forin_count` are cleared per FUNCTION, while `aq_k` is cleared at the entry
//! to each array-equality construct, which is stricter. Demanding one scope would flag the other.
//!
//! It checks the weaker property that catches the actual defect: **a counter in this class is
//! assigned zero somewhere in its own stage.** Before the 2026-08-27 repair, `forin_count` was
//! assigned zero nowhere at all.
//!
//! # The class, audited when this was written
//!
//! The class held three members when this was written, all in `parse.kel`: `forlimit_count` and
//! `forin_count`, reset per function, and `aq_k`, reset at both of its construct entry points.
//! **No member was found unreset.**
//!
//! # THE CLASS SITS IN ONE STAGE, AND THAT IS A REACH RESULT RATHER THAN A SUSPICIOUS ONE
//!
//! All three members are in `parse.kel`, which reads like an extraction that only matches one
//! file. It is not. The accumulator half of the extraction fires in **every one of the twelve
//! stage sources**, from one accumulating field in the smallest to thirty-five in `parse.kel`, so
//! the scan is not blind to the others' syntax.
//!
//! What is rare is the second half: being multiplied into an index. `parse.kel` is the stage that
//! **emits records with packed arguments**, so it is the stage where a counter becomes an offset
//! into a record's own parts. The concentration follows from what the stage does.
//!
//! Checking this mattered: a guard that reports clean about files it never really examined is the
//! failure this tree opened a session with, six instruments deep.
//!
//! No count of the wider accumulator population is quoted here. It would drift with every
//! increment and nothing would check it, which is a defect this line has recorded seven times. The
//! only figure stated is the class size, and the assertion below is what keeps it honest.
//!
//! The negative is a result about the tree only because the guard is shown able to fail. It was
//! mutation-tested by **deleting the 2026-08-27 repair itself** — removing `forin_count`'s reset
//! reproduces the historical defect, and the guard names the field.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn stage_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/selfhost/kel")
}

/// `struct.field` pairs assigned as `s.f = s.f + <n>`.
fn accumulators(src: &str) -> BTreeSet<(String, String)> {
    let mut out = BTreeSet::new();
    for line in src.lines() {
        let line = line.split("//").next().unwrap_or("");
        let Some((lhs, rhs)) = line.split_once('=') else {
            continue;
        };
        let target = lhs.trim();
        let Some((s, f)) = target.split_once('.') else {
            continue;
        };
        if !s.chars().all(|c| c.is_ascii_lowercase())
            || !f
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit())
        {
            continue;
        }
        // `s.f = s.f + 1`
        let rhs = rhs.trim().trim_end_matches(';');
        let prefix = format!("{s}.{f}");
        if let Some(rest) = rhs.strip_prefix(&prefix)
            && let Some(add) = rest.trim_start().strip_prefix('+')
            && add.trim().chars().all(|c| c.is_ascii_digit())
            && !add.trim().is_empty()
        {
            out.insert((s.to_string(), f.to_string()));
        }
    }
    out
}

/// Whether `s.f` appears multiplied by a literal, `<n> * s.f`, which is the record-index shape.
fn used_as_index_multiplier(src: &str, s: &str, f: &str) -> bool {
    let needle = format!("* {s}.{f}");
    src.lines()
        .map(|l| l.split("//").next().unwrap_or(""))
        .any(|l| {
            l.match_indices(&needle).any(|(at, _)| {
                l[..at]
                    .trim_end()
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_ascii_digit())
            })
        })
}

/// Whether `s.f = 0` appears anywhere.
fn is_zeroed(src: &str, s: &str, f: &str) -> bool {
    let needle = format!("{s}.{f}");
    src.lines()
        .map(|l| l.split("//").next().unwrap_or(""))
        .filter_map(|l| l.split_once('='))
        .any(|(lhs, rhs)| lhs.trim() == needle && rhs.trim().trim_end_matches(';').trim() == "0")
}

fn stage_sources() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(stage_dir()).expect("read the stage directory") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) == Some("kel") {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .expect("stage file name")
                .to_string();
            out.push((name, std::fs::read_to_string(&path).expect("read a stage")));
        }
    }
    out.sort();
    assert!(
        !out.is_empty(),
        "no stage sources found; the guard is scanning nothing"
    );
    out
}

#[test]
fn every_record_indexing_counter_is_reset_somewhere() {
    let mut offenders = Vec::new();
    let mut in_class = 0usize;

    for (name, src) in stage_sources() {
        for (s, f) in accumulators(&src) {
            if !used_as_index_multiplier(&src, &s, &f) {
                continue;
            }
            in_class += 1;
            if !is_zeroed(&src, &s, &f) {
                offenders.push(format!("{name}: {s}.{f}"));
            }
        }
    }

    // **Non-vacuity.** If nothing lands in the class the guard passes while checking nothing, which
    // is the failure this repository has shipped twice in coverage assertions. The class held three
    // members when written; requiring two leaves room for a rename without pinning the exact set.
    assert!(
        in_class >= 2,
        "only {in_class} counter(s) matched the record-indexing class, so this guard is no longer \
         checking the shape it exists for. Either the extraction stopped matching the stage's \
         syntax or the counters were rewritten; fix the extraction rather than lowering this bound."
    );

    assert!(
        offenders.is_empty(),
        "these stage counters accumulate and are multiplied into a record index, but are never \
         assigned zero, so they carry across whatever scope they were meant to be local to: {}\n\
         This is the shape of the `forin_count` defect, where the second and every later function \
         containing a bare `for` emitted a record pointing past its own parts.",
        offenders.join(", ")
    );
}

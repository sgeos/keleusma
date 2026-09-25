//! Census: every external path the suite READS must lie within the reporter's reach.
//!
//! # The hole this closes, and how it was found
//!
//! `tools/gate-status.sh` answers whether a recorded gate verdict still speaks to
//! HEAD. It can only be right if it watches every input the suite reads. That set was
//! built by hand on 2026-09-24, twice, and was wrong BOTH times:
//!
//! 1. The first version watched `native_codegen/` alone, while `handoff_figures.rs`
//!    reads the handoff.
//! 2. The second added six paths by grepping, and still missed **root `src/`** —
//!    where `stage_differential.rs` reads `../src/selfhost/kel/*.kel`, the stage
//!    differential's own SUBJECTS, and other tests read `../src/bytecode.rs`,
//!    `compiler.rs`, `vm.rs` and `wire_format.rs`.
//!
//! **The first telling of point 2 claimed root `src/` was the other line's most active
//! directory, and the measurement refuted it**: over their 30 most recent commits,
//! `REVERSE_PROMPT.md` was touched 14 times, `docs/decisions` 7, root `src/` 3, and
//! `src/selfhost/kel` and `examples/scripts` not once. The hole was real; the reason
//! given for it was an unmeasured ranking. The figures live in `tools/gate-status.sh`
//! beside the set they describe.
//!
//! Fixing an instance twice is the signal to close the class. A hand-maintained list
//! of inputs drifts from the inputs exactly as a hand-maintained figure drifts from
//! the tree, which is this session's recurring finding.
//!
//! # What this guard claims, and what it explicitly does NOT
//!
//! It is a CENSUS, the shape this package already uses for host buffers and comment
//! citations: an enumerated population, each member either covered or excused with a
//! stated reason, so that a NEW member forces a decision instead of passing silently.
//!
//! **It does not claim to find every input.** It reads string literals, and knows two
//! spellings: a `../`-prefixed path, and a repository-root-relative path whose first
//! component is a top-level directory absent from this package (which is how
//! `handoff_figures.rs` spells the handoff, and why a scan keyed only on `../` has a
//! false negative). A path assembled from computed components, or held in a variable,
//! is invisible to it. **That limitation is the reason this is a census and not a
//! proof**, and it is why the floor below exists: a scan that silently matched
//! nothing would otherwise pass.
//!
//! **The blind spot is known precisely, and reach is shaped around it.** Four names --
//! `examples`, `src`, `tests`, `tools` -- exist both inside this package and at the
//! repository root, so a literal such as `"examples/scripts"` cannot be classified from
//! the text: `tests/common/mod.rs` joins that very spelling against `".."`, making it
//! repo-relative, while other files resolve the same spelling package-locally. Such
//! literals are invisible here. The mitigation is not better vision but BREADTH in the
//! set being checked: `../src` and `../examples` are watched whole, so a corpus root
//! added under either is covered even though this census never saw it. That is why the
//! reach set is deliberately coarser than the paths the tests name.
//!
//! A text scan was ABANDONED in this package once before, for gated module paths.
//! That one needed brace-aware scoping to match attributes to items, and each fix
//! traded one false-positive class for another. This scan has no such structure to
//! parse: a quoted literal is self-delimiting. Where a literal genuinely cannot be
//! classified, it is excused by name with a reason, never by loosening the matcher.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Repository top-level directories, used ONLY when the name does not also exist
/// inside this package. Locality is MEASURED, not assumed: `native_codegen/examples/`
/// exists, so `"examples/motor_policy/policy.kel"` is package-local -- and the first
/// version of this guard hardcoded `examples` as external and reported that test's own
/// subject as an uncovered input. A hardcoded assumption about the tree is the same
/// defect class this guard exists to catch, one level up.
const REPO_ROOTS: [&str; 11] = [
    "docs",
    "examples",
    "compiler",
    "book",
    "scripts",
    "keleusma-arena",
    "keleusma-wire",
    "keleusma-wire-derive",
    "keleusma-cli",
    "keleusma-bench",
    "keleusma-macros",
];

/// Paths the census sees but which are deliberately outside reach, each with a reason.
/// A bare list would rot; a reason makes an entry arguable.
///
/// **Empty, and that is a result rather than an oversight.** The one candidate,
/// `../.frozen_run_guard_probe` -- a scratch path `frozen-run.sh` creates and removes
/// to prove its own tree check fires -- needs no excuse, because classification already
/// rejects a single-component path. It WAS excused here first, and
/// [`no_excuse_outlives_its_subject`] then failed on it: the excuse had outlived its
/// subject within one increment of being written, which is the sixth time this line has
/// caught a record describing nothing.
const EXCUSED: [(&str, &str); 0] = [];

/// Minimum external paths the scan must find. A scan that matched nothing would pass
/// every check below, which is the failure shape this package refuses.
const CENSUS_FLOOR: usize = 20;

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The reach set, parsed from the reporter itself so there is ONE source of truth.
fn reach_set() -> Vec<String> {
    let text = std::fs::read_to_string(manifest().join("tools/gate-status.sh"))
        .expect("the reporter must be readable; this guard is about its reach set");
    let body = text
        .split_once("REACH=(")
        .expect("tools/gate-status.sh must declare a REACH set")
        .1
        .split_once(')')
        .expect("the REACH set must be closed")
        .0;
    body.lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .filter(|l| !l.is_empty())
        .map(|l| l.trim_matches('"').to_string())
        .collect()
}

/// Classify one string literal. Returns the path normalised relative to this package
/// when the literal names something OUTSIDE it, and `None` otherwise.
///
/// Degenerate forms are rejected deliberately: a trailing slash marks a `starts_with`
/// prefix rather than a file to read, a brace marks a format template, and a literal
/// with nothing after the prefix names nothing.
fn classify(lit: &str) -> Option<String> {
    if !lit.contains('/')
        || lit.contains(' ')
        || lit.contains('\\')
        || lit.contains('{')
        || lit.ends_with('/')
    {
        return None;
    }
    let external_spelling = lit.starts_with("../");
    let body = lit.strip_prefix("../").unwrap_or(lit);
    let first = body.split('/').next()?;
    if first.is_empty() || !body.contains('/') {
        return None;
    }
    // Locality is measured against the package, never assumed.
    if !external_spelling && manifest().join(first).exists() {
        return None;
    }
    if external_spelling {
        return Some(format!("../{body}"));
    }
    if REPO_ROOTS.contains(&first) {
        return Some(format!("../{lit}"));
    }
    None
}

/// Is `path` inside some reach entry? `.` covers the package and never an external path.
fn covered(path: &str, reach: &[String]) -> bool {
    reach
        .iter()
        .filter(|r| *r != ".")
        .any(|r| path == r.as_str() || path.starts_with(&format!("{r}/")))
}

/// Double-quoted literals on one line. Crude by design: see the module note.
fn literals(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur: Option<String> = None;
    let mut prev = '\0';
    for c in line.chars() {
        match (&mut cur, c) {
            (Some(s), '"') if prev != '\\' => {
                out.push(std::mem::take(s));
                cur = None;
            }
            (Some(s), _) => s.push(c),
            (None, '"') => cur = Some(String::new()),
            _ => {}
        }
        prev = c;
    }
    out
}

fn rust_files(dir: &Path, into: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            rust_files(&p, into);
        } else if p.extension().is_some_and(|x| x == "rs") {
            into.push(p);
        }
    }
}

/// Every external path the suite's sources name.
fn census() -> BTreeSet<String> {
    let mut files = Vec::new();
    rust_files(&manifest().join("tests"), &mut files);
    rust_files(&manifest().join("src"), &mut files);
    let mut found = BTreeSet::new();
    for f in files {
        // A guard's own fixtures are not the package's inputs. This file necessarily
        // contains example paths, and counting them would have the census census itself.
        if f.file_name().is_some_and(|n| n == "reach_census.rs") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&f) else {
            continue;
        };
        for line in text.lines() {
            for lit in literals(line) {
                if let Some(p) = classify(&lit) {
                    found.insert(p);
                }
            }
        }
    }
    found
}

#[test]
fn the_census_is_not_vacuous() {
    let n = census().len();
    assert!(
        n >= CENSUS_FLOOR,
        "the census found only {n} external paths, below the floor of {CENSUS_FLOOR}. \
         This is a BROKEN SCAN, not a package that stopped reading its inputs."
    );
}

#[test]
fn every_reach_entry_exists_on_disk() {
    for r in reach_set() {
        let p = manifest().join(&r);
        assert!(
            p.exists(),
            "reach entry `{r}` does not exist. A misspelled entry silently covers \
             nothing, so the reporter would under-report exactly where it claims to \
             watch."
        );
    }
}

#[test]
fn every_external_path_the_suite_reads_is_within_reach() {
    let reach = reach_set();
    let excused: Vec<&str> = EXCUSED.iter().map(|(p, _)| *p).collect();
    let missing: Vec<String> = census()
        .into_iter()
        .filter(|p| !covered(p, &reach) && !excused.contains(&p.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "{} external path(s) the suite reads lie OUTSIDE the reporter's reach: {missing:?}. \
         A recorded gate verdict would claim to still speak to HEAD while one of these \
         had changed underneath it. Add the path to the REACH set in \
         tools/gate-status.sh, or excuse it by name with a reason.",
        missing.len()
    );
}

/// Every excused entry must still be something the census actually sees. An excuse for
/// a path that no longer appears is a record outliving its subject, which this line has
/// found six times.
#[test]
fn no_excuse_outlives_its_subject() {
    let seen = census();
    for (p, why) in EXCUSED {
        assert!(
            seen.contains(p),
            "`{p}` is excused ({why}) but the census no longer sees it. Remove the \
             excuse rather than leaving it to describe nothing."
        );
    }
}

// ---------------------------------------------------------------------------
// The checker's reach, on input built to defeat it.
// ---------------------------------------------------------------------------

#[test]
fn guard_rejects_an_uncovered_path() {
    let reach: Vec<String> = ["..".to_string(), "../docs".to_string()]
        .into_iter()
        .collect();
    // Inside reach.
    assert!(covered("../docs/process/x.md", &reach));
    // Outside it: this is the failure the real test above reports.
    assert!(!covered(
        "../examples/scripts/01_arithmetic.kel",
        &["../docs".to_string()]
    ));
    // `.` must never cover an external path, or the package entry would cover the world.
    assert!(!covered("../src/vm.rs", &[".".to_string()]));
}

#[test]
fn classification_separates_external_from_package_local() {
    // External, both spellings.
    assert_eq!(classify("../src/vm.rs").as_deref(), Some("../src/vm.rs"));
    assert_eq!(
        classify("docs/process/handoffs/v0.3.0.md").as_deref(),
        Some("../docs/process/handoffs/v0.3.0.md"),
        "the repository-relative spelling is how the handoff is named; a scan keyed \
         only on `../` misses it, which is the false negative that made this a census"
    );
    // Package-local, because these directories exist HERE. `examples` is the one that
    // caught the first version of this guard out.
    assert_eq!(classify("examples/motor_policy/policy.kel"), None);
    assert_eq!(classify("tests/common/mod.rs"), None);
    assert_eq!(classify("src/region.rs"), None);
    assert_eq!(classify("tools/gate-status.sh"), None);
    // Not paths at all.
    assert_eq!(classify("PASS"), None);
    assert_eq!(classify("a | b"), None);
    // Degenerate forms: a prefix test, a format template, and a bare parent.
    assert_eq!(classify("docs/"), None);
    assert_eq!(classify("../{rest}"), None);
    assert_eq!(classify("../"), None);
}

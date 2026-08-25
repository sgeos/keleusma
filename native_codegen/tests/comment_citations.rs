//! **A CITATION TO A TEST THAT DOES NOT EXIST CANNOT FAIL.**
//!
//! This package's comments are dense with claims of the form *"`some_test_name`
//! covers this"*. Each is load-bearing: it is how a reader decides a route is
//! guarded without re-deriving the guard. **And a citation naming a test that
//! was never written reads exactly like one naming a test that passes.**
//!
//! # Why this file exists, and it is not a tidiness pass
//!
//! `src/lib.rs` claimed *"the list is a claim and
//! `the_float_guard_closes_every_route_it_names` tests each one"* about the four
//! routes a `Float` can take into a module. **That test was never written**, and
//! **route 3 -- the native return shape -- had no test at all.**
//!
//! Proved rather than asserted: disabling the route-3 guard in `src/lib.rs` made
//! **only** the newly-added `a_native_declaring_a_float_return_refuses_the_module`
//! fail. Every other test in `float_guard_routes.rs` still passed. So the route
//! could have been deleted outright and nothing would have gone red.
//!
//! **The `v0.2.3` line found this class in `src/` and `tests/` first** -- 24
//! unresolved citations, including one citing
//! `op_is_struct_still_has_producers_and_two_still_trap`, also never written.
//! **Their scanner does not reach here**, because `native_codegen` is a detached
//! workspace that their suite never builds and CI never touches. Scoping by class
//! rather than by where the first instance turned up is the whole point.
//!
//! # What counts as a citation, and why the threshold is TWO words
//!
//! A backticked lower-snake-case identifier of **two or more underscore-separated
//! words**.
//!
//! **THE FIRST DRAFT SET THIS AT FOUR AND THAT WAS A GUESS. IT WAS MEASURED
//! INSTEAD, AND THE GUESS WAS HIDING MOST OF THE FINDINGS:**
//!
//! | threshold | citations | unresolved |
//! |---|---|---|
//! | four words | 79 | 3 |
//! | three words | 183 | 10 |
//! | **two words** | **407** | **16** |
//!
//! The four-word cut found `disagrees_with_typed_verifier` and **missed its two
//! siblings in the same list** -- `negative_depth` (two words) and
//! `predicted_against_measured` (three) -- so it reported one third of a
//! three-part finding. It also missed **`slot_entry`**, which is cited four times
//! across two files as the function closing route 4 of the float guard and **does
//! not exist**; the real one is `resolve_shared_scalar`.
//!
//! **A threshold that hides two thirds of a finding it half-reports is not
//! precision, it is a blind spot with a rationale.** At two words the extra
//! entries are overwhelmingly mangled symbol names, which excuse cleanly as a
//! class because they are constructed at run time and can never resolve.
//!
//! Below two words the population becomes ordinary single identifiers and the
//! signal would drown; that boundary is a judgement, and it is the only one here
//! that is not backed by a count.
//!
//! # Three false-positive classes, each found by hand-checking before believing
//!
//! The first draft of this scan reported five. **Three were artefacts**, and
//! shipping them would have made the guard noise:
//!
//! 1. **Names spanning a line break.** A citation wrapped across two `//` lines
//!    looks like two truncated fragments. Comment blocks are therefore JOINED
//!    before scanning -- the same repair the `v0.2.3` line made.
//! 2. **File names.** `probe_nesting_and_breaks` resolves to
//!    `tests/probe_nesting_and_breaks.rs` and is a perfectly good citation. File
//!    stems are part of the resolvable universe.
//! 3. **Illustrative strings that are not citations at all.** `kel_native_host_play`
//!    is what `host::play` MANGLES TO -- prose about an output, not a reference to
//!    a definition. These are constructed at run time and can never resolve, so
//!    they are excused BY NAME with a reason.
//!
//! # The excuse list is the dangerous part, so it carries its own guard
//!
//! An excuse that outlives the citation it excuses is the same failure one level
//! up: a suppression that cannot fail. `every_excuse_still_has_a_citation`
//! asserts each entry is still cited somewhere, so the list cannot rot into a
//! permanent silencer.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Names that appear as citations but can never resolve to a definition,
/// each with the reason it cannot.
///
/// **Keep this list short and every entry justified.** An unexplained entry here
/// is indistinguishable from the defect this file exists to catch.
const EXCUSED: &[(&str, &str)] = &[
    // ---- CLASS 1: mangling outputs. Constructed at run time; no such
    // identifier exists in any source and none should.
    (
        "kel_native_host_play",
        "a MANGLING OUTPUT, not a definition: prose showing what `host::play` \
         becomes",
    ),
    (
        "kel_native_host_two",
        "the symbol `host::two` mangles to, contrasting a different separator",
    ),
    ("host_play", "the unmangled half of the same example"),
    ("host_two", "the unmangled half of the same example"),
    (
        "kel_chunk_2",
        "an emitted chunk symbol, numbered at lowering time",
    ),
    (
        "kel_chunk_4",
        "an emitted chunk symbol, numbered at lowering time",
    ),
    (
        "kel_chunk_21",
        "an emitted chunk symbol, numbered at lowering time",
    ),
    (
        "kel_chunk_24",
        "an emitted chunk symbol, numbered at lowering time",
    ),
    // ---- CLASS 2: names this file QUOTES as examples of the defect. Each
    // provably resolves to nothing, and that is precisely why it is quoted.
    // **If one of these ever starts resolving, the excuse is wrong** and
    // `no_excused_name_has_started_resolving` says so.
    (
        "the_float_guard_closes_every_route_it_names",
        "the citation that started this file. Quoted in three places as the \
         worked example; it was never written",
    ),
    (
        "op_is_struct_still_has_producers_and_two_still_trap",
        "the `v0.2.3` line's equivalent finding, quoted for the parallel",
    ),
    (
        "slot_entry",
        "quoted here as a worked example: cited four times as the function \
         closing route 4 of the float guard, and it never existed. The real one \
         is `resolve_shared_scalar`",
    ),
    (
        "slot_entr",
        "a deliberate truncation in this file's prose, demonstrating that the \
         prefix rule does NOT resolve one. The guard flagged it on the run that \
         introduced it, which is the behaviour being demonstrated",
    ),
    (
        "zz_touch",
        "a Keleusma `fn` written inside a Rust test string, quoted here as the \
         worked example of what string-stripping drops. **It dangled on the run \
         that introduced the sentence describing it**, which is the limitation \
         demonstrating itself",
    ),
    (
        "some_test_name",
        "a placeholder in this file's own prose, illustrating the SHAPE of a \
         citation rather than naming one",
    ),
    // ---- CLASS 3: named instruments that were PLANNED and never built.
    // Retained as citations because `spike_opcode_stack_audit.rs` now states
    // plainly that they do not exist; excusing them keeps that honest header
    // from failing the guard that produced it.
    (
        "disagrees_with_typed_verifier",
        "a planned instrument, never built. The header naming it now says so",
    ),
    (
        "predicted_against_measured",
        "a planned instrument, never built. The header naming it now says so",
    ),
    (
        "negative_depth",
        "the one planned instrument that WAS built, under the name \
         `audit_1_which_synthetic_cases_drive_the_model_negative`. The label is \
         kept beside it in the plan table",
    ),
];

fn package_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The repository root, for building the resolvable universe. Citations here
/// legitimately name things in the parent crate.
fn repo_root() -> PathBuf {
    package_root()
        .parent()
        .expect("native_codegen has a parent directory")
        .to_path_buf()
}

fn rust_and_kel_sources(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            // `target` holds generated code that would resolve names no human
            // wrote, and `.git` holds blobs. Both would weaken the universe.
            if p.is_dir() {
                if name != "target" && name != ".git" && name != "node_modules" {
                    stack.push(p);
                }
            } else if matches!(
                p.extension().and_then(|s| s.to_str()),
                Some("rs") | Some("kel")
            ) {
                out.push(p);
            }
        }
    }
    out
}

/// Blank out double-quoted spans, keeping the line's shape so identifier
/// positions outside them still tokenize.
fn strip_string_literals(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_str = false;
    let mut escaped = false;
    for c in line.chars() {
        if in_str {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
            }
            out.push(' ');
        } else if c == '"' {
            in_str = true;
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

fn is_comment(line: &str) -> bool {
    line.trim_start().starts_with("//")
}

/// Consecutive comment lines joined into one string, so a backticked name split
/// across a line break is seen whole.
fn comment_blocks(text: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut cur: Vec<String> = Vec::new();
    for line in text.lines() {
        if is_comment(line) {
            let t = line.trim_start();
            let t = t
                .strip_prefix("//!")
                .or_else(|| t.strip_prefix("///"))
                .or_else(|| t.strip_prefix("//"))
                .unwrap_or(t);
            cur.push(t.trim().to_string());
        } else if !cur.is_empty() {
            blocks.push(cur.join(" "));
            cur.clear();
        }
    }
    if !cur.is_empty() {
        blocks.push(cur.join(" "));
    }
    blocks
}

/// Backticked lower-snake-case names of four or more words, with internal
/// whitespace stripped so a wrapped name is recovered rather than reported as a
/// fragment.
fn citations(block: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes: Vec<char> = block.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != '`' {
            i += 1;
            continue;
        }
        let Some(close) = (i + 1..bytes.len()).find(|&j| bytes[j] == '`') else {
            break;
        };
        let span: String = bytes[i + 1..close].iter().collect();
        let span: String = span.split_whitespace().collect::<Vec<_>>().join("");
        let ok = !span.is_empty()
            && span.chars().next().is_some_and(|c| c.is_ascii_lowercase())
            && span
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            && span.matches('_').count() >= 1;
        if ok {
            out.push(span);
        }
        i = close + 1;
    }
    out
}

/// Every identifier written in a NON-comment line anywhere in the repository,
/// plus every source file's stem. This is what a citation may resolve to.
fn resolvable_universe() -> BTreeSet<String> {
    let mut universe = BTreeSet::new();
    for p in rust_and_kel_sources(&repo_root()) {
        if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
            universe.insert(stem.to_string());
        }
        // **A DIRECTORY IS A REAL THING A COMMENT MAY NAME.** `native_codegen`
        // is this package's directory and is cited in three files; it is not an
        // identifier anywhere, and excusing it would have been the wrong repair
        // for a citation that is perfectly good.
        for anc in p.ancestors().skip(1) {
            if let Some(d) = anc.file_name().and_then(|s| s.to_str()) {
                universe.insert(d.to_string());
            }
        }
        let Ok(text) = std::fs::read_to_string(&p) else {
            continue;
        };
        let rust = p.extension().and_then(|s| s.to_str()) == Some("rs");
        for line in text.lines() {
            if is_comment(line) {
                continue;
            }
            // **A NAME INSIDE A STRING IS NOT A DEFINITION**, and conflating the
            // two made this guard useless on its first run: `EXCUSED` lists its
            // entries as string literals, so every excused name "resolved" --
            // to the excuse list itself. The registry would have vouched for
            // every name it suppressed.
            let line = if rust {
                strip_string_literals(line)
            } else {
                line.to_string()
            };
            let line = line.as_str();
            let mut cur = String::new();
            for c in line.chars() {
                if c.is_ascii_alphanumeric() || c == '_' {
                    cur.push(c);
                } else if !cur.is_empty() {
                    universe.insert(std::mem::take(&mut cur));
                }
            }
            if !cur.is_empty() {
                universe.insert(cur);
            }
        }
    }
    universe
}

/// Every citation in this package, paired with the file that makes it.
fn citations_in_package() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for dir in ["src", "tests"] {
        for p in rust_and_kel_sources(&package_root().join(dir)) {
            let Ok(text) = std::fs::read_to_string(&p) else {
                continue;
            };
            let shown = p
                .strip_prefix(package_root())
                .unwrap_or(&p)
                .display()
                .to_string();
            for block in comment_blocks(&text) {
                for name in citations(&block) {
                    out.push((name, shown.clone()));
                }
            }
        }
    }
    out
}

/// Whether a cited name resolves: exactly, or as a **prefix at an underscore
/// boundary** of something that exists.
///
/// The prefix rule earns its keep on two real cases and costs little. `audit_1`
/// abbreviates `audit_1_which_synthetic_cases_drive_the_model_negative`, and
/// `rogue_ai` names the `rogue_ai_boss` / `rogue_ai_hunter` corpus FAMILY rather
/// than any one file. Both are citations a reader can follow, which is the whole
/// standard here.
///
/// **It is bounded by requiring the underscore.** A truncation like `slot_entr`
/// does not resolve, and neither does `slot_entry`, which is the finding this
/// file was built on -- nothing in the tree is named `slot_entry_*` either.
fn resolves(name: &str, universe: &BTreeSet<String>) -> bool {
    if universe.contains(name) {
        return true;
    }
    let with_sep = format!("{name}_");
    universe
        .range(with_sep.clone()..)
        .next()
        .is_some_and(|c| c.starts_with(&with_sep))
}

/// **THE GUARD.** Every citation this package makes must name something that
/// exists.
#[test]
fn every_comment_citation_resolves_to_something_real() {
    let universe = resolvable_universe();
    let excused: BTreeSet<&str> = EXCUSED.iter().map(|(n, _)| *n).collect();

    let mut dangling: Vec<(String, String)> = citations_in_package()
        .into_iter()
        .filter(|(n, _)| !resolves(n, &universe) && !excused.contains(n.as_str()))
        .collect();
    dangling.sort();
    dangling.dedup();

    assert!(
        dangling.is_empty(),
        "{} comment citation(s) name nothing that exists. A citation to a test \
         that was never written CANNOT FAIL, so it reads as coverage while being \
         none -- that is how route 3 of the float guard went untested while \
         `src/lib.rs` claimed all four were covered.\n\
         Fix by writing the thing, correcting the name, or -- only if it can \
         never resolve -- adding it to EXCUSED with the reason.\n{}",
        dangling.len(),
        dangling
            .iter()
            .map(|(n, f)| format!("  `{n}`  cited in {f}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// **THE GUARD ON THE GUARD.** An excuse that outlives its citation is a
/// suppression that can never fail -- the same defect one level up.
#[test]
fn every_excuse_still_has_a_citation() {
    let cited: BTreeSet<String> = citations_in_package().into_iter().map(|(n, _)| n).collect();
    let stale: Vec<&str> = EXCUSED
        .iter()
        .map(|(n, _)| *n)
        .filter(|n| !cited.contains(*n))
        .collect();
    assert!(
        stale.is_empty(),
        "EXCUSED names {stale:?}, which nothing cites any more. Remove the \
         entries. An excuse list that outlives what it excuses is a silencer \
         waiting for a future citation to collide with it"
    );
}

/// **AN EXCUSE THAT HAS BECOME FALSE IS A LIE THAT PASSES.**
///
/// Every entry in `EXCUSED` claims its name cannot resolve. If someone later
/// writes a function or test with that name, the claim is false and the excuse
/// is silently suppressing a citation that now resolves perfectly well -- or,
/// worse, one that resolves to something unrelated.
#[test]
fn no_excused_name_has_started_resolving() {
    let universe = resolvable_universe();
    let now_real: Vec<&str> = EXCUSED
        .iter()
        .map(|(n, _)| *n)
        .filter(|n| resolves(n, &universe))
        .collect();
    assert!(
        now_real.is_empty(),
        "{now_real:?} are excused as names that CANNOT resolve, and they now do. \
         Either something adopted the name -- in which case drop the excuse, the \
         citation is fine -- or the excuse was wrong when written. Do not leave \
         an excuse standing over a name that resolves"
    );
}

/// **STRIPPING STRING LITERALS CAN SWALLOW REAL CODE, AND THIS PROVES IT DOES
/// NOT HERE.**
///
/// Reported by the `v0.2.3` line against their own measurement script: they added
/// a whole-file regex strip and **an unbalanced quote inside a comment paired with
/// the next quote elsewhere in the file, deleting the real code between them.**
/// Their run then reported two `pub fn`s as undefined. They caught it because
/// those two names happened to be ones they knew — *"luck rather than method"*,
/// their words. This test is the method.
///
/// # Why this scanner is not exposed to their failure, verified rather than argued
///
/// Two structural reasons, both checked below rather than asserted:
///
/// 1. **Comment lines never reach the stripper.** `resolvable_universe` skips
///    them before stripping, so a lone quote in prose cannot open a span at all.
/// 2. **The stripper is per-LINE**, with its state reset at every line, so an
///    unbalanced quote in code can blank the rest of its own line and nothing
///    beyond it. There is no cross-line span to run away.
///
/// # What it DOES drop, stated because it is a real limitation
///
/// Keleusma sources embedded in Rust string literals. `is_ident_cont`,
/// `keyword_code` and `zz_touch` are `fn`s written inside test strings; they are
/// real code in the repository and invisible to this universe. **A citation to
/// one would be reported as dangling.** None is cited today. If one ever is, the
/// fix is to cite the Rust test that holds it, not to widen the universe —
/// widening is what reintroduces the self-vouching hole this stripping closed.
#[test]
fn the_string_stripper_does_not_swallow_a_definition() {
    let universe = resolvable_universe();

    // Every `pub fn` this package declares must survive stripping. These are
    // ordinary declarations in files that embed no Keleusma sources, so any
    // absence here is the stripper eating code rather than a known limitation.
    let mut declared = Vec::new();
    for p in rust_and_kel_sources(&package_root().join("src")) {
        let Ok(text) = std::fs::read_to_string(&p) else {
            continue;
        };
        for line in text.lines() {
            if is_comment(line) {
                continue;
            }
            let t = line.trim_start();
            if let Some(rest) = t.strip_prefix("pub fn ") {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    declared.push(name);
                }
            }
        }
    }
    declared.sort();
    declared.dedup();
    assert!(
        declared.len() >= 10,
        "only {} `pub fn` declarations found in src/; 14 were measured on          2026-08-24, so the walk is not reading this package and the check below          proves nothing",
        declared.len()
    );

    let missing: Vec<&String> = declared.iter().filter(|n| !universe.contains(*n)).collect();
    assert!(
        missing.is_empty(),
        "{missing:?} are declared `pub fn` in this package and absent from the          resolvable universe. The string stripper has eaten real code, which is          exactly the failure the `v0.2.3` line hit. Every citation to these names          would now be reported as dangling"
    );

    // The parent crate too, since citations here routinely name it. Chosen
    // because they are the two the other line's broken script reported.
    for probe in ["module_wcmu", "call_with_shared"] {
        assert!(
            universe.contains(probe),
            "`{probe}` is a `pub fn` in the parent crate and is absent from the              universe; the stripper is eating code across files"
        );
    }
}

/// The stripper's own containment property, tested directly rather than inferred
/// from the whole-tree result above.
#[test]
fn an_unbalanced_quote_cannot_reach_beyond_its_own_line() {
    // A lone quote opens a span that runs to end of line and no further.
    let bitten = strip_string_literals(r#"let c = '"'; fn eaten_here() {}"#);
    assert!(
        !bitten.contains("eaten_here"),
        "the stripper did not blank after a lone quote, so the containment claim          below is not the one being tested: {bitten}"
    );

    // **THE PROPERTY THAT MATTERS.** The next line is independent, because the
    // stripper is called per line and holds no state between calls.
    let next = strip_string_literals("pub fn survives_the_previous_line() {}");
    assert!(
        next.contains("survives_the_previous_line"),
        "a definition on the line AFTER an unbalanced quote was blanked. The          stripper has become stateful across lines and can now swallow code the          way the `v0.2.3` line's whole-file regex did: {next}"
    );
}

/// **THE SCAN MUST REACH SOMETHING**, or an empty `dangling` above means only
/// that nothing was read.
///
/// The floor is deliberately loose. Pinning an exact count would make every
/// added comment a failure; what matters is that the walk found this package's
/// files and that its citations are a real population.
#[test]
fn the_scan_is_not_vacuous() {
    let cites = citations_in_package();
    assert!(
        cites.len() > 250,
        "only {} citation(s) found across src/ and tests/; 407 were measured on \
         2026-08-24. The walk is not reaching this package's files, so the guard \
         above proves nothing. **THIS FLOOR IS SET FROM A MEASUREMENT** -- the \
         first draft guessed 100 and the real figure at the four-word threshold \
         was 79, so the guess would have failed the suite on its first run",
        cites.len()
    );
    let universe = resolvable_universe();
    assert!(
        universe.len() > 10_000,
        "the resolvable universe holds only {} names, which is far too few for \
         this repository. Every citation would then look dangling",
        universe.len()
    );
    assert!(
        universe.contains("every_comment_citation_resolves_to_something_real"),
        "the universe does not contain this file's own test name, so the walk is \
         not reading `tests/` at all"
    );
}

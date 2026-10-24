//! **EVERY COMMAND THE RELEASE GATE RUNS HAS A COUNTERPART IN CONTINUOUS INTEGRATION.**
//!
//! `CLAUDE.md` and `docs/process/GIT_STRATEGY.md` permit a merge on continuous integration
//! alone, and justify that with "CI is a verified strict superset of the local gate". The
//! word *verified* was unearned: nothing checked it, and on 2026-09-27 the claim was
//! **false**. The gate's `--miri` step ran
//! `c1_null_text_pointer_marshals_to_empty_string_not_ub` under Tree Borrows and the Miri
//! job ran only `-p keleusma-arena`, so a named undefined-behaviour pin was exercised by no
//! merge — only by a human running the gate before a publication.
//!
//! **WHAT THIS ESTABLISHES, AND WHAT IT DOES NOT.** It establishes that for each command
//! the gate runs, a command of the same shape — same subcommand, same package, same feature
//! selection — appears in a workflow. It does **not** establish that the two do equivalent
//! work. `cargo test --workspace` and `cargo nextest run --profile ci --workspace` are
//! intended to match and differ in runner, profile, and doctest handling. Saying so is the
//! point: a guard that quietly implied equivalence would be the next unearned "verified".
//!
//! **THE DIRECTION IS DELIBERATE.** Only gate-to-workflow is checked. Continuous integration
//! has many jobs the gate lacks — the MSRV checks, `no_std`, the RTOS cross-build, the SDL3
//! examples, the language server, the playground, the book — and that is the intended shape.
//!
//! Both inputs are read structurally rather than by text search: the workflows are parsed as
//! YAML-ish block scalars and the gate's commands are taken from command positions. The
//! repository's comment-matching sweep records nine guards that matched a comment instead of
//! code, and this file is shaped to avoid being the tenth.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// A cargo invocation reduced to the part worth comparing.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Invocation {
    /// `test`, `clippy`, `fmt`, `doc`, `run`, `build`, `check`, `miri-test`.
    verb: String,
    /// The `-p` package, or `--workspace`, or the empty string when neither appears.
    package: String,
    /// Sorted feature names, plus the distinguishing flags that change what is covered.
    selectors: BTreeSet<String>,
}

/// Reduce one shell command line to an `Invocation`, or `None` when it is not a cargo
/// invocation whose coverage is meaningful to compare.
fn parse_invocation(line: &str) -> Option<Invocation> {
    let line = line.trim();
    // Strip a leading environment assignment run, e.g. `MIRIFLAGS="..." cargo ...`, and any
    // `( cd dir && ` wrapper, keeping only the cargo call itself.
    let mut rest = line;
    loop {
        let t = rest.trim_start();
        if let Some(r) = t.strip_prefix("(") {
            rest = r;
            continue;
        }
        if t.starts_with("cd ") {
            // `cd compiler && cargo ...` — the subproject's own commands.
            let at = t.find("&&")?;
            rest = &t[at + 2..];
            continue;
        }
        // An environment assignment is NAME=value with no spaces before the `=`.
        if let Some(eq) = t.find('=') {
            let head = &t[..eq];
            if !head.is_empty()
                && !head.contains(' ')
                && head.chars().all(|c| c.is_ascii_uppercase() || c == '_')
            {
                // Skip the assignment's value, which may be quoted.
                let after = &t[eq + 1..];
                let cut = if let Some(q) = after.strip_prefix('"') {
                    q.find('"').map(|i| eq + 3 + i).unwrap_or(t.len())
                } else {
                    after
                        .find(char::is_whitespace)
                        .map(|i| eq + 1 + i)
                        .unwrap_or(t.len())
                };
                rest = &t[cut.min(t.len())..];
                continue;
            }
        }
        rest = t;
        break;
    }

    let rest = rest.trim();
    let rest = rest.strip_prefix("cargo ")?;
    let mut toks: Vec<&str> = rest.split_whitespace().collect();
    // Drop a toolchain selector like `+nightly`.
    if toks.first().is_some_and(|t| t.starts_with('+')) {
        toks.remove(0);
    }
    if toks.is_empty() {
        return None;
    }

    // Normalise the subcommand. `nextest run` is this project's `test`; `miri test` is its
    // own verb, since a Miri run covers something a plain test run does not.
    let (verb, skip) = match toks[0] {
        "nextest" if toks.get(1) == Some(&"run") => ("test".to_string(), 2),
        "miri" if toks.get(1) == Some(&"test") => ("miri-test".to_string(), 2),
        "test" | "clippy" | "fmt" | "doc" | "run" | "build" | "check" => (toks[0].to_string(), 1),
        _ => return None,
    };

    let mut package = String::new();
    let mut selectors = BTreeSet::new();
    let mut i = skip;
    let mut doc_only = false;
    while i < toks.len() {
        match toks[i] {
            "-p" | "--package" => {
                if let Some(p) = toks.get(i + 1) {
                    package = (*p).to_string();
                }
                i += 2;
                continue;
            }
            "--workspace" => {
                package = "--workspace".to_string();
            }
            "--features" => {
                if let Some(f) = toks.get(i + 1) {
                    for one in f.split(',') {
                        let one = one.trim();
                        if !one.is_empty() {
                            selectors.insert(one.to_string());
                        }
                    }
                }
                i += 2;
                continue;
            }
            "--all-features" => {
                selectors.insert("*all*".to_string());
            }
            "--no-default-features" => {
                selectors.insert("*none*".to_string());
            }
            "--doc" => doc_only = true,
            "--test" => {
                // A single test binary. Keep it: a Miri run of ONE binary is not the same
                // coverage as a whole-package run, and conflating them is how this guard
                // would pass by construction.
                if let Some(t) = toks.get(i + 1) {
                    selectors.insert(format!("test-bin:{t}"));
                }
                i += 2;
                continue;
            }
            "--target" => {
                if let Some(t) = toks.get(i + 1) {
                    selectors.insert(format!("target:{t}"));
                }
                i += 2;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    if doc_only {
        selectors.insert("*doctests*".to_string());
    }
    // `-q`, `--profile ci`, `--all-targets`, `--release`, `-- -D warnings`, `--locked` and
    // the trailing test-name filter are deliberately NOT selectors: they do not change which
    // package and feature combination is covered.
    Some(Invocation {
        verb,
        package,
        selectors,
    })
}

/// Commands from `scripts/release-gate.sh`, taken from command positions only.
fn gate_invocations() -> Vec<(usize, String, Invocation)> {
    let path = root().join("scripts/release-gate.sh");
    let text = std::fs::read_to_string(&path).expect("the release gate script");
    let mut out = Vec::new();
    let mut joined = String::new();
    let mut start_line = 0usize;
    for (n, raw) in text.lines().enumerate() {
        let line = raw.trim_end();
        // A comment line cannot contribute a command. This is what keeps the guard from
        // reading the script's prose, which is the failure the sweep records.
        if line.trim_start().starts_with('#') {
            continue;
        }
        if joined.is_empty() {
            start_line = n + 1;
        }
        if let Some(head) = line.strip_suffix('\\') {
            joined.push_str(head);
            joined.push(' ');
            continue;
        }
        joined.push_str(line);
        let whole = std::mem::take(&mut joined);
        if let Some(inv) = parse_invocation(&whole) {
            out.push((start_line, whole.trim().to_string(), inv));
        }
    }
    out
}

/// Commands from every workflow, taken from `run:` bodies only.
fn workflow_invocations() -> Vec<Invocation> {
    let dir = root().join(".github/workflows");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("the workflows directory") {
        let path = entry.expect("a workflow entry").path();
        if !path.extension().is_some_and(|x| x == "yml" || x == "yaml") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("read a workflow");
        // Take every line that sits in a command position: either `run: <cmd>` or a line
        // inside a block scalar opened by `run: |`. A `#` comment line is skipped, so a
        // command mentioned in a workflow comment cannot satisfy anything.
        let mut in_block = false;
        let mut block_indent = 0usize;
        let mut pending = String::new();
        for raw in text.lines() {
            let indent = raw.len() - raw.trim_start().len();
            let t = raw.trim();
            if in_block {
                if t.is_empty() {
                    continue;
                }
                if indent <= block_indent {
                    in_block = false;
                } else {
                    if !t.starts_with('#') {
                        if let Some(head) = t.strip_suffix('\\') {
                            pending.push_str(head);
                            pending.push(' ');
                            continue;
                        }
                        pending.push_str(t);
                        let whole = std::mem::take(&mut pending);
                        if let Some(inv) = parse_invocation(&whole) {
                            out.push(inv);
                        }
                    }
                    continue;
                }
            }
            if t.starts_with('#') {
                continue;
            }
            // A step may be written as a list item, `- run: cargo ...`, or as a mapping key
            // under `- name:`. BOTH forms must be read. Taking only the bare key left this
            // parse blind to every inline step while the non-vacuity floor still passed,
            // because the block-scalar steps alone exceeded it -- a floor is necessary and
            // is not sufficient.
            let t = t.strip_prefix("- ").map(str::trim_start).unwrap_or(t);
            if let Some(after) = t.strip_prefix("run:") {
                let a = after.trim();
                if a == "|" || a == ">" || a == "|-" || a == ">-" {
                    in_block = true;
                    block_indent = indent;
                    pending.clear();
                } else if let Some(inv) = parse_invocation(a) {
                    out.push(inv);
                }
            }
        }
    }
    out
}

/// Gate commands with a deliberately different counterpart. **Each entry must state why**,
/// because an unexplained exemption is how a real gap becomes normalised.
///
/// **EMPTY, AND THAT WAS CHECKED RATHER THAN ASSUMED.** The first draft exempted `cargo run`
/// on the theory that the link-checker invocation could not be matched. Removing the entry
/// left the test green: the gate's command and the docs-links job's are identical, so they
/// reduce to the same shape with no special case. An exemption keyed on a VERB would also
/// have silently covered every future `cargo run` the gate gained, which is the coarse form
/// this comment warns about — written, then deleted on measuring that it bought nothing.
const EXEMPT: &[(&str, &str)] = &[];

/// **THE GATE'S COVERAGE IS A SUBSET OF THE WORKFLOWS'.**
#[test]
fn every_release_gate_command_has_a_continuous_integration_counterpart() {
    let gate = gate_invocations();
    let ci = workflow_invocations();

    // NON-VACUITY on both sides. A parse that produced nothing would satisfy the
    // containment below, which is the shape of failure this repository keeps meeting.
    assert!(
        gate.len() >= 15,
        "parsed only {} commands from the release gate, so the parse has broken rather than \
         the gate having shrunk",
        gate.len()
    );
    assert!(
        ci.len() >= 30,
        "parsed only {} commands from the workflows, so the parse has broken",
        ci.len()
    );

    let have: BTreeSet<&Invocation> = ci.iter().collect();
    let mut missing = Vec::new();
    for (line, text, inv) in &gate {
        if EXEMPT.iter().any(|(v, _)| *v == inv.verb.as_str()) {
            continue;
        }
        if !have.contains(inv) {
            missing.push(format!(
                "scripts/release-gate.sh:{line}: `{text}` reduces to {inv:?} and no workflow \
                 command reduces to the same shape"
            ));
        }
    }
    assert!(
        missing.is_empty(),
        "{} release-gate command(s) have no continuous-integration counterpart, so the \
         documented claim that CI covers the gate is false and a merge would proceed on \
         evidence the project does not have:\n{}",
        missing.len(),
        missing.join("\n")
    );
}

/// **NO FILE CLAIMS THE RELATIONSHIP RUNS THE OTHER WAY.**
///
/// The gate asserted for months that it was a superset of continuous integration and that
/// continuous integration never saw the detached subproject. Both were true when written and
/// were falsified by two later commits, one of whose subject line is "close the coverage
/// holes so CI is a superset of the local gate". Prose that contradicts the tree is worse
/// than absent prose, because it is read as current.
///
/// **A RETRACTION MUST PARAPHRASE, NOT QUOTE.** This is an ABSENCE assertion, so a file that
/// quotes the retracted wording while explaining that it was wrong satisfies the search
/// exactly as a live claim does. The first draft of the gate's own correction note quoted
/// the phrases and turned this test red -- the tenth instance of the comment-matching class
/// the repository's sweep documents, produced inside the increment that added this guard.
/// Knowing the class does not prevent producing it; running the check does.
#[test]
fn nothing_claims_the_gate_is_a_superset_or_that_ci_skips_the_subproject() {
    for rel in [
        "scripts/release-gate.sh",
        "CLAUDE.md",
        "docs/process/GIT_STRATEGY.md",
        "docs/process/RELEASE_PROCESS.md",
    ] {
        let path = root().join(rel);
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let claims = stale_coverage_claims(&text, rel == "scripts/release-gate.sh");
        assert!(
            claims.is_empty(),
            "{rel} contains stale coverage claims at lines {claims:?}. The compiler/ package \
             has a selfhost-compiler workflow job. Check the named package's actual coverage."
        );
    }
}

/// The phrase check is deliberately narrow. It is not a natural-language coverage proof.
fn stale_coverage_claims(text: &str, release_gate: bool) -> Vec<(usize, &'static str)> {
    let mut claims = Vec::new();
    for (line, text) in text.lines().enumerate() {
        let lower = text.to_lowercase();
        // The native package is a detached workspace with its own local gate. This
        // exact step label makes no claim about compiler/. Exempt only this label,
        // only in the gate script. Adjacent claims and prose remain checked.
        // Deliberately do not infer package scope from a nearby mention or heading.
        let native_step = release_gate
            && lower.trim()
                == r#"step "detached native_codegen/ subproject (fmt, clippy, tests — gated nowhere else)""#;
        for claim in [
            "gate a superset of ci",
            "gated nowhere else",
            "which never sees the subproject",
        ] {
            if lower.contains(claim) && !(native_step && claim == "gated nowhere else") {
                claims.push((line + 1, claim));
            }
        }
    }
    claims
}

#[test]
fn native_gate_label_does_not_claim_compiler_coverage() {
    let native =
        r#"step "Detached native_codegen/ subproject (fmt, clippy, tests — gated nowhere else)""#;
    assert!(stale_coverage_claims(native, true).is_empty());
    // An identically worded label for the compiler remains a regression.
    let compiler = native.replace("native_codegen/", "compiler/");
    assert_eq!(
        stale_coverage_claims(&compiler, true),
        [(1, "gated nowhere else")]
    );
}

#[test]
fn native_label_does_not_exempt_other_lines_or_documents() {
    let native =
        r#"step "Detached native_codegen/ subproject (fmt, clippy, tests — gated nowhere else)""#;
    let mixed = format!("{native}\n# The compiler is gated nowhere else\n");
    assert_eq!(
        stale_coverage_claims(&mixed, true),
        [(2, "gated nowhere else")]
    );
    assert_eq!(
        stale_coverage_claims(native, false),
        [(1, "gated nowhere else")]
    );
}

#[test]
fn unsupported_global_coverage_claims_remain_detected() {
    assert_eq!(
        stale_coverage_claims(
            "# Gate a superset of CI\n# which never sees the subproject",
            true
        ),
        [
            (1, "gate a superset of ci"),
            (2, "which never sees the subproject")
        ]
    );
}

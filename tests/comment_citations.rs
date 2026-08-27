//! A comment that names an identifier which does not exist cannot be wrong out
//! loud.
//!
//! # The defect this exists for
//!
//! `src/compiler.rs` carried a comment asserting that two `Op::IsStruct` routes
//! "verify, receive a memory bound, load, and then trap `InvalidBytecode`" —
//! **the exact class `verify()` exists to exclude** — and cited a test named
//! op_is_struct_still_has_producers_and_two_still_trap as the pin. That test
//! was **never written**. The routes were closed; the comment outlived them and
//! went on asserting a live breach of the load-time guarantee while the tests
//! beside it proved the opposite. An auditor reading the paragraph would
//! conclude the guarantee was broken.
//!
//! `tests/proof_evidence_index.rs` already guards this for the proof documents,
//! on the same reasoning: a claim marked EXECUTED whose test no longer exists
//! still reads as evidence. **Nothing guarded it for source comments**, and the
//! same file turned out to hold a second dangling citation of the same kind.
//!
//! # What this checks, and what it deliberately does not
//!
//! It scans comment lines in `src/` and `tests/` for backtick-quoted
//! `snake_case` identifiers of at least four underscore-separated words — the
//! shape a test name has and a field name almost never does — and requires each
//! to be defined somewhere in the repository as a function, type, constant,
//! module, binding, struct field, or function parameter, in Rust **or** in a
//! `.kel` stage source. **Definitions are taken from code only** — see
//! `defined_names`, which skips comment lines so that this guard cannot vouch
//! for prose with prose.
//!
//! **The four-word threshold is a heuristic and it hides things.** It does not
//! claim every citation is a test, and it cannot: a citation may name a
//! concept, a historical identifier, or something in a document. What it claims
//! is narrower and is the useful half — **a citation that resolves to nothing
//! is at best unhelpful and at worst a false claim about the tree**, and a NEW
//! one is now a failure rather than a silence.
//!
//! # The threshold is measured, not asserted
//!
//! The `v0.3.0` line found that its own four-word cut reported one third of a
//! three-part finding and missed the other two, and made the fair point that a
//! threshold defended by rationale rather than measurement is a blind spot with
//! a story attached. Measured 2026-08-25:
//!
//! | minimum words | citations | unresolved | enforced? |
//! |---|---|---|---|
//! | two | ~910 | 74 | no |
//! | three | ~460 | 31 | no |
//! | four | ~180 | **13** | the unresolved count only |
//!
//! **THE TWO-WORD FIGURE WAS FIRST WRITTEN AS 76 BY SUBTRACTING** the eight
//! removed entries from the previous 84, in this file, one paragraph below a
//! heading about measurements that are not measured. It is 74. Two of the eight
//! repairs resolved names that a shorter cut counts and a four-word cut does
//! not, so the arithmetic does not carry across rows. Derived, not subtracted,
//! and the miss is left here because the alternative is a table that models the
//! discipline it describes and was produced by ignoring it.
//!
//! **THE TOTALS ARE APPROXIMATE ON PURPOSE, AND THAT IS NOT MODESTY.** This
//! scanner counts citations in this repository, and this file is in this
//! repository — so **the record of the measurement lives inside the population
//! the measurement counts**, and writing the number down changes it. Two
//! attempts at an exact table were wrong at the moment they were committed: the
//! first read 897/104, 453/48, 175/21, measured before the same commit widened
//! the universe; the second read 905/454/176 and was 906/455/177 immediately
//! after the prose stating it was added. Each correction added prose, and prose
//! contains citations.
//!
//! **An exact total is not a property this file can hold**, so chasing one is
//! the error, not the staleness. The test to apply before publishing a figure
//! is *does writing this down change what it counts?*
//!
//! **The unresolved counts are exact and held across every re-derivation** —
//! 84, 39, 21 — because added prose contributes citations that RESOLVE rather
//! than dangle. Totals are self-inclusive and unstable; findings are not.
//! Precision is kept where it means something and dropped where it cannot be
//! held. The `v0.3.0` line found this property first, in its own file, and the
//! split is theirs.
//!
//! **Only the four-word unresolved count is enforced**, by
//! `the_unresolved_backlog_is_recorded`.
//!
//! **The 63 unresolved that a two-word cut adds are dominated by names this
//! scan has no business resolving**: standard-library and language items (`as_bytes`,
//! `catch_unwind`, `size_of`, `unwrap_or`, and about twenty more), `.kel` stage
//! FILE stems rather than functions, target names, and ordinary prose.
//!
//! **THREE OF THEM WERE FORWARDED TO ANOTHER LINE AS WORTH INVESTIGATING, AND
//! TWO WERE THIS SCANNER'S OWN FAULT.** (A third false positive of the same
//! family, tuple-destructured bindings, was found during the 2026-08-25 triage
//! and fixed the same way.) `must_contain` and `head_name` are
//! function parameters written inline in a single-line signature, which the
//! `name:` rule below did not reach until it was widened. They were never
//! defects. Only one of the three was real, and it was a stale name rather than
//! a missing thing: a comment in `tests/selfhost_wire.rs` cited a VACUITY
//! CONTROL that exists under a different name, so a reader checking whether
//! that slice could go vacuous would have found nothing and concluded there was
//! no control. Repaired.
//!
//! The `v0.3.0` line ran all four names through a token-based universe built on
//! a different principle and reached the same three verdicts, which is
//! corroboration rather than agreement. **The two scanners fail in opposite
//! directions**: this one is declaration-based and can MANUFACTURE a finding,
//! theirs is token-based and can MISS one where a citation names something
//! other than what it claims. Neither subsumes the other.
//!
//! Four words is therefore kept **as a signal-to-noise judgement with the noise
//! measured**, not as a claim of completeness. Lowering it is a real option and
//! costs an excuse list of 104 mostly-spurious names, which nobody would
//! maintain and which would make this guard the thing readers mute. **The
//! honest statement is that this catches four-word citations and says nothing
//! about shorter ones.**
//!
//! # The allow list is a debt, not a baseline
//!
//! **21 on 2026-08-24, thirteen on 2026-08-25.** The fall is deliberately
//! reported in two categories, because they are not the same event:
//!
//! - **Seven citations repaired.** Each now names something a reader can find,
//!   verified by reading the named item and confirming it supports the claim
//!   the citing sentence makes. **Two of the seven were REVERSALS** — a
//!   citation naming a test that asserts the OPPOSITE of what the tree does,
//!   which is worse than naming nothing because it looks like a reference. Both
//!   needed the surrounding prose rewritten, not just the pointer: one asserted
//!   a trailing semicolon after `for` is rejected when it is accepted, the
//!   other asserted two compilers diverge on a string literal when they now
//!   agree.
//! - **One was never a defect.** `shared_data_flat_bytes` is an ordinary local
//!   in `compile_with_target`, written `let (a, b) = ...`, which `defined_names`
//!   could not see — the same class as the inline-parameter miss. Fixed in the
//!   scan, not in the comment.
//!
//! **Conflating those two would overstate the repair.** A register that falls
//! because the scanner stopped manufacturing findings has not been paid down.
//!
//! **ONE OF THE SEVEN WAS REPAIRED TWICE, AND THE FIRST REPAIR RESOLVED WITHOUT
//! SUPPORTING THE CLAIM.** A comment saying pass 1b resolves field and variant
//! type expressions "through X" was repointed at
//! `from_expr_with_params_and_frac`, which exists — but that pass calls
//! `Ctx::resolve_type_with_params`, and the method's own documentation says it
//! is reached THROUGH that. **A citation can resolve and still be wrong.**
//!
//! That is the reversal above with the volume turned down, and it is why this
//! increment's completion condition asks for the named item to SUPPORT the
//! claim rather than merely exist. **The guard's green is evidence about the
//! NAME and says nothing about the claim** — nothing here can tell a correct
//! repointing from an incorrect one, because both resolve. Anyone shrinking the
//! register below should read that sentence twice.
//!
//! The thirteen that remain are listed rather than fixed because verifying each
//! means establishing what it was meant to name, and none has a near match in
//! the tree — they are archaeology rather than renames.
//! **Shrink this list; never grow it.** `no_allow_list_entry_is_stale` enforces
//! both directions: an entry whose citation was deleted, and an entry whose
//! name has since come to exist, are equally stale. So the list cannot quietly
//! outlive its own justification, which is the very failure this file exists
//! for.
//!
//! **The list does not vouch for itself.** The `v0.3.0` line's equivalent guard
//! built its universe of resolvable names from all non-comment lines, which
//! included its own excuse table's string literals, so every excused name
//! resolved — to the excuse list. Checked here: none of the 21 entries matches
//! either rule in `defined_names`, since an array element carries neither a
//! declaration keyword nor a `name:` field form. `no_allow_list_entry_is_stale`
//! would fail if one did, so the property is enforced rather than asserted.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Citations that resolve to nothing as of 2026-08-24, recorded so a new one is
/// distinguishable from the backlog.
///
/// Each is a claim someone made in a comment that the reader cannot follow.
/// None has been shown harmless; none has been shown harmful either.
const UNRESOLVED: &[&str] = &[
    "chunks_exact_to_as_chunks",
    "enforce_chunk_size_limit",
    "hot_swap_new_schema_replaced",
    "layout_has_flat_text",
    "narrow_runtime_can_register_text_library_via_lifted_impl",
    "nested_option_match_is_a_language_limitation",
    "the_pipeline_rows_are_the_declared_subset",
    "the_reserved_kinds_are_not_emitted_by_any_encoder",
    "the_rules_reach_only_literal_direct_occurrences",
    "the_token_cap_binds_only_the_collecting_feed",
    "yield_dynamic_string_fails",
    "yield_tuple_with_dynamic_string_fails",
];

/// Directories never scanned or indexed: build output, vendored code, and the
/// translated book, none of which is this repository's source.
const SKIP_DIRS: &[&str] = &[".git", "target", "node_modules", "book"];

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if SKIP_DIRS.contains(&name.as_ref()) {
                continue;
            }
            walk(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// Every name the repository defines, across both languages.
///
/// Deliberately GENEROUS. A false "defined" costs a missed citation; a false
/// "undefined" costs a spurious failure that trains a reader to ignore this
/// test, which is worse.
fn defined_names() -> BTreeSet<String> {
    let mut files = Vec::new();
    walk(&root(), &mut files);
    let mut names = BTreeSet::new();
    for path in files {
        let ext = path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        if ext != "rs" && ext != "kel" {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for line in text.lines() {
            let t = line.trim_start();
            // **PROSE IS NOT EVIDENCE OF A DEFINITION.** A comment containing
            // `fn foo` or `bar: Baz` would otherwise put those names in the
            // universe, so a citation could resolve against another comment —
            // this guard vouching for prose with prose.
            //
            // The `v0.3.0` line found the same coupling in a corpus-coverage
            // audit of theirs, where two modules' exemptions were being
            // satisfied by a paragraph rather than by the harness that actually
            // drove them.
            //
            // **The exclusion costs nothing, and the evidence is this suite
            // rather than a side measurement.** Every test here still passes
            // with comment lines skipped, and
            // `every_comment_citation_resolves_or_is_a_recorded_debt` is
            // exactly the check that would fail if any citation had been
            // resolving against prose. `a_name_only_a_comment_mentions_does_
            // not_resolve` pins the property directly.
            //
            // A throwaway script written to size the class first reported "1194
            // comment-only names, six citation-shaped". **Those six do not
            // exist anywhere in the tree** — the script was wrong and the
            // figures are not repeated here. It was caught only because one of
            // the six was picked to build a test around and would not grep.
            if t.starts_with("//") || t.starts_with('*') || t.starts_with("/*") {
                continue;
            }
            // Declarations: `fn f`, `struct S`, `let x`, `const C`, and the rest.
            for kw in [
                "fn ",
                "struct ",
                "enum ",
                "trait ",
                "const ",
                "static ",
                "mod ",
                "type ",
                "let ",
                "macro_rules! ",
            ] {
                let mut rest = t;
                while let Some(i) = rest.find(kw) {
                    rest = &rest[i + kw.len()..];
                    let after = rest.trim_start_matches("mut ");
                    // A TUPLE-DESTRUCTURED BINDING is still a binding.
                    // `let (a, b) = ...` used to yield nothing, because the
                    // character after `let ` is `(`. `shared_data_flat_bytes`
                    // was reported as a dangling citation on that ground alone
                    // and is an ordinary local in `compile_with_target` — the
                    // same class as the inline-parameter miss this scan was
                    // already corrected for.
                    let after = after.trim_start_matches(['(', ' ']);
                    for part in after.split(',') {
                        let ident: String = part
                            .trim_start_matches([' ', '(', '&'])
                            .trim_start_matches("mut ")
                            .chars()
                            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                            .collect();
                        if ident.is_empty() {
                            break;
                        }
                        names.insert(ident);
                    }
                }
            }
            // Anything bound with a `name:` type ascription: a struct or enum
            // field, a named record argument, and — the case this missed —
            // **a function parameter written inline in a single-line
            // signature**.
            //
            // The rule was once "at the start of an indented line", which
            // covered fields and missed `fn f(src: &str, must_contain: &str)`
            // entirely. That MANUFACTURED findings: two of three citations
            // this file's author forwarded to another line as worth
            // investigating were parameters of exactly that shape, and were
            // not defects at all. A guard that invents findings is worse than
            // a narrow one, because it trains its reader to disregard it. So
            // every `name:` on the line is taken, not just the first.
            for (i, _) in t.match_indices(':') {
                // A `::` path separator is not an ascription.
                if t[..i].ends_with(':') || t[i + 1..].starts_with(':') {
                    continue;
                }
                let head: String = t[..i]
                    .chars()
                    .rev()
                    .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_')
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                if !head.is_empty() {
                    names.insert(head);
                }
            }
        }
    }
    names
}

/// Every backticked `snake_case` citation of at least four words that appears in
/// a comment under `src/` or `tests/`.
///
/// **Comment blocks are joined before scanning, and whitespace inside a
/// backticked span is removed**, because a long identifier gets WRAPPED across
/// two comment lines. The first draft of this scanner did not, and reported
/// `a_struct_pattern_against_a_foreign_type_is_refused_` — a fragment of a name
/// that exists — as a dangling citation. **A guard that manufactures its own
/// findings is worse than no guard**, so the wrap is handled rather than
/// tolerated.
fn citations() -> Vec<(String, String)> {
    let mut files = Vec::new();
    for dir in ["src", "tests"] {
        walk(&root().join(dir), &mut files);
    }
    files.sort();
    let mut found = Vec::new();
    for path in files {
        if path.extension().unwrap_or_default() != "rs" {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let rel = path
            .strip_prefix(root())
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();

        // Comment lines with their marker stripped, paired with their number.
        // A non-comment line ends the block, so a backtick left open by code
        // cannot swallow the rest of the file.
        let mut block: Vec<(usize, &str)> = Vec::new();
        let flush = |block: &mut Vec<(usize, &str)>, found: &mut Vec<(String, String)>| {
            if block.is_empty() {
                return;
            }
            let line_no = block[0].0;
            let joined: String = block.iter().map(|(_, t)| *t).collect::<Vec<_>>().join("\n");
            for piece in joined.split('`').skip(1).step_by(2) {
                let name: String = piece.chars().filter(|c| !c.is_whitespace()).collect();
                if !is_citation_shaped(&name) {
                    continue;
                }
                found.push((name, format!("{rel}:{line_no}")));
            }
            block.clear();
        };
        for (i, line) in text.lines().enumerate() {
            let t = line.trim_start();
            let stripped = t
                .strip_prefix("//!")
                .or_else(|| t.strip_prefix("///"))
                .or_else(|| t.strip_prefix("//"))
                .or_else(|| t.strip_prefix('*'));
            match stripped {
                Some(body) => block.push((i + 1, body)),
                None => flush(&mut block, &mut found),
            }
        }
        flush(&mut block, &mut found);
    }
    found
}

/// Does this look like an identifier someone meant a reader to go and find?
///
/// Four underscore-separated words is the threshold: a test name reaches it
/// routinely and a field name almost never does. A doubled underscore is
/// excluded because it marks a MANGLED name — `origin__type_args` is a
/// monomorphization spelling, not something a reader can look up.
fn is_citation_shaped(name: &str) -> bool {
    name.len() >= 4
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !name.contains("__")
        && !name.ends_with('_')
        && name.matches('_').count() >= 3
}

/// **THE GUARD.** A citation must resolve, or be a recorded debt.
#[test]
fn every_comment_citation_resolves_or_is_a_recorded_debt() {
    let defined = defined_names();
    let allow: BTreeSet<&str> = UNRESOLVED.iter().copied().collect();

    let mut fresh: Vec<(String, String)> = Vec::new();
    for (name, site) in citations() {
        if defined.contains(&name) || allow.contains(name.as_str()) {
            continue;
        }
        fresh.push((name, site));
    }

    assert!(
        fresh.is_empty(),
        "these comments cite identifiers that exist NOWHERE in the repository, \
         in Rust or in a `.kel` stage source:\n{}\n\nA citation that resolves \
         to nothing cannot be wrong out loud. Either name something that \
         exists, state the claim without a citation, or — if it is genuinely a \
         historical name worth keeping — add it to `UNRESOLVED` with a reason. \
         Adding to that list is the option of last resort; it is a debt \
         register, not a baseline.",
        fresh
            .iter()
            .map(|(n, s)| format!("  `{n}` at {s}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// A function parameter written inline in a single-line signature resolves.
///
/// **This is the rule whose absence manufactured findings.** `must_contain` and
/// `head_name` are parameters of `match_body` and `assert_multihead_matches`
/// respectively; with the old "`name:` at the start of an indented line" rule
/// they resolved to nothing, and both were forwarded to another line as
/// citations worth investigating. Neither was a defect.
///
/// Both are below this file's four-word citation threshold, so no citation
/// check would catch a regression here. The rule is therefore tested directly.
/// To see it fail, restrict the `name:` scan back to indented lines.
#[test]
fn an_inline_function_parameter_is_a_defined_name() {
    let defined = defined_names();
    for (name, owner) in [
        ("must_contain", "match_body"),
        ("head_name", "assert_multihead_matches"),
    ] {
        assert!(
            defined.contains(name),
            "`{name}`, a parameter of `{owner}`, does not resolve. The scan has \
             stopped reaching inline signatures, and citations naming such a \
             parameter will be reported as dangling when they are not."
        );
    }
}

/// A name that only a comment mentions is NOT a definition.
///
/// The name below appears in this repository exactly once, in the line above
/// it, written as a declaration inside a comment. If `defined_names` ever
/// admits it, this guard has started vouching for prose with prose: a citation
/// could then resolve against another comment rather than against anything
/// that exists.
///
/// To see it fail, stop skipping comment lines in `defined_names`.
// fn a_name_only_a_comment_mentions() {}
#[test]
fn a_name_only_a_comment_mentions_does_not_resolve() {
    // Assembled at run time so that writing this test does not put the name
    // into the very universe it is checking — the hazard one line up.
    let planted = ["a_name", "only_a", "comment", "mentions"].join("_");
    assert!(
        !defined_names().contains(&planted),
        "`{planted}` appears only inside a comment, and `defined_names` \
         admitted it. Comments are not definitions; a guard that treats them as \
         definitions can satisfy one comment's claim with another comment."
    );
}

/// The allow list may not outlive the citations it excuses.
///
/// **Without this, the list is the same defect one level up**: an entry whose
/// citation was fixed or deleted would sit there forever, and the count would
/// read as a backlog that is not shrinking when in fact it no longer exists.
#[test]
fn no_allow_list_entry_is_stale() {
    let cited: BTreeSet<String> = citations().into_iter().map(|(n, _)| n).collect();
    let defined = defined_names();
    let stale: Vec<&str> = UNRESOLVED
        .iter()
        .copied()
        .filter(|n| !cited.contains(*n) || defined.contains(*n))
        .collect();
    assert!(
        stale.is_empty(),
        "these `UNRESOLVED` entries are no longer needed — the citation was \
         removed, or the identifier now exists. Delete them; the list must \
         shrink: {stale:?}"
    );
}

/// The debt is 12 and the direction that matters is down.
///
/// **This is a MEASUREMENT, not an invariant.** It is pinned so that a change
/// is deliberate: shrinking it is the point, and growing it means the guard
/// above was answered by widening the excuse rather than by fixing the comment.
///
/// # History, so the direction is visible
///
/// 21 at introduction, then 13, then **12 on 2026-08-27**.
///
/// The entry retired was the one naming a type-rejection test for derived operands. **Its
/// identifier is deliberately not spelled here**: backticking a dead name re-creates the very
/// citation this file exists to catch, and doing so in the comment explaining the retirement
/// has now tripped this guard three times. Prose about a name is the same characters as a use
/// of it.
///
/// That entry named a test that no longer exists — commit `63574d1f` closed the arithmetic
/// half of the gap with a bounded fixpoint — and **three live comments cited it**, all
/// asserting that an arithmetic result is still unreachable. The register excused all three,
/// so nothing failed while three comments and one handoff entry said something untrue.
///
/// **A citation in this register is not a citation that is right.** It is one that has been
/// excused from being checked. The three comments now name
/// `a_derived_operand_from_a_field_read_is_still_unreached`, which holds the edge that
/// genuinely remains.
#[test]
fn the_unresolved_backlog_is_recorded() {
    assert_eq!(
        UNRESOLVED.len(),
        12,
        "the recorded backlog of unresolved citations changed. Down is the \
         whole point — update this number and say which citation you resolved. \
         Up needs a reason in the commit message."
    );
}

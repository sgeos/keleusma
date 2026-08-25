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
//! module, binding, or struct field, in Rust **or** in a `.kel` stage source.
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
//! a story attached. Measured here on 2026-08-24:
//!
//! | minimum words | citations | unresolved |
//! |---|---|---|
//! | two | 897 | 104 |
//! | three | 453 | 48 |
//! | four | 175 | 21 |
//!
//! **The 83 additional at two words are dominated by names this scan has no
//! business resolving**: standard-library and language items (`as_bytes`,
//! `catch_unwind`, `size_of`, `unwrap_or`, and about twenty more), `.kel` stage
//! FILE stems rather than functions, target names, and ordinary prose. A
//! handful — `orders_differ_somewhere`, `must_contain`, `head_name` — would
//! repay triage.
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
//! Measured 2026-08-24: **21 citations** resolve to nothing. They are listed
//! rather than fixed because verifying each means establishing what it was
//! meant to name, which is per-item work this increment did not do.
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
    "a_trailing_semicolon_after_for_is_rejected_where_after_if_it_is_accepted",
    "all_wire_ops_decode",
    "chunks_exact_to_as_chunks",
    "closures_rejected_at_typecheck",
    "enforce_chunk_size_limit",
    "first_class_function_rejected_at_compile",
    "from_expr_with_params",
    "hot_swap_new_schema_replaced",
    "layout_has_flat_text",
    "narrow_runtime_can_register_text_library_via_lifted_impl",
    "nested_option_match_is_a_language_limitation",
    "shared_data_flat_bytes",
    "the_pipeline_rows_are_the_declared_subset",
    "the_reserved_kinds_are_not_emitted_by_any_encoder",
    "the_rules_reach_only_literal_direct_occurrences",
    "the_rules_still_do_not_reach_a_derived_operand",
    "the_token_cap_binds_only_the_collecting_feed",
    "the_two_self_hosted_compilers_disagree_on_a_string_literal",
    "type_flat_scalar_kind",
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
                    let ident: String = rest
                        .trim_start_matches("mut ")
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect();
                    if !ident.is_empty() {
                        names.insert(ident);
                    }
                }
            }
            // Struct and enum fields, and named record arguments: `name:` at the
            // start of an indented line.
            if line.starts_with(' ')
                && let Some(colon) = t.find(':')
            {
                let head = t[..colon]
                    .trim_start_matches("pub ")
                    .trim_start_matches("pub(crate) ")
                    .trim();
                if !head.is_empty()
                    && head
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                {
                    names.insert(head.to_string());
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

/// The debt is 21 and the direction that matters is down.
///
/// **This is a MEASUREMENT, not an invariant.** It is pinned so that a change
/// is deliberate: shrinking it is the point, and growing it means the guard
/// above was answered by widening the excuse rather than by fixing the comment.
#[test]
fn the_unresolved_backlog_is_recorded() {
    assert_eq!(
        UNRESOLVED.len(),
        21,
        "the recorded backlog of unresolved citations changed. Down is the \
         whole point — update this number and say which citation you resolved. \
         Up needs a reason in the commit message."
    );
}

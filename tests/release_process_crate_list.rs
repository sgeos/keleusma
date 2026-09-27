//! The release process's crate list must equal the set of crates that actually publish.
//!
//! # Why this file exists
//!
//! On 2026-09-01 a census found that `RELEASE_PROCESS.md` said FIVE crates publish to
//! crates.io when there are SEVEN. `keleusma-wire` and `keleusma-wire-derive` appeared in
//! none of its four enumerations. **Following the document as written loses money**: it
//! publishes `keleusma-macros` and `keleusma-arena`, both irreversible, and then fails on
//! `keleusma`, because the registry has no `keleusma-wire` to resolve. The failure lands
//! after the point where the abort criteria still help.
//!
//! # The property that made it invisible, which is the reason for a test rather than a fix
//!
//! **Nothing was inconsistent; something was absent.** Both wire crates are marked
//! publishable, carry a description, licence and repository, have their own
//! continuous-integration job, and are covered by the release gate. Every artifact the
//! tooling can inspect said they were ready. The one document the tooling cannot inspect
//! had never heard of them. **A missing entry has no line number**, so no reviewer,
//! linter or diff could point at it.
//!
//! Correcting the document closed the instance. It did not close the class: the next crate
//! added to this workspace can be omitted from the list in exactly the same silent way.
//! This test closes the class, by deriving one side from the filesystem instead of trusting
//! both sides to be edited together.
//!
//! # Why the manifests are the authority and the document is the claim
//!
//! `publish` in a manifest is what `cargo publish` obeys. The document is prose describing
//! it. When they disagree the manifest is right by construction, so the manifest set is
//! derived and the document set is checked against it, never the reverse.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Collect every tracked `Cargo.toml`, skipping build output, ignored scratch
/// directories and hidden directories.
///
/// `tmp/` is gitignored and holds vendored third-party workspaces; including it would put
/// crate names from other projects into the population and make this guard fail for a
/// reason that has nothing to do with this release.
fn manifests(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if name == "target" || name == "tmp" || name.starts_with('.') {
                continue;
            }
            manifests(&path, out);
        } else if name == "Cargo.toml" {
            out.push(path);
        }
    }
}

/// The package name and whether it publishes, for one manifest. `None` when the file is a
/// workspace root with no `[package]` of its own.
fn package_of(path: &Path) -> Option<(String, bool)> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut name = None;
    let mut publishes = true;
    for line in text.lines() {
        let line = line.trim();
        if name.is_none()
            && let Some(rest) = line.strip_prefix("name = ")
        {
            name = Some(rest.trim_matches('"').to_string());
        }
        if let Some(rest) = line.strip_prefix("publish")
            && rest.contains("false")
        {
            publishes = false;
        }
    }
    name.map(|n| (n, publishes))
}

/// Crate names in the numbered list under the crates heading.
///
/// **Scoped to that one section, not to the whole document.** Today no other numbered list
/// in the file begins with a backticked `keleusma` name, so an unscoped scan would give the
/// same answer -- which is exactly the condition under which an over-broad scan looks
/// correct and later stops being. A future numbered list naming a crate that must NOT
/// publish, such as the language server, would make an unscoped guard fail for a reason
/// unrelated to the property it guards, and a guard that manufactures its own findings gets
/// disabled rather than fixed.
fn listed_in_document(doc: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    // The section runs from its heading to the next heading of the same level.
    let section = match doc.split_once("\n## The crates") {
        Some((_, rest)) => rest.split("\n## ").next().unwrap_or(rest),
        None => panic!(
            "RELEASE_PROCESS.md has no `## The crates` section; this guard is reading the \
             wrong document or the heading was renamed, and either way it is measuring nothing"
        ),
    };
    for line in section.lines() {
        let t = line.trim_start();
        // `1. `keleusma-macros` — ...`
        let Some(rest) = t
            .split_once(". ")
            .map(|(n, r)| (n.parse::<u32>().is_ok(), r))
        else {
            continue;
        };
        if !rest.0 {
            continue;
        }
        let after = rest.1.trim_start();
        if let Some(inner) = after.strip_prefix('`')
            && let Some(end) = inner.find('`')
        {
            let candidate = &inner[..end];
            if candidate.starts_with("keleusma") {
                out.insert(candidate.to_string());
            }
        }
    }
    out
}

#[test]
fn the_release_process_names_exactly_the_crates_that_publish() {
    let mut found = Vec::new();
    manifests(&root(), &mut found);

    let mut publishable = BTreeSet::new();
    let mut suppressed = BTreeSet::new();
    for path in &found {
        if let Some((name, publishes)) = package_of(path) {
            if publishes {
                publishable.insert(name);
            } else {
                suppressed.insert(name);
            }
        }
    }

    // NON-VACUOUS. A walk that found nothing, or a parse that recognised nothing, would
    // otherwise satisfy an equality of two empty sets while checking nothing at all. This
    // repository has had two derivations pass that way.
    assert!(
        publishable.len() >= 5,
        "only {} publishable crates were found, so the manifest walk is not working: {publishable:?}",
        publishable.len()
    );
    assert!(
        !suppressed.is_empty(),
        "no crate was found with publish = false, so the publish flag is not being parsed \
         and every crate would look publishable"
    );

    let doc_path = root().join("docs/process/RELEASE_PROCESS.md");
    let doc = std::fs::read_to_string(&doc_path).expect("read RELEASE_PROCESS.md");
    let listed = listed_in_document(&doc);

    let missing: Vec<_> = publishable.difference(&listed).collect();
    let extra: Vec<_> = listed.difference(&publishable).collect();

    assert!(
        missing.is_empty(),
        "these crates PUBLISH but the release process does not list them: {missing:?}. \
         Publishing in the documented order would run the irreversible publishes first and \
         then fail at the registry, which is the defect this guard exists for."
    );
    assert!(
        extra.is_empty(),
        "the release process lists these, but no manifest publishes them: {extra:?}"
    );

    // The stated count is a second, independent claim in the same document, and the
    // original defect was a wrong count sitting above a short list. Checking the list
    // alone would have let `FIVE` stand over seven correct entries.
    let word = match publishable.len() {
        5 => "FIVE",
        6 => "SIX",
        7 => "SEVEN",
        8 => "EIGHT",
        9 => "NINE",
        n => panic!("no count word for {n} crates; extend this table"),
    };
    assert!(
        doc.contains(word),
        "there are {} publishable crates, so the document should state {word}, and it does not",
        publishable.len()
    );
}

/// Every publishable crate is assigned a versioning policy, and the crates that track
/// `keleusma` actually do.
///
/// # The hazard, which is the crate-list defect wearing a different hat
///
/// The release process states two policies: four crates track the major-minor of
/// `keleusma`, and three version independently. **Nothing checked that every
/// publishable crate falls under one of them.** A crate added to the workspace and
/// correctly listed in the publish order can still be absent from the versioning
/// sentence, and then nobody knows whether to bump it at release time. That is the
/// same shape as the omission that made this file necessary: consistent everywhere
/// the tooling looks, absent from the one place it does not.
///
/// # Why the partition is checked, not just the arithmetic
///
/// Comparing version numbers alone would pass while a crate sat in neither group.
/// The partition is the property with teeth: **every publishable crate in exactly one
/// policy, no crate in both, and no policy naming a crate that does not publish.**
#[test]
fn every_publishable_crate_has_a_versioning_policy_and_the_tracking_ones_track() {
    let mut found = Vec::new();
    manifests(&root(), &mut found);

    let mut publishable = BTreeSet::new();
    let mut version_of = alloc_map();
    for path in &found {
        if let Some((name, publishes)) = package_of(path)
            && publishes
        {
            if let Some(v) = version_in(path) {
                version_of.insert(name.clone(), v);
            }
            publishable.insert(name);
        }
    }
    assert!(
        publishable.len() >= 5,
        "the manifest walk found only {} publishable crates",
        publishable.len()
    );

    let doc = std::fs::read_to_string(root().join("docs/process/RELEASE_PROCESS.md"))
        .expect("read RELEASE_PROCESS.md");

    // **SCOPED TO THE POLICY PARAGRAPH, NOT TO A PREFIX OF THE DOCUMENT.** The first
    // draft took everything before the phrase "track the", which swept in the
    // numbered publish list above it and put all seven crates in the tracking set.
    // The test then failed on a true statement -- that the arena crate does not
    // track -- for a reason that had nothing to do with the tree. Scoping to a
    // prefix rather than to the sentence is the same defect this file's other guard
    // was already corrected for.
    let para = policy_paragraph(&doc);
    let cut = para
        .find("track the")
        .expect("the release process states a major-minor tracking policy");
    let tracking = backticked_crates(&para[..cut], &publishable);
    let rest = &para[cut..];
    let cut2 = rest
        .find("have their own versions")
        .expect("the release process states an independent-versioning policy");
    // `keleusma` appears again in "major-minor of `keleusma`"; it is already in the
    // tracking set, so excluding names already claimed there removes it without a
    // special case.
    let independent: BTreeSet<String> = backticked_crates(&rest[..cut2], &publishable)
        .difference(&tracking)
        .cloned()
        .collect();

    let both: Vec<_> = tracking.intersection(&independent).collect();
    assert!(
        both.is_empty(),
        "these crates are in both policies: {both:?}"
    );

    let classified: BTreeSet<String> = tracking.union(&independent).cloned().collect();
    let unclassified: Vec<_> = publishable.difference(&classified).collect();
    assert!(
        unclassified.is_empty(),
        "these crates publish but no versioning policy names them: {unclassified:?}. \
         At release time nobody knows whether to bump them."
    );

    // And the tracking crates must actually track.
    let anchor = version_of
        .get("keleusma")
        .expect("the runtime crate has a version");
    let anchor_mm = major_minor(anchor);
    for c in &tracking {
        let v = version_of
            .get(c)
            .unwrap_or_else(|| panic!("no version for {c}"));
        assert_eq!(
            major_minor(v),
            anchor_mm,
            "{c} is {v} but is declared to track the major-minor of keleusma at {anchor}"
        );
    }
}

/// Backticked crate names in a slice of the document, restricted to names that
/// actually publish so prose mentioning a non-publishing crate cannot enter a policy
/// set.
fn backticked_crates(text: &str, publishable: &BTreeSet<String>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for (i, piece) in text.split('`').enumerate() {
        if i % 2 == 1 && publishable.contains(piece) {
            out.insert(piece.to_string());
        }
    }
    out
}

fn version_in(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    for line in text.lines() {
        if let Some(rest) = line.trim().strip_prefix("version = ") {
            return Some(rest.trim_matches('"').to_string());
        }
    }
    None
}

fn major_minor(v: &str) -> (String, String) {
    let mut it = v.split('.');
    (
        it.next().unwrap_or_default().to_string(),
        it.next().unwrap_or_default().to_string(),
    )
}

fn alloc_map() -> std::collections::BTreeMap<String, String> {
    std::collections::BTreeMap::new()
}

/// The paragraph stating the versioning policies, bounded by blank lines.
///
/// Returned as a slice so both policy sentences are read from the same paragraph;
/// searching the whole document instead lets an unrelated sentence elsewhere -- the
/// language server's diagnostics "track the grammar", for one -- move the split.
fn policy_paragraph(doc: &str) -> &str {
    let anchor = doc
        .find("major-minor")
        .expect("the release process states a major-minor policy");
    let start = doc[..anchor].rfind("\n\n").map(|i| i + 2).unwrap_or(0);
    let end = doc[anchor..]
        .find("\n\n")
        .map(|i| anchor + i)
        .unwrap_or(doc.len());
    &doc[start..end]
}

/// **THE TARBALL STILL EXCLUDES WHAT MUST NOT SHIP.**
///
/// The strongest entry is `secret/`. It holds the operator's out-of-repository requirements
/// note, and publishing to a registry is **irreversible**: a version, once uploaded, cannot be
/// withdrawn, only yanked, and the tarball remains downloadable. Losing that exclusion would
/// disclose material the repository is explicitly instructed never to place in tracked files,
/// let alone in a public artifact.
///
/// Measured 2026-09-27: `cargo package --list -p keleusma` shows `secret/`, `compiler/`,
/// `examples/rtos/` and `book/` all absent, so the declaration is currently effective.
///
/// **THE TOTAL FILE COUNT IS DELIBERATELY NOT QUOTED HERE.** The first draft of this comment
/// said 335 files. It was 336 within the same increment, because this increment adds a test
/// file and tests ship in the tarball. Two increments earlier this session removed
/// `wrote pulse.bin (2372 bytes)` and `wrote tune.kel.bin (2400 bytes)` from the guide for
/// exactly that reason, and then reproduced the shape here -- which is the repository's own
/// observation that knowing a failure class does not prevent producing it. What matters is
/// the ABSENCES, which are properties of the exclude list rather than of the tree's size. `secret/` is in fact protected twice, by `.gitignore` and by this list;
/// both would have to be lost. **This guard found nothing.** It exists because the failure is
/// irreversible and the cost of the check is a few lines.
///
/// **WHAT THIS CHECKS, AND WHAT IT DOES NOT.** It checks the manifest's DECLARATION. It does
/// not run `cargo package`, so it cannot prove a built tarball omits a path — a second
/// exclusion mechanism could be removed while this list stayed intact, or a future Cargo could
/// interpret the list differently. Saying so is the point: a guard that implied it had verified
/// the artifact would be the kind of overclaim this suite exists to prevent.
#[test]
fn the_package_manifest_still_excludes_the_paths_that_must_not_be_published() {
    let manifest = root().join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest).expect("the root manifest");

    let open = text.find("exclude = [").expect(
        "the root manifest no longer has an `exclude` list, so nothing is withheld from \
                 the published tarball",
    );
    let rest = &text[open..];
    let close = rest.find(']').expect("an unterminated exclude list");
    let body = &rest[..close];

    // Comment lines inside the list explain entries; an entry named only in a comment must not
    // satisfy this check.
    let entries: Vec<String> = body
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with('#'))
        .flat_map(|l| l.split(','))
        .map(|p| p.trim().trim_matches('"').trim().to_string())
        .filter(|p| !p.is_empty() && p != "exclude = [")
        .collect();

    // NON-VACUITY: an extraction that produced nothing would satisfy every containment below.
    assert!(
        entries.len() >= 8,
        "parsed only {} exclude entries, so the extraction has broken rather than the list \
         having shrunk: {entries:?}",
        entries.len()
    );

    for (path, why) in [
        (
            "secret/",
            "the operator's out-of-repository notes; publishing is irreversible",
        ),
        (
            "tmp/",
            "scratch space, which may hold anything a session left behind",
        ),
        (
            "keleusma-lsp/",
            "a detached developer-tooling crate released separately",
        ),
        (
            "keleusma-wasm/",
            "a detached developer-tooling crate released separately",
        ),
        (
            "compiler/",
            "the self-hosted compiler subproject, not part of the consumed runtime",
        ),
        ("editors/", "editor integrations, not consumer material"),
        ("tools/", "contributor tooling, not consumer material"),
    ] {
        assert!(
            entries.iter().any(|e| e == path),
            "`{path}` is no longer excluded from the published tarball ({why}). Entries found: \
             {entries:?}"
        );
    }
}

/// The documents that present a COMPLETE list of the workspace's crates, each with the reason it
/// is in scope. **The set is named and justified here on purpose**: a named set nobody can audit
/// is how the next entry point gets added without this guard following it, which is exactly what
/// happened between `RELEASE_PROCESS.md` and these two.
const ENTRY_POINTS: &[(&str, &str)] = &[
    (
        "README.md",
        "what crates.io renders, so it is the project's most publicly visible statement of its \
         own crate set",
    ),
    (
        "AGENTS.md",
        "the model-facing entry point, named by llms.txt as the first thing to read; no test read \
         it at all before 2026-09-27",
    ),
    (
        "docs/process/RELEASE_PROCESS.md",
        "the document the first instance of this defect was found in, kept in scope so the \
         original site cannot regress",
    ),
];

/// Whether `name` occurs in `text` as a whole token rather than as the prefix of a longer name.
///
/// **`keleusma-wire` is a prefix of `keleusma-wire-derive`.** A plain containment check finds the
/// shorter name inside the longer one and reports coverage that does not exist — the same prefix
/// hazard that defeated an abandoned scan for `keleusma::selfhost`, which also matched
/// `selfhost_host`. A raw `grep -c` for `keleusma-wire` in the corrected README returns 2 for one
/// real mention, which is this hazard in miniature.
fn names_whole_token(text: &str, name: &str) -> bool {
    let mut from = 0usize;
    while let Some(rel) = text[from..].find(name) {
        let at = from + rel;
        let after = text[at + name.len()..].chars().next();
        let continues = after.is_some_and(|c| c.is_alphanumeric() || c == '-' || c == '_');
        if !continues {
            return true;
        }
        from = at + name.len();
    }
    false
}

/// **EVERY ENTRY-POINT DOCUMENT NAMES EVERY CRATE THAT PUBLISHES.**
///
/// `RELEASE_PROCESS.md` once said five crates when seven publish, and this file's opening comment
/// records that "correcting the document closed the instance. It did not close the class." It then
/// closed the class for one file. Measured 2026-09-27, `README.md` said "Five crates:" and
/// `AGENTS.md` said "Five workspace crates", both omitting `keleusma-wire` and
/// `keleusma-wire-derive` entirely — the identical omission, at two sites the guard did not reach.
///
/// The crate set is DERIVED from the manifests, so no number is written into this test. Writing
/// "seven" here would reproduce the defect one release later.
#[test]
fn every_entry_point_document_names_every_publishing_crate() {
    let mut paths = Vec::new();
    manifests(&root(), &mut paths);
    let publishing: BTreeSet<String> = paths
        .iter()
        .filter_map(|p| package_of(p))
        .filter(|(_, publishes)| *publishes)
        .map(|(name, _)| name)
        .collect();

    // NON-VACUITY on the derivation. An empty set satisfies every containment below.
    assert!(
        publishing.len() >= 5,
        "derived only {} publishing crates from the manifests, so the derivation has broken",
        publishing.len()
    );

    let mut missing = Vec::new();
    for (doc, why) in ENTRY_POINTS {
        let text = std::fs::read_to_string(root().join(doc))
            .unwrap_or_else(|e| panic!("read the entry-point document {doc}: {e}"));
        for crate_name in &publishing {
            if !names_whole_token(&text, crate_name) {
                missing.push(format!("{doc} does not name `{crate_name}` ({why})"));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "{} entry-point omission(s), the same class this file was written for:\n{}",
        missing.len(),
        missing.join("\n")
    );
}

/// **NO ENTRY-POINT DOCUMENT STATES A CRATE COUNT THAT DISAGREES WITH THE MANIFESTS.**
///
/// Separate from the naming check on purpose: a document can name every crate and still open with
/// a stale total, and conflating the two lets one failure mask the other. The expected word is
/// derived from the manifest count rather than written down.
#[test]
fn no_entry_point_document_states_a_stale_crate_count() {
    const WORDS: &[(usize, &str)] = &[
        (3, "three"),
        (4, "four"),
        (5, "five"),
        (6, "six"),
        (7, "seven"),
        (8, "eight"),
        (9, "nine"),
        (10, "ten"),
    ];

    let mut paths = Vec::new();
    manifests(&root(), &mut paths);
    let n = paths
        .iter()
        .filter_map(|p| package_of(p))
        .filter(|(_, publishes)| *publishes)
        .count();
    let expected = WORDS
        .iter()
        .find(|(k, _)| *k == n)
        .unwrap_or_else(|| panic!("no English word tabulated for {n} crates; extend the table"))
        .1;

    let mut wrong = Vec::new();
    for (doc, _) in ENTRY_POINTS {
        // LEDGER ENTRIES ARE EXCLUDED, AND THIS IS NOT A CONVENIENCE.
        //
        // The first run of this check flagged `RELEASE_PROCESS.md` for "published all four
        // crates". That is a dated lesson recording what an agent did during V0.2.2, when four
        // was the number -- history, not a live claim. `CLAUDE.md` states the rule directly:
        // history recording what was true at an increment is not stale, and rewriting it
        // corrupts the record. This repository keeps such entries in blockquotes, so the check
        // reads only unquoted lines.
        //
        // The trade is stated rather than hidden: a live claim written inside a blockquote would
        // escape this check. That is the lesser error, because the alternative pressures a future
        // reader into editing the ledger to make a test pass.
        let text: String = std::fs::read_to_string(root().join(doc))
            .unwrap_or_else(|e| panic!("read {doc}: {e}"))
            .lines()
            .filter(|l| !l.trim_start().starts_with('>'))
            .collect::<Vec<_>>()
            .join("\n")
            .to_lowercase();
        for (k, word) in WORDS {
            if *k == n {
                continue;
            }
            for phrase in [format!("{word} crates"), format!("{word} workspace crates")] {
                if text.contains(&phrase) {
                    wrong.push(format!(
                        "{doc} says \"{phrase}\" and the manifests give {n} ({expected})"
                    ));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// **NO DOCUMENT REPORTS A PUBLISHED RELEASE OLDER THAN THE MANIFEST CARRIES.**
///
/// `AGENTS.md` reported V0.2.0 as published while the workspace was at 0.2.2, two releases on. A
/// stale status line tells a model the wrong thing about what consumers can actually depend on.
#[test]
fn no_entry_point_reports_a_published_release_older_than_the_manifest() {
    let manifest = std::fs::read_to_string(root().join("Cargo.toml")).expect("the root manifest");
    let at = manifest
        .find("\nversion = \"")
        .expect("the root manifest states no version");
    let ver: String = manifest[at + 12..]
        .chars()
        .take_while(|c| *c != '"')
        .collect();
    assert!(
        ver.split('.').count() == 3,
        "parsed `{ver}` as the workspace version, which is not a triple"
    );

    let mut stale = Vec::new();
    for (doc, _) in ENTRY_POINTS {
        let text =
            std::fs::read_to_string(root().join(doc)).unwrap_or_else(|e| panic!("{doc}: {e}"));
        // Only the "published to crates.io" claim is checked. A document may legitimately discuss
        // an older release historically; claiming it is the published one is the defect.
        for line in text.lines() {
            if !line.contains("published to crates.io") {
                continue;
            }
            let claimed: Vec<&str> = line
                .split_whitespace()
                .filter(|w| w.starts_with('V') && w[1..].split('.').count() == 3)
                .collect();
            for c in claimed {
                let num = c.trim_start_matches('V').trim_end_matches('.');
                if num != ver {
                    stale.push(format!(
                        "{doc} reports {c} published while the workspace is at {ver}"
                    ));
                }
            }
        }
    }
    assert!(stale.is_empty(), "{}", stale.join("\n"));
}

/// **NO DOCUMENT CLAIMS A CRATE IS PUBLISHED WHEN ITS CHANGELOG SAYS IT IS NOT.**
///
/// **This guard exists because the increment that added the checks above got this wrong.** While
/// correcting `README.md` from "Five crates:" to name all seven, I wrote "Seven crates, all
/// published to crates.io". Seven crates are PUBLISHABLE — no manifest sets `publish = false`, and
/// `RELEASE_PROCESS.md` lists seven for a release — but `keleusma-wire` and `keleusma-wire-derive`
/// are at `0.1.0` and their own changelogs say, in as many words, "This crate has never been
/// published."
///
/// **That distinction is the original defect's own mechanism.** "Will publish at the next release"
/// and "is on crates.io today" are different claims, and collapsing them is how a document comes to
/// describe a crate set that does not exist. The earlier five-versus-seven confusion lived in the
/// same gap.
///
/// The unpublished set is DERIVED from the changelogs rather than listed here, so a crate's first
/// publication retires its entry automatically.
#[test]
fn no_entry_point_claims_an_unpublished_crate_is_already_on_crates_io() {
    let mut paths = Vec::new();
    manifests(&root(), &mut paths);

    // Crates whose own changelog states they have never been published.
    let mut unpublished: BTreeSet<String> = BTreeSet::new();
    for mani in &paths {
        let Some((name, publishes)) = package_of(mani) else {
            continue;
        };
        if !publishes {
            continue;
        }
        let changelog = mani.parent().map(|d| d.join("CHANGELOG.md"));
        let Some(cl) = changelog else { continue };
        if let Ok(text) = std::fs::read_to_string(&cl)
            && text.contains("has never been published")
        {
            unpublished.insert(name);
        }
    }

    // NON-VACUITY, in the direction that matters. If this set is empty the check below passes
    // trivially, so an empty set must be explained rather than assumed: it means every publishable
    // crate has shipped at least once, which is a real state and not a parse failure. The floor is
    // therefore on the DERIVATION reaching the changelogs at all.
    let with_changelogs = paths
        .iter()
        .filter(|m| m.parent().is_some_and(|d| d.join("CHANGELOG.md").exists()))
        .count();
    assert!(
        with_changelogs >= 4,
        "found only {with_changelogs} manifests with a sibling changelog, so the derivation has \
         broken rather than the crates having lost their changelogs"
    );

    let mut wrong = Vec::new();
    for (doc, _) in ENTRY_POINTS {
        let text =
            std::fs::read_to_string(root().join(doc)).unwrap_or_else(|e| panic!("{doc}: {e}"));
        for line in text.lines() {
            let l = line.trim_start();
            if l.starts_with('>') {
                continue; // a ledger entry, per the rule above
            }
            let lower = line.to_lowercase();
            if !(lower.contains("all published") || lower.contains("all are published")) {
                continue;
            }
            for name in &unpublished {
                wrong.push(format!(
                    "{doc} says every crate is published, but `{name}`'s changelog states it has \
                     never been. Publishable is not published."
                ));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

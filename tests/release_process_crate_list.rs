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

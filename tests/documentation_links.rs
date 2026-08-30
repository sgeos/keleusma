//! **THE DOCUMENTATION KNOWLEDGE GRAPH'S RELATIVE LINKS RESOLVE.**
//!
//! `docs/README.md` calls the documentation a knowledge graph and the strategy document treats it
//! as a first-class artifact. **Nothing checked that its edges exist.** `cargo doc -D warnings`
//! catches broken *intra-doc* links in Rust source and says nothing about markdown; the
//! continuous-integration workflow regenerates one book page and diffs it, which is a different
//! question again.
//!
//! # Measured before it was written, and the first measurement was too narrow
//!
//! A first sweep over `docs/` plus the two root documents reported **116 files, 728 links, zero
//! broken** — and a clean result is evidence about the checker's reach before it is evidence
//! about the tree. Widened to the whole working directory it reported **203 broken links**, all
//! under `tmp/research/`. Checking rather than reporting: `.gitignore` carries `tmp/*` with
//! `!tmp/.gitkeep`, so that is a deliberate scratch area and not repository content.
//!
//! Restricted to the directories that hold tracked documentation, the figure is **zero broken over
//! roughly twelve hundred links**. This test keeps that true.
//!
//! # WHY THIS IS SAFE TO PIN WHERE AN EARLIER TEST IN THIS FAMILY WAS NOT
//!
//! The recorded failure was a test that **scanned a directory while pinning its answer as a
//! constant**, which made it wrong on a branch carrying more files. This one pins no count. Its
//! expectation is a PROPERTY — every relative link resolves — which a branch adding documents can
//! only satisfy or genuinely violate. A branch that adds a broken link SHOULD fail here.
//!
//! The reach assertions below are the other half: a walk that silently stopped finding files would
//! otherwise pass while checking nothing.
//!
//! # Anchors ARE resolved, and that gap was closed rather than left named
//!
//! A first revision stripped anchor fragments and said so as a known gap. Measured, the tree
//! carries **100 anchor links and all of them resolve**, so the gap was closable rather than
//! merely reportable and `every_documentation_anchor_resolves` closes it.
//!
//! **THE ZERO IS EVIDENCE, NOT SILENCE.** The check is exact set membership against the slugs of a
//! target document's headings, so a slug rule that disagreed with the one the documents were
//! written for would produce FALSE POSITIVES rather than a quiet pass. Getting zero over a hundred
//! links is evidence the rule matches.
//!
//! # What it deliberately does not check
//!
//! External `http`/`https` targets are skipped — reaching the network in a test is
//! non-deterministic and this repository prefers determinism. A link into a NON-markdown file's
//! anchor is skipped, since only markdown headings are slugged here.

#![cfg(feature = "compile")]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Directories holding tracked documentation, plus the repository root for its own documents.
///
/// **AN EXPLICIT LIST RATHER THAN A GIT QUERY.** Shelling out to `git ls-files` would be the
/// faithful definition of "tracked", and it would make this test fail for anyone running the
/// published crate's tests from a tarball, where there is no repository. The cost of the list is
/// that a new documentation directory is not covered until it is added here.
const ROOTS: &[&str] = &[
    "docs",
    "book/src",
    "compiler",
    "examples",
    "keleusma-arena",
    "keleusma-wire",
    "keleusma-bench",
    "keleusma-cli",
    "keleusma-macros",
];

/// Directories never descended into: build output, and the ignored scratch area.
const SKIP: &[&str] = &["target", "tmp", ".git", "node_modules"];

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.filter_map(Result::ok) {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if p.is_dir() {
            if !SKIP.contains(&name.as_str()) {
                collect(&p, out);
            }
        } else if name.ends_with(".md") {
            out.push(p);
        }
    }
}

/// Every relative markdown link in the tracked documentation points at something that exists.
#[test]
fn every_relative_documentation_link_resolves() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    let mut files: Vec<PathBuf> = Vec::new();
    for r in ROOTS {
        collect(&root.join(r), &mut files);
    }
    for e in std::fs::read_dir(root).expect("read the repository root") {
        let p = e.expect("a root entry").path();
        if p.is_file() && p.extension().is_some_and(|x| x == "md") {
            files.push(p);
        }
    }
    files.sort();
    files.dedup();

    let mut checked = 0usize;
    let mut broken: BTreeSet<String> = BTreeSet::new();

    for f in &files {
        let Ok(text) = std::fs::read_to_string(f) else {
            continue;
        };
        let dir = f.parent().expect("a markdown file has a parent");
        // `[label](target)`. Found by scanning for `](` and taking to the matching `)`, which is
        // enough for the link shapes this documentation uses and does not pretend to be a
        // markdown parser.
        for (at, _) in text.match_indices("](") {
            let rest = &text[at + 2..];
            let Some(end) = rest.find(')') else { continue };
            let target = rest[..end].trim();
            if target.starts_with("http://")
                || target.starts_with("https://")
                || target.starts_with("mailto:")
                || target.starts_with('#')
                || target.is_empty()
            {
                continue;
            }
            // Anchors are stripped, not resolved. See the module note.
            let path_part = target.split('#').next().unwrap_or("").trim();
            if path_part.is_empty() {
                continue;
            }
            checked += 1;
            if !dir.join(path_part).exists() {
                broken.insert(format!(
                    "{} -> {target}",
                    f.strip_prefix(root).unwrap_or(f).display()
                ));
            }
        }
    }

    // REACH, asserted before the verdict. A walk that stopped finding files would otherwise
    // report zero broken links while checking nothing, which is the failure mode this repository
    // has recorded as "a clean guard proves its reach first".
    assert!(
        files.len() >= 100,
        "only {} documentation files were found; the walk is mis-scoped and this test would pass \
         while checking almost nothing",
        files.len()
    );
    assert!(
        checked >= 800,
        "only {checked} relative links were checked across {} files; the extractor is finding far \
         fewer links than this documentation contains",
        files.len()
    );

    assert!(
        broken.is_empty(),
        "{} relative documentation link(s) do not resolve:\n  {}",
        broken.len(),
        broken.iter().cloned().collect::<Vec<_>>().join("\n  ")
    );
}

/// The slugs a markdown document's headings expose as anchor targets.
///
/// Approximates the rule the documents were written against: lower-case, punctuation dropped,
/// whitespace to hyphens, with inline code and link syntax unwrapped first. **Fenced code blocks
/// are skipped**, because a `#` inside one is a comment rather than a heading — that omission
/// would invent anchors nothing links to and mask a real miss.
fn heading_slugs(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut in_fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let trimmed = line.trim_end();
        if !trimmed.starts_with('#') {
            continue;
        }
        let heading = trimmed.trim_start_matches('#').trim();
        if heading.is_empty() {
            continue;
        }
        // Unwrap `code`, [label](target) and emphasis, then slug.
        let mut h = String::new();
        let mut depth = 0usize;
        let mut skipping_target = false;
        for c in heading.chars() {
            match c {
                '`' | '*' | '_' => {}
                '[' => depth += 1,
                ']' => depth = depth.saturating_sub(1),
                '(' if depth == 0 && h.ends_with(|c: char| c.is_alphanumeric()) => {
                    skipping_target = true;
                }
                ')' if skipping_target => skipping_target = false,
                _ if skipping_target => {}
                _ => h.push(c),
            }
        }
        let slug: String = h
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c == '-' || *c == '_')
            .collect();
        let slug = slug.split_whitespace().collect::<Vec<_>>().join("-");
        if !slug.is_empty() {
            out.insert(slug);
        }
    }
    out
}

/// **EVERY ANCHOR FRAGMENT IN A DOCUMENTATION LINK NAMES A HEADING THAT EXISTS.**
///
/// The companion test above resolves the FILE half of a link. This one resolves the `#fragment`
/// half, which is the difference between "the document exists" and "the section you were sent to
/// exists". A link into a heading that has since been renamed lands the reader at the top of a
/// long document with no indication anything is wrong.
///
/// A duplicate heading is tolerated in its `-1`, `-2` suffixed forms, which is how the renderer
/// disambiguates them; without that a legitimate link would read as broken.
#[test]
fn every_documentation_anchor_resolves() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    let mut files: Vec<PathBuf> = Vec::new();
    for r in ROOTS {
        collect(&root.join(r), &mut files);
    }
    for e in std::fs::read_dir(root).expect("read the repository root") {
        let p = e.expect("a root entry").path();
        if p.is_file() && p.extension().is_some_and(|x| x == "md") {
            files.push(p);
        }
    }
    files.sort();
    files.dedup();

    let mut checked = 0usize;
    let mut broken: BTreeSet<String> = BTreeSet::new();

    for f in &files {
        let Ok(text) = std::fs::read_to_string(f) else {
            continue;
        };
        let dir = f.parent().expect("a markdown file has a parent");
        for (at, _) in text.match_indices("](") {
            let rest = &text[at + 2..];
            let Some(end) = rest.find(')') else { continue };
            let target = rest[..end].trim();
            if target.starts_with("http://")
                || target.starts_with("https://")
                || target.starts_with("mailto:")
            {
                continue;
            }
            let Some((path_part, fragment)) = target.split_once('#') else {
                continue;
            };
            if fragment.is_empty() {
                continue;
            }
            let doc = if path_part.is_empty() {
                f.clone()
            } else {
                dir.join(path_part)
            };
            if doc.extension().is_none_or(|x| x != "md") || !doc.exists() {
                continue;
            }
            let Ok(target_text) = std::fs::read_to_string(&doc) else {
                continue;
            };
            checked += 1;
            let slugs = heading_slugs(&target_text);
            let want = fragment.to_lowercase();
            let base = want.rsplit_once('-').map_or(want.as_str(), |(b, n)| {
                if n.chars().all(|c| c.is_ascii_digit()) {
                    b
                } else {
                    want.as_str()
                }
            });
            if !slugs.contains(&want) && !slugs.contains(base) {
                broken.insert(format!(
                    "{} -> {target}",
                    f.strip_prefix(root).unwrap_or(f).display()
                ));
            }
        }
    }

    assert!(
        checked >= 60,
        "only {checked} anchor links were checked; the extractor is finding far fewer than this \
         documentation contains and would pass while measuring almost nothing"
    );
    assert!(
        broken.is_empty(),
        "{} documentation anchor(s) name a heading that does not exist:\n  {}",
        broken.len(),
        broken.iter().cloned().collect::<Vec<_>>().join("\n  ")
    );
}

//! **THE TRANSLATED BOOK'S CATALOGUE STILL DESCRIBES THE ENGLISH SOURCE.**
//!
//! The guide ships in English and Japanese. The Japanese text lives in `book/po/ja.po`,
//! keyed by the English it translates, and `book/po/messages.pot` is the extracted
//! catalogue of translatable messages. When English prose changes and the catalogue is not
//! regenerated, the Japanese build falls back to English for the changed paragraph **and
//! says nothing**, while `ja.po` continues to report itself fully translated.
//!
//! Measured 2026-09-26 with the project's own extractor, before this file existed: the
//! committed `messages.pot` held **2782** messages against **3072** the source contains,
//! and the Japanese catalogue left **30** messages untranslated with **6** translations
//! orphaned. `ja.po`'s own header reported zero untranslated, which was true of the
//! catalogue and false of the book.
//!
//! **WHY THIS GUARD COMPARES THE TWO COMMITTED FILES TO EACH OTHER, AND NEVER TO THE
//! MARKDOWN.** `DESIGN_JOURNAL.md` records an earlier attempt to size this class by
//! matching each message against its source line. It reported 2,329 stale of 2,926, which
//! was not a finding but a wrong model of `mdbook-i18n-helpers`: the extractor takes INLINE
//! content and rewrites markdown syntax, so escaped punctuation and any chapter that
//! documents escape sequences produce false positives. That instrument was deleted rather
//! than repaired. A second hand-rolled attempt on 2026-09-26 reproduced the same class at
//! smaller scale, 34 with visible false positives, which is why this file does not try.
//!
//! Both sides here are gettext files written by the same tool, so their escaping is
//! identical by construction and a set comparison needs no normalisation at all. Whether
//! the catalogue matches the MARKDOWN is a different question, answered by regenerating it
//! with the real extractor where that tool is available.

use std::path::{Path, PathBuf};

fn po_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("book/po")
}

/// One catalogue entry: the raw msgid text as gettext wrote it, whether its translation is
/// empty, and whether the entry is commented out as obsolete.
struct Entry {
    msgid: String,
    translated: bool,
    obsolete: bool,
}

/// Concatenate the quoted string continuations that follow a `msgid`/`msgstr` keyword.
/// The bytes are kept verbatim, escapes included: both files were written by the same
/// tool, so a byte comparison is exactly the right one.
fn quoted_run(lines: &[&str], from: usize, keyword: &str) -> (String, usize) {
    let mut out = String::new();
    let mut i = from;
    let first = lines[i].trim_start().trim_start_matches("#~").trim_start();
    if let Some(rest) = first.strip_prefix(keyword) {
        out.push_str(&unquote(rest.trim()));
    }
    i += 1;
    while i < lines.len() {
        let t = lines[i].trim_start().trim_start_matches("#~").trim_start();
        if t.starts_with('"') {
            out.push_str(&unquote(t));
            i += 1;
        } else {
            break;
        }
    }
    (out, i)
}

/// Strip the surrounding quotes from one continuation line, keeping the contents verbatim.
fn unquote(s: &str) -> String {
    let t = s.trim();
    match (t.strip_prefix('"'), t.strip_suffix('"')) {
        (Some(a), _) if a.ends_with('"') => a[..a.len() - 1].to_string(),
        _ => t.trim_matches('"').to_string(),
    }
}

fn parse(path: &Path) -> Vec<Entry> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read the catalogue at {}: {e}", path.display()));
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < lines.len() {
        let raw = lines[i];
        let t = raw.trim_start();
        let obsolete = t.starts_with("#~");
        let body = t.trim_start_matches("#~").trim_start();
        if body.starts_with("msgid ") && !body.starts_with("msgid_plural") {
            let (id, next) = quoted_run(&lines, i, "msgid ");
            // The translation follows, possibly after a msgid_plural we do not model.
            let mut j = next;
            while j < lines.len() {
                let b = lines[j].trim_start().trim_start_matches("#~").trim_start();
                if b.starts_with("msgstr") {
                    break;
                }
                if b.starts_with("msgid ") || b.is_empty() {
                    break;
                }
                j += 1;
            }
            let translated = if j < lines.len() {
                let b = lines[j].trim_start().trim_start_matches("#~").trim_start();
                if b.starts_with("msgstr") {
                    let kw = if b.starts_with("msgstr[") {
                        "msgstr[0] "
                    } else {
                        "msgstr "
                    };
                    let (s, after) = quoted_run(&lines, j, kw);
                    j = after;
                    !s.is_empty()
                } else {
                    false
                }
            } else {
                false
            };
            if !id.is_empty() {
                out.push(Entry {
                    msgid: id,
                    translated,
                    obsolete,
                });
            }
            i = j.max(next);
            continue;
        }
        i += 1;
    }
    out
}

/// The ceilings. These are a RECORDED DEBT, not a target: each unit is one paragraph a
/// Japanese reader sees in English. They exist so the number cannot grow unnoticed, and
/// lowering them is the point. Raising one requires saying why in the decision document.
const MAX_UNTRANSLATED: usize = 45;
const MAX_OBSOLETE: usize = 20;

/// **THE TWO COMMITTED CATALOGUES DESCRIBE THE SAME SET OF MESSAGES.**
///
/// A message extracted from the source but absent from the translation is a paragraph that
/// renders in English with nothing recording it.
#[test]
fn the_translation_covers_every_message_the_catalogue_extracts() {
    let pot = parse(&po_dir().join("messages.pot"));
    let po = parse(&po_dir().join("ja.po"));

    // NON-VACUITY. A parser that found nothing satisfies every containment below, which is
    // the failure this repository has met in six other costumes.
    assert!(
        pot.len() >= 2000 && po.len() >= 2000,
        "the catalogue parser found {} template and {} translation entries, so it has \
         broken rather than the book having shrunk",
        pot.len(),
        po.len()
    );

    let live: std::collections::BTreeSet<&str> = po
        .iter()
        .filter(|e| !e.obsolete)
        .map(|e| e.msgid.as_str())
        .collect();
    let missing: Vec<&str> = pot
        .iter()
        .filter(|e| !e.obsolete)
        .map(|e| e.msgid.as_str())
        .filter(|id| !live.contains(id))
        .collect();
    // MUTUAL CONSISTENCY IN BOTH DIRECTIONS. The containment check alone is satisfied by a
    // template that has gone stale in the SHRINKING direction: if the extraction stopped
    // covering chapters, its messages would all still be present in a larger translation.
    // Measured 2026-09-26 on the committed pair, the template held 2782 messages and the
    // translation 3047, and 56 template messages were absent from the translation -- the two
    // files diverged in BOTH directions, each generated at a different time, and neither
    // described the other. Requiring the sizes to agree is what notices that.
    let live_pot = pot.iter().filter(|e| !e.obsolete).count();
    let live_po = po.iter().filter(|e| !e.obsolete).count();
    let gap = live_pot.abs_diff(live_po);
    assert!(
        gap <= 8,
        "the template holds {live_pot} messages and the translation {live_po}, a gap of \
         {gap}. They are generated from one source and should agree; a gap means one of them \
         was not regenerated. NEITHER this check nor the containment below can tell whether \
         the TEMPLATE still matches the markdown -- that needs the real extractor, which the \
         book workflow runs."
    );

    assert!(
        missing.len() <= 2,
        "{} extracted message(s) have no entry in the Japanese catalogue, so they render in \
         English and nothing records it. First: {:?}. Regenerate the catalogue and merge \
         the translation.",
        missing.len(),
        missing.first().map(|s| &s[..s.len().min(90)])
    );
}

/// **THE UNTRANSLATED AND ORPHANED COUNTS ARE AT OR BELOW THEIR RECORDED DEBT.**
///
/// Both numbers are invisible from inside the rendered book: an untranslated message falls
/// back to English silently, and an orphaned translation simply stops being used.
#[test]
fn the_translation_debt_has_not_grown() {
    let po = parse(&po_dir().join("ja.po"));
    let untranslated = po.iter().filter(|e| !e.obsolete && !e.translated).count();
    let obsolete = po.iter().filter(|e| e.obsolete).count();
    let live = po.iter().filter(|e| !e.obsolete).count();

    assert!(
        live >= 2000,
        "only {live} live entries parsed; the parser has broken"
    );
    assert!(
        untranslated <= MAX_UNTRANSLATED,
        "{untranslated} messages are untranslated, ceiling {MAX_UNTRANSLATED}. Each is a \
         paragraph a Japanese reader sees in English. Translate them or state why the \
         ceiling moves, in docs/decisions/BOOK_TRANSLATION_CURRENCY.md."
    );
    assert!(
        obsolete <= MAX_OBSOLETE,
        "{obsolete} translations are orphaned, ceiling {MAX_OBSOLETE}. An orphan means the \
         English it translated changed; the replacement is untranslated."
    );
}

/// **THE DEBT IS WRITTEN DOWN WHERE A READER WILL FIND IT.**
///
/// A ceiling with no explanation is a number someone will raise to make a test pass.
#[test]
fn the_translation_debt_is_documented_with_its_derivation() {
    let doc =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/decisions/BOOK_TRANSLATION_CURRENCY.md");
    let text = std::fs::read_to_string(&doc)
        .expect("the translation-currency decision document, which records the debt");
    for needle in ["msgmerge", "mdbook-xgettext", "untranslated"] {
        assert!(
            text.contains(needle),
            "the decision document does not mention {needle}, so it does not say how the \
             figures are re-derived"
        );
    }
}

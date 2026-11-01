//! **A TEST THAT SAYS IT DOES NOT RUN, AND RUNS.**
//!
//! # What happened
//!
//! `how_deep_does_the_undetected_set_go` carried a doc block reading *"⚠ OPT-IN:
//! this does NOT run in the everyday gate … measured at 710s … it is over, so it
//! stays opt-in … its last green result is a dated measurement, not a standing
//! guarantee."*
//!
//! **It carries `#[test]` and no `#[ignore]`, and had run on every gate for two
//! weeks** — since `e55f307e` restored it deliberately, after removing a
//! duplicated variant axis took it from 712s to 401s.
//!
//! **The sixth record in this package found outliving its subject, and the most
//! expensive.** The others misstated a figure or a report's status. This one
//! misdescribed the gate's cost structure to anyone deciding what to do about it:
//! read as current, it justifies deleting coverage that was restored on purpose.
//! It was found while looking for something to make cheaper.
//!
//! # What this guard checks, and what it cannot
//!
//! A claim of the form *"this does not run in the everyday gate"* is mechanically
//! checkable against the attribute that decides it. That pairing is what this
//! file asserts: **a test whose documentation claims it is opt-in must actually
//! carry `#[ignore]`.**
//!
//! It does NOT check the cost figures. A duration is a measurement, not a
//! property of the text, and this package already rejected a general prose-drift
//! census ON MEASUREMENT — the candidate matchers returned populations dominated
//! by narrative. **This guard works because its population is one pairing with a
//! mechanical answer**, not because prose drift is now solved.

use std::path::PathBuf;

/// Phrases by which a doc block claims its test is not part of the ordinary run.
///
/// Deliberately narrow. A broad matcher over this file's prose would sweep in
/// every sentence ABOUT opt-in tests, which is the failure that sank the general
/// census.
/// **Keyed on a single TOKEN, because a phrase wraps.** Matching
/// `"does NOT run in the everyday gate"` found nothing: `cargo fmt` had split it
/// across two doc lines, and only this file's non-vacuity check revealed it —
/// the third time this session a matcher silently matched nothing because of
/// formatting.
const OPT_IN_TOKEN: &str = "OPT-IN";

/// Words that must appear NEAR the token for it to be this kind of claim, joined
/// across the wrap.
const OPT_IN_CONTEXT: &str = "everyday gate";

fn test_sources() -> Vec<(String, String)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).expect("tests directory") {
        let p = e.expect("entry").path();
        if p.extension().is_some_and(|x| x == "rs")
            && let Ok(s) = std::fs::read_to_string(&p)
        {
            out.push((p.file_name().unwrap().to_string_lossy().into_owned(), s));
        }
    }
    out
}

#[test]
fn a_test_documented_as_opt_in_is_actually_ignored() {
    let sources = test_sources();
    assert!(
        sources.len() >= 100,
        "read only {} test sources; a broken probe rather than a small suite",
        sources.len()
    );

    // Non-vacuity of a different kind: the phrase this guard keys on must still
    // appear SOMEWHERE, or the guard is watching for something that no longer
    // exists in any form and would pass for that reason alone.
    let mut claim_sites = 0;
    let mut wrong = Vec::new();

    for (name, src) in &sources {
        // **The file that DEFINES the phrases is not making the claim.** Its
        // const rows contain them as data, and the next `fn` below them is a
        // helper. `comment_citations.rs` excludes its own quoted examples for the
        // same reason.
        if src.contains("const OPT_IN_TOKEN") {
            continue;
        }
        for (i, line) in src.lines().enumerate() {
            if !line.contains(OPT_IN_TOKEN) {
                continue;
            }
            // The context may wrap, so look at the token's line and the few
            // after it with their comment markers and newlines flattened.
            let window: String = src
                .lines()
                .skip(i)
                .take(4)
                .map(|l| l.trim_start().trim_start_matches("///").trim())
                .collect::<Vec<_>>()
                .join(" ");
            if !window.contains(OPT_IN_CONTEXT) {
                continue;
            }
            claim_sites += 1;
            // The claim governs the next `fn` below it. Find the attributes
            // between here and that function.
            let rest: Vec<&str> = src.lines().skip(i + 1).collect();
            let Some(fn_at) = rest.iter().position(|l| l.trim_start().starts_with("fn ")) else {
                continue;
            };
            let between = rest[..fn_at].join("\n");
            let func = rest[fn_at].trim();
            // A block that quotes the claim in order to CORRECT it is not making
            // the claim. The correction marker is what distinguishes them.
            let quoting = src
                .lines()
                .skip(i.saturating_sub(6))
                .take(12)
                .any(|l| l.contains("Corrected") || l.contains("It read"));
            // **A MENTION IS NOT AN ATTRIBUTE.** `between.contains("#[ignore")`
            // was satisfied by this very correction's prose, which names the
            // attribute while explaining that the test lacks it — so the guard
            // passed on the exact historical state it exists to catch. Caught by
            // mutation-testing against that state. An attribute is a line that
            // STARTS with it, once comment markers are excluded.
            let really_ignored = between.lines().any(|l| {
                let t = l.trim_start();
                !t.starts_with("//") && t.starts_with("#[ignore")
            });
            if !really_ignored && !quoting {
                wrong.push(format!("{name}: {func}"));
            }
        }
    }

    assert!(
        claim_sites > 0,
        "no opt-in claim found in any test source. This guard keys on that \
         phrasing; if the wording changed, re-key it rather than leaving a guard \
         that passes because it matches nothing."
    );
    assert!(
        wrong.is_empty(),
        "{} test(s) are documented as not running in the everyday gate but carry \
         no `#[ignore]`: {wrong:?}. Either the attribute or the documentation is \
         wrong. `how_deep_does_the_undetected_set_go` said this for two weeks \
         while costing 419s of every gate run, and reading it as current would \
         justify deleting coverage that was restored deliberately.",
        wrong.len()
    );
}

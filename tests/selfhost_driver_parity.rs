//! **THE SHIPPING DRIVER AND ITS TEST-FILE COPY, COMPARED BY STRUCTURE RATHER THAN BY CORPUS.**
//!
//! # Why this exists
//!
//! `src/selfhost/mod.rs` and `tests/selfhost_codegen.rs` contain two implementations of the same
//! pipeline driver. In one session, four separate silent defects were found in the shipping one,
//! every single one of them something the copy handled and it did not:
//!
//! | defect | symptom | would THIS test have caught it? |
//! |---|---|---|
//! | the constant-pool tag was discarded | a string constant became the integer of its intern id | **no** |
//! | struct/trait/impl declarations had no skip state | the driver faulted on 29 boundary cases | yes |
//! | the eager `and`/`or` ids were never seeded | `a and b` compiled to `a` | yes |
//! | op tag 53 had no flat-nested arm | a struct-typed tuple element faulted in kind decoding | yes |
//!
//! **Three of four, and saying so precisely matters more than the three.** The pool-tag defect is
//! invisible here because both files *read* the tag stream; they differed in what they did with it
//! afterwards, which is semantics inside an arm rather than the presence of an arm.
//!
//! # What this is, and the objection to it
//!
//! This is a **textual guard over source text**, which is the weakest shape of test and this
//! project has argued against it elsewhere: it cannot see behaviour, and a rename can make it
//! measure nothing. Two things make it worth having anyway.
//!
//! First, the corpus guard it complements
//! (`selfhost_codegen::the_shipping_compiler_matches_the_boundary_it_is_recorded_against`) is
//! only as good as the cases in its table; a divergence no case exercises is invisible to it.
//! This one does not depend on corpus coverage at all.
//!
//! Second, **every set it derives is asserted non-vacuous**. A regex that stops matching produces
//! an empty set, and an empty set here fails loudly rather than passing. That is the specific
//! failure mode that made an earlier guard in this tree pass while checking nothing.
//!
//! It is a smoke alarm for a structural class. It is not a substitute for deleting the duplicate.
//!
//! # THE EXTRACTION IS PARSED, NOT WINDOWED, AND THAT DISTINCTION HAS A COST RECORD
//!
//! Every derivation here matches structure — brace depth, paren matching — rather than a fixed
//! number of characters. A window is a guess about FORMATTING, and rustfmt decides formatting.
//!
//! Two failure directions, and guarding one does not guard the other:
//!
//! - **Too loose**: a marker written in a form prose can take will eventually match prose. This
//!   tree has three instances — a must-fire guard that fired on the comment explaining the fix it
//!   guarded, a no-copies guard that flagged itself, and the `v0.3.0` line's witness extractor
//!   matching its own English header.
//! - **Too tight**: a pattern narrower than the thing it describes silently omits members. The
//!   `v0.3.0` line hit this the same day — a grep for `mis-compilation` missed four sites saying
//!   `mis-compiled`, reporting a class of three where there were seven — **in the very file where
//!   they had just written down the too-loose rule.**
//!
//! The seeding extractor here was too tight: it asked whether `set_shared` appeared within sixty
//! characters before a slot name. Mutation-tested both ways. With a call reformatted past that
//! window, the old form reports the slot seeded ZERO times when it is seeded once — **the guard
//! still fails, but with the wrong number, sending its reader to hunt a deletion that never
//! happened.** Too-tight does not only cause silent passes; it also causes confidently wrong
//! failures.

#![cfg(feature = "self-host")]

use std::collections::BTreeSet;

const LIBRARY: &str = "src/selfhost/mod.rs";
const COPY: &str = "tests/selfhost_codegen.rs";

fn read(rel: &str) -> String {
    for prefix in ["", "../"] {
        let p = format!("{prefix}{rel}");
        if let Ok(s) = std::fs::read_to_string(&p) {
            return s;
        }
    }
    panic!("cannot read {rel} from {:?}", std::env::current_dir());
}

/// The body of the first `match` on `name` whose arms include `must_contain`, brace-matched.
///
/// Brace-matched rather than taken as a fixed window: an earlier version of this extraction used
/// a byte window and silently ran off the end of the block into an unrelated `match`, reporting
/// arms that were not in the dispatch at all. A window is a guess about comment length.
fn match_body(src: &str, header: &str, must_contain: &str) -> String {
    let mut from = 0;
    while let Some(rel) = src[from..].find(header) {
        let start = from + rel + header.len() - 1; // at the '{'
        let mut depth = 0usize;
        let bytes = src.as_bytes();
        let mut i = start;
        while i < bytes.len() {
            match bytes[i] {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        let body = &src[start..i];
        if body.contains(must_contain) {
            return body.to_string();
        }
        from = start + 1;
    }
    panic!("no `{header}` block containing {must_contain:?}");
}

/// Match-arm patterns at the TOP LEVEL of `body`: `<digits> =>`, `<digits> if <guard> =>`,
/// `a..=b =>`.
///
/// # Depth matters, and ignoring it produced a false positive on this test's first run
///
/// The copy inlines its scalar-kind and composite-variant decoding as nested `match` expressions
/// where the library factors them into `scalar_kind_from_tag` / `composite_kind_from_tag`. A
/// depth-blind scan therefore collected the nested `0 => ScalarKind::Unit` arms from the copy and
/// nothing corresponding from the library, and reported a divergence that did not exist.
///
/// **The instrument was wrong, not the code.** Recorded here rather than silently corrected,
/// because a guard that fires on its own extraction error is the cheap version of the failure it
/// exists to prevent, and the next person to widen this should know depth is load-bearing.
fn arm_patterns(body: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut depth = 0i32;
    for line in body.lines() {
        let t = line.trim_start();
        // Depth BEFORE this line decides whether its arm is top-level; the opening brace of the
        // match body itself is depth 0, so its arms sit at depth 1.
        let at_top = depth == 1;
        depth += line.matches('{').count() as i32 - line.matches('}').count() as i32;
        if !at_top {
            continue;
        }
        let Some(arrow) = t.find("=>") else { continue };
        let pat = t[..arrow].trim();
        if pat.is_empty() {
            continue;
        }
        // Keep the guard's PRESENCE: an arm losing its guard is exactly the tuple-field defect,
        // and a comparison blind to guards would call that agreement.
        let (disc, guarded) = match pat.find(" if ") {
            Some(k) => (&pat[..k], true),
            None => (pat, false),
        };
        let disc = disc.trim();
        let numericish = disc
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == '=')
            && disc.chars().any(|c| c.is_ascii_digit());
        if numericish {
            out.insert(if guarded {
                format!("{disc}(guarded)")
            } else {
                disc.to_string()
            });
        }
    }
    out
}

/// **THE OP-WORD DECODERS MUST RECOGNISE THE SAME TAGS, WITH THE SAME GUARDS.**
///
/// Op tag 53 carries two forms distinguished only by operand magnitude, and the shipping driver
/// decoded one of them. The guard's PRESENCE is part of the comparison for that reason: an arm
/// set that matched while one side had lost its guard would report agreement on the exact defect
/// this pins.
#[test]
fn both_drivers_decode_the_same_op_tags() {
    // The `match tag {` BLOCK, not the enclosing function body: `match_body` anchors on the
    // header's final brace, so a header without one hands back a body one level too shallow and
    // the depth filter then finds nothing. The two callers in this file must agree about that,
    // and the non-vacuity assertion below is what caught it when they did not.
    //
    // Disambiguated by `52 =>`, because `src/selfhost/mod.rs` has a second `match tag` — the
    // constant-pool tag mapping — whose arms are 0, 1 and 2.
    let lib = arm_patterns(&match_body(&read(LIBRARY), "match tag {", "        52 =>"));
    let cop = arm_patterns(&match_body(&read(COPY), "match tag {", "        52 =>"));

    assert!(
        lib.len() >= 55 && cop.len() >= 55,
        "op-tag extraction produced {} and {} arms; the regex has stopped matching and this \
         test is measuring nothing",
        lib.len(),
        cop.len()
    );
    assert_eq!(
        lib, cop,
        "the two drivers no longer decode the same op tags. An arm present in the copy and \
         absent from the library is a construct the shipping compiler cannot lower; note that a \
         `(guarded)` suffix marks an arm with a match guard, and losing one is how a struct-typed \
         tuple element came to be decoded as a scalar kind"
    );
}

/// **THE SHIPPING DRIVER MUST SEED EVERY SLOT THE COPY DOES, AT EVERY FEED.**
///
/// `parse.kel` guards several recognitions on a host-supplied interned id being greater than
/// zero, so an unseeded slot does not fail — it silently selects older behaviour. That is how the
/// eager `and`/`or` operators came to be dropped along with their right operands, and how `true`
/// and `false` were resolved as variable references before that.
///
/// # Counted, not merely present, and the first version of this test was wrong about that
///
/// The library has TWO token feeds — collecting and windowed — and seeds each slot once per feed.
/// An earlier version compared SETS of names, and a mutation removing one of the two seedings
/// passed it: the name was still present via the other feed. **A slot seeded on one path and not
/// the other is exactly the defect class this file exists for**, and set comparison is blind to
/// it.
///
/// The threshold is calibrated against `BR_P_WORD_ID` rather than written as a literal, so adding
/// a third feed does not silently weaken the test. That slot is the reference because it has been
/// seeded at every feed since before any of this, so its count IS the feed count.
///
/// **A superset is permitted deliberately.** The library legitimately seeds slots the copy does
/// not; the failure direction that matters is the library seeding fewer, or on fewer paths.
#[test]
fn the_shipping_driver_seeds_every_slot_the_copy_does_at_every_feed() {
    /// Slot names being SET, counted by parsing each `set_shared` call's ARGUMENT LIST.
    ///
    /// # Why not a fixed lookback, which is what this was
    ///
    /// The first version asked whether `set_shared` appeared within sixty characters before the
    /// name. That is a guess about FORMATTING, and rustfmt decides formatting. A call broken
    /// across more lines than the window spans would be missed, the slot would count as unseeded,
    /// and **the guard would pass while the defect it exists for was present** — the too-tight
    /// direction of the same failure that makes a marker match prose.
    ///
    /// The `v0.3.0` line hit the too-tight form the same day: a grep for `mis-compilation` missed
    /// four sites saying `mis-compiled`, reporting a class of three where there were seven. **They
    /// had guarded the too-loose direction in that very file and never considered the other one.**
    ///
    /// Paren-matching the argument list has no window to outgrow.
    fn seed_counts(s: &str) -> std::collections::BTreeMap<String, usize> {
        let mut out = std::collections::BTreeMap::new();
        let bytes = s.as_bytes();
        let mut from = 0;
        while let Some(rel) = s[from..].find("set_shared") {
            let call = from + rel;
            // Advance to the opening paren, then match to its close. A call that never opens one
            // is not a call, and skipping it is correct rather than a silent loss.
            let Some(open_rel) = s[call..].find('(') else {
                break;
            };
            let open = call + open_rel;
            let (mut depth, mut i) = (0usize, open);
            while i < bytes.len() {
                match bytes[i] {
                    b'(' => depth += 1,
                    b')' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            let args = &s[open..i.min(bytes.len())];
            let mut at = 0;
            while let Some(k) = args[at..].find("BR_P_") {
                let b = at + k;
                let e = args[b..]
                    .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                    .map_or(args.len(), |q| b + q);
                *out.entry(args[b..e].to_string()).or_insert(0) += 1;
                at = e;
            }
            from = i.max(call + 1);
        }
        out
    }
    let lib = seed_counts(&read(LIBRARY));
    let cop = seed_counts(&read(COPY));

    assert!(
        cop.len() >= 10 && lib.len() >= 10,
        "seed extraction found {} copy and {} library slots; it has broken and this test is \
         measuring nothing",
        cop.len(),
        lib.len()
    );
    let feeds = *lib
        .get("BR_P_WORD_ID")
        .expect("BR_P_WORD_ID is the calibration slot and is no longer seeded at all");
    assert!(
        feeds >= 2,
        "the calibration slot is seeded {feeds} time(s); with fewer than two feeds this test \
         cannot distinguish per-feed seeding from presence"
    );

    let mut short: Vec<String> = Vec::new();
    for (name, want_present) in &cop {
        let got = lib.get(name).copied().unwrap_or(0);
        if got < feeds {
            short.push(format!(
                "{name}: seeded {got} time(s) in the driver, {feeds} expected (copy seeds it \
                 {want_present})"
            ));
        }
    }
    assert!(
        short.is_empty(),
        "the shipping driver does not seed these on every token feed, while the test-file copy \
         does. `parse.kel` guards on them being > 0 and silently keeps older behaviour when they \
         are not, so the symptom is a wrong lowering on one code path rather than an error:\n  {}",
        short.join("\n  ")
    );
}

/// **BOTH DRIVERS MUST HANDLE THE SAME TOP-LEVEL DECLARATION RECORD CODES.**
///
/// A record code with no arm falls through to the function dispatch, where it arrives with no
/// declaration open. That is what a `struct` declaration did in the shipping driver, on every one
/// of the 29 boundary cases that declare one.
///
/// The dispatch is identified by the arm it must contain rather than by line or by enclosing
/// function name, because `tests/selfhost_codegen.rs` holds a SECOND record dispatch, in
/// `parse_function_records`, whose arm set legitimately differs (it handles codes 2 and 3 and not
/// 6 and 7). Matching the wrong one would report a divergence that is not one.
#[test]
fn both_drivers_handle_the_same_declaration_records() {
    let lib = arm_patterns(&match_body(&read(LIBRARY), "match code {", "18..=20"));
    let cop_src = read(COPY);
    // The comparator is the dispatch that handles the parameter-TYPE and return-TYPE records
    // (6 and 7), which is `parse_functions`; the other dispatch has no such arm.
    let cop = arm_patterns(&match_body(&cop_src, "match code {", "            7 =>"));

    assert!(
        lib.len() >= 10 && cop.len() >= 10,
        "declaration-record extraction produced {} and {} arms; it has broken",
        lib.len(),
        cop.len()
    );
    assert_eq!(
        lib, cop,
        "the two drivers no longer handle the same declaration record codes. A code the copy \
         handles and the library does not will reach the library's function dispatch with no \
         declaration open and fault there"
    );
}

/// **THE TWO FILES ARE STILL TWO FILES, AND THIS TEST IS THE NOTICE WHEN THEY ARE NOT.**
///
/// The whole premise here is a duplicate that the open accessor decision would remove. If the
/// copy is deleted, these comparisons become vacuous rather than false — they would pass by
/// having nothing to compare — so the premise itself is asserted.
///
/// Deleting this file is the correct response to that failure, not repairing it.
#[test]
fn the_duplicate_this_test_exists_for_is_still_present() {
    let cop = read(COPY);
    assert!(
        cop.contains("fn decode_op(") && cop.contains("fn parse_functions(src: &str)"),
        "`{COPY}` no longer carries its own copy of the driver. If the duplicate was removed, \
         DELETE this file: its comparisons are vacuous without a second implementation to \
         compare against"
    );
}

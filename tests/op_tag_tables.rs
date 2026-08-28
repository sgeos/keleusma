//! The op-tag tables, and what actually checks them.
//!
//! **THE FINDING THIS FILE CLOSES CAME FROM THE `v0.3.0` LINE**, which could not close it: the
//! decoder it names is private and `src/selfhost/mod.rs` is read-only there. Their words, kept
//! because the qualification is the accurate part:
//!
//! > `codegen.kel`'s 63 inline op tags and `decode_op`'s mapping are two independently
//! > hand-maintained tables of the same numbers. Their guard, `all_wire_op_tags_decode`,
//! > asserts only that `decode_op` does not panic over `1..=63`. **A transposition passes it.**
//! > No disagreement has been observed; the claim is about what is CHECKED, not what is wrong.
//!
//! **THERE ARE THREE TABLES, NOT TWO.** Besides the stage's emitter and the shipping driver's
//! decoder there is a third in `tests/selfhost_codegen.rs`, which the driver's own comment calls
//! the source it was "ported verbatim" from and is "kept in lockstep with". **Nothing checked the
//! lockstep**, and that is the `five defects, one cause` shape exactly — the driver and its
//! test-file copy drifting — with the copy being the one the differential oracle runs.
//!
//! # What the byte-identity oracle already covers, so this file does not overclaim
//!
//! A transposition in the emitter ALONE, or in either decoder ALONE, changes the module bytes and
//! the corpus catches it — **for any tag the corpus exercises**. A renumbering applied
//! CONSISTENTLY across all three composes to the identity; the op word is internal to the pipeline
//! and is not a wire format, so that is **harmless and must not be reported as a defect**.
//!
//! The exposure is therefore the tags the corpus does NOT exercise, which is this project's
//! standing lesson in a new costume: anything the corpus does not contain is unverified by
//! construction. `the_stage_corpus_leaves_sixteen_op_tags_unexercised_and_names_them` measures that set.
//!
//! **PROPORTIONALITY.** `self_hosted_compile` cross-checks against the reference compiler and
//! refuses on divergence, so a table defect yields a loud error rather than a wrong module. The
//! exposure is to direct callers of the `self_host_compile*` entry points.
//!
//! # The extractor hazard, met on this tree before this file was written
//!
//! A naive line pattern over `^\s+[0-9]+ =>` inside the decoder's text reports **63 arms in
//! `src/selfhost/mod.rs` and 111 in `tests/selfhost_codegen.rs`**. The excess is arms of NESTED
//! matches — the scalar-kind and composite-kind decoders — whose `0 =>` and `1 =>` are not op
//! tags. Everything here matches by BRACE DEPTH, and
//! `the_arm_extraction_finds_the_same_shape_in_both_decoders` exists so a future extractor that
//! regresses to the naive form fails rather than silently comparing the wrong sets.

use std::collections::{BTreeMap, BTreeSet};

const CODEGEN_KEL: &str = include_str!("../src/selfhost/kel/codegen.kel");
const DRIVER: &str = include_str!("../src/selfhost/mod.rs");
const ORACLE: &str = include_str!("selfhost_codegen.rs");

/// The number of op tags the stage assigns. Not a free parameter: the driver's own
/// `all_wire_op_tags_decode` sweeps `1..=63` and its documentation says to extend the bound when
/// codegen first assigns a tag at or above 64. A change here must move that sweep too.
const ASSIGNED_TAGS: i64 = 63;

// ---------------------------------------------------------------------------
// Extraction. Brace depth throughout; see the module header for why.
// ---------------------------------------------------------------------------

/// The inside of the first balanced `{...}` at or after `from`.
fn balanced_braces(src: &str, from: usize) -> &str {
    let open = from + src[from..].find('{').expect("an opening brace");
    let bytes = src.as_bytes();
    let mut depth = 0usize;
    for (i, b) in bytes.iter().enumerate().skip(open) {
        match b {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return &src[open + 1..i];
                }
            }
            _ => {}
        }
    }
    panic!("unbalanced braces from byte {from}");
}

/// Strip `//` line comments.
///
/// **A COMMENT HAS ALREADY BROKEN AN INSTRUMENT ON THIS TREE**: a divergence detector matched a
/// commented-out `for k in 0..3` and predicted four diverging functions against an observed two.
/// The finding was right and the instrument was wrong.
fn strip_line_comments(src: &str) -> String {
    src.lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The stage's op-tag table: the fields of `const data wire` that precede the radix fields.
///
/// The block also carries encoding radices, work-item kinds and category codes, and **their values
/// COLLIDE with tag values** (`visit` is 1, as is `konst`), so they cannot be separated by value.
/// They are separated positionally, at the first non-tag field. That is not fragile in the way it
/// looks: if a non-tag field were ever inserted among the tags, the bijection assertion in
/// `the_stage_tag_table_assigns_each_number_once_and_leaves_no_gap` fires.
fn stage_tag_table() -> BTreeMap<String, i64> {
    let start = CODEGEN_KEL
        .find("const data wire {")
        .expect("the stage's wire constant block");
    let block = strip_line_comments(balanced_braces(CODEGEN_KEL, start));
    let mut out = BTreeMap::new();
    for line in block.lines() {
        let line = line.trim().trim_end_matches(',');
        let Some((name, rest)) = line.split_once(": Word = ") else {
            continue;
        };
        // The tag run ends at the first radix field; everything after is encoding constants.
        if name == "radix" {
            break;
        }
        let value: i64 = rest.trim().parse().expect("a decimal field value");
        assert!(
            out.insert(name.to_string(), value).is_none(),
            "the stage declares `{name}` twice"
        );
    }
    out
}

/// One decoder's `tag -> arm body`, by brace depth.
fn decoder_arms(src: &str) -> BTreeMap<i64, String> {
    let stripped = strip_line_comments(src);
    let sig = stripped
        .find("fn decode_op(w: i64) -> Op")
        .expect("a decode_op signature");
    let body = balanced_braces(&stripped, sig);
    let m = body.find("match tag").expect("a match on the tag");
    let arms = balanced_braces(body, m);

    let mut out = BTreeMap::new();
    let mut depth = 0i32;
    let mut current = String::new();
    let flush = |text: &str, out: &mut BTreeMap<i64, String>| {
        let Some((head, tail)) = text.split_once("=>") else {
            return;
        };
        let Ok(tag) = head.trim().parse::<i64>() else {
            return;
        };
        assert!(
            out.insert(tag, canonicalise(tail)).is_none(),
            "tag {tag} is decoded twice"
        );
    };
    for ch in arms.chars() {
        match ch {
            '{' | '(' | '[' => depth += 1,
            '}' | ')' | ']' => depth -= 1,
            _ => {}
        }
        if ch == ',' && depth == 0 {
            flush(&current, &mut out);
            current.clear();
        } else {
            current.push(ch);
        }
    }
    flush(&current, &mut out);
    out
}

/// Collapse the scalar-kind and composite-kind decoding to a single token.
///
/// **THE TWO DECODERS DIFFER HERE AND THE DIFFERENCE IS A REFACTOR, NOT A DRIFT.** The shipping
/// driver factors that decoding into `scalar_kind_from_tag` and `composite_kind_from_tag`; the
/// oracle's copy inlines the same match. Seven arms differ textually for that reason alone and
/// **all sixty-three agree once it is collapsed**, which is measured by
/// `the_two_decoders_agree_on_every_op_tag` rather than assumed from the "ported verbatim" comment
/// the driver carries.
fn canonicalise(arm: &str) -> String {
    let s: String = arm.chars().filter(|c| !c.is_whitespace()).collect();
    let mut out = String::new();
    let mut i = 0usize;
    while i < s.len() {
        let rest = &s[i..];
        let opener = if rest.starts_with("match") {
            Some('{')
        } else if rest.starts_with("scalar_kind_from_tag(")
            || rest.starts_with("composite_kind_from_tag(")
        {
            Some('(')
        } else {
            None
        };
        match opener {
            Some(open_ch) => {
                let close_ch = if open_ch == '{' { '}' } else { ')' };
                let open = i + rest.find(open_ch).expect("the construct opens a bracket");
                let mut depth = 0i32;
                let mut end = None;
                for (j, c) in s.char_indices().skip_while(|(j, _)| *j < open) {
                    if c == open_ch {
                        depth += 1;
                    } else if c == close_ch {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(j);
                            break;
                        }
                    }
                }
                out.push_str("KIND");
                i = end.expect("a balanced kind decoder") + 1;
            }
            None => {
                let ch = rest.chars().next().expect("a character");
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    out
}

/// The `Op` variant an arm produces.
fn arm_variant(arm: &str) -> String {
    let rest = arm.strip_prefix("Op::").unwrap_or_else(|| {
        panic!("an arm that does not produce an Op: {arm}");
    });
    rest.chars().take_while(|c| c.is_alphanumeric()).collect()
}

// ---------------------------------------------------------------------------
// The name correspondence.
//
// **WRITTEN FROM THE NAMES, NEVER FROM THE NUMBERS.** This is a FOURTH hand-maintained table and
// that is the hazard: a check built from the same model as the thing it checks confirms the model,
// recorded six times on this line in one session. It earns its place only because it is a
// DIFFERENT KIND of derivation — names to names, where the others are numbers to numbers. So a
// one-sided transposition breaks it (the number moves, the name does not) while a consistent
// renumbering does not. Had it been produced by copying the numbers across it would check nothing.
//
// Eleven names are deliberate ALIASES: the stage gives the match-control forms their own tags so
// `emit_op` can backpatch them, and they decode to the same reference ops as the structured forms.
// ---------------------------------------------------------------------------
const NAME_CORRESPONDENCE: &[(&str, &str)] = &[
    ("konst", "Const"),
    ("ret", "Return"),
    ("getlocal", "GetLocal"),
    ("checkedmul", "CheckedMul"),
    ("checkedadd", "CheckedAdd"),
    ("popn", "PopN"),
    ("setlocal", "SetLocal"),
    ("checkedsub", "CheckedSub"),
    ("divop", "Div"),
    ("modop", "Mod"),
    ("cmpeq", "CmpEq"),
    ("cmpne", "CmpNe"),
    ("cmplt", "CmpLt"),
    ("cmpgt", "CmpGt"),
    ("cmple", "CmpLe"),
    ("cmpge", "CmpGe"),
    ("iff", "If"),
    ("els", "Else"),
    ("endif", "EndIf"),
    ("lnot", "Not"),
    ("call", "Call"),
    ("dup", "Dup"),
    ("checkedneg", "CheckedNeg"),
    ("bitand", "BitAnd"),
    ("bitor", "BitOr"),
    ("bitxor", "BitXor"),
    ("getdata", "GetData"),
    ("setdata", "SetData"),
    ("getdataix", "GetDataIndexed"),
    ("setdataix", "SetDataIndexed"),
    ("loopop", "Loop"),
    ("breakif", "BreakIf"),
    ("endloop", "EndLoop"),
    ("pushimm", "PushImmediate"),
    ("mbreak", "Break"),
    ("trap", "Trap"),
    ("mif", "If"),
    ("mendif", "EndIf"),
    ("mloop", "Loop"),
    ("mendloop", "EndLoop"),
    ("mbreakif", "BreakIf"),
    ("yieldop", "Yield"),
    ("stream", "Stream"),
    ("reset", "Reset"),
    ("bytetoword", "ByteToWord"),
    ("newcomposite", "NewComposite"),
    ("getfield", "GetField"),
    ("getfieldnested", "GetField"),
    ("getindex", "GetIndex"),
    ("newcompositearray", "NewComposite"),
    ("newcompositeenum", "NewComposite"),
    ("newcompositetuple", "NewComposite"),
    ("gettuplefield", "GetTupleField"),
    ("isenum", "IsEnum"),
    ("getenumfield", "GetEnumField"),
    ("getindexnested", "GetIndex"),
    ("getenumfieldnested", "GetEnumField"),
    ("wordtobyte", "WordToByte"),
    ("addop", "Add"),
    ("subop", "Sub"),
    ("mulop", "Mul"),
    ("shl", "Shl"),
    ("shr", "Shr"),
];

// ---------------------------------------------------------------------------
// The guards.
// ---------------------------------------------------------------------------

/// The stage assigns each tag exactly once, over a contiguous run from one.
///
/// A duplicate or a gap fires. **A pure SWAP of two names' numbers does not** — the set is still a
/// bijection — which is why `every_stage_tag_name_matches_the_operation_its_number_decodes_to`
/// exists alongside this.
#[test]
fn the_stage_tag_table_assigns_each_number_once_and_leaves_no_gap() {
    let table = stage_tag_table();
    assert!(
        table.len() >= 60,
        "non-vacuity: the extraction found only {} tags, so the guard would pass by \
         checking almost nothing. Two derivations on this tree fired on their first run for \
         exactly this reason.",
        table.len()
    );
    let values: BTreeSet<i64> = table.values().copied().collect();
    assert_eq!(
        values.len(),
        table.len(),
        "the stage assigns some tag number to two names"
    );
    let expected: BTreeSet<i64> = (1..=table.len() as i64).collect();
    assert_eq!(
        values,
        expected,
        "the assigned tags are not the contiguous run 1..={}",
        table.len()
    );
    assert_eq!(
        table.len() as i64,
        ASSIGNED_TAGS,
        "the number of assigned op tags moved. The driver's `all_wire_op_tags_decode` sweeps \
         1..=63 and its own documentation says to extend that bound when codegen first assigns \
         a tag at or above 64; move both together."
    );
}

/// The extraction sees the same shape in both decoders.
///
/// **THIS IS THE GUARD ON THE INSTRUMENT, NOT ON THE TABLES.** A naive line-pattern extractor
/// reports 63 arms for the driver and about 111 for the oracle's copy, because the latter inlines
/// nested matches whose arms look identical to op-tag arms. If a future revision regresses to that
/// form, this fails instead of quietly comparing two different populations.
#[test]
fn the_arm_extraction_finds_the_same_shape_in_both_decoders() {
    let driver = decoder_arms(DRIVER);
    let oracle = decoder_arms(ORACLE);
    assert_eq!(
        driver.len(),
        ASSIGNED_TAGS as usize,
        "the driver's decoder yielded {} arms, not {ASSIGNED_TAGS}. A count far above this is \
         the extractor picking up nested match arms.",
        driver.len()
    );
    assert_eq!(
        oracle.len(),
        ASSIGNED_TAGS as usize,
        "the oracle's decoder yielded {} arms, not {ASSIGNED_TAGS}. A count near 111 is the \
         extractor picking up the inlined scalar-kind and composite-kind matches.",
        oracle.len()
    );
    let tags: BTreeSet<i64> = (1..=ASSIGNED_TAGS).collect();
    assert_eq!(driver.keys().copied().collect::<BTreeSet<_>>(), tags);
    assert_eq!(oracle.keys().copied().collect::<BTreeSet<_>>(), tags);
}

/// The shipping driver's decoder and the oracle's copy agree on every tag.
///
/// The driver's comment claims the copy is "ported verbatim" and "kept in lockstep". **Nothing
/// checked that**, and the two files are the pair whose divergence produced five defects from one
/// cause on this tree. This is now a measurement.
#[test]
fn the_two_decoders_agree_on_every_op_tag() {
    let driver = decoder_arms(DRIVER);
    let oracle = decoder_arms(ORACLE);
    let mut disagreements = Vec::new();
    for (tag, d) in &driver {
        let o = oracle.get(tag).expect("both decoders carry every tag");
        if d != o {
            disagreements.push(format!("tag {tag}:\n  driver {d}\n  oracle {o}"));
        }
    }
    assert!(
        disagreements.is_empty(),
        "the shipping decoder and the oracle's copy have drifted:\n{}",
        disagreements.join("\n")
    );
}

/// Each stage tag NAME decodes to the operation its name says it does.
///
/// **THIS IS THE ONE THAT CATCHES A ONE-SIDED TRANSPOSITION.** Swapping two names' numbers in the
/// stage source alone leaves the table a bijection and leaves the two decoders agreeing with each
/// other, so neither of the other guards can see it. Here the number moves and the name does not.
///
/// A renumbering applied consistently to the emitter and both decoders still passes, and that is
/// correct: the op word is internal to the pipeline, so a consistent renumbering is harmless.
#[test]
fn every_stage_tag_name_matches_the_operation_its_number_decodes_to() {
    let table = stage_tag_table();
    let arms = decoder_arms(DRIVER);
    let expected: BTreeMap<&str, &str> = NAME_CORRESPONDENCE.iter().copied().collect();
    let mut wrong = Vec::new();
    for (name, tag) in &table {
        let want = expected
            .get(name.as_str())
            .unwrap_or_else(|| panic!("the correspondence does not name the stage tag `{name}`"));
        let arm = arms
            .get(tag)
            .unwrap_or_else(|| panic!("no decoder arm for tag {tag} (`{name}`)"));
        let got = arm_variant(arm);
        if got != *want {
            wrong.push(format!(
                "`{name}` is tag {tag}, which decodes to Op::{got}, not Op::{want}"
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "a stage tag name disagrees with the operation its number decodes to:\n{}",
        wrong.join("\n")
    );
}

/// The correspondence covers every assigned tag and names nothing that does not exist.
///
/// Totality in both directions, so the guard above cannot pass by checking a subset.
#[test]
fn the_name_correspondence_is_total_over_the_stage_table() {
    let table = stage_tag_table();
    let named: BTreeSet<&str> = NAME_CORRESPONDENCE.iter().map(|(n, _)| *n).collect();
    assert_eq!(
        named.len(),
        NAME_CORRESPONDENCE.len(),
        "the correspondence names some tag twice"
    );
    let declared: BTreeSet<&str> = table.keys().map(|s| s.as_str()).collect();
    assert_eq!(
        named, declared,
        "the correspondence and the stage's tag table describe different sets of names"
    );
}

// ---------------------------------------------------------------------------
// The census.
//
// Gated on `compile`, which is a DEFAULT feature, so it runs under every feature set continuous
// integration uses except `--no-default-features`. The guards above read source text only and are
// deliberately NOT gated: gating a text guard behind an off-by-default feature hides it from every
// set that lacks the feature, which is how four continuous-integration jobs went red last session.
// ---------------------------------------------------------------------------

/// The twelve stage sources. `verify_types.kel` is the one with no byte-identity test.
const STAGE_SOURCES: &[(&str, &str)] = &[
    ("analyze", include_str!("../src/selfhost/kel/analyze.kel")),
    ("codegen", include_str!("../src/selfhost/kel/codegen.kel")),
    ("lexer", include_str!("../src/selfhost/kel/lexer.kel")),
    ("parse", include_str!("../src/selfhost/kel/parse.kel")),
    (
        "reconstruct",
        include_str!("../src/selfhost/kel/reconstruct.kel"),
    ),
    (
        "verify_datalayout",
        include_str!("../src/selfhost/kel/verify_datalayout.kel"),
    ),
    (
        "verify_depth",
        include_str!("../src/selfhost/kel/verify_depth.kel"),
    ),
    (
        "verify_structural",
        include_str!("../src/selfhost/kel/verify_structural.kel"),
    ),
    (
        "verify_typed",
        include_str!("../src/selfhost/kel/verify_typed.kel"),
    ),
    (
        "verify_types",
        include_str!("../src/selfhost/kel/verify_types.kel"),
    ),
    (
        "verify_yield",
        include_str!("../src/selfhost/kel/verify_yield.kel"),
    ),
    ("wire", include_str!("../src/selfhost/kel/wire.kel")),
];

/// The stages the byte-identity oracle actually covers, derived from the oracle's own test names.
fn corpus_stages() -> Vec<&'static str> {
    STAGE_SOURCES
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| {
            ORACLE.contains(&format!(
                "fn self_host_compiles_{name}_kel_byte_identically"
            ))
        })
        .collect()
}

/// **THE STAGE CORPUS CANNOT SEE A TRANSPOSITION AMONG SIXTEEN OF THE SIXTY-THREE TAGS.**
///
/// The byte-identity oracle compiles eleven stage sources and compares bytes, so it checks a tag
/// only if some stage emits it. Sixteen tags decode to operations that appear **nowhere** in any
/// of the eleven — the entire composite family, the unchecked arithmetic, and `CheckedNeg`. For
/// those, a transposition in the emitter produces no byte difference to detect, and this is the
/// project's standing lesson in a new costume: **anything the corpus does not contain is
/// unverified by construction.**
///
/// **THE SCOPE IS THE STAGE CORPUS, AND SAYING SO MATTERS.** The per-construct byte-identity
/// tests in `tests/selfhost_codegen.rs` compile struct constructions, array indexing, enum
/// payloads and tuple fields through the self-hosted compiler, so the composite family is not
/// unchecked in general. That is a **different population**, this test does not measure it, and
/// reading this as "sixteen tags are unchecked" would overstate it.
///
/// The direction of the assertion is the useful one: a tag LEAVING this list means the stage
/// corpus grew to cover it, which is a gain and should be recorded rather than absorbed silently.
#[test]
#[cfg(feature = "compile")]
fn the_stage_corpus_leaves_sixteen_op_tags_unexercised_and_names_them() {
    use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

    let stages = corpus_stages();
    assert_eq!(
        stages.len(),
        11,
        "the byte-identity corpus size moved; it is pinned at eleven stages elsewhere too"
    );

    let mut present = BTreeSet::new();
    for (name, src) in STAGE_SOURCES {
        if !stages.contains(name) {
            continue;
        }
        let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse"))
            .unwrap_or_else(|e| panic!("the reference must compile the stage `{name}`: {e:?}"));
        assert!(
            !module.chunks.is_empty(),
            "non-vacuity: stage `{name}` produced no chunks"
        );
        for chunk in &module.chunks {
            for op in &chunk.ops {
                present.insert(
                    format!("{op:?}")
                        .chars()
                        .take_while(|c| c.is_alphanumeric())
                        .collect::<String>(),
                );
            }
        }
    }
    assert!(
        present.len() > 10,
        "non-vacuity: the corpus exercised only {} distinct operations",
        present.len()
    );

    let table = stage_tag_table();
    let arms = decoder_arms(DRIVER);
    let unexercised: BTreeSet<&str> = table
        .iter()
        .filter(|(_, tag)| {
            !present.contains(&arm_variant(arms.get(tag).expect("an arm for every tag")))
        })
        .map(|(name, _)| name.as_str())
        .collect();

    let expected: BTreeSet<&str> = EXPECTED_UNEXERCISED.iter().copied().collect();
    assert_eq!(
        unexercised, expected,
        "the set of op tags the stage corpus cannot check has moved. A tag LEAVING it is a gain \
         and should be recorded; a tag JOINING it means the corpus lost coverage."
    );
    assert!(
        !unexercised.is_empty(),
        "non-vacuity: if this set were empty the measurement would be asserting nothing"
    );
}

/// The tags no stage source exercises, measured 2026-08-28 over the eleven-stage corpus.
///
/// Not a design choice — a measurement, and the reason it is written out rather than counted is
/// that the NAMES are the deliverable. They are where a transposition would hide from the oracle.
const EXPECTED_UNEXERCISED: &[&str] = &[
    "addop",
    "checkedneg",
    "getenumfield",
    "getenumfieldnested",
    "getfield",
    "getfieldnested",
    "getindex",
    "getindexnested",
    "gettuplefield",
    "isenum",
    "mulop",
    "newcomposite",
    "newcompositearray",
    "newcompositeenum",
    "newcompositetuple",
    "subop",
];

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
///
/// **A SECOND POPULATION NOW ANSWERS PART OF THAT CAVEAT.**
/// `the_shipped_examples_narrow_the_unexercised_tags_and_the_residue_is_named` measures the
/// shipped example corpus and finds it covers TWELVE of these sixteen — the whole composite
/// family. Four remain unreached by either corpus. The per-construct tests are still a third
/// population and still unmeasured, and saying so remains accurate.
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

/// **A SECOND POPULATION, BECAUSE THE FIRST CENSUS NAMED A LIMIT IT DID NOT MEASURE.**
///
/// `the_stage_corpus_leaves_sixteen_op_tags_unexercised_and_names_them` reports which tags the
/// eleven-stage byte-identity corpus cannot check, and says in its own words that the
/// per-construct tests are "a different population, this test does not measure it". **That
/// caveat was honest and it left the interesting question open.**
///
/// This measures the SHIPPED EXAMPLE corpus — the programs a user actually reads — and reports
/// which tags neither corpus exercises. It is still not the per-construct population, and this
/// does not claim to be: it is a second real corpus, and naming what BOTH of them miss is worth
/// more than naming what one misses.
///
/// # THE POPULATION IS NAMED, NOT DISCOVERED, AND THAT IS A CORRECTION
///
/// A first revision scanned `examples/scripts` for every `*.kel`. **That made the expectation
/// branch-dependent.** The `v0.3.0` line carries six further witness programs, one of which does
/// `Byte` arithmetic in `byte_mix`, so on that branch the residue is `checkedneg` ALONE and the
/// pinned set was wrong — wrong in the direction this test's own message calls "a coverage gain".
/// Its `-prod` lowers to `Op::Neg`, which is not one of the sixty-three stage tags, which is why
/// `checkedneg` survives there.
///
/// **Reported by the `v0.3.0` line, who declined to edit it.** They hold `src/` and `tests/`
/// byte-identical to `v0.2.3` as an invariant and said that editing another line's test would
/// destroy the property making their ownership checks meaningful. That was the right call, and
/// the defect is mine: **the test's INPUT lived outside the region its expectation was pinned
/// in.** A directory scan is not a corpus; it is whatever the branch happens to contain.
///
/// The population is now the fifteen programs in `CENSUS_EXAMPLES`, which exist on both lines, so
/// the exact-set assertion keeps BOTH directions — a tag leaving means a gain, a tag joining
/// means a corpus lost reach — without depending on which branch runs it. Verified by adding the
/// six `v0.3.0` witnesses to this tree and re-running: it passes with twenty-one examples present
/// exactly as with fifteen.
///
/// **What that trade gives up, said plainly:** a NEW shipped example is no longer folded into the
/// census automatically, so a program that would narrow the residue does not narrow this number
/// until someone names it. That is the price of the expectation and its input living in the same
/// file, and it is the right way round — a census whose population moves under it is not a census.
///
/// # The number is derived and the residue is named
///
/// A count alone would say nothing useful. What matters is WHICH tags no realistic program in
/// either corpus reaches, because those are the ones where a transposition produces no byte
/// difference for any oracle this project runs.
#[test]
#[cfg(feature = "compile")]
fn the_shipped_examples_narrow_the_unexercised_tags_and_the_residue_is_named() {
    use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

    let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/scripts"));
    let table = stage_tag_table();
    let arms = decoder_arms(DRIVER);

    // The operations a set of example files exercises. A refusal is REPORTED rather than skipped:
    // silently dropping an input that would not compile is how a census comes to describe a
    // smaller population than it claims.
    let ops_present = |files: &[String]| -> BTreeSet<String> {
        let mut present = BTreeSet::new();
        let mut refused: Vec<String> = Vec::new();
        for name in files {
            let src = std::fs::read_to_string(dir.join(name))
                .unwrap_or_else(|e| panic!("read the shipped example `{name}`: {e}"));
            match tokenize(&src)
                .ok()
                .and_then(|t| parse(&t).ok())
                .and_then(|a| compile(&a).ok())
            {
                Some(module) => {
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
                None => refused.push(name.clone()),
            }
        }
        assert!(
            refused.is_empty(),
            "the reference compiler refused shipped examples, so this census covers fewer \
             programs than it names: {refused:?}. Either the examples are broken or the feature \
             set this test runs under cannot compile them; both need saying rather than skipping."
        );
        present
    };

    let residue = |present: &BTreeSet<String>| -> BTreeSet<&'static str> {
        EXPECTED_UNEXERCISED
            .iter()
            .copied()
            .filter(|name| {
                let tag = table
                    .get(*name)
                    .expect("every expected name is a stage tag");
                let variant = arm_variant(arms.get(tag).expect("an arm for every tag"));
                !present.contains(&variant)
            })
            .collect()
    };

    // EXISTENCE AND NON-VACUITY. Every named program must be present; a rename or a deletion
    // fails loudly rather than quietly shrinking the population the conclusion describes.
    let missing: Vec<&&str> = CENSUS_EXAMPLES
        .iter()
        .filter(|n| !dir.join(n).exists())
        .collect();
    assert!(
        missing.is_empty(),
        "shipped examples this census names are absent: {missing:?}"
    );
    assert!(
        CENSUS_EXAMPLES.len() >= 12,
        "only {} examples are named, so the census measures almost nothing",
        CENSUS_EXAMPLES.len()
    );

    let named: Vec<String> = CENSUS_EXAMPLES.iter().map(|s| (*s).to_string()).collect();
    let still_unexercised = residue(&ops_present(&named));
    assert_eq!(
        still_unexercised,
        SHIPPED_EXAMPLES_ALSO_MISS.iter().copied().collect(),
        "the set of op tags that NEITHER the stage corpus NOR the NAMED shipped examples \
         exercise has moved. Fewer is a coverage gain and worth recording; more means a corpus \
         lost reach."
    );

    // The claim only means something if the second corpus actually covered some of them.
    assert!(
        still_unexercised.len() < EXPECTED_UNEXERCISED.len(),
        "the named examples narrowed nothing, so this second population adds no information \
         and the test is asserting the first census twice"
    );

    // NO SECOND PASS OVER THE DIRECTORY, AND THE REASON IS THAT IT COULD NOT FIRE.
    //
    // A first attempt at this fix added a scan of whatever `.kel` files the directory holds and
    // asserted their residue was a SUBSET of the named one, as a net that would survive a branch
    // carrying extra examples. **It is unreachable.** Once the existence check above passes, the
    // directory is a SUPERSET of the named files, so it exercises at least as many operations and
    // its residue is a subset by construction. The assertion could never fail.
    //
    // The case it was meant to catch -- a named example modified to cover less -- is already
    // caught, because the exact-set assertion above reads those same files.
    //
    // Recorded rather than silently dropped: a guard that cannot fire is the failure this file's
    // own header describes, and writing one while fixing a different defect in the same test is
    // exactly how they get in.
}

/// The shipped example programs this census covers, NAMED rather than discovered.
///
/// **A directory scan made the expectation branch-dependent**, which the `v0.3.0` line found and
/// reported without editing this file. They carry six further witness programs, one of which does
/// `Byte` arithmetic, so on that branch the residue is `checkedneg` alone and the pinned set below
/// was wrong — wrong in the direction this test's own message calls "a coverage gain".
///
/// **The defect was mine and its shape is worth keeping**: the test's INPUT lived outside the
/// region its expectation was pinned in. A directory scan is not a corpus; it is whatever the
/// branch happens to contain.
const CENSUS_EXAMPLES: &[&str] = &[
    "01_arithmetic.kel",
    "02_struct_field.kel",
    "03_enum_match.kel",
    "04_for_in.kel",
    "05_pipeline.kel",
    "06_multiheaded.kel",
    "07_refinement.kel",
    "08_method_dispatch.kel",
    "09_big_numbers.kel",
    "10_multbyte.kel",
    "11_signed.kel",
    "12_sensor_window.kel",
    "13_telemetry_stream.kel",
    "14_frame_log.kel",
    "15_pixel_blend.kel",
];

/// Of the sixteen tags no stage source reaches, the four the shipped examples miss too.
///
/// Measured 2026-08-28. **The shipped examples cover twelve of the sixteen** — the entire
/// composite family, which the stage corpus never touches because the stages are written in a
/// restricted subset that constructs no struct, tuple or enum value. That is a substantially
/// better position than the first census alone suggested, and it is why a second population was
/// worth measuring rather than assuming.
///
/// **What remains is one coherent group and not a scattering.** `addop`, `subop` and `mulop` are
/// the UNCHECKED arithmetic — the lowering `Byte` operands take through promote-operate-truncate,
/// where `Word` arithmetic lowers to the `checked*` tags that both corpora exercise heavily. So
/// the residue is "byte arithmetic and unary negation", which reads as a corpus gap with a shape
/// rather than as noise.
///
/// **THIS CONSTANT IS ABOUT THE SHIPPED EXAMPLES AND STAYS AT FOUR. THE OVERALL RESIDUE IS
/// THREE.** A later increment measured the THIRD population this file names below and had not
/// measured — the per-construct boundary table — and it **reaches `addop`**, through
/// `scalar/byte_arith` and `scope/float_arith__GAP`, both of the shape `a + b`. See
/// `the_boundary_table_is_the_third_op_tag_population_and_leaves_the_same_four` in
/// `tests/selfhost_codegen.rs`.
///
/// So `addop` was exercised by a corpus, and what no corpus reached was `subop`, `mulop` and
/// `checkedneg`. **Both witnesses were ADDITION, which is exactly why one of the four escaped and
/// the other three did not** — and that shape said how to close them. Two byte-identical cases
/// were added to the boundary table, `scalar/byte_sub_mul` and `scalar/word_unary_neg`, and **the
/// residue over all three populations is now EMPTY.**
///
/// **THIS CONSTANT IS STILL FOUR AND MUST STAY FOUR.** It records what the SHIPPED EXAMPLES miss,
/// and they miss all four exactly as before. The closed claim is "no corpus reaches these"; this
/// one is "the shipped examples do not". Collapsing the two would make this file assert something
/// it never measured.
///
/// **The earlier wording said this was where a transposition hides from every oracle this project
/// runs. That was true of four tags when two populations had been measured and is true of three
/// now.** It is corrected rather than deleted, because the correction is the argument for
/// measuring a population instead of describing it.
///
/// That closure needed no change to any guard here, no language change and no stage change — only
/// two source snippets in a corpus that had never carried a `Byte` subtraction.
const SHIPPED_EXAMPLES_ALSO_MISS: &[&str] = &["addop", "checkedneg", "mulop", "subop"];

/// **`Op::Neg` IS OUTSIDE THE SELF-HOSTED SUBSET, WHICH IS STRONGER THAN BEING UNTAGGED.**
///
/// The `v0.3.0` line sharpened a finding this file's census produced, and the sharpening is
/// recorded here as a CHECK rather than as prose, because it is exactly the kind of claim that
/// goes stale silently.
///
/// The census reports `checkedneg` among the tags no corpus reaches. Investigating why their
/// witness program covered the other three but not that one turned up the real statement:
/// **`codegen.kel` emits `checkedneg` for source-level unary negation and the decoder has no arm
/// producing `Op::Neg` at all.** So the self-hosted compiler cannot emit `Neg` from any source,
/// and that is a property of the SUBSET rather than of any corpus.
///
/// # The three halves, because two of them alone would mislead
///
/// - The reference compiler **does** emit `Op::Neg`, for `Byte` negation. Without this the claim
///   would read as "a dead opcode", and this project has called an unwitnessed opcode unreachable
///   and been wrong before.
/// - The stage emits `checkedneg` where the reference emits `Neg` for the same construct.
/// - No decoder arm produces `Neg`, so nothing the stage emits could decode to it either.
///
/// **What this does NOT say.** It says nothing about whether a native backend can lower `Neg` —
/// the `v0.3.0` line checked their own lowering census and it is unaffected, because that census
/// counts what the backend can lower over the corpus it has. Two different populations, and an
/// earlier version of this note conflated them.
#[test]
#[cfg(feature = "compile")]
fn the_unchecked_negation_is_outside_the_self_hosted_subset() {
    use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

    const CODEGEN_SRC: &str = include_str!("../src/selfhost/kel/codegen.kel");

    // ONE: the reference emits `Neg` for Byte negation, so the opcode is live.
    let ops_of = |src: &str| -> BTreeSet<String> {
        compile(&parse(&tokenize(src).expect("lex")).expect("parse"))
            .expect("the reference must compile the witness")
            .chunks
            .iter()
            .flat_map(|c| c.ops.iter())
            .map(|op| {
                format!("{op:?}")
                    .chars()
                    .take_while(|c| c.is_alphanumeric())
                    .collect::<String>()
            })
            .collect()
    };
    let byte_neg =
        ops_of("fn f(a: Byte, b: Byte) -> Byte { let p = a * b; -p }\nfn main() -> Word { 0 }");
    assert!(
        byte_neg.contains("Neg"),
        "the reference no longer emits the unchecked Neg for byte negation, so the claim below \
         would be about a dead opcode rather than about the self-hosted subset: {byte_neg:?}"
    );

    // TWO: the stage emits `checkedneg` for source-level unary negation.
    let push_neg = CODEGEN_SRC
        .find("fn push_neg(")
        .expect("codegen.kel lowers unary negation");
    let body_end = CODEGEN_SRC[push_neg..]
        .find("\n}")
        .expect("push_neg has a body");
    let body = &CODEGEN_SRC[push_neg..push_neg + body_end];
    assert!(
        body.contains("wire.checkedneg"),
        "the stage's unary-negation lowering no longer emits the checked tag, so this note's \
         account of why `checkedneg` is the residue may be wrong: {body}"
    );

    // THREE: no decoder arm produces `Neg`, so nothing the stage emits can decode to it.
    let arms = decoder_arms(DRIVER);
    let producing_neg: Vec<i64> = arms
        .iter()
        .filter(|(_, arm)| arm_variant(arm) == "Neg")
        .map(|(tag, _)| *tag)
        .collect();
    assert!(
        producing_neg.is_empty(),
        "a decoder arm now produces Op::Neg (tags {producing_neg:?}). THIS IS THE SUBSET WIDENING: \
         the self-hosted compiler can reach an operation it could not before, which is a gain and \
         should be recorded rather than absorbed silently"
    );

    // NON-VACUITY on the extraction, since an empty arm table would satisfy the check above.
    assert_eq!(
        arms.len(),
        ASSIGNED_TAGS as usize,
        "the arm extraction returned {} arms, so the emptiness check above establishes nothing",
        arms.len()
    );
}

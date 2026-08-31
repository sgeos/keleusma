//! **WHICH KIND-DISCRIMINATED LOWERING ARMS DOES NOTHING EXERCISE?**
//!
//! # Why the unit is finer than an opcode
//!
//! The tree already accounts for opcodes: 63 of 66 lower, `Len` refused, `Reset`
//! never visited. **That granularity could not have caught the case that
//! prompted this file.** Nested float composite bodies were ACCEPTED and
//! unverified for an increment, while `GetField` was counted as covered, because
//! coverage was tracked per opcode and the gap lived in one of its KIND ARMS.
//!
//! A refusal is loud. An accepted path that no test executes ships a plausible
//! wrong number, and this backend's correctness argument is a differential —
//! worth exactly as much as the paths it actually drives.
//!
//! # What this measures, and the distinction it keeps
//!
//! For the four composite read operands that carry a `ScalarKind`, and for the
//! shared-slot layout tags, it measures **which (family, kind) combinations the
//! CORPUS produces**. That is not the same claim as "nothing exercises it": the
//! hand-written tests are a second population, and every unreached combination
//! below is resolved explicitly against them rather than assumed either way.
//!
//! # The enumeration is TYPE-DRIVEN, not text-driven
//!
//! `kind_name` matches on `ScalarKind` exhaustively, so a new variant fails to
//! COMPILE rather than slipping past a regular expression over source text.
//! Parsing source when the data is reachable as data is the instrument error
//! this package has already made four times.
//!
//! **The first draft of that sentence named a test that does not exist**, and
//! the citation guard caught it — the second time in one session. A dead name in
//! a comment is a citation that cannot fail, so it reads as coverage while being
//! none, whether or not it was meant as a citation. The mechanism is named here
//! instead, and the mechanism is a function that really is in this file.

mod common;

use keleusma::bytecode::{ArrayElem, EnumField, Op, StructField, TupleField};
use keleusma::value_layout::ScalarKind as SK;
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use std::collections::BTreeMap;

/// The read families that carry a scalar kind. Named here so the table below is
/// legible; the variants themselves come from the bytecode types.
const FAMILIES: [&str; 5] = [
    "GetField(Flat)",
    "GetTupleField(Flat)",
    "GetEnumField(Flat)",
    "GetIndex(Flat)",
    "shared slot",
];

fn kind_name(k: SK) -> &'static str {
    // **EXHAUSTIVE ON PURPOSE.** A new `ScalarKind` variant breaks this build,
    // which is the point: the census cannot silently omit a kind it has never
    // heard of.
    match k {
        SK::Unit => "Unit",
        SK::Bool => "Bool",
        SK::Byte => "Byte",
        SK::Int => "Int",
        SK::Fixed => "Fixed",
        // **NO `cfg` GUARD HERE, DELIBERATELY.** `floats` is a feature of the
        // `keleusma` crate, not of this package, so a `#[cfg(feature = "floats")]`
        // in a test of THIS crate is evaluated against the wrong feature set and
        // silently drops the arm — which is how the first draft failed to
        // compile against a `ScalarKind` that does have `Float`. The rest of this
        // backend names `SK::Float` unguarded for the same reason.
        SK::Float => "Float",
        SK::Text => "Text",
        SK::Opaque => "Opaque",
    }
}

/// Every kind the type defines, so the table has a row per kind whether or not
/// the corpus ever produces it. An absent row and a zero row are different
/// findings and only the second is honest.
fn all_kinds() -> Vec<SK> {
    vec![
        SK::Unit,
        SK::Bool,
        SK::Byte,
        SK::Int,
        SK::Fixed,
        SK::Float,
        SK::Text,
        SK::Opaque,
    ]
}

fn tag_to_kind(tag: u8) -> Option<SK> {
    all_kinds().into_iter().find(|k| k.to_tag() == tag)
}

/// `(family, kind) -> occurrences`, over the compiled corpus.
fn corpus_reach() -> (BTreeMap<(&'static str, &'static str), usize>, usize, usize) {
    let mut tally: BTreeMap<(&'static str, &'static str), usize> = BTreeMap::new();
    let mut modules = 0usize;
    let mut chunks = 0usize;

    for p in common::corpus_sources() {
        let Ok(src) = std::fs::read_to_string(&p) else {
            continue;
        };
        let Some(m) = tokenize(&src)
            .ok()
            .and_then(|t| parse(&t).ok())
            .and_then(|a| compile(&a).ok())
        else {
            continue;
        };
        modules += 1;
        for c in &m.chunks {
            chunks += 1;
            for op in &c.ops {
                let hit = match op {
                    Op::GetField(StructField::Flat { kind, .. }) => Some(("GetField(Flat)", *kind)),
                    Op::GetTupleField(TupleField::Flat { kind, .. }) => {
                        Some(("GetTupleField(Flat)", *kind))
                    }
                    Op::GetEnumField(EnumField::Flat { kind, .. }) => {
                        Some(("GetEnumField(Flat)", *kind))
                    }
                    Op::GetIndex(ArrayElem::Flat { kind }) => Some(("GetIndex(Flat)", *kind)),
                    _ => None,
                };
                if let Some((fam, k)) = hit {
                    *tally.entry((fam, kind_name(k))).or_default() += 1;
                }
            }
        }
        if let Some(dl) = m.data_layout.as_ref() {
            for e in &dl.shared_layout {
                if let Some(k) = tag_to_kind(e.kind) {
                    *tally.entry(("shared slot", kind_name(k))).or_default() += 1;
                }
            }
        }
    }
    (tally, modules, chunks)
}

/// The table, and the residue named.
#[test]
fn every_kind_arm_is_reached_by_the_corpus_or_named_as_unreached() {
    let (tally, modules, chunks) = corpus_reach();

    println!("\n================ KIND-DISCRIMINATED ARMS, CORPUS REACH");
    println!("  population: {modules} modules, {chunks} chunks");
    let mut unreached: Vec<String> = Vec::new();
    for fam in FAMILIES {
        println!("  {fam}");
        for k in all_kinds() {
            let n = tally.get(&(fam, kind_name(k))).copied().unwrap_or(0);
            println!("    {:<8} {n}", kind_name(k));
            if n == 0 {
                unreached.push(format!("{fam} x {}", kind_name(k)));
            }
        }
    }
    println!("  ----");
    println!(
        "  combinations the corpus does NOT reach: {}",
        unreached.len()
    );
    for u in &unreached {
        println!("    {u}");
    }
    println!(
        "\n  **THIS IS CORPUS REACH, NOT COVERAGE.** A combination absent here may\n  \
         still be exercised by a hand-written test; that second population is\n  \
         resolved by `the_unreached_combinations_are_each_accounted_for`."
    );
    println!("================\n");

    // **NON-VACUITY, BOTH DIRECTIONS.** The sweep must have seen the corpus, and
    // it must be capable of reporting BOTH a reached and an unreached
    // combination — a table that was all-zero or all-nonzero would be
    // indistinguishable from a broken instrument.
    assert!(
        chunks > 100,
        "the sweep saw only {chunks} chunks over {modules} modules, so it is \
         measuring the harness rather than the corpus"
    );
    assert!(
        !tally.is_empty(),
        "no kind-carrying read was found anywhere, so every 'unreached' row \
         below is an artefact of the sweep rather than a fact about the corpus"
    );
    assert!(
        !unreached.is_empty(),
        "every combination is reached, which would be a RESULT worth recording \
         — but a sweep that cannot report an unreached combination is the \
         likelier cause and must be excluded first"
    );
}

/// **THE RESIDUE, RESOLVED ONE BY ONE.** Corpus silence is not coverage. Each
/// combination the corpus never produces is either attributed to a named
/// hand-written test or recorded as UNEXERCISED, and the list is pinned so a new
/// gap announces itself rather than joining a count nobody reads.
#[test]
fn the_unreached_combinations_are_each_accounted_for() {
    let (tally, _, _) = corpus_reach();

    // `covered by` names a test that drives the combination; `UNEXERCISED`
    // records that nothing does. **A kind the backend REFUSES is still listed**,
    // because "refused" is a fact about the lowering and this table is about
    // evidence.
    //
    // **THIS TABLE IS HAND-MAINTAINED, AND IT WAS WRONG ON ITS FIRST DRAFT.** It
    // listed `GetField(Flat) x Byte` as unexercised;
    // `differential::a_byte_field_zero_extends_like_the_vm` drives exactly that,
    // deliberately with a byte above 127 because a sign-extending load would
    // read `0xFF` as `-1`. Found by READING the tests rather than by assuming,
    // which is the whole reason corpus reach and coverage are kept apart here.
    //
    // It cannot silently go stale in the other direction: the assertion below
    // fails if the corpus starts reaching an attributed row. A MISSING row still
    // needs a human, and that asymmetry is stated rather than hidden.
    let accounted: &[(&str, &str, &str)] = &[
        (
            "GetField(Flat)",
            "Byte",
            "differential::a_byte_field_zero_extends_like_the_vm",
        ),
        (
            "GetIndex(Flat)",
            "Byte",
            "float_composite::a_byte_array_element_zero_extends_like_the_vm",
        ),
        (
            "GetTupleField(Flat)",
            "Byte",
            "float_composite::a_byte_tuple_member_zero_extends_like_the_vm",
        ),
        (
            "GetEnumField(Flat)",
            "Byte",
            "float_composite::a_byte_enum_payload_zero_extends_like_the_vm",
        ),
        (
            "GetField(Flat)",
            "Bool",
            "narrow_composite::a_bool_struct_field_reads_back_on_both_paths",
        ),
        (
            "GetIndex(Flat)",
            "Bool",
            "narrow_composite::a_bool_array_element_agrees_with_the_vm",
        ),
        (
            "GetTupleField(Flat)",
            "Fixed",
            "narrow_composite::a_fixed_tuple_member_agrees_with_the_vm",
        ),
        (
            "GetTupleField(Flat)",
            "Bool",
            "narrow_composite::a_bool_tuple_member_agrees_with_the_vm",
        ),
        (
            "GetEnumField(Flat)",
            "Bool",
            "narrow_composite::a_bool_enum_payload_agrees_with_the_vm",
        ),
        (
            "GetEnumField(Flat)",
            "Fixed",
            "narrow_composite::a_fixed_enum_payload_agrees_with_the_vm",
        ),
        (
            "GetEnumField(Flat)",
            "Float",
            "float_composite::a_float_enum_payload_agrees_with_the_vm",
        ),
        (
            "shared slot",
            "Bool",
            "shared_data::a_bool_shared_slot_agrees_in_value_and_in_buffer",
        ),
        (
            "GetField(Flat)",
            "Float",
            "float_composite::a_float_struct_field_agrees_with_the_vm",
        ),
        (
            "GetIndex(Flat)",
            "Float",
            "float_composite::an_array_of_floats_indexes_at_the_right_stride",
        ),
        (
            "shared slot",
            "Float",
            "shared_data::a_float_shared_slot_agrees_in_value_and_in_buffer",
        ),
        (
            "GetTupleField(Flat)",
            "Float",
            "float_composite::a_float_in_a_tuple_agrees_with_the_vm",
        ),
    ];

    let mut unresolved: Vec<String> = Vec::new();
    let mut resolved = 0usize;
    let mut unexercised: Vec<String> = Vec::new();

    println!("\n================ UNREACHED COMBINATIONS, RESOLVED");
    for fam in FAMILIES {
        for k in all_kinds() {
            let name = kind_name(k);
            if tally.get(&(fam, name)).copied().unwrap_or(0) > 0 {
                continue;
            }
            match accounted.iter().find(|(f, kk, _)| *f == fam && *kk == name) {
                Some((_, _, by)) => {
                    resolved += 1;
                    println!("  {fam} x {name}\n      covered by {by}");
                }
                None => {
                    unexercised.push(format!("{fam} x {name}"));
                    println!("  {fam} x {name}\n      UNEXERCISED by corpus or by any named test");
                }
            }
        }
    }
    println!("  ----");
    println!("  resolved to a named test : {resolved}");
    println!("  UNEXERCISED              : {}", unexercised.len());
    println!("================\n");

    // Every entry in the attribution table must still be unreached by the
    // corpus; otherwise the table is stale and claims credit for a combination
    // the corpus now covers on its own.
    for (fam, name, by) in accounted {
        if tally.get(&(*fam, *name)).copied().unwrap_or(0) > 0 {
            unresolved.push(format!(
                "{fam} x {name} is now reached by the CORPUS, so attributing it to \
                 {by} is stale — remove the row rather than leaving two claims"
            ));
        }
    }
    assert!(unresolved.is_empty(), "{unresolved:#?}");

    // **THE FOUR CONSTRUCTION-SIDE REFUSALS BELOW WERE CLOSED, NOT WORKED
    // AROUND.** They were refused by `NewComposite` for an operand of unknown
    // packed width, which turned out to be three PRODUCERS dropping it —
    // `PushImmediate`, `WordToFixed`, and the integer half of the comparison
    // arm, whose float twin had always set it. See
    // `docs/decisions/OPERAND_WIDTH_GAP_BRIEF.md`. The `Fixed` ARRAY element is
    // covered by the corpus and so never appears in this table.
    //
    // **WHY THE REST STAY UNEXERCISED, MEASURED RATHER THAN ASSUMED.**
    // `probe_float_composite::which_narrow_and_fixed_composite_reads_are_reachable_from_source`
    // asked the reference compiler and the backend directly. Of the shapes that
    // ordinary source can express, only the `Byte` tuple member and the `Byte`
    // enum payload LOWER, and both are witnessed above. **A `Bool` field, a
    // `Bool` element, a `Fixed` tuple member and a `Fixed` array element all
    // compile and are then REFUSED** -- not by a kind arm, but by
    // `NewComposite` reporting an operand of unknown packed width. That is a
    // loud refusal in the safe direction, and it is a DIFFERENT cause from the
    // one this table is about, which is why it is named here rather than
    // silently counted as a coverage gap.
    //
    // **WHAT REMAINS IS NOW ENTIRELY REFUSED KINDS, PLUS ONE OPERATOR
    // QUESTION.** `Unit`, `Text` and `Opaque` are refused across all five
    // families; the `Fixed` SHARED SLOT is refused because the host-visible
    // fraction-bit scale is unspecified, which is an open ruling and not a gap
    // in evidence. **Every combination this backend accepts is now driven by
    // something**, which is the state this census was built to reach and is a
    // claim that will decay the moment a new arm lands.
    //
    // **THE UNEXERCISED SET IS NOT ASSERTED EMPTY.** Most of it is kinds this
    // backend refuses outright, and a contrived witness for a refused kind would
    // be worse than an honest gap. What is asserted is that the attribution
    // table is not vacuous: if it resolved nothing, this file would be a
    // listing rather than an accounting.
    assert!(
        resolved > 0,
        "the attribution table resolved nothing, so this test is a listing \
         rather than an accounting"
    );
}

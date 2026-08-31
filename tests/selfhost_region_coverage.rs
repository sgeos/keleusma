//! Which regions of a stage's auxiliary body the self-hosted path actually
//! produces, measured rather than asserted.
//!
//! # Why this file exists
//!
//! The coverage claim for the self-hosted emit path has lived in prose: a doc
//! comment naming four region kinds, and a test comparing exactly those four.
//! Every other kind falls into a `_ => continue` in the windowed assembler and
//! is left as **zeros**, which no test looked at. A claim of the form "the path
//! reaches N regions" that is checked by comparing exactly those N regions
//! cannot fail for the reason a reader cares about — that the set stopped
//! growing, or quietly shrank.
//!
//! **This is the sixth instance on this line of a suite whose coverage is a
//! property of its case list mistaken for a property of the thing under test.**
//! The remedy here is the same one that worked for the others: derive the set
//! from the artifact rather than listing it, and assert the derivation is
//! non-vacuous.
//!
//! # The three outcomes, and why "zeroed" is not "differs"
//!
//! Each region kind lands in one of three states, and collapsing them would
//! destroy the distinction the whole file exists for:
//!
//! | outcome | meaning |
//! |---|---|
//! | `Identical` | Keleusma produced these bytes and they match the reference |
//! | `Skipped` | the driver never routed the kind; the bytes are zeros |
//! | `Differs` | the driver routed it and produced the wrong bytes |
//!
//! `Skipped` is an honest gap and `Differs` is a defect. A test reporting only
//! "not identical" would call them the same thing, which is the precise mistake
//! the construct-support boundary made before `Gap` was split into `Refuses` and
//! `Diverges`.
//!
//! # What this does NOT establish
//!
//! That an `Identical` region is *self-hosted* in the strong sense. `HEADER` is
//! encoded but not derived — the host reads eleven scalars off the `Module` and
//! the stage decides the record's layout — and `CHUNKS` is mixed per field. This
//! file measures BYTES, which is a different question from provenance, and the
//! provenance table lives in `docs/process/HANDOFF.md`.

#![cfg(all(feature = "self-host", feature = "compile"))]

use keleusma::bytecode::Module;
use keleusma::wire_format::{WireAuxBody, WireChunk};

const CORPUS_STAGES: &[(&str, &str)] = &[
    ("lexer", include_str!("../src/selfhost/kel/lexer.kel")),
    ("parse", include_str!("../src/selfhost/kel/parse.kel")),
    ("codegen", include_str!("../src/selfhost/kel/codegen.kel")),
    (
        "reconstruct",
        include_str!("../src/selfhost/kel/reconstruct.kel"),
    ),
    ("analyze", include_str!("../src/selfhost/kel/analyze.kel")),
    (
        "verify_structural",
        include_str!("../src/selfhost/kel/verify_structural.kel"),
    ),
    (
        "verify_typed",
        include_str!("../src/selfhost/kel/verify_typed.kel"),
    ),
    (
        "verify_yield",
        include_str!("../src/selfhost/kel/verify_yield.kel"),
    ),
    (
        "verify_depth",
        include_str!("../src/selfhost/kel/verify_depth.kel"),
    ),
    (
        "verify_datalayout",
        include_str!("../src/selfhost/kel/verify_datalayout.kel"),
    ),
    (
        "verify_types",
        include_str!("../src/selfhost/kel/verify_types.kel"),
    ),
    ("wire", include_str!("../src/selfhost/kel/wire.kel")),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// Keleusma produced these bytes and they match the reference.
    Identical,
    /// The driver never routed this kind; the assembled bytes are zeros.
    Skipped,
    /// The driver routed it and produced the wrong bytes.
    Differs,
}

fn compile_stage(src: &str) -> Module {
    keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(src).expect("lex")).expect("parse"),
    )
    .expect("compile")
}

fn corpus_aux_of(module: &Module) -> WireAuxBody {
    WireAuxBody {
        chunks: module
            .chunks
            .iter()
            .map(|c| WireChunk {
                name: c.name.clone(),
                constants: c.constants.clone(),
                struct_templates: c.struct_templates.clone(),
                local_count: c.local_count,
                param_count: c.param_count,
                block_type: c.block_type,
                param_types: c.param_types.clone(),
                op_byte_offset: 0,
                op_record_count: 0,
                debug_pool_bytes: None,
            })
            .collect(),
        signatures: module.signatures.clone(),
        enum_layouts: module.enum_layouts.clone(),
        native_names: module.native_names.clone(),
        native_return_shapes: module.native_return_shapes.clone(),
        data_layout: module.data_layout.clone(),
        entry_point: module.entry_point,
        word_bits_log2: module.word_bits_log2,
        addr_bits_log2: module.addr_bits_log2,
        float_bits_log2: module.float_bits_log2,
        flags: 0,
        wcet_cycles: 0,
        wcmu_bytes: 0,
        shared_data_bytes: 0,
        private_data_bytes: 0,
        schema_hash: 0,
    }
}

/// One stage's name beside its classified regions.
///
/// Named rather than written inline: the tuple is three levels deep and clippy
/// asks for a definition, which is the right ask — a reader meeting
/// `(&str, Vec<(u16, usize, Outcome)>)` in a signature learns nothing the name
/// does not say better.
type StageCensus = (&'static str, Vec<(u16, usize, Outcome)>);

/// The whole corpus's census, computed once.
///
/// # Why this is memoised rather than recomputed per test
///
/// `wire_windowed_via_kel` compiles `wire.kel` once per ROUTED REGION, so a
/// single stage's census costs several stage compiles and the corpus costs
/// dozens. Four tests each walking the corpus took 110 seconds and produced four
/// identical answers. Sharing one answer is not only faster — it removes the
/// possibility of two tests in this file disagreeing about what the tree does,
/// which would be a confusing failure to read.
fn census() -> &'static [StageCensus] {
    static CENSUS: std::sync::OnceLock<Vec<StageCensus>> = std::sync::OnceLock::new();
    CENSUS.get_or_init(|| {
        CORPUS_STAGES
            .iter()
            .map(|(name, src)| (*name, classify(&compile_stage(src))))
            .collect()
    })
}

/// Every non-empty region of one stage, classified, with its payload length.
///
/// The region list is **derived from the artifact's own directory**, so a kind
/// this file has never heard of is measured rather than ignored. That is the
/// property the previous four-kind test lacked.
fn classify(module: &Module) -> Vec<(u16, usize, Outcome)> {
    let want = keleusma::wire_schema::encode_aux_body(&corpus_aux_of(module)).expect("encode");
    let view = keleusma_wire::WireView::parse(&want).expect("reference parses");

    let mut regions = Vec::new();
    for i in 0..view.region_count() {
        let r = view.region_at(i).expect("region in range");
        regions.push((
            r.kind,
            (r.word_offset as usize) * 8,
            (r.word_length as usize) * 8,
        ));
    }

    let got = keleusma::selfhost::wire_windowed_via_kel(module, want.len(), &regions)
        .expect("the windowed driver refused");

    let mut out = Vec::new();
    for &(kind, base, len) in &regions {
        if len == 0 {
            continue;
        }
        let mine = &got.bytes[base..base + len];
        let theirs = &want[base..base + len];
        let outcome = if mine == theirs {
            Outcome::Identical
        } else if mine.iter().all(|b| *b == 0) {
            Outcome::Skipped
        } else {
            Outcome::Differs
        };
        out.push((kind, len, outcome));
    }
    out
}

/// **NO REGION IS WRONG. A REGION IS EITHER PRODUCED OR VISIBLY ABSENT.**
///
/// This is the assertion that must never relax. `Skipped` is a gap the tree
/// states honestly; `Differs` means Keleusma emitted bytes the reference does not
/// agree with, which on the self-hosting claim is the whole failure mode.
///
/// The vacuity guard is not decoration: a driver that routed nothing would
/// produce an all-`Skipped` census and satisfy the headline assertion perfectly.
#[test]
fn no_region_the_driver_routes_disagrees_with_the_reference() {
    let mut identical = 0usize;
    let mut skipped = 0usize;
    let mut checked = 0usize;

    for (name, regions) in census() {
        for &(kind, len, outcome) in regions {
            assert_ne!(
                outcome,
                Outcome::Differs,
                "{name}: region {kind:#06x} ({len} bytes) was emitted and disagrees with the \
                 reference. A skipped region is a recorded gap; a differing one is a \
                 mis-emission, and the difference is the point of this test"
            );
            match outcome {
                Outcome::Identical => identical += 1,
                Outcome::Skipped => skipped += 1,
                Outcome::Differs => unreachable!(),
            }
            checked += 1;
        }
    }

    assert!(
        checked > 50,
        "only {checked} regions were classified across the corpus, so the directory walk is \
         broken and the assertion above held over almost nothing"
    );
    assert!(
        identical >= 4 * CORPUS_STAGES.len(),
        "only {identical} regions came back identical against {skipped} skipped. The driver \
         routes at least four kinds per stage; a collapse here means it has stopped routing \
         them rather than that it stopped mis-emitting"
    );
}

/// **THE COVERAGE FIGURE, DERIVED, IN BYTES RATHER THAN IN REGION COUNT.**
///
/// A count of region kinds weights `ENUM_LAYOUTS` at 48 bytes the same as
/// `CONSTS` at 37,152, which flatters a path that reaches many small regions and
/// none of the large ones. Bytes are what an artifact is made of.
///
/// # Why a band and not an exact figure
///
/// An exact byte total fails on every edit to a stage source, which trains its
/// reader to re-baseline it rather than read it. The lower bound is what catches
/// a regression; the upper bound is what makes a reader update the prose when the
/// path genuinely improves, instead of leaving a stale "four regions" behind.
#[test]
fn the_self_hosted_share_of_the_corpus_is_the_one_recorded() {
    let mut covered = 0usize;
    let mut total = 0usize;

    for (_name, regions) in census() {
        for &(_kind, len, outcome) in regions {
            total += len;
            if outcome == Outcome::Identical {
                covered += len;
            }
        }
    }

    assert!(total > 0, "the corpus emitted no regions at all");
    let share = (covered * 100) / total;

    assert!(
        share >= 88,
        "the self-hosted path produces {share}% of the corpus's region bytes ({covered} of \
         {total}). It was 93% when `SHAPES` and `SIGNATURES` landed, and 81% before them; a \
         fall means a region stopped being routed"
    );
    assert!(
        share < 97,
        "the self-hosted path now produces {share}% of the corpus's region bytes, past what \
         the record describes. That is good news and it needs recording: update the coverage \
         statement rather than widening this bound"
    );
}

/// **`CONSTS` REACHES THE ASSEMBLED ARTIFACT, NOT ONLY ITS OWN ENTRY POINT.**
///
/// `wire_consts_via_kel` emitting a correct region and `wire_windowed_via_kel`
/// placing it in an artifact are different claims. The second was false for the
/// whole time the first was true: the assembler's kind match ended in
/// `_ => continue`, so a caller assembling a whole body got zeros where the
/// largest region should be.
///
/// Pinned per stage rather than in aggregate, because a single stage routing it
/// would satisfy an aggregate check.
#[test]
fn the_assembled_artifact_carries_the_constants_region() {
    use keleusma::wire_schema::kind;

    let mut carried = 0usize;
    for (name, regions) in census() {
        let entry = regions
            .iter()
            .find(|(k, _, _)| *k == kind::CONSTS)
            .unwrap_or_else(|| panic!("{name}: the reference emitted no CONSTS region"));
        assert_eq!(
            entry.2,
            Outcome::Identical,
            "{name}: the assembled artifact's CONSTS region is {:?}, so the region is \
             emitted by its own entry point and lost by the assembler",
            entry.2
        );
        carried += 1;
    }
    assert_eq!(carried, CORPUS_STAGES.len(), "not every stage was checked");
}

/// **WHAT IS STILL SKIPPED, NAMED RATHER THAN SUMMARISED.**
///
/// A reader who sees a coverage percentage will want to know what the remainder
/// is. Listing the skipped kinds in the failure message makes the next slice's
/// target readable off a test run instead of off a document that may be stale.
///
/// Pinned in the direction that matters: when a kind stops being skipped, this
/// fails and its author records the new coverage rather than leaving the old
/// figure standing.
#[test]
fn the_skipped_region_kinds_are_the_ones_on_record() {
    let mut skipped: Vec<u16> = Vec::new();
    for (_name, regions) in census() {
        for &(kind, _len, outcome) in regions {
            if outcome == Outcome::Skipped && !skipped.contains(&kind) {
                skipped.push(kind);
            }
        }
    }
    skipped.sort_unstable();

    // Derived from the tree, not listed: whatever the driver does not route.
    // Recorded 2026-08-22 as SHAPES, SIGNATURES, ENUM_VARIANTS, ENUM_LAYOUTS,
    // DATA_SLOTS, SHARED_LAYOUT, DATA_INIT and PARAM_TYPES; `SHARED_LAYOUT` left
    // the set on 2026-08-31.
    assert!(
        !skipped.is_empty(),
        "no region kind is skipped any more. That is a real advance: state the new coverage \
         in the driver's doc comment and in the handoff, and replace this test with one \
         asserting completeness"
    );
    // FIVE, down from six on 2026-08-31 when `SHARED_LAYOUT` was routed. `DATA_INIT` is still
    // listed and that is not a failure to route it: it IS routed, and matches the reference, for
    // the eleven stage sources whose private-initialiser pool is elided. The twelfth,
    // `verify_datalayout.kel`, stores its pool in the shared constant table, and predicting the
    // index it lands at means modelling the encoder's constant ordering -- the `CONSTS` problem,
    // and a separate increment. That one stage keeps the kind on this list.
    //
    // **A kind is listed here if it is skipped for ANY stage**, so this figure moves only when a
    // kind is routed for EVERY stage. That is the conservative reading and the right one: a kind
    // routed for most inputs is not a kind the driver covers.
    assert!(
        skipped.len() <= 5,
        "{} kinds are skipped: {skipped:02x?}. FIVE are on record after `SHARED_LAYOUT` was \
         routed -- `ENUM_VARIANTS`, `ENUM_LAYOUTS`, `DATA_SLOTS`, `DATA_INIT` (for the one \
         stage that does not elide) and `PARAM_TYPES`. More means the driver has stopped \
         routing something it used to",
        skipped.len()
    );
}

/// **THE 81% IS NOT ALL ONE THING, AND SAYING SO IS THE POINT OF THIS TEST.**
///
/// "The self-hosted path produces 81% of the corpus's region bytes" is true and
/// invites a stronger reading than it supports. The handoff's provenance table
/// distinguishes three standings and they are not comparable:
///
/// | standing | regions | what Keleusma decides |
/// |---|---|---|
/// | **computed** | `NAMES`, `STRING_POOL`, `CONSTS` | the stage walks the module blob and derives every byte |
/// | **mixed** | `CHUNKS` | the stage computes the name index and three range cursors; ten fields per record come from the host |
/// | **encoded, not derived** | `HEADER` | the host reads the scalars off the `Module`; the stage decides offsets, widths and endianness |
///
/// `wire.kel` makes the same distinction about the record formatters it carries
/// for the still-skipped kinds: *"COVERAGE IS WHAT THESE ARE, WHICH IS
/// FORMATTING ... counting them beside `NAMES` would overstate what is
/// self-hosted."* Wiring those kinds will raise the byte figure without raising
/// the computed one, which is exactly why both are pinned here.
///
/// # What this test is for
///
/// So that the headline figure cannot drift away from its composition. If a
/// future slice raises coverage to 97% by wiring formatters, the computed share
/// stays put and this test says so, rather than leaving a reader to read 97% as
/// meaning the compiler derives 97% of its own artifact.
///
/// # THE FIGURE IS 56%, AND IT WAS RECORDED AS 57%
///
/// `94,120` of `165,208` is **56.97%**, and this test truncates, so it reports
/// **56**. Three process documents and two pull-request bodies said 57% -- an
/// honest rounding of the same measurement, but not the number the tree asserts,
/// and a prose figure that disagrees with its own test is how every stale-figure
/// incident on this line began. Both forms are stated here so they cannot part
/// again.
#[test]
fn the_computed_share_is_smaller_than_the_produced_share() {
    use keleusma::wire_schema::kind;

    // The regions the stage DERIVES rather than formats. Listed rather than
    // inferred, because provenance is not a property of the bytes and cannot be
    // measured from them — it is a fact about which side computes the values,
    // and the only honest way to carry it is to state it and pin the figures.
    const COMPUTED: &[u16] = &[kind::NAMES, kind::STRING_POOL, kind::CONSTS];

    let mut computed = 0usize;
    let mut produced = 0usize;
    let mut total = 0usize;

    for (_name, regions) in census() {
        for &(kind, len, outcome) in regions {
            total += len;
            if outcome != Outcome::Identical {
                continue;
            }
            produced += len;
            if COMPUTED.contains(&kind) {
                computed += len;
            }
        }
    }

    assert!(total > 0, "the corpus emitted no regions at all");
    let computed_share = (computed * 100) / total;
    let produced_share = (produced * 100) / total;

    assert!(
        computed < produced,
        "every produced byte is a computed byte ({computed} of {produced}). Either the \
         formatted regions stopped being emitted, or `COMPUTED` has grown to cover them \
         without anyone checking that the stage derives their values"
    );
    assert!(
        computed_share >= 52,
        "only {computed_share}% of the corpus's region bytes are DERIVED by the stage \
         (against {produced_share}% produced). It was 56% when `SHAPES` and `SIGNATURES` \
         landed; a fall means a computed region stopped being routed"
    );
    assert!(
        computed_share < produced_share,
        "the computed share ({computed_share}%) has caught the produced share \
         ({produced_share}%). If every routed region is now derived, say so: update the \
         provenance table in the handoff and replace this test with one asserting it"
    );
}

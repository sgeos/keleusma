//! What the `CONSTS` region of the eleven stage artifacts is actually made of.
//!
//! # Why this file exists
//!
//! `CONSTS` is the largest region in the auxiliary body by an order of magnitude
//! and it is the remaining bulk of the self-hosted emit path. The plan for
//! reaching it rested on two obstacles recorded in the doc comment on
//! [`keleusma::selfhost::wire_regions_via_kel`], and one of the two turned out to
//! have **no instances in this corpus**. This file pins the measurements that
//! establish that, so the plan rests on a checked fact rather than on a sentence
//! in a comment that nothing re-runs.
//!
//! Every figure quoted in a doc comment or a decision document about this region
//! should be derivable from a test here. Six of the seven stale-figure incidents
//! on this line were in documents no test reads.
//!
//! # What was measured, and the order it had to be measured in
//!
//! The first attempt classified only `Chunk::constants` and concluded that the
//! flattener interns nothing. That conclusion was right and the measurement did
//! not support it: the chunk pools are 2,245 of the corpus's 40,332 constants,
//! and the other 38,087 arrive through `DataLayout::private_init`, which the
//! probe never walked.
//!
//! The second attempt compared the string pool of a full artifact against one
//! with every constant removed, saw a 5,264-byte difference for `parse`, and
//! nearly recorded the opposite conclusion. That measurement could not
//! discriminate either: clearing `private_init` also removes the slot names that
//! `add_data_layout` interns **directly**, which is a separate contributor.
//!
//! Only the third form separates them, and it is the one
//! [`the_flattener_interns_no_name_for_any_stage`] performs: hold the data layout
//! in place, clear only what the flattener sees, and compare. A conclusion
//! supported by measurements that cannot discriminate is not a measured
//! conclusion.
//!
//! # What the answer turned out to be
//!
//! Neither recorded obstacle is what blocks the region. The flattener is already
//! driven from real modules and already emits a byte-identical `CONSTS` region.
//! What excludes the eleven stages is the 170-node walk cap, pinned by
//! [`the_node_walk_cap_is_what_excludes_the_stages`], together with the fact that
//! widening the walk to clear the cap costs six times what it would emit
//! ([`widening_the_walk_costs_more_than_the_region_it_would_emit`]). Batching is
//! the route, and this corpus is the easy case for it: a forest of scalars with
//! no interning and no children carries no state between batches.

#![cfg(all(feature = "compile", feature = "self-host"))]

use keleusma::bytecode::{ConstValue, Module};
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
];

/// The `CONSTS` region kind, restated rather than imported: this file is about
/// what the encoder emits, and pinning the number here means a renamed constant
/// cannot silently redirect the test at a different region.
const KIND_CONSTS: u16 = 0x0012;
const KIND_STRING_POOL: u16 = 0x0010;
/// `ConstRecord` stride, pinned for the same reason.
const CONST_RECORD_BYTES: usize = 16;

fn compile_stage(src: &str) -> Module {
    keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(src).expect("lex")).expect("parse"),
    )
    .expect("compile")
}

/// The same auxiliary body the wire corpus tests build, so the figures here are
/// the figures those tests compare against.
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

/// A region's bytes, or an empty slice when the artifact carries no such region.
fn region_bytes(artifact: &[u8], kind: u16) -> Vec<u8> {
    let view = keleusma_wire::WireView::parse(artifact).expect("reference artifact parses");
    for i in 0..view.region_count() {
        let r = view.region_at(i).expect("region in range");
        if r.kind == kind {
            let start = (r.word_offset as usize) * 8;
            let len = (r.word_length as usize) * 8;
            return artifact[start..start + len].to_vec();
        }
    }
    Vec::new()
}

fn encode(aux: &WireAuxBody) -> Vec<u8> {
    keleusma::wire_schema::encode_aux_body(aux).expect("encode aux body")
}

/// Counts nodes that would make the flattener intern a name, recursing into
/// composites because a name-bearing node can sit at any depth.
fn name_bearing_nodes(v: &ConstValue) -> usize {
    match v {
        ConstValue::StaticStr(_) => 1,
        ConstValue::Tuple(xs) | ConstValue::Array(xs) => xs.iter().map(name_bearing_nodes).sum(),
        ConstValue::Struct { fields, .. } => {
            1 + fields
                .iter()
                .map(|(_, x)| name_bearing_nodes(x))
                .sum::<usize>()
        }
        ConstValue::Enum { fields, .. } => 1 + fields.iter().map(name_bearing_nodes).sum::<usize>(),
        _ => 0,
    }
}

/// Every constant the flattener sees, from both of its two sources.
///
/// The second source is the one the first probe missed, and it is the larger of
/// the two by a factor of seventeen.
fn all_constants(m: &Module) -> Vec<ConstValue> {
    let mut out: Vec<ConstValue> = Vec::new();
    for c in &m.chunks {
        out.extend(c.constants.iter().cloned());
    }
    if let Some(dl) = &m.data_layout {
        out.extend(dl.private_init.iter().cloned());
    }
    out
}

/// **THE LOAD-BEARING FACT.** No stage's constants make the flattener intern a
/// name, so the two paths' interning orders cannot conflict on this corpus.
///
/// The recorded second obstacle to emitting `CONSTS` from the self-hosted path
/// is that the module walk interns in preorder by linear scan while the
/// flattener interns breadth-first, and that the difference is observable in
/// `NAMES`. That is a true statement about the general case. It has **zero
/// instances here**, because the flattener only interns for `StaticStr`,
/// `Struct` and `Enum` nodes and the corpus contains none of the three.
///
/// If this test ever fails, the conflict has become real and the ordering
/// question has to be answered before `CONSTS` can be emitted. That is the whole
/// reason it is a test and not a sentence.
#[test]
fn the_flattener_interns_no_name_for_any_stage() {
    let mut checked = 0;
    for (name, src) in CORPUS_STAGES {
        let m = compile_stage(src);

        // Direct half: count the node kinds that intern.
        let bearing: usize = all_constants(&m).iter().map(name_bearing_nodes).sum();
        assert_eq!(
            bearing, 0,
            "{name}: {bearing} constant nodes would make the flattener intern a name. The \
             preorder-versus-breadth-first interning conflict is now reachable and the \
             ordering question has to be settled."
        );

        // Observational half, and THE ONE THAT DISCRIMINATES. Clear only what
        // the flattener sees and leave the data layout in place, so the slot
        // names `add_data_layout` interns directly are held constant. Clearing
        // the layout as well moves the pool for a reason that has nothing to do
        // with the flattener, which is what made an earlier measurement of this
        // read the wrong way round.
        let base = corpus_aux_of(&m);
        let mut stripped = base.clone();
        for c in stripped.chunks.iter_mut() {
            c.constants.clear();
        }
        if let Some(dl) = stripped.data_layout.as_mut() {
            dl.private_init.clear();
        }

        let with = region_bytes(&encode(&base), KIND_STRING_POOL);
        let without = region_bytes(&encode(&stripped), KIND_STRING_POOL);
        assert_eq!(
            with,
            without,
            "{name}: removing every constant changed the string pool by {} bytes, so the \
             flattener does intern names for this stage after all.",
            with.len() as i64 - without.len() as i64
        );
        checked += 1;
    }
    assert_eq!(checked, CORPUS_STAGES.len(), "not every stage was checked");
}

/// **THE ELISION, MEASURED AT THE ARTIFACT.** The wholly-default initialiser pool
/// contributes no records at all.
///
/// This test previously asserted the opposite — that the region holds the sum of
/// both sources — and that assertion is what quantified the waste this change
/// removes. It is inverted rather than deleted, because the property worth
/// guarding now is that the elision actually reaches the bytes. A test of the
/// encoder's intent that never looked at an artifact would pass for an encoder
/// that computed the elision and then stored the records anyway.
///
/// Both halves are asserted. A stage whose data segment is wholly default must
/// contribute nothing, and the pool must be non-empty first, or the case is
/// vacuous: a stage with no data segment would satisfy the first half for a
/// reason that has nothing to do with eliding anything.
#[test]
fn the_all_default_initialiser_pool_is_elided_from_the_region() {
    let mut with_elision = 0usize;
    let mut elided_records = 0usize;
    for (name, src) in CORPUS_STAGES {
        let m = compile_stage(src);
        let from_chunks: usize = m.chunks.iter().map(|c| c.constants.len()).sum();
        let from_data = m.data_layout.as_ref().map_or(0, |d| d.private_init.len());
        let all_default = from_data > 0
            && m.data_layout.as_ref().is_some_and(|d| {
                d.private_init
                    .iter()
                    .all(|v| matches!(v, ConstValue::Int(0)))
            });

        let records =
            region_bytes(&encode(&corpus_aux_of(&m)), KIND_CONSTS).len() / CONST_RECORD_BYTES;

        if all_default {
            assert_eq!(
                records, from_chunks,
                "{name}: the region holds {records} records against {from_chunks} chunk \
                 constants, so {from_data} wholly-default initialisers reached the artifact \
                 after all"
            );
            with_elision += 1;
            elided_records += from_data;
        } else {
            assert_eq!(
                records,
                from_chunks + from_data,
                "{name}: a pool that is not wholly default must be stored in full"
            );
        }
    }

    assert!(
        with_elision >= 8,
        "only {with_elision} stages had a wholly-default pool to elide, so the corpus no \
         longer exercises the case this encoding exists for"
    );
    assert!(
        elided_records > 30_000,
        "only {elided_records} records were elided; the measurement that justified this \
         encoding found 38,087, and a collapse of that figure means the data segment \
         changed shape"
    );
}

/// A pool that is NOT wholly default is stored in full.
///
/// **MUST-NOT-FIRE, and the corpus cannot supply it.** Every stage's data segment
/// is entirely zero, so nothing above exercises the fallback, and an encoder that
/// elided unconditionally would pass every test in this file. The value here is
/// deliberately non-zero and deliberately last, which is also the position a
/// trailing-run scheme would get wrong.
#[test]
fn a_pool_with_any_non_default_value_is_stored_in_full() {
    let m = compile_stage(
        "private data d { xs: [Word; 4], flag: Word = 7 }\n\
         fn main() -> Word { d.xs[0] = 1; d.flag }",
    );
    let dl = m.data_layout.as_ref().expect("a private data segment");
    assert!(
        dl.private_init
            .iter()
            .any(|v| !matches!(v, ConstValue::Int(0))),
        "the subject has no non-default initialiser, so it tests nothing"
    );

    let from_chunks: usize = m.chunks.iter().map(|c| c.constants.len()).sum();
    let records = region_bytes(&encode(&corpus_aux_of(&m)), KIND_CONSTS).len() / CONST_RECORD_BYTES;
    assert_eq!(
        records,
        from_chunks + dl.private_init.len(),
        "a pool carrying a non-default value must be stored whole; eliding it would lose \
         the value"
    );

    // And it must survive the round trip, which is the property that actually
    // matters to a host: the encoder may store or elide, but the decoder must
    // return what went in either way.
    let bytes = encode(&corpus_aux_of(&m));
    let back = keleusma::wire_schema::decode_aux_body(&bytes).expect("decode");
    assert_eq!(
        back.data_layout.as_ref().map(|d| &d.private_init),
        Some(&dl.private_init),
        "the initialisers did not round-trip"
    );
}

/// The elided pool round-trips, which is the only property a host depends on.
///
/// The encoder storing nothing is worth having only if the decoder reconstructs
/// exactly what was elided. Asserted against a real stage rather than a
/// constructed one, so the count is whatever the compiler actually emits.
#[test]
fn an_elided_pool_round_trips_to_the_values_it_replaced() {
    let m = compile_stage(CORPUS_STAGES[1].1);
    let before = m
        .data_layout
        .as_ref()
        .map(|d| d.private_init.clone())
        .expect("parse has a private data segment");
    assert!(
        before.len() > 1000,
        "the subject carries only {} initialisers, too few to be the case of interest",
        before.len()
    );

    let bytes = encode(&corpus_aux_of(&m));
    let back = keleusma::wire_schema::decode_aux_body(&bytes).expect("decode");
    let after = back
        .data_layout
        .as_ref()
        .map(|d| d.private_init.clone())
        .expect("the decoded layout is present");
    assert_eq!(
        after, before,
        "the elided pool decoded to something other than the values it replaced"
    );
}

/// Every data-segment initialiser in the corpus is the integer zero.
///
/// Measured at 38,087 records, which is 609,392 bytes at the 16-byte stride, and
/// approximately 85% of the 712,936-byte corpus auxiliary body. The encoder
/// spends that on one fixed-size record per zero.
///
/// **This is recorded, not acted on.** A run-length or implicit-zero encoding
/// would collapse it, and it would also lift the artifact-size ceiling that
/// currently keeps `parse` out of reach of a single-window emit — its `CONSTS`
/// region alone is 278,256 bytes against a 65,536-byte stage buffer. Both are
/// changes to the wire format, which moves every artifact the byte-identical
/// differential compares against, so the decision belongs to the operator.
#[test]
fn every_data_segment_initialiser_is_zero() {
    let (mut zero, mut nonzero) = (0usize, 0usize);
    for (_name, src) in CORPUS_STAGES {
        let m = compile_stage(src);
        let Some(dl) = &m.data_layout else { continue };
        for k in &dl.private_init {
            match k {
                ConstValue::Int(0) => zero += 1,
                _ => nonzero += 1,
            }
        }
    }
    assert!(zero > 0, "the corpus carries no data initialisers at all");
    assert_eq!(
        nonzero,
        0,
        "{nonzero} of {} data initialisers are not the integer zero. The claim that the \
         region is compressible to almost nothing no longer holds as stated.",
        zero + nonzero
    );
}

/// The 170-node FLATTENER cap, read out of `wire.kel` rather than restated.
///
/// `wire.fin` is 1,024 words and a constant node costs six, so the flattener
/// walks at most 170 nodes in one call.
///
/// **There is a second node cap and conflating the two is an error already made
/// once here.** The module-input walk refuses past 1,024 NODES
/// (`nm_max_names`, error `-240`), which `wire.kel` hits at 1,148 chunk
/// constants. That bound has nothing to do with `wire.fin`'s width; it merely
/// shares the number 1,024, which is what made the two look like one figure
/// stated two ways.
///
/// Reading the figure from the source is
/// the same discipline `highest_command` uses: a bound restated in a second
/// place is a bound that can drift.
fn declared_node_cap() -> usize {
    const SRC: &str = include_str!("../src/selfhost/kel/wire.kel");
    let line = SRC
        .lines()
        .find(|l| l.trim_start().starts_with("fn fl_max_nodes()"))
        .expect("wire.kel declares fl_max_nodes");
    line.rsplit_once('{')
        .and_then(|(_, tail)| tail.split_once('}'))
        .and_then(|(n, _)| n.trim().parse().ok())
        .expect("fl_max_nodes has a literal body")
}

/// **THE ACTUAL BLOCKER.** The walk cap, not any ordering question, is what keeps
/// the eleven stages out of the emit path.
///
/// The flattener is already driven from real modules and already produces a
/// byte-identical `CONSTS` region — `keleusma_flattens_a_constant_forest_breadth_first`
/// in `tests/selfhost_wire.rs` does exactly that. What excludes the stages is
/// that their forests do not fit in one call.
///
/// This test reports the margin rather than merely asserting a failure, because
/// the interesting quantity is HOW FAR over the cap the corpus sits. At the time
/// of writing two stages fit and `parse` needs a hundred and two calls.
#[test]
fn the_node_walk_cap_is_what_excludes_the_stages() {
    let cap = declared_node_cap();
    assert!(cap > 0, "the cap parsed as zero, so the reader is broken");

    let mut fitting = 0usize;
    let mut worst = (0usize, "");
    for (name, src) in CORPUS_STAGES {
        let n = all_constants(&compile_stage(src)).len();
        if n <= cap {
            fitting += 1;
        }
        if n > worst.0 {
            worst = (n, name);
        }
    }

    assert!(
        fitting < CORPUS_STAGES.len(),
        "every stage now fits the {cap}-node cap, so the batching this bound motivates is \
         no longer needed and the plan that rests on it is stale"
    );
    assert!(
        worst.0 > cap * 4,
        "the largest forest is {} nodes ({}) against a {cap}-node cap. The bound was recorded \
         as the dominant obstacle; if the margin has closed, say so rather than keeping the \
         claim.",
        worst.0,
        worst.1
    );
}

/// Widening the walk to fit the corpus costs more than the corpus.
///
/// A stage's private data array is initialised one `Int(0)` per word, so a `fin`
/// wide enough for N nodes adds `6 * N` records to the walking stage's own
/// `CONSTS` region. This test states the resulting ratio, which is what makes
/// "enlarge the array" a non-answer rather than merely an expensive one.
#[test]
fn widening_the_walk_costs_more_than_the_region_it_would_emit() {
    const NODE_WORDS: usize = 6;
    let worst = CORPUS_STAGES
        .iter()
        .map(|(n, src)| (all_constants(&compile_stage(src)).len(), *n))
        .max()
        .expect("a corpus");

    // What the walking stage would spend on its own data segment to hold that
    // forest, against what the forest costs to emit.
    let widened_records = worst.0 * NODE_WORDS;
    let cost_to_hold = widened_records * CONST_RECORD_BYTES;
    let cost_to_emit = worst.0 * CONST_RECORD_BYTES;

    assert!(
        cost_to_hold > cost_to_emit,
        "holding {} nodes costs {cost_to_hold} bytes of the walker's own CONSTS against \
         {cost_to_emit} to emit them ({}). If this ever inverts, widening the array becomes \
         a real option and the batching plan should be revisited.",
        worst.0,
        worst.1
    );
    assert_eq!(
        cost_to_hold / cost_to_emit,
        NODE_WORDS,
        "the ratio is the node width, and it stopped being so"
    );
}

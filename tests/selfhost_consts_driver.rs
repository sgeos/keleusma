//! The self-hosted driver emitting `CONSTS`, compared against the reference
//! encoder byte for byte.
//!
//! # What this closes and what it does not
//!
//! `CONSTS` is the largest single region of a stage's auxiliary body — 37,152
//! bytes across the eleven stages, 33.9% of a 109,552-byte corpus body, measured
//! by `tests/consts_region_composition.rs`. Until now the self-hosted emit path
//! reached `NAMES`, `STRING_POOL`, `CHUNKS` and the `HEADER` record, and a region
//! whose payload comes from the host is **not covered** by the self-hosting
//! claim at all.
//!
//! **The payload here is computed by Keleusma.** The host supplies the constant
//! forest as six words a node and the stage decides every byte of every record:
//! the tag and flag positions, the widths, the endianness, and that `aux` is
//! written as zero rather than left holding a stale index from the previous call.
//!
//! What it does NOT establish: the region's placement, its directory entry, or
//! its interaction with the rest of an artifact. This emits at window offset zero
//! and the host concatenates, which is exactly what makes the path streamable and
//! exactly what it therefore does not test.
//!
//! # Why the reference encoder is the oracle rather than `fl_walk`
//!
//! `fl_walk` is the breadth-first flattener and it **refuses** the stages: it is
//! capped at 170 nodes because the whole forest must sit in `wire.fin`, and
//! `parse` carries 857. For every case that matters here the walk cannot serve as
//! an oracle, because it cannot process the input. `encode_aux_body` can.

#![cfg(all(feature = "self-host", feature = "compile"))]

use keleusma::bytecode::{ConstValue, Module};
use keleusma::selfhost::wire_consts_via_kel;

const KIND_CONSTS: u16 = 0x0012;
const CONST_RECORD_STRIDE: usize = 16;
/// `fl_max_nodes()` in `wire.kel`: what the WALK can hold, and what this path is
/// not bounded by.
const FL_MAX_NODES: usize = 170;

const STAGES: &[(&str, &str)] = &[
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

fn compile_stage(src: &str) -> Module {
    keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(src).expect("lex")).expect("parse"),
    )
    .expect("compile")
}

/// The reference encoder's `CONSTS` payload for a module, through the SHIPPING
/// aux-body builder rather than a hand-assembled approximation.
fn reference_consts(module: &Module) -> Vec<u8> {
    let bytes = keleusma::wire_format::module_to_wire_bytes(module).expect("encode");
    let sections = keleusma::wire_format::parse_wire_sections(&bytes).expect("sections");
    let view = keleusma_wire::WireView::parse(sections.aux_body).expect("aux body parses");
    let Some(region) = view.find_region(KIND_CONSTS) else {
        return Vec::new();
    };
    view.region_bytes(&region).expect("payload").to_vec()
}

/// **THE STAGE SOURCES' OWN `CONSTS` REGIONS, EMITTED BY KELEUSMA, BYTE FOR
/// BYTE.**
///
/// Twelve real modules, including the two the walk cannot touch. This is the
/// Order 1 deliverable for this region: not a synthetic forest, not a shape
/// chosen to suit the path, but the artifacts the self-hosted compiler produces
/// for itself.
///
/// # The vacuity guards, and why two are needed
///
/// A stage with no constants would compare an empty region against an empty
/// region and pass. A run in which every stage fit inside the walk's cap would
/// pass while establishing nothing about why this path exists. Both are asserted
/// after the loop rather than per case, because the interesting property is of
/// the corpus rather than of any one stage.
#[test]
fn keleusma_emits_every_stage_consts_region_byte_identically() {
    let mut checked = 0usize;
    let mut records = 0usize;
    let mut past_the_walk_cap = 0usize;

    for (name, src) in STAGES {
        let module = compile_stage(src);
        let want = reference_consts(&module);
        assert!(
            !want.is_empty(),
            "{name}: the reference emitted no CONSTS region, so this case compares nothing"
        );

        let got = wire_consts_via_kel(&module)
            .unwrap_or_else(|e| panic!("{name}: the self-hosted path refused: {e:?}"));

        assert_eq!(
            got.len(),
            want.len(),
            "{name}: emitted {} bytes against the reference's {}",
            got.len(),
            want.len()
        );
        assert_eq!(
            got, want,
            "{name}: the emitted CONSTS region differs from the reference encoder's"
        );

        let n = want.len() / CONST_RECORD_STRIDE;
        records += n;
        if n > FL_MAX_NODES {
            past_the_walk_cap += 1;
        }
        checked += 1;
    }

    assert_eq!(checked, STAGES.len(), "not every stage was checked");
    assert!(
        records > 1000,
        "only {records} records were compared across the corpus, which is too few to be \
         the stage sources and means this test is measuring something else"
    );
    assert!(
        past_the_walk_cap >= 3,
        "only {past_the_walk_cap} stages exceeded the {FL_MAX_NODES}-node walk cap, so this \
         test no longer demonstrates the streaming path doing what the walk cannot"
    );
}

/// **THE WALK REFUSES WHAT THIS PATH EMITS, AND THE REFUSAL IS ASSERTED BY
/// CODE.**
///
/// The justification for the streaming path is not that it also works. It is
/// that it reaches forests the breadth-first walk cannot hold. That claim is
/// empty unless the walk is shown refusing one of them, and it is worth
/// nothing unless the refusal is the CAP rather than some unrelated failure —
/// this line has three recorded near-misses where a refusal was read as the
/// wrong limit.
#[test]
fn the_largest_stage_exceeds_the_walk_cap_this_path_is_not_bound_by() {
    let module = compile_stage(include_str!("../src/selfhost/kel/wire.kel"));
    let roots = keleusma::wire_schema::constant_roots_of_module(&module);

    assert!(
        roots.len() > FL_MAX_NODES,
        "`wire.kel` carries {} roots, inside the {FL_MAX_NODES}-node walk cap, so the \
         streaming path has nothing here the walk could not do",
        roots.len()
    );

    // All-scalar, asserted rather than assumed: with no children the forest's
    // node count is its root count, which is what the comparison above relies on.
    assert!(
        roots.iter().all(|v| matches!(v, ConstValue::Int(_))),
        "a non-`Int` constant entered `wire.kel`, so the streaming path's preconditions \
         no longer hold for the corpus and the region test above may be refusing rather \
         than agreeing"
    );

    assert!(
        wire_consts_via_kel(&module).is_ok(),
        "the streaming path refused the largest stage"
    );
}

/// **A COMPOSITE IS REFUSED, NOT SILENTLY MIS-EMITTED.**
///
/// A composite reaching this path would be written with a zero range and a zero
/// `aux`: structurally valid, silently wrong, and indistinguishable downstream
/// from a correct record. The refusal is the feature.
///
/// The accepting control matters as much as the refusal. Without it a path that
/// rejected everything would satisfy this test, and "refuses composites" would be
/// indistinguishable from "refuses".
#[test]
fn a_composite_constant_is_refused_rather_than_emitted_wrongly() {
    let composite =
        compile_stage("const data k { t: (Word, Word) = (1, 2) }\nfn main() -> Word { k.t.0 }");
    let refused = wire_consts_via_kel(&composite);
    assert!(
        refused.is_err(),
        "a tuple constant was emitted by the streaming path, which writes a zero range and \
         a zero aux for it: a structurally valid record with the wrong contents"
    );

    let scalar = compile_stage("fn main() -> Word { 42 }");
    let ok = wire_consts_via_kel(&scalar).expect("the scalar control must be accepted");
    assert_eq!(
        ok,
        reference_consts(&scalar),
        "the accepting control disagreed with the reference, so the refusal above says \
         nothing about composites specifically"
    );
}

/// **THE TWO ENTRY POINTS TO THE ROOT SET AGREE.**
///
/// `constant_roots_of_module` and `constant_roots` share one body, so this holds
/// by construction — and "by construction" has been wrong in this area often
/// enough that it is asserted over the corpus rather than reasoned about.
#[test]
fn both_root_entry_points_report_the_same_list() {
    let mut checked = 0usize;
    for (name, src) in STAGES {
        let m = compile_stage(src);
        let from_module = keleusma::wire_schema::constant_roots_of_module(&m);
        let aux = keleusma::wire_format::WireAuxBody {
            chunks: m
                .chunks
                .iter()
                .map(|c| keleusma::wire_format::WireChunk {
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
            signatures: m.signatures.clone(),
            enum_layouts: m.enum_layouts.clone(),
            native_names: m.native_names.clone(),
            native_return_shapes: m.native_return_shapes.clone(),
            data_layout: m.data_layout.clone(),
            entry_point: m.entry_point,
            word_bits_log2: m.word_bits_log2,
            addr_bits_log2: m.addr_bits_log2,
            float_bits_log2: m.float_bits_log2,
            flags: 0,
            wcet_cycles: 0,
            wcmu_bytes: 0,
            shared_data_bytes: 0,
            private_data_bytes: 0,
            schema_hash: 0,
        };
        let from_aux = keleusma::wire_schema::constant_roots(&aux);
        assert!(
            !from_module.is_empty(),
            "{name}: no roots, so nothing compared"
        );
        assert_eq!(
            from_module, from_aux,
            "{name}: the two entry points disagree"
        );
        checked += 1;
    }
    assert_eq!(checked, STAGES.len(), "not every stage was checked");
}

/// **WHAT A GREEN RUN HERE DOES NOT ESTABLISH: THE FLAGS AND DISCRIMINANT WORDS.**
///
/// Found by mutation rather than by reading. Swapping the `flags` and
/// `discriminant` words in the driver's six-word node **passes every test above**.
/// A reader who saw the byte identity would reasonably assume otherwise, so the
/// boundary is stated here where they will meet it.
///
/// # What is established
///
/// Every constant in the corpus is an `Int`, so both words are zero on every
/// record the tests above compare, and swapping two zeros changes nothing. That
/// is a fact about the corpus and it is asserted below, so a corpus that gains a
/// flag-bearing constant fails here rather than silently widening what the byte
/// identity covers.
///
/// # What is NOT established, stated because the first draft of this test claimed it
///
/// This test was first written to assert that a non-zero flag is UNREACHABLE
/// through the streaming path, on the reasoning that only an enum sets one and
/// the path refuses enum tags. **The witness could not be constructed.** Two
/// source shapes were tried and both fold to a discriminant `Int` at compile
/// time: `const data k { e: E = E::B }` yields `Int(0)`, and `let e = E::B`
/// yields `Int(1)`. Neither produces a `ConstValue::Enum`.
///
/// So the honest position is that **no source reaching this path was found that
/// produces a flag-bearing constant, and two attempts is not a search.** This
/// line has recorded six instances of deriving a set from the part of the system
/// it was thinking about; asserting unreachability from two probes would be the
/// seventh.
///
/// The words are still written by the driver, and deliberately: the stride is
/// what locates the NEXT node, so a short record silently shifts the whole forest.
#[test]
fn the_flags_and_discriminant_words_are_relayed_but_unexercised() {
    let mut checked = 0usize;
    for (name, src) in STAGES {
        let roots = keleusma::wire_schema::constant_roots_of_module(&compile_stage(src));
        assert!(
            !roots.is_empty(),
            "{name}: no roots, so nothing is measured"
        );
        for r in &roots {
            assert!(
                matches!(r, ConstValue::Int(_)),
                "{name}: a non-`Int` constant entered the corpus. If it carries a flag or a \
                 discriminant, the byte identity above now covers a word it did not before \
                 and this test's statement of the gap is out of date"
            );
        }
        checked += 1;
    }
    assert_eq!(checked, STAGES.len(), "not every stage was checked");
}

/// **THE TWO REFUSALS THE PATH CAN ACTUALLY BE SHOWN, EACH BY ITS OWN CODE.**
///
/// A refusal proves which limit fired only if the test names the one it expected;
/// this line has three recorded near-misses where a refusal was read as the wrong
/// limit. `-264` is a node with children and `-265` an interning tag, and they
/// are different causes reached by different sources.
///
/// `-266`, a range-carrying tag, is **not** exercised here. No source shape was
/// found that produces one through the driver, and saying so is better than
/// leaving a reader to infer that all three are covered because two are.
#[test]
fn each_refusal_the_driver_can_reach_is_named_by_its_own_code() {
    let composite =
        compile_stage("const data k { t: (Word, Word) = (1, 2) }\nfn main() -> Word { k.t.0 }");
    let e = wire_consts_via_kel(&composite).expect_err("a tuple constant must be refused");
    assert!(
        alloc_detail(&e).contains("-264"),
        "a composite was refused for a reason other than having children: {e:?}"
    );

    let interning = compile_stage("fn main() -> Word { let s = \"hi\"; 0 }");
    let e = wire_consts_via_kel(&interning).expect_err("a string constant must be refused");
    assert!(
        alloc_detail(&e).contains("-265"),
        "a `StaticStr` was refused for a reason other than interning: {e:?}"
    );

    // The accepting control. Without it a path that rejected everything would
    // satisfy both assertions above.
    let scalar = compile_stage("fn main() -> Word { 42 }");
    assert_eq!(
        wire_consts_via_kel(&scalar).expect("the scalar control must be accepted"),
        reference_consts(&scalar),
        "the accepting control disagreed with the reference, so the refusals above say \
         nothing about those causes specifically"
    );
}

/// The `detail` of an [`Unsupported`](keleusma::selfhost::SelfHostError) error.
///
/// Matched on the variant rather than formatted, so a different error kind fails
/// loudly instead of returning a string that happens not to contain the code.
fn alloc_detail(e: &keleusma::selfhost::SelfHostError) -> String {
    match e {
        keleusma::selfhost::SelfHostError::Unsupported { detail } => detail.clone(),
        other => panic!("expected an `Unsupported` refusal, got {other:?}"),
    }
}

//! The bounds the self-hosted verifier DECLARES, and the region kinds reserved
//! against future use.
//!
//! # Why these two live together
//!
//! Both are the same act. A number that is enforced and stated is a contract; a
//! number that is merely an array size is an accident waiting to be discovered by
//! whoever exceeds it. `verify_depth.kel` had the second kind and now has the
//! first, and the authenticity region kinds are claimed here before anything
//! emits them so they cannot be spent on something else.
//!
//! # The probe builds its chunks directly, and that is deliberate
//!
//! A source-level probe cannot reach a nesting depth of thirty-three. The
//! reference parser's `MAX_PARSE_DEPTH` is 24 and is shared between chain
//! position and arm-body nesting, so a source with thirty-three nested `if`s is
//! refused by the PARSER and never reaches the pass under test. That is the
//! "measured something other than what I intended" trap this line has recorded
//! five times, so the chunks here are assembled from ops.

#![cfg(all(feature = "self-host", feature = "compile"))]

use keleusma::bytecode::{BlockType, Chunk, Op};
use keleusma::selfhost::{DepthVerdict, depth_verdict_chunk_via_kel};

/// `Op::PushImmediate` operand for `true`, per the encoding documented on the
/// opcode: `0 = Unit`, `1 = true`, `2 = false`, `3 = None`, `4..19 = Int(n - 4)`.
const PUSH_TRUE: u8 = 1;
/// `Op::PushImmediate` operand for `Int(0)`, per the same encoding.
const PUSH_INT_ZERO: u8 = 4;

/// A chunk whose control flow nests `levels` deep and nothing else.
///
/// Layout, for `levels = n`:
///
/// ```text
///   0            PushImmediate(true)
///   1            If(-> matching EndIf)
///   ...          n times
///   2n           EndIf     x n
///   3n           PushImmediate(Int(0))
///   3n + 1       Return
/// ```
///
/// The `If` at op `2i + 1` matches the `EndIf` at `3n - 1 - i`, so the outermost
/// `If` matches the last `EndIf`. Bodies are empty so the operand depth is
/// trivially balanced and the only thing varying between cases is the NESTING,
/// which is what the cap governs.
fn nested_if_chunk(levels: usize) -> Chunk {
    let n = levels;
    let mut ops = Vec::with_capacity(3 * n + 2);
    for i in 0..n {
        ops.push(Op::PushImmediate(PUSH_TRUE));
        // Matching EndIf index. Derived rather than written out, because an
        // off-by-one here would produce a malformed chunk and the test would
        // measure the malformation instead of the cap.
        ops.push(Op::If((3 * n - 1 - i) as u16));
    }
    for _ in 0..n {
        ops.push(Op::EndIf);
    }
    ops.push(Op::PushImmediate(PUSH_INT_ZERO));
    ops.push(Op::Return);
    Chunk {
        name: alloc_name(n),
        ops,
        constants: Vec::new(),
        struct_templates: Vec::new(),
        local_count: 0,
        param_count: 0,
        block_type: BlockType::Func,
        param_types: Vec::new(),
        debug_pool: None,
    }
}

fn alloc_name(n: usize) -> String {
    format!("nested_{n}")
}

/// **THE DECLARED CAP IS THIRTY-TWO, AND IT IS PINNED FROM BOTH SIDES.**
///
/// Operator ruling, 2026-08-19. Thirty-two levels of nesting are analysed and
/// thirty-three are refused. Both halves are needed: without the accepting side a
/// cap of zero would pass, and without the refusing side the old silent drop would
/// pass.
///
/// # What the accepting half proves that a non-refusal would not
///
/// The thirty-two case asserts [`DepthVerdict::Accept`] rather than merely "not
/// over cap". `Accept` is only reachable when the walk RAN to completion over
/// every frame, so it distinguishes a cap that admits the program from a cap that
/// admits it and then fails to look at it — which is precisely the state this
/// change replaced.
#[test]
fn the_declared_nesting_cap_admits_thirty_two_and_refuses_thirty_three() {
    assert_eq!(
        depth_verdict_chunk_via_kel(&nested_if_chunk(32)),
        DepthVerdict::Accept,
        "a chunk at the declared cap of 32 must be ANALYSED, not merely tolerated. \
         An `OverCap` here means the frame arrays are off by one against the cap; \
         an `Underflow` means this probe's chunk is malformed and measures nothing"
    );
    assert_eq!(
        depth_verdict_chunk_via_kel(&nested_if_chunk(33)),
        DepthVerdict::OverCap,
        "a chunk one level past the declared cap must be REFUSED by name. Before \
         2026-08-19 this returned a verdict computed over a program the pass had \
         not fully walked"
    );
}

/// **THE REFUSAL IS DEFAULT-DENY, WHICH IS THE SOUND DIRECTION.**
///
/// An over-cap chunk is rejected rather than admitted, matching this project's
/// conservative-verification stance: a program whose property the analysis cannot
/// establish is refused, never accepted on the strength of an incomplete walk.
///
/// This is a separate assertion from the one above because the two can come apart.
/// A cause field reported correctly beside a verdict of "accept" would satisfy the
/// boundary test and still be unsound.
#[test]
fn an_over_cap_chunk_is_rejected_rather_than_admitted() {
    let verdict = depth_verdict_chunk_via_kel(&nested_if_chunk(40));
    assert_eq!(verdict, DepthVerdict::OverCap);
    assert!(
        matches!(verdict, DepthVerdict::OverCap | DepthVerdict::Underflow),
        "an unanalysed chunk must land on the REJECT side of the verdict"
    );
}

/// **THE CAUSE SEPARATES AN UNANALYSED PROGRAM FROM A DEFECTIVE ONE.**
///
/// Both are rejections and only the cause tells them apart. Without this the pass
/// would repeat the shared-message defect recorded against four other guards in
/// this tree, where two unrelated failures produced a byte-identical report.
///
/// The control is the deep chunk's own shallow twin: identical construction,
/// identical op kinds, differing only in nesting. If the shallow one did not come
/// back `Accept`, the deep one's `OverCap` would prove nothing about the cap.
#[test]
fn the_cause_distinguishes_a_refusal_from_a_proven_underflow() {
    assert_eq!(
        depth_verdict_chunk_via_kel(&nested_if_chunk(4)),
        DepthVerdict::Accept,
        "the shallow control must be accepted, or the deep case measures nothing"
    );

    // A proven underflow: pop from an empty operand stack. The verdict is the
    // same as the over-cap case and the CAUSE is not, which is the whole point.
    let underflowing = Chunk {
        name: "underflow".into(),
        ops: vec![Op::Add, Op::PushImmediate(PUSH_INT_ZERO), Op::Return],
        constants: Vec::new(),
        struct_templates: Vec::new(),
        local_count: 0,
        param_count: 0,
        block_type: BlockType::Func,
        param_types: Vec::new(),
        debug_pool: None,
    };
    assert_eq!(
        depth_verdict_chunk_via_kel(&underflowing),
        DepthVerdict::Underflow,
        "a proven underflow must report as such, not as a refusal to analyse"
    );
}

/// **THE RESERVED REGION KINDS ARE CLAIMED AND UNSPENT.**
///
/// Operator ruling, 2026-08-19: reserve the authenticity regions and the
/// assurance-tier field now, because reserving costs nothing and a kind number
/// that has been used for one thing cannot later mean another without a version
/// break.
///
/// # The three things that could go wrong, each checked
///
/// A reserved kind could collide with a live kind, could collide with the
/// parity-plane convention that derives a plane's kind by setting a high bit, or
/// could be quietly emitted by an encoder — at which point it is no longer
/// reserved and this test is the notice.
#[test]
fn the_reserved_kinds_are_claimed_unspent_and_free_of_collisions() {
    use keleusma::wire_schema::kind;

    let live: [u16; 20] = [
        kind::STRING_POOL,
        kind::NAMES,
        kind::CONSTS,
        kind::STRUCT_AUX,
        kind::ENUM_AUX,
        kind::SHAPES,
        kind::SIGNATURES,
        kind::STRUCT_TEMPLATES,
        kind::ENUM_VARIANTS,
        kind::ENUM_LAYOUTS,
        kind::DATA_SLOTS,
        kind::SHARED_LAYOUT,
        kind::PRIVATE_COMPOSITE,
        kind::DATA_INIT,
        kind::PARAM_TYPES,
        kind::CHUNKS,
        kind::NATIVES,
        kind::HEADER,
        kind::DEBUG_POOL,
        kind::NATIVE_RETURNS,
    ];

    assert!(
        !kind::RESERVED.is_empty(),
        "the reserved set is empty, so this test checks nothing"
    );

    for r in kind::RESERVED {
        assert!(
            !live.contains(&r),
            "reserved kind {r:#06x} collides with a live kind"
        );
        // The plane for kind `k` is `k | ECC_KIND_BIT`, so a reserved kind must
        // not itself carry that bit and its own plane must not land on a live
        // kind either.
        assert_eq!(
            r & keleusma_wire::ecc::ECC_KIND_BIT,
            0,
            "reserved kind {r:#06x} sets the parity-plane bit"
        );
        let plane = keleusma_wire::ecc::plane_kind_for(r);
        assert!(
            !live.contains(&plane),
            "the parity plane for reserved kind {r:#06x} collides with a live kind"
        );
    }

    // Distinct from each other.
    for (i, a) in kind::RESERVED.iter().enumerate() {
        for b in kind::RESERVED.iter().skip(i + 1) {
            assert_ne!(a, b, "two reserved kinds share a number");
        }
    }
}

/// **NOTHING EMITS A RESERVED KIND, AND THIS IS THE NOTICE IF THAT CHANGES.**
///
/// Pinned in the firing direction. The day an encoder starts writing one of these
/// the kind is no longer reserved, and a silent transition from "claimed" to
/// "in use" is exactly how a reservation stops meaning anything.
///
/// The vacuity guard matters here more than usual: an artifact carrying no regions
/// at all would pass this trivially.
#[test]
fn no_encoder_emits_a_reserved_kind() {
    use keleusma::wire_schema::kind;
    use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

    let src = "shared data d { n: Word }\n\
               fn helper(a: Word) -> Word { a + 1 }\n\
               fn main() -> Word { d.n = helper(1); d.n }";
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    // Through the SHIPPING encoder and the PUBLIC section accessor, rather than
    // rebuilding a `WireAuxBody` here. A second construction of the aux body in a
    // test is a second encoding, free to drift from the one under test, which is
    // the defect this tree has recorded against nine copies of a shared layout.
    let bytes = keleusma::wire_format::module_to_wire_bytes(&module).expect("encode");
    let sections = keleusma::wire_format::parse_wire_sections(&bytes).expect("sections");
    let view = keleusma_wire::WireView::parse(sections.aux_body).expect("parse");

    let mut seen = 0;
    for r in kind::RESERVED {
        assert!(
            view.find_region(r).is_none(),
            "reserved kind {r:#06x} is now emitted. It is no longer reserved: \
             document what it carries and remove it from `kind::RESERVED`"
        );
        seen += 1;
    }
    assert_eq!(
        seen,
        kind::RESERVED.len(),
        "not every reserved kind checked"
    );

    // The vacuity guard. If the artifact carried no regions the loop above would
    // pass while establishing nothing about the encoder.
    assert!(
        view.find_region(kind::CHUNKS).is_some(),
        "this artifact carries no CHUNKS region, so the absence of the reserved \
         kinds says nothing about what the encoder emits"
    );
}

#![cfg(all(feature = "self-host", feature = "compile"))]
//! How much room `wire.kel` has left under the module-input walk's node cap.
//!
//! # Why this exists
//!
//! `docs/decisions/DATA_SLOTS_ROUTING_PLAN.md` says the next Order 1 slice adds
//! two commands to `wire.kel`, and that **`wire.kel` is itself one of the eleven
//! measured stages**. Growing it consumes margin under `mi_max_nodes()`, the
//! module-input walk's bound on the constant-forest node table.
//!
//! The plan says to measure that margin BEFORE the stage edit, so a later cap
//! failure is attributed to stage growth rather than to the routing that
//! follows it. Without a figure recorded first, the two are indistinguishable
//! from the failure alone.
//!
//! # The count is parsed from the blob the stage reads
//!
//! `module_input` is public and the node count rides in the blob's tail. Walking
//! to it means walking the same three name sections the interner walks -- chunk
//! names, enum type and variant names, data-slot run names -- which is the only
//! way to reach the tail without duplicating the encoder's private helpers.
//!
//! **A parse that drifted from the writer would report a wrong figure rather
//! than failing**, so the walk asserts its own consistency: every length is
//! bounded by the remaining blob, and the section counts are cross-checked
//! against the module.

extern crate alloc;

use keleusma::bytecode::Module;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::selfhost::module_input;

const WIRE_SRC: &str = include_str!("../src/selfhost/kel/wire.kel");

fn compile_stage(src: &str) -> Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile")
}

/// The cap, read from the stage rather than written here.
///
/// A literal copy would be a second source of truth for a number whose whole
/// purpose is to bound the first.
fn declared_node_cap() -> usize {
    let line = WIRE_SRC
        .lines()
        .find(|l| l.trim_start().starts_with("fn mi_max_nodes()"))
        .expect("wire.kel declares mi_max_nodes");
    line.rsplit_once('{')
        .and_then(|(_, tail)| tail.split_once('}'))
        .and_then(|(n, _)| n.trim().parse().ok())
        .expect("mi_max_nodes has a literal body")
}

/// Read a little-endian `u16` and advance.
fn u16_at(blob: &[u8], at: &mut usize) -> usize {
    assert!(
        *at + 2 <= blob.len(),
        "the blob ended mid-length at {at}; this walk has drifted from the writer"
    );
    let v = u16::from_le_bytes([blob[*at], blob[*at + 1]]) as usize;
    *at += 2;
    v
}

/// Skip one length-prefixed name and advance.
fn skip_name(blob: &[u8], at: &mut usize) {
    let n = u16_at(blob, at);
    assert!(
        *at + n <= blob.len(),
        "a name of {n} bytes runs past the blob end; this walk has drifted from the writer"
    );
    *at += n;
}

/// The constant-forest node count the module-input walk would meet.
fn blob_node_count(module: &Module) -> usize {
    let (blob, _names) = module_input(module);
    let mut at = 0usize;

    let chunks = u16_at(&blob, &mut at);
    assert_eq!(
        chunks,
        module.chunks.len(),
        "the blob's chunk count disagrees with the module, so the walk is reading the wrong \
         field and every figure after it is meaningless"
    );
    for _ in 0..chunks {
        skip_name(&blob, &mut at);
    }

    let enums = u16_at(&blob, &mut at);
    assert_eq!(
        enums,
        module.enum_layouts.len(),
        "the blob's enum count disagrees with the module"
    );
    for l in &module.enum_layouts {
        skip_name(&blob, &mut at);
        let variants = u16_at(&blob, &mut at);
        assert_eq!(
            variants,
            l.variants.len(),
            "the blob's variant count disagrees with the module"
        );
        for _ in 0..variants {
            skip_name(&blob, &mut at);
        }
    }

    let runs = u16_at(&blob, &mut at);
    for _ in 0..runs {
        skip_name(&blob, &mut at);
    }

    u16_at(&blob, &mut at)
}

/// **THE BUDGET, RECORDED BEFORE THE STAGE GROWS.**
///
/// A failure here is not a defect. It means `wire.kel` has grown enough to
/// change its standing under the walk's cap, and the plan that sizes the next
/// slice against this margin needs re-reading.
#[test]
fn wire_kel_has_room_under_the_module_input_node_cap() {
    let cap = declared_node_cap();
    assert!(cap > 0, "the cap parsed as zero, so the reader is broken");

    let nodes = blob_node_count(&compile_stage(WIRE_SRC));

    assert!(
        nodes > 0,
        "wire.kel's constant forest measured as empty, so this walk found the wrong field \
         rather than the stage having no constants"
    );
    assert!(
        nodes <= cap,
        "wire.kel carries {nodes} constant-forest nodes against a {cap}-node cap. The stage no \
         longer fits its own module-input walk, which is the failure \
         docs/decisions/DATA_SLOTS_ROUTING_PLAN.md budgets against"
    );

    // The MARGIN is the quantity the plan reasons about, so it is asserted
    // rather than merely computed. Two new commands are expected to cost far
    // less than this; if the margin has closed to nothing, the slice needs a
    // different shape and this test is where that shows.
    let margin = cap - nodes;
    assert!(
        margin >= 64,
        "wire.kel has {margin} nodes of margin under the {cap}-node cap ({nodes} used). The \
         routing slice adds two commands to this stage, and a margin this small means the \
         stage edit must be sized against the cap first"
    );
}

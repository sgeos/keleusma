//! **A refusal's class is carried by its type, not by its word order.**
//!
//! `LowerError::UnsupportedOp` was a `String` constructed at 31 sites carrying
//! four unrelated conditions: an opcode with no lowering, a type this backend
//! lacks, an input whose own integrity failed, and a defect in this crate. The
//! only thing distinguishing them was the English in the message.
//!
//! That was load-bearing. `isa_lowering_census` built its NAMED REFUSED column
//! by taking the leading alphanumeric run of the sentence and keeping it when it
//! matched an ISA opcode name, so `Const(60000) out of range` — a malformed
//! constant INDEX — was credited to the `Const` OPCODE, which this backend
//! lowers in nearly every module of the corpus. The demonstration lives in
//! `isa_lowering_census::a_non_opcode_refusal_must_not_be_attributed_to_an_opcode`.
//!
//! **The corpus never fired a misattributing site**, so every published figure
//! was correct. The column was clean because of what the corpus happens to
//! contain, not because the query could not go wrong — the distinction between
//! a guard that holds and a guard that was never reached.
//!
//! These tests fix the classes in place so the conflation cannot return.

use keleusma::bytecode::Op;
use keleusma_native::{LowerError, LowerOptions, module_lowered_op_indices};
mod common;

use std::collections::BTreeSet;

fn compiled(src: &str) -> Option<keleusma::bytecode::Module> {
    keleusma::lexer::tokenize(src)
        .ok()
        .and_then(|t| keleusma::parser::parse(&t).ok())
        .and_then(|a| keleusma::compiler::compile(&a).ok())
}

/// Every refusal a module produces, as `(class label, rendered sentence)`.
fn refusals_of(m: &keleusma::bytecode::Module) -> Vec<(&'static str, String)> {
    module_lowered_op_indices(m, LowerOptions::default())
        .0
        .iter()
        .map(|(_, e)| {
            let label = match e {
                LowerError::UnsupportedOp { .. } => "UnsupportedOp",
                LowerError::UnsupportedShape(_) => "UnsupportedShape",
                LowerError::MalformedInput(_) => "MalformedInput",
                LowerError::Internal(_) => "Internal",
                _ => "other",
            };
            (label, format!("{e}"))
        })
        .collect()
}

/// The declared ISA, read from the crate source rather than listed here, so a
/// new opcode cannot silently fall outside this test's notion of the set.
fn declared_isa() -> BTreeSet<String> {
    let src = std::fs::read_to_string("../src/bytecode.rs").expect("read bytecode.rs");
    let mut set = BTreeSet::new();
    let mut in_enum = false;
    for line in src.lines() {
        let t = line.trim();
        if t.starts_with("pub enum Op") {
            in_enum = true;
            continue;
        }
        if in_enum {
            if t == "}" {
                break;
            }
            let name: String = t
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();
            if !name.is_empty() && name.starts_with(|c: char| c.is_ascii_uppercase()) {
                set.insert(name);
            }
        }
    }
    assert!(
        set.len() > 40,
        "extracted only {} opcodes, so this test's ISA notion is broken and every \
         assertion below would be weak rather than failing",
        set.len()
    );
    set
}

/// **A malformed operand is not a claim about the opcode's lowerability.**
///
/// An out-of-range constant index is a property of the input. Reporting it as
/// "does not yet support opcode Const" tells a user to stop using constants,
/// when the backend lowers them throughout the corpus.
#[test]
fn a_malformed_operand_is_classified_as_malformed_input() {
    let mut m = compiled("fn main() -> Word { 0 }").expect("probe compiles");
    m.chunks[0].ops.insert(0, Op::Const(60_000));

    let seen = refusals_of(&m);
    assert!(
        !seen.is_empty(),
        "the injected out-of-range constant produced NO refusal, so this test \
         would pass without exercising the classification at all"
    );
    assert!(
        seen.iter().all(|(c, _)| *c == "MalformedInput"),
        "an out-of-range constant INDEX was not classified as malformed input: \
         {seen:?}"
    );
    assert!(
        seen.iter().all(|(_, s)| !s.contains("support opcode")),
        "a malformed operand still renders as an unsupported opcode: {seen:?}"
    );
}

/// **An opcode with no lowering IS a claim about the opcode, and names it.**
///
/// The must-fire counterpart. Without it the test above would pass under a
/// classification that called everything malformed input.
#[test]
fn an_unlowerable_opcode_is_classified_as_such_and_names_itself() {
    let isa = declared_isa();
    let mut m = compiled("fn main() -> Word { 0 }").expect("probe compiles");
    // Far above any supported word width, so it is out of range under every
    // narrow-word configuration rather than only the default one.
    m.chunks[0].ops.insert(0, Op::FixedMul(200));

    let seen = refusals_of(&m);
    assert!(
        seen.iter().any(|(c, _)| *c == "UnsupportedOp"),
        "an unsupported FixedMul produced no UnsupportedOp: {seen:?}"
    );
    let all = module_lowered_op_indices(&m, LowerOptions::default());
    let named: Vec<&String> = all
        .0
        .iter()
        .filter_map(|(_, e)| match e {
            LowerError::UnsupportedOp { op, .. } => Some(op),
            _ => None,
        })
        .collect();
    assert!(
        named.iter().any(|o| *o == "FixedMul"),
        "the refusal did not carry `FixedMul` as data, so the census would have \
         to recover it from prose again. Named: {named:?}"
    );
    assert!(
        named.iter().all(|o| isa.contains(*o)),
        "an UnsupportedOp named something that is not a declared opcode: {named:?}"
    );
}

/// **A float is a property of a type, not of an opcode.**
///
/// This is the refusal that rendered as "native lowering does not yet support
/// opcode chunk 0 has a Float in its signature" — a sentence naming an opcode
/// called `chunk`.
///
/// **The subject changed once.** It was a float chunk signature, which the
/// entry ABI then opened, so no refusal fires there any more. The subject is
/// now route 3, a native declaring a Float RETURN SHAPE — the one float
/// refusal still reachable by compiling a program in this build, uncalled so
/// that no other route can fire first (the isolation argument is in
/// `float_guard_routes.rs`).
#[test]
fn a_float_is_classified_as_an_unsupported_shape() {
    let m = compiled("use host::read_temp() -> Float\n\nfn main() -> Word { 0 }")
        .expect("a float-returning native must still COMPILE; the guard is in the backend");
    assert!(
        {
            // The float scalar's wire tag. Debug renders `Scalar { kind: 5 }`,
            // so a text search for "Float" finds nothing and would make this
            // premise check pass by never looking.
            const FLOAT_TAG: u8 = 5;
            let is_float = |w: &keleusma::bytecode::WireShape| matches!(w, keleusma::bytecode::WireShape::Scalar { kind } if *kind == FLOAT_TAG);
            m.native_return_shapes.iter().any(is_float)
        },
        "no Float reached the module's native return shapes, so the float route \
         cannot fire and this test would pass without testing anything"
    );

    let seen = refusals_of(&m);
    assert!(
        seen.iter().any(|(c, _)| *c == "UnsupportedShape"),
        "a Float was not classified as an unsupported shape: {seen:?}"
    );
    assert!(
        seen.iter()
            .all(|(_, s)| !s.contains("support opcode chunk")),
        "the float refusal still renders as an opcode named `chunk`: {seen:?}"
    );
}

/// **Nothing the corpus produces claims an opcode that does not exist.**
///
/// The sweep, rather than the three constructed subjects. If any refusal over
/// the shipped corpus renders "does not yet support opcode X" for an X outside
/// the instruction set, the sentence is describing something that is not an
/// opcode.
#[test]
fn no_corpus_refusal_names_a_nonexistent_opcode() {
    let isa = declared_isa();
    // **The one canonical walk.** This sweep previously carried its own copy —
    // and an earlier version of it was non-recursive, seeing 35 modules where
    // its consumers saw 74. Sharing the enumeration makes that impossible here
    // rather than merely unlikely.
    let sources = common::corpus_sources();

    let mut checked = 0usize;
    let mut refusals = 0usize;
    {
        for p in &sources {
            let Ok(src) = std::fs::read_to_string(p) else {
                continue;
            };
            let Some(m) = compiled(&src) else { continue };
            checked += 1;
            for (_, e) in module_lowered_op_indices(&m, LowerOptions::default()).0 {
                refusals += 1;
                if let LowerError::UnsupportedOp { op, .. } = &e {
                    assert!(
                        isa.contains(op),
                        "{}: a refusal names opcode `{op}`, which is not in the \
                         instruction set",
                        p.display()
                    );
                }
                let s = format!("{e}");
                assert!(
                    !s.contains("support opcode chunk") && !s.contains("support opcode native"),
                    "{}: refusal renders a non-opcode as an opcode: {s}",
                    p.display()
                );
            }
        }
    }
    assert!(
        checked > 40,
        "only {checked} corpus modules compiled, so this sweep is far narrower \
         than the population the censuses read"
    );
    assert!(
        refusals > 0,
        "the sweep observed ZERO refusals across {checked} modules, so every \
         assertion above was vacuous"
    );
    println!("\n================ REFUSAL SENTENCE SWEEP");
    println!("  corpus modules compiled : {checked}");
    println!("  refusals rendered       : {refusals}");
    println!("  each names a real opcode or does not claim to name one");
    println!("================\n");
}

/// **The `Internal` class was NOT fired, and this records the search.**
///
/// Its three sites are reached only when this crate's own invariants break: a
/// `Call` surviving to `lower_chunk` that `lower_module` should have resolved, a
/// `NewComposite` without the region pointer, and a `NewComposite` the region
/// planner did not place. Firing one would require constructing a state the
/// crate treats as impossible.
///
/// **That is a fact about this search, not a proof of unreachability.** What is
/// asserted is only what can be: the class exists, is distinct, and renders as a
/// defect in this crate rather than as an unimplemented feature — so a consumer
/// reaching one is not told to rewrite a program that was never the problem.
#[test]
fn the_internal_class_renders_as_a_defect_rather_than_a_missing_feature() {
    let e = LowerError::Internal("the region planner placed no site".into());
    let s = format!("{e}");
    assert!(
        s.contains("defect in this crate"),
        "an internal invariant violation does not say so: {s}"
    );
    assert!(
        !s.contains("does not yet support"),
        "an internal defect renders as an unimplemented feature, which tells a \
         user to rewrite a program that was never the problem: {s}"
    );
    assert!(
        !matches!(e, LowerError::UnsupportedOp { .. }),
        "the internal class is not distinguishable from an opcode refusal"
    );
}

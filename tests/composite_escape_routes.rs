#![cfg(all(feature = "compile", feature = "verify"))]
//! Every way a composite can outlive the loop iteration that built it, ENUMERATED
//! FROM THE INSTRUCTION SET rather than from imagination.
//!
//! # What this is for
//!
//! `docs/proofs/COMPOSITE_REGION_REUSE.md` §6.3 on the `v0.3.0` line asks whether
//! `yield` is the only route by which a composite escapes its iteration, and
//! warns that **one survivor makes the restriction unsound rather than
//! incomplete**. An enumeration produced by listing the routes one happens to
//! think of cannot answer that. This one starts from the 66 opcodes and
//! classifies every single one, so a route can only be missed by a
//! MISCLASSIFICATION, never by an omission — and a new opcode fails the test
//! rather than slipping through unclassified.
//!
//! # The question each opcode is classified against
//!
//! *Can this instruction make a composite body readable after the iteration that
//! constructed it has ended, by ALIASING the body's arena region rather than
//! copying its bytes?*
//!
//! Aliasing is the operative word. The proof is about giving one static slot to a
//! construction site, so what matters is whether a second reader reaches the same
//! ADDRESS — not whether a value merely lives longer.
//!
//! # What this test does NOT establish
//!
//! **The classification of each individual opcode is analysis, not proof.** What
//! is mechanically guaranteed here is that the classification is TOTAL over the
//! instruction set and stays total as the set changes. Three entries are backed
//! by execution — see the tests below and `tests/composite_escape_window.rs` —
//! and the rest by reading the virtual machine's dispatch.
//!
//! **`CallExternalNative` is a trust boundary, not a route this line can close.**
//! A host native receives a composite and what it does with it is the host's
//! affair. It is classified as escaping because it must be assumed to be.

use std::collections::BTreeMap;

/// What an opcode can do to a composite's region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Route {
    /// Cannot hold a composite operand at all, or consumes one and produces a
    /// scalar. No region outlives anything through it.
    NoRegion,
    /// Moves a composite only within the iteration's own dataflow: the operand
    /// stack, or a frame slot that dies with the iteration.
    WithinIteration,
    /// Moves a composite outward but COPIES the bytes, so the destination does
    /// not alias the source region.
    CopiesOut,
    /// Makes the constructed body itself readable beyond the iteration.
    Escapes,
}

/// The classification. Every one of the instruction set's opcodes appears
/// exactly once; `the_classification_is_total` proves it against the enum.
fn classification() -> BTreeMap<&'static str, Route> {
    use Route::*;
    [
        // --- Escaping routes. THIS IS THE ANSWER TO §6.3. ---
        //
        // The host receives the `Value`, which carries the arena handle rather
        // than a copy. Verified by execution in `composite_escape_window.rs`.
        ("Yield", Escapes),
        // A frame slot whose binding was declared OUTSIDE the loop keeps the
        // handle after the iteration ends. The opcode cannot distinguish an
        // inner binding from an outer one — that is a property of the slot the
        // compiler assigned — so the opcode must be classified by its worst case.
        ("SetLocal", Escapes),
        // Hands the value to the caller's frame, or to the host at chunk exit.
        // A `return` from inside a loop exits that loop, so the returning
        // iteration is the last one; but a callee INVOKED from the loop body
        // returning a composite it built is a different case and this covers it.
        ("Return", Escapes),
        // A native receives the composite. What it retains is the host's affair.
        // Not closable from this side; assumed hostile.
        ("CallExternalNative", Escapes),
        ("CallVerifiedNative", Escapes),
        // --- Copying routes: outward, but not aliasing. ---
        //
        // `write_data_slot` packs a flat body into the persistent composite pool
        // at its baked offset, so no ephemeral handle is stored. Verified by
        // execution below: the value survives resets that reclaim its region.
        ("SetData", CopiesOut),
        ("SetDataIndexed", CopiesOut),
        // The flat path packs operands directly into the new allocation,
        // resolving any nested arena child IN PLACE. Verified by execution
        // below: the nested body's words appear inline in the parent's bytes.
        // The BOXED path stores operands as separate values and therefore does
        // alias; it is unreachable for the transitively-scalar composites this
        // proof concerns, and that limit is stated rather than assumed away.
        ("NewComposite", CopiesOut),
        // --- Within the iteration only. ---
        ("Call", WithinIteration),
        ("Dup", WithinIteration),
        ("GetLocal", WithinIteration),
        ("GetData", WithinIteration),
        ("GetDataIndexed", WithinIteration),
        ("PopN", WithinIteration),
        // Projections. A scalar field copies its word out; a NESTED composite
        // field yields a view onto the parent's bytes, which aliases the parent
        // rather than creating a new escape. Either way the projection cannot
        // outlive the iteration on its own — it needs one of the escaping
        // opcodes above to go anywhere, and those are already counted.
        ("GetField", WithinIteration),
        ("GetIndex", WithinIteration),
        ("GetTupleField", WithinIteration),
        ("GetEnumField", WithinIteration),
        // --- No region can leave through these. ---
        ("Const", NoRegion),
        ("PushImmediate", NoRegion),
        ("BoundsCheck", NoRegion),
        ("Add", NoRegion),
        ("Sub", NoRegion),
        ("Mul", NoRegion),
        ("Div", NoRegion),
        ("Mod", NoRegion),
        ("Neg", NoRegion),
        // Comparisons admit composite operands and produce a boolean.
        ("CmpEq", NoRegion),
        ("CmpNe", NoRegion),
        ("CmpLt", NoRegion),
        ("CmpGt", NoRegion),
        ("CmpLe", NoRegion),
        ("CmpGe", NoRegion),
        ("Not", NoRegion),
        ("If", NoRegion),
        ("Else", NoRegion),
        ("EndIf", NoRegion),
        ("Loop", NoRegion),
        ("EndLoop", NoRegion),
        ("Break", NoRegion),
        ("BreakIf", NoRegion),
        ("Stream", NoRegion),
        // Reset ENDS the window rather than opening one: it reclaims the
        // ephemeral region and advances the epoch, which is what makes every
        // outstanding handle Stale.
        ("Reset", NoRegion),
        // Consume a composite, produce a scalar.
        ("Len", NoRegion),
        ("IsEnum", NoRegion),
        ("IsStruct", NoRegion),
        ("IntToFloat", NoRegion),
        ("FloatToInt", NoRegion),
        ("WordToByte", NoRegion),
        ("ByteToWord", NoRegion),
        ("WordToFixed", NoRegion),
        ("FixedToWord", NoRegion),
        ("FixedMul", NoRegion),
        ("FixedDiv", NoRegion),
        ("Trap", NoRegion),
        ("CheckedAdd", NoRegion),
        ("CheckedSub", NoRegion),
        ("CheckedMul", NoRegion),
        ("CheckedNeg", NoRegion),
        ("CheckedDiv", NoRegion),
        ("CheckedMod", NoRegion),
        ("BitAnd", NoRegion),
        ("BitOr", NoRegion),
        ("BitXor", NoRegion),
        ("Shl", NoRegion),
        ("Shr", NoRegion),
    ]
    .into_iter()
    .collect()
}

/// The opcode names, read out of the instruction set's own definition.
///
/// Derived rather than restated. A list written by hand beside the enum is a
/// property of the list, and this repository has paid for that distinction
/// repeatedly.
fn opcode_names() -> Vec<String> {
    let src = include_str!("../src/bytecode.rs");
    let start = src
        .find("pub enum Op {")
        .expect("src/bytecode.rs declares `pub enum Op`");
    let body = &src[start..];
    let end = body.find("\n}").expect("the enum closes");
    let mut names = Vec::new();
    for line in body[..end].lines().skip(1) {
        let t = line.trim();
        if t.is_empty() || t.starts_with("//") || t.starts_with("#[") {
            continue;
        }
        let Some(cut) = t.find(['(', ',']) else {
            continue;
        };
        let ident = &t[..cut];
        if !ident.is_empty()
            && ident.starts_with(|c: char| c.is_ascii_uppercase())
            && ident.chars().all(|c| c.is_ascii_alphanumeric())
        {
            names.push(ident.to_string());
        }
    }
    names
}

#[test]
fn the_classification_is_total_over_the_instruction_set() {
    let names = opcode_names();
    assert_eq!(
        names.len(),
        66,
        "the instruction set is no longer 66 opcodes. That is not a failure of \
         this test; it means the escape enumeration must be revisited, which is \
         exactly what it is here to force."
    );

    let table = classification();
    let missing: Vec<&String> = names
        .iter()
        .filter(|n| !table.contains_key(n.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "these opcodes are unclassified: {missing:?}. An enumeration with a hole \
         cannot answer whether the escape set is exhaustive, and the proof's §6.3 \
         warns that one survivor makes the restriction UNSOUND rather than \
         incomplete."
    );

    let extra: Vec<&&str> = table
        .keys()
        .filter(|k| !names.iter().any(|n| n == *k))
        .collect();
    assert!(
        extra.is_empty(),
        "these classified names are not opcodes: {extra:?}. A stale entry means \
         the table was maintained against memory rather than the enum."
    );
}

#[test]
fn the_escaping_set_is_exactly_the_five_recorded_routes() {
    let table = classification();
    let escapes: Vec<&str> = table
        .iter()
        .filter(|(_, r)| **r == Route::Escapes)
        .map(|(k, _)| *k)
        .collect();

    assert_eq!(
        escapes,
        vec![
            "CallExternalNative",
            "CallVerifiedNative",
            "Return",
            "SetLocal",
            "Yield",
        ],
        "the escaping set moved. Every addition or removal changes what a \
         reuse restriction must exclude, so it is pinned rather than described."
    );

    // Non-vacuity: every category must be populated, or the classification has
    // collapsed into one bucket and the totality check above proves nothing
    // interesting.
    for route in [
        Route::NoRegion,
        Route::WithinIteration,
        Route::CopiesOut,
        Route::Escapes,
    ] {
        assert!(
            table.values().any(|r| *r == route),
            "no opcode is classified {route:?}; the partition has collapsed"
        );
    }
}

// ---------------------------------------------------------------------------
// The two `CopiesOut` entries, backed by execution rather than by reading the
// dispatch. They are the load-bearing SAFE claims: if either is wrong, a
// restriction built on this enumeration is unsound rather than merely loose.
// ---------------------------------------------------------------------------

fn compile(src: &str) -> keleusma::bytecode::Module {
    keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(src).expect("lex")).expect("parse"),
    )
    .expect("compile")
}

/// A composite written to a private data slot survives resets that reclaim the
/// region it was built in.
///
/// **This is the discriminator, and it is sharp.** If the slot stored the
/// ephemeral arena handle, the first `Reset` would advance the epoch and the
/// read would fail `Stale`. It reads correctly across two of them, so the slot
/// holds a COPY in the persistent region.
#[test]
fn a_composite_written_to_private_data_is_copied_not_aliased() {
    const SRC: &str = "\
struct P { a: Word, b: Word, c: Word }
private data store { kept: P, seen: Word }
loop main(t: Word) -> Word {
    if store.seen == 0 {
        store.kept = P { a: 42, b: 43, c: 44 };
        store.seen = 1;
    }
    let _ = yield store.kept.a;
    0
}
";
    use keleusma::bytecode::GenericValue as Value;
    use keleusma::vm::{Vm, VmState};

    let module = compile(SRC);
    let need = keleusma::vm::required_persistent_capacity_for(&module);
    let mut arena = keleusma_arena::Arena::with_capacity((1 << 16) + need);
    arena.resize_persistent(need).expect("persistent region");
    let mut vm = Vm::new(module, &arena).expect("verify");
    let mut shared = vec![0u8; vm.shared_data_bytes()];

    let mut reads = Vec::new();
    let mut resets = 0;
    let mut state = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call");
    let mut epochs = Vec::new();
    for _ in 0..5 {
        epochs.push(vm.arena().epoch());
        match &state {
            VmState::Yielded(Value::Int(n)) => reads.push(*n),
            VmState::Reset => resets += 1,
            other => panic!("unexpected state {other:?}"),
        }
        state = vm
            .resume_with_shared(&mut shared, Value::Int(0))
            .expect("resume");
    }

    assert!(
        resets >= 2,
        "the run must cross at least two resets, or it never reclaims the region \
         the composite was built in and proves nothing about copying"
    );
    assert!(
        epochs.last() > epochs.first(),
        "the epoch never advanced, so no region was ever reclaimed: {epochs:?}"
    );
    assert!(
        reads.iter().all(|n| *n == 42),
        "the field read back differently after a reset: {reads:?}. A slot holding \
         a handle rather than a copy would fail here, which is the point."
    );
    assert!(
        reads.len() >= 2,
        "at least two reads, one of them post-reset"
    );
}

/// Nesting a composite into a flat composite packs its bytes inline.
///
/// The parent's body is its own allocation containing the child's words, not a
/// handle to the child's. So a nested construction does not alias the child's
/// region and reusing the child's slot would not corrupt the parent.
///
/// **Stated with its limit**: this is the FLAT path. The boxed path stores
/// operands as separate values and does alias. Boxed bodies do not arise for the
/// transitively-scalar composites this enumeration concerns, and saying so is
/// better than a claim that reads as universal.
#[test]
fn nesting_a_composite_into_a_flat_one_copies_its_bytes_inline() {
    const SRC: &str = "\
struct Inner { x: Word, y: Word }
struct Outer { i: Inner, z: Word }
loop main(t: Word) -> Outer {
    let inner = Inner { x: 11, y: 22 };
    let outer = Outer { i: inner, z: 33 };
    let _ = yield outer;
    Outer { i: Inner { x: 0, y: 0 }, z: 0 }
}
";
    use keleusma::bytecode::{GenericValue as Value, StructBody};
    use keleusma::vm::{Vm, VmState};

    let module = compile(SRC);
    let arena = keleusma_arena::Arena::with_capacity(1 << 16);
    let mut vm = Vm::new(module, &arena).expect("verify");

    let state = vm.call(&[Value::Int(0)]).expect("call");
    let VmState::Yielded(Value::Struct(StructBody::Flat(body))) = &state else {
        panic!("expected a flat struct, got {state:?}");
    };
    let bytes = body.resolve(vm.arena()).expect("resolves");

    let words: Vec<i64> = bytes
        .as_chunks::<8>()
        .0
        .iter()
        .map(|c| i64::from_le_bytes(*c))
        .collect();
    assert_eq!(
        words,
        vec![11, 22, 33],
        "the outer composite does not contain the inner's words inline, so the \
         nesting is by reference and this entry's `CopiesOut` classification is \
         wrong"
    );
    assert_eq!(
        bytes.len(),
        24,
        "three words packed contiguously; a handle-carrying body would not be \
         this size"
    );
}

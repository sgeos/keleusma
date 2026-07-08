#![cfg(all(feature = "compile", feature = "verify"))]
//! A.2.1 typed operand-stack pass — conformance corpus (Phase 5).
//!
//! Hostile-input tests mirroring the audit findings the typed pass closes.
//! Each case compiles a valid program (which `verify` accepts), injects a
//! single mutation that recreates the finding's vulnerability in the compiled
//! bytecode, and asserts `verify` now rejects it. This exercises the pass
//! through the wired-in public [`verify`] entry (Phase 6A) — the same path
//! `Vm::new` runs on untrusted bytecode — so the corpus is a direct test of
//! the "attacker-supplied bytecode" threat model.
//!
//! Traceability (finding → test):
//! - B1/B2 (flat field offset trusted, not verified) → [`b1_b2_flat_field_offset_overrun_rejected`]
//! - B6 (shared-slot offset trusted, not verified) → [`b6_shared_slot_offset_overrun_rejected`]
//! - B8 (attacker-choosable enum `min_payload`) → [`b8_enum_min_payload_mutation_rejected`]
//! - finding 3 / B4 (loop back-edge stack growth) → [`finding3_loop_stack_growth_rejected`]
//!
//! The B4/B5 branch-imbalance and neutrality checks additionally have dedicated
//! hand-built unit coverage in `src/verify_typed.rs`; here they are exercised
//! against real compiled bytecode.

use keleusma::bytecode::{Module, Op, StructField};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::verify::verify;

fn compile_module(src: &str) -> Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile")
}

/// Whether the rejection came from the typed operand-stack pass (rather than an
/// incidental structural failure), so the corpus proves the pass is the guard.
fn rejected_by_typed_pass(m: &Module) -> bool {
    match verify(m) {
        Ok(()) => false,
        Err(e) => e.message.contains("typed operand-stack"),
    }
}

// B1/B2: a compiler-baked flat field offset is trusted at runtime (the
// `nested_view` debug_assert is the only guard). Pushing it past the composite
// body must be a load-time MUST-REJECT.
#[test]
fn b1_b2_flat_field_offset_overrun_rejected() {
    let src = "struct P { x: Word, y: Word }\n\
               fn main() -> Word { let p = P { x: 1, y: 2 }; p.x }";
    let mut m = compile_module(src);
    assert!(verify(&m).is_ok(), "baseline program must verify");

    let mut mutated = false;
    for chunk in &mut m.chunks {
        for op in &mut chunk.ops {
            if let Op::GetField(StructField::Flat { offset, .. }) = op {
                *offset = 4096; // far past the 16-byte struct body
                mutated = true;
            }
        }
    }
    assert!(mutated, "expected a flat struct-field access to mutate");
    assert!(
        rejected_by_typed_pass(&m),
        "an out-of-bounds flat field offset must be rejected by the typed pass"
    );
}

// C2: a flat `Text` field read is the sibling of the B1/B2 Word-field path,
// reaching `read_flat_scalar`'s Text branch. When the composite shape is
// reconstructible the typed pass bounds the two-word (ptr, len) read and an
// out-of-bounds offset is a load-time MUST-REJECT, exactly as for a Word field.
// The `read_flat_scalar` runtime bounds guard (audit C2) is the defer-on-Top
// backstop for shapes the pass cannot reconstruct, for example a native return
// with no declared shape; it is not reached on this reconstructible path.
//
// Gated to a build wide enough that a `Text` struct field lowers to a flat
// two-word body with a `GetField(StructField::Flat)` access. Under the narrow
// 8-bit framing features the representation differs, so the mutation target is
// absent; the finding and its guard are word-width-independent.
#[cfg(not(any(feature = "narrow-word-8", feature = "narrow-address-8")))]
#[test]
fn c2_flat_text_field_offset_overrun_rejected() {
    // `w.s` is the only field access, so the single flat GetField is the Text
    // field's two-word read.
    let src = "struct W { s: Text, n: Word }\n\
               fn main() -> Text { let w = W { s: \"hi\", n: 5 }; w.s }";
    let mut m = compile_module(src);
    assert!(verify(&m).is_ok(), "baseline program must verify");

    let mut mutated = false;
    for chunk in &mut m.chunks {
        for op in &mut chunk.ops {
            if let Op::GetField(StructField::Flat { offset, .. }) = op {
                *offset = 40000; // far past the struct body
                mutated = true;
            }
        }
    }
    assert!(mutated, "expected a flat Text field access to mutate");
    assert!(
        rejected_by_typed_pass(&m),
        "an out-of-bounds flat Text field offset must be rejected by the typed pass"
    );
}

// B6: a shared data slot's byte offset is trusted; the runtime reads and writes
// the slot at it. An offset past the shared-data buffer must be rejected.
#[test]
fn b6_shared_slot_offset_overrun_rejected() {
    let src = "data state { a: Word, b: Word }\n\
               fn main() -> Word { state.a }";
    let mut m = compile_module(src);
    assert!(verify(&m).is_ok(), "baseline program must verify");

    let buffer = m.shared_data_bytes;
    let layout = m.data_layout.as_mut().expect("a shared data layout");
    assert!(
        !layout.shared_layout.is_empty(),
        "expected a shared-slot layout entry"
    );
    // Push the first slot's offset past the end of the shared-data buffer.
    layout.shared_layout[0].offset = (buffer + 8) as u16;
    assert!(
        rejected_by_typed_pass(&m),
        "a shared-slot offset past the buffer must be rejected by the typed pass"
    );
}

// B8: the enum layout table's `min_payload` is attacker-choosable. Mutating it
// shifts the declared flat enum body size away from the sizes the bytecode's
// `NewComposite` ops were baked with, so the construction no longer matches.
#[test]
fn b8_enum_min_payload_mutation_rejected() {
    let src = "enum E { A(Word), B }\n\
               fn main() -> Word { match E::A(7) { E::A(v) => v, E::B => 0 } }";
    let mut m = compile_module(src);
    assert!(verify(&m).is_ok(), "baseline program must verify");

    assert!(!m.enum_layouts.is_empty(), "expected an enum-layout entry");
    // Enlarge the declared largest-variant payload; the baked construction
    // size (word_bytes + original min_payload) no longer matches.
    m.enum_layouts[0].min_payload += 8;
    assert!(
        rejected_by_typed_pass(&m),
        "a mutated enum min_payload must be rejected by the typed pass"
    );
}

// finding 3 / B4: a loop body that does not restore the operand stack across
// the back-edge grows it every iteration, escaping the worst-case bound.
// Deleting a `PopN` from inside a loop body recreates this; the neutrality
// check must reject it. (The depth pass's `max`-of-depths did not.)
#[test]
fn finding3_loop_stack_growth_rejected() {
    let src = "fn main() -> Word { for i in 0..4 { i } 0 }";
    let mut m = compile_module(src);
    assert!(verify(&m).is_ok(), "baseline program must verify");

    // Neutralize a `PopN` inside a loop body: find a Loop, then the first
    // `PopN` between it and its `EndLoop`, and set its count to zero so the
    // value it should have discarded now accumulates across iterations.
    let mut mutated = false;
    'chunks: for chunk in &mut m.chunks {
        let mut depth = 0i32;
        for op in &mut chunk.ops {
            match op {
                Op::Loop(_) => depth += 1,
                Op::EndLoop(_) => depth -= 1,
                Op::PopN(n) if depth > 0 && *n > 0 => {
                    *n = 0;
                    mutated = true;
                    break 'chunks;
                }
                _ => {}
            }
        }
    }
    assert!(mutated, "expected a PopN inside a loop body to neutralize");
    assert!(
        rejected_by_typed_pass(&m),
        "a non-neutral loop body must be rejected by the typed pass"
    );
}

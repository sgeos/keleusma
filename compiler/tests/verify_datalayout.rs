//! Conformance gate for the self-hosted A.2.1 data-layout validation
//! (`compiler/kel/verify_datalayout.kel`, driven by `selfhost::dl_reject_module_via_kel`).
//!
//! The stage reproduces the reference `validate_data_layout` (audit B6/C4): the shared-slot
//! reconcile (contiguity/count), the shared-slot buffer bounds, and the private-composite
//! monotonicity. The reference runs it as the first step of `typed_check_module`, so a module
//! whose only defect is its data layout is rejected there; that is the oracle. This slice
//! processes a single batch of up to 1024 entries, so the corpus is small data programs (the
//! self-hosted stages' layouts expand past that and need the batched driver extension).

use keleusma::bytecode::{Module, SlotVisibility};
use keleusma::verify_typed::typed_check_module;
use keleusma_selfhost::selfhost::{compile_src, dl_reject_module_via_kel};

const WB: usize = 8;
const FB: usize = 8;

fn read_stage(rel: &str) -> String {
    std::fs::read_to_string(rel)
        .or_else(|_| std::fs::read_to_string(format!("compiler/{rel}")))
        .unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

// The self-hosted stages have very large data layouts (lexer.kel alone has ~76k shared slots, one
// per array element), so accepting them exercises the batched marshalling on real tables.
#[test]
fn stage_data_layouts_are_accepted() {
    for stage in [
        "kel/lexer.kel",
        "kel/parse.kel",
        "kel/reconstruct.kel",
        "kel/codegen.kel",
        "kel/analyze.kel",
    ] {
        let m = compile_src(&read_stage(stage));
        assert!(
            typed_check_module(&m, WB, FB).is_ok(),
            "{stage}: the reference must accept the data layout"
        );
        assert!(
            !dl_reject_module_via_kel(&m),
            "{stage}: the batched data-layout stage must accept the large valid layout"
        );
    }
}

// ---- POSITIVE: valid data layouts are accepted --------------------------------------------

fn assert_accepted(label: &str, src: &str) {
    let m = compile_src(src);
    assert!(
        typed_check_module(&m, WB, FB).is_ok(),
        "{label}: the reference must accept this module for the oracle to bind"
    );
    assert!(
        !dl_reject_module_via_kel(&m),
        "{label}: the data-layout stage must not reject a valid layout"
    );
}

#[test]
fn valid_shared_layout_is_accepted() {
    assert_accepted(
        "two-shared-words",
        "shared data io { a: Word, b: Word }\n\
         loop main(r: Word) -> Word { io.a = r; io.b = r; yield io.a }",
    );
}

#[test]
fn mixed_shared_then_private_is_accepted() {
    // Shared slots precede private slots -- a contiguous shared prefix, which B6 requires.
    assert_accepted(
        "shared-then-private",
        "shared data io { a: Word }\n\
         private data p { c: Word }\n\
         loop main(r: Word) -> Word { io.a = r; p.c = r; yield io.a }",
    );
}

#[test]
fn no_data_layout_is_accepted() {
    assert_accepted("no-data", "loop main(r: Word) -> Word { yield r }");
}

#[test]
fn valid_private_composite_layout_is_accepted() {
    // A private composite (tuple) slot produces a private-composite layout entry, exercising the
    // C4 monotonicity path positively.
    assert_accepted(
        "private-tuple",
        "private data p { pt: (Word, Word) }\n\
         loop main(r: Word) -> Word { p.pt = (r, r); yield r }",
    );
}

// ---- NEGATIVE: each B6/C4 violation, injected into a valid module's data layout -------------

/// The stage must reject the mutated module, and the reference `typed_check_module` (which runs
/// `validate_data_layout` first) must reject it too. Only the data layout is mutated, so the
/// per-chunk interpretation stays valid and the rejection is the data-layout check.
fn base_shared() -> Module {
    compile_src(
        "shared data io { a: Word, b: Word }\n\
         loop main(r: Word) -> Word { io.a = r; io.b = r; yield io.a }",
    )
}

#[test]
fn shared_slot_offset_out_of_bounds_is_rejected() {
    let mut m = base_shared();
    // Push the first shared slot's byte offset past the shared-data buffer (B6 bounds).
    let buffer = m.shared_data_bytes;
    let layout = m.data_layout.as_mut().expect("data layout");
    layout.shared_layout[0].offset = buffer + 64;
    assert!(
        typed_check_module(&m, WB, FB).is_err(),
        "reference must reject the out-of-bounds shared slot"
    );
    assert!(
        dl_reject_module_via_kel(&m),
        "stage must reject the out-of-bounds shared slot"
    );
}

#[test]
fn shared_slots_not_a_contiguous_prefix_is_rejected() {
    let mut m = base_shared();
    // Make the first slot private while a later slot stays shared: the shared slots are no longer
    // a contiguous prefix, and the shared-layout count no longer matches (B6 contiguity/count).
    let layout = m.data_layout.as_mut().expect("data layout");
    layout.slots[0].visibility = SlotVisibility::Private;
    assert!(
        typed_check_module(&m, WB, FB).is_err(),
        "reference must reject the non-contiguous shared prefix"
    );
    assert!(
        dl_reject_module_via_kel(&m),
        "stage must reject the non-contiguous shared prefix"
    );
}

#[test]
fn private_composite_offset_out_of_pool_is_rejected() {
    let mut m = compile_src(
        "private data p { pt: (Word, Word) }\n\
         loop main(r: Word) -> Word { p.pt = (r, r); yield r }",
    );
    // Push the private composite's pool offset outside the persistent composite pool (C4).
    let pool = m.persistent_composite_bytes;
    let layout = m.data_layout.as_mut().expect("data layout");
    layout.private_composite_layout[0].offset = pool + 64;
    assert!(
        typed_check_module(&m, WB, FB).is_err(),
        "reference must reject the out-of-pool private composite offset"
    );
    assert!(
        dl_reject_module_via_kel(&m),
        "stage must reject the out-of-pool private composite offset"
    );
}

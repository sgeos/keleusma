//! The native region supplement folded into `auto_arena_capacity_for`.
//!
//! The operator ruled on question 22, confirmed first-hand on 2026-10-31,
//! that the runtime's sizing figure include the arena bytes a native
//! compilation reserves. `src/native_region.rs` reimplements the formula the
//! V0.3.X line sent as data, and the native backend stays the authority.
//!
//! The pinned figures are the ones V0.3.X measured with
//! `keleusma_native::region::host_arena_supplement_bytes` at its tree
//! `630e68f0`, given as the figure and the entry's total before the copy at
//! the yield. A disagreement here means this copy has drifted from the
//! backend, or that the two lines' compilers emit different chunks for the
//! same source, and either is a finding to report to V0.3.X rather than a pin
//! to move.
#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::native_region::native_region_supplement_bytes;

fn module(path: &str) -> keleusma::bytecode::Module {
    let full = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    let source = std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("{path}: {e}"));
    let tokens = keleusma::lexer::tokenize(&source).expect("lex");
    keleusma::compiler::compile(&keleusma::parser::parse(&tokens).expect("parse"))
        .unwrap_or_else(|e| panic!("{path}: {}", e.message))
}

#[test]
fn the_supplement_matches_the_backend_figures() {
    for (path, figure) in [
        ("examples/scripts/02_struct_field.kel", 16),
        ("examples/scripts/10_multbyte.kel", 192),
        ("examples/scripts/12_sensor_window.kel", 80),
        ("examples/scripts/13_telemetry_stream.kel", 720),
        ("src/hosted/interp.kel", 4032),
    ] {
        assert_eq!(
            native_region_supplement_bytes(&module(path)),
            figure,
            "{path}"
        );
    }
}

#[test]
fn the_copy_at_the_yield_follows_the_entry_total() {
    // 13_telemetry_stream.kel's entry total is 648 and its copy is four slots
    // of 16 bytes and a cursor word, 72 bytes, so the figure is 720.
    let m = module("examples/scripts/13_telemetry_stream.kel");
    assert_eq!(native_region_supplement_bytes(&m), 648 + 72);
}

#[test]
fn a_module_without_an_entry_needs_no_supplement() {
    let mut m = module("examples/scripts/02_struct_field.kel");
    m.entry_point = None;
    assert_eq!(native_region_supplement_bytes(&m), 0);
}

#[test]
fn the_sizing_figure_includes_the_supplement() {
    let m = module("examples/scripts/13_telemetry_stream.kel");
    let with = keleusma::vm::auto_arena_capacity_for(&m, &[]).expect("bounded");
    assert!(with >= native_region_supplement_bytes(&m) as usize);
    let mut no_entry = m.clone();
    no_entry.entry_point = None;
    let without = keleusma::vm::auto_arena_capacity_for(&no_entry, &[]).expect("bounded");
    assert_eq!(with - without, 720, "the fold adds exactly the supplement");
}

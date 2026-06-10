#![cfg(all(feature = "compile", feature = "verify"))]
//! The runtime ephemeral tracking-list arena budget (B28 P3 item 5).
//!
//! The opaque registry lives in the arena bottom region, so the arena must
//! be sized to hold it. The compiler records a worst-case figure in
//! `Module::aux_arena_bytes` and `auto_arena_capacity_for` adds it.
//!
//! The figure is gated on whether the module can intern an opaque at all
//! (the registry tightening): a module that never constructs a flat
//! composite able to intern a host opaque — the dominant case — records
//! zero, rather than the loose heap-derived bound it reserved before. A
//! module that can intern an opaque falls back to the sound heap-derived
//! bound `ceil(max_stream_heap / word_bytes) * size_of::<Arc>`.

extern crate alloc;

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::auto_arena_capacity_for;

fn compile_src(src: &str) -> keleusma::bytecode::Module {
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    compile(&program).expect("compile")
}

/// A Stream that yields a flat struct carrying a host opaque each iteration.
/// `make_handle` returns the opaque type `Handle`, so constructing `Holder`
/// flat interns one `Arc`, and the module must reserve a registry.
const OPAQUE_STREAM: &str = "use make_handle() -> Handle\n\
     struct Holder { h: Handle, n: Word }\n\
     loop main(seed: Word) -> Holder { yield Holder { h: make_handle(), n: seed } }";

/// A Stream that yields a flat all-scalar struct each iteration. It produces
/// a heap allocation but interns no opaque, so the registry budget is zero.
const SCALAR_STREAM: &str = "struct Point { x: Word, y: Word }\n\
     loop main(seed: Word) -> Point { yield Point { x: seed, y: 2 } }";

#[test]
fn opaque_free_stream_records_zero_aux_arena_bytes() {
    // The registry tightening: a heap-producing but opaque-free Stream needs
    // no opaque registry, so the budget is zero (previously it reserved the
    // loose heap-derived figure).
    let module = compile_src(SCALAR_STREAM);
    assert_eq!(
        module.aux_arena_bytes, 0,
        "an opaque-free Stream must record a zero registry budget, got {}",
        module.aux_arena_bytes
    );
}

#[test]
fn stream_interning_opaque_records_aux_arena_bytes() {
    // A flat composite with an opaque field interns an `Arc`, so the module
    // must reserve a non-zero registry budget.
    let module = compile_src(OPAQUE_STREAM);
    assert!(
        module.aux_arena_bytes > 0,
        "a Stream that interns an opaque must record a non-zero registry bound, got {}",
        module.aux_arena_bytes
    );
}

#[test]
fn auto_arena_capacity_includes_aux_arena_bytes() {
    // The autosize must provision for the registry: for an opaque-interning
    // module its result is strictly larger than the recorded aux figure
    // (the per-iteration script values also count).
    let module = compile_src(OPAQUE_STREAM);
    let cap = auto_arena_capacity_for(&module, &[]).expect("autosize");
    assert!(
        cap > module.aux_arena_bytes as usize,
        "autosize {} must cover the registry bound {} plus the per-iteration values",
        cap,
        module.aux_arena_bytes
    );
}

#[test]
fn func_only_module_records_no_aux_arena_bytes() {
    // The bound is per-Stream-iteration; a Func-only module has no Stream
    // chunk and therefore no registry budget.
    let module = compile_src("fn main() -> Word { 7 }");
    assert_eq!(module.aux_arena_bytes, 0);
}

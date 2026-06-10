#![cfg(all(feature = "compile", feature = "verify"))]
//! The runtime ephemeral tracking-list arena budget (B28 P3 item 5, Phase C).
//!
//! The opaque registry lives in the arena bottom region (Phase C2), so the
//! arena must be sized to hold it. The compiler records a worst-case figure in
//! `Module::aux_arena_bytes` — a sound upper bound on the registry's
//! per-iteration peak derived from the heap WCMU (every distinct interned
//! opaque has its word-sized index stored in a live flat-composite body, so
//! the intern count is at most `heap_bytes / word_bytes`) — and
//! `auto_arena_capacity_for` adds it to the arena size.

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

#[test]
fn stream_building_flat_composite_records_aux_arena_bytes() {
    // A Stream main that yields a flat two-word struct each iteration produces
    // a heap (top-region) allocation, so the registry bound is non-zero. With
    // a 16-byte flat body and an 8-byte word, the bound is at most two interns
    // times the size of an `Arc`.
    let module = compile_src(
        "struct Point { x: Word, y: Word }\n\
         loop main(seed: Word) -> Point { yield Point { x: seed, y: 2 } }",
    );
    assert!(
        module.aux_arena_bytes > 0,
        "a heap-producing Stream chunk must record a non-zero registry bound, got {}",
        module.aux_arena_bytes
    );
}

#[test]
fn auto_arena_capacity_includes_aux_arena_bytes() {
    // The autosize must provision for the registry: its result is at least the
    // recorded aux figure (and strictly larger than it, since the per-iteration
    // script values also count).
    let module = compile_src(
        "struct Point { x: Word, y: Word }\n\
         loop main(seed: Word) -> Point { yield Point { x: seed, y: 2 } }",
    );
    let cap = auto_arena_capacity_for(&module, &[]).expect("autosize");
    assert!(
        cap >= module.aux_arena_bytes as usize,
        "autosize {} must cover the registry bound {}",
        cap,
        module.aux_arena_bytes
    );
    assert!(
        cap > module.aux_arena_bytes as usize,
        "autosize must also include the per-iteration script values"
    );
}

#[test]
fn func_only_module_records_no_aux_arena_bytes() {
    // The bound is per-Stream-iteration; a Func-only module has no Stream
    // chunk and therefore no registry budget.
    let module = compile_src("fn main() -> Word { 7 }");
    assert_eq!(module.aux_arena_bytes, 0);
}

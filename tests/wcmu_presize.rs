#![cfg(all(feature = "compile", feature = "verify"))]
//! Accurate worst-case-memory-usage pre-sizing (B28 P3 item 5, priority 1).
//!
//! Under the no-allocation-after-initialisation directive (JPL Power-of-10
//! rule 3), the runtime pre-allocates its bottom-region working set — the
//! operand stack and the call frames — at construction, sized to the
//! module's exact worst-case footprint rather than a tiny minimum that
//! later grows mid-stream. `auto_arena_capacity_for` must report a figure
//! that accounts for every component (operand stack at the runtime's real
//! slot width, call frames at their real width, heap, and the opaque
//! registry), so a host sizing an arena from it can both construct and run
//! the module with zero margin.

extern crate alloc;

use keleusma::Arena;
use keleusma::Value;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::verify;
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for};

fn compile_src(src: &str) -> keleusma::bytecode::Module {
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    compile(&program).expect("compile")
}

/// A heap-free Stream program with a two-deep call chain. Frame depth is
/// two (`main` then `helper`); no composite or string allocation occurs,
/// so the per-iteration heap is zero. The whole arena requirement is
/// therefore the bottom region alone: operand stack plus call frames.
const CALL_CHAIN: &str = "fn helper(x: Word) -> Word { x + 1 }\n\
     loop main(seed: Word) -> Word { let a = helper(seed); yield a }";

#[test]
fn runtime_footprint_counts_call_frame_depth() {
    let module = compile_src(CALL_CHAIN);
    let fp = verify::module_runtime_footprint(&module, &[]).expect("footprint");
    assert_eq!(
        fp.max_frame_depth, 2,
        "main -> helper is a two-frame chain, got {}",
        fp.max_frame_depth
    );
    assert!(
        fp.max_operand_slots > 0,
        "the program uses operand-stack slots"
    );
    assert_eq!(fp.max_heap_bytes, 0, "no composite or string allocation");
}

#[test]
fn auto_arena_capacity_is_sufficient_with_zero_margin() {
    // The accurate figure must admit construction with no host margin. A
    // VM built with exactly `auto_arena_capacity_for` bytes constructs.
    let module = compile_src(CALL_CHAIN);
    let cap = auto_arena_capacity_for(&module, &[]).expect("autosize");
    let arena = Arena::with_capacity(cap);
    Vm::new(module, &arena).expect("VM must construct at the auto-sized capacity");
}

#[test]
fn auto_arena_capacity_is_tight_for_a_heap_free_program() {
    // Exactly one byte below the reported figure must not admit the module.
    // Because the program allocates no heap, the whole figure is the
    // pre-sized bottom region (operand stack + call frames), consumed at
    // construction, so the shortfall surfaces at `Vm::new`. This pins the
    // figure as the byte-exact minimum, not a loose over-bound.
    let module = compile_src(CALL_CHAIN);
    let cap = auto_arena_capacity_for(&module, &[]).expect("autosize");
    let arena = Arena::with_capacity(cap - 1);
    assert!(
        Vm::new(module, &arena).is_err(),
        "a capacity one byte below the auto-sized figure ({} bytes) must be rejected",
        cap
    );
}

#[test]
fn frame_depth_tracks_call_chain_length() {
    // main -> a -> b -> c is a four-frame chain. The depth must follow the
    // longest root-to-leaf path in the static call graph.
    let module = compile_src(
        "fn c(x: Word) -> Word { x + 1 }\n\
         fn b(x: Word) -> Word { c(x) }\n\
         fn a(x: Word) -> Word { b(x) }\n\
         loop main(seed: Word) -> Word { let r = a(seed); yield r }",
    );
    let fp = verify::module_runtime_footprint(&module, &[]).expect("footprint");
    assert_eq!(
        fp.max_frame_depth, 4,
        "main -> a -> b -> c is four frames, got {}",
        fp.max_frame_depth
    );
}

#[test]
fn stream_with_heap_runs_at_auto_capacity_with_zero_margin() {
    // A Stream that yields a flat composite each iteration allocates in the
    // arena top region. The auto-sized capacity must include that heap
    // component, so the VM both constructs and runs to its first yield with
    // no host margin (previously the examples needed `.max(4096)`).
    let module = compile_src(
        "struct Point { x: Word, y: Word }\n\
         loop main(seed: Word) -> Point { yield Point { x: seed, y: 2 } }",
    );
    let cap = auto_arena_capacity_for(&module, &[]).expect("autosize");
    let arena = Arena::with_capacity(cap);
    let mut vm = Vm::new(module, &arena).expect("construct at the auto-sized capacity");
    match vm.call(&[Value::Int(1)]).expect("run to first yield") {
        VmState::Yielded(_) => {}
        other => panic!("expected a yield, got {:?}", other),
    }
}

#[test]
fn wcmu_counts_composite_shared_read_copyout() {
    // A Stream that reads a whole composite shared slot copies the body out of
    // the borrowed host buffer into the arena on each read
    // (`read_shared_from_buffer`); the WCMU heap bound must include that
    // allocation (B28 item 2 / task #57). Before the fix `GetData` reported zero
    // heap, so a composite-shared-reading Stream under-counted its worst-case
    // memory by the copy-out size. The whole-composite read goes through the
    // module-level analysis, which carries the shared-slot layout the copy-out
    // size is read from.
    let module = compile_src(
        "data s { pos: (Word, Word) }\n\
         loop main(seed: Word) -> Word { let p = s.pos; yield p.0 + p.1 + seed }",
    );
    let word = (1usize << module.word_bits_log2) / 8;
    let per_chunk = verify::module_wcmu(&module, &[]).expect("module wcmu");
    let main_idx = module
        .chunks
        .iter()
        .position(|c| c.name == "main")
        .expect("main chunk");
    let (_, heap) = per_chunk[main_idx];
    // The only per-iteration arena allocation is the copy-out of the two-word
    // `pos` tuple; `p` is read from shared, not constructed, so nothing else
    // allocates.
    assert_eq!(
        heap,
        (2 * word) as u32,
        "Stream heap must include the {}-byte composite shared-read copy-out",
        2 * word
    );

    // A baseline Stream that touches no composite shared slot allocates nothing,
    // confirming the copy-out is the sole contributor above.
    let baseline = compile_src(
        "data s { hp: Word }\n\
         loop main(seed: Word) -> Word { yield s.hp + seed }",
    );
    let base_chunk = verify::module_wcmu(&baseline, &[]).expect("baseline wcmu");
    let base_idx = baseline
        .chunks
        .iter()
        .position(|c| c.name == "main")
        .expect("main chunk");
    assert_eq!(
        base_chunk[base_idx].1, 0,
        "a scalar shared read copies nothing out"
    );
}

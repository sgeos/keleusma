//! **ORDER 4, WORKSTREAM E: THE EXPORTED MACHINE STACK BOUND RESPONDS TO DEPTH.**
//!
//! `static_library_linkage.rs` measures the lexer's stack in a linked host and
//! requires it to stay within `<prefix>_stack_bytes`. A bound that ignored the
//! call graph could still pass that test whenever one frame dominated. This
//! file checks the two properties a longest-path bound must have, on programs
//! whose structure is known.
//!
//! 1. A deeper chain of calls that cannot be inlined away bounds higher than a
//!    single function.
//! 2. The bound is at least the largest single frame the code generator
//!    reported plus the per-frame allowance, and at most the sum of every
//!    frame with its allowance. A traversal that lost a level or counted a
//!    function twice would leave one of these envelopes.
mod common;

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
};
use keleusma_native::coroutine::HostContracts;
use keleusma_native::{linkage, stack};

fn machine() -> TargetMachine {
    Target::initialize_native(&InitializationConfig::default()).unwrap();
    let triple = TargetMachine::get_default_triple();
    Target::from_triple(&triple)
        .unwrap()
        .create_target_machine(
            &triple,
            "generic",
            "",
            OptimizationLevel::Default,
            RelocMode::PIC,
            CodeModel::Default,
        )
        .unwrap()
}

/// The exported bound, the frames of the same module measured again, and the
/// number of functions the module defines.
fn bound_and_frames(src: &str) -> (u64, Vec<u64>, u64) {
    let tm = machine();
    let ctx = Context::create();
    let program = common::build(src);
    let lib = linkage::build(&ctx, &program, &tm, "kel_depth", &HostContracts::default())
        .unwrap_or_else(|e| panic!("{src}: {e:?}"));
    let copy = lib.module().clone();
    let (written, frames) =
        stack::with_frame_sizes(&copy, || tm.write_to_memory_buffer(&copy, FileType::Object))
            .unwrap();
    written.unwrap();
    let defined = lib
        .module()
        .get_functions()
        .filter(|f| !f.as_global_value().is_declaration())
        .count() as u64;
    (lib.stack_bytes, frames.into_values().collect(), defined)
}

/// The arrays make each helper's frame real and their reads keep the calls,
/// so the optimiser cannot fold the chain into its caller entirely.
const FLAT: &str = "fn f1(x: Word) -> Word { let a = [x, x + 1, x + 2, x + 3]; a[x % 4] } \
                    loop main(t: Word) -> Word { let r = yield f1(t); yield r + 1 }";
const DEEP: &str = "fn f1(x: Word) -> Word { let a = [x, x + 1, x + 2, x + 3]; a[x % 4] } \
                    fn f2(x: Word) -> Word { let a = [f1(x), f1(x + 1), x]; a[x % 3] + f1(a[0]) } \
                    fn f3(x: Word) -> Word { let a = [f2(x), f2(x + 2), x]; a[x % 3] + f2(a[1]) } \
                    fn f4(x: Word) -> Word { let a = [f3(x), f3(x + 3), x]; a[x % 3] + f3(a[2]) } \
                    loop main(t: Word) -> Word { let r = yield f4(t); yield r + f4(r) }";

#[test]
fn a_deeper_call_chain_raises_the_exported_bound() {
    let (flat, ..) = bound_and_frames(FLAT);
    let (deep, ..) = bound_and_frames(DEEP);
    println!("  stack bound: one helper {flat} bytes, four nested helpers {deep} bytes");
    assert!(
        deep > flat,
        "a four-level chain must bound higher than one helper: deep {deep}, flat {flat}"
    );
}

#[test]
fn the_bound_lies_between_the_largest_frame_and_the_sum_of_all_frames() {
    for src in [FLAT, DEEP] {
        let (bound, frames, defined) = bound_and_frames(src);
        let a = stack::FRAME_ALLOWANCE;
        let largest = frames.iter().max().copied().unwrap_or(0) + a;
        // Every defined function once, each with its allowance, whether or
        // not the code generator reported a frame for it.
        let total: u64 = frames.iter().sum::<u64>() + a * defined;
        println!("  frames {frames:?}, bound {bound}");
        assert!(
            !frames.is_empty(),
            "no frame was reported, so nothing was measured"
        );
        assert!(
            bound >= largest,
            "the bound {bound} is below the largest frame {largest}"
        );
        assert!(
            bound <= total,
            "the bound {bound} exceeds every frame summed with allowances {total}"
        );
    }
}

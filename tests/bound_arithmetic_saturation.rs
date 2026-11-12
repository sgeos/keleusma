#![cfg(all(feature = "compile", feature = "verify"))]
//! The time and memory bounds saturate, and the slot width is validated.
//!
//! The proofs line found two arithmetic defects in `src/verify.rs` with Kani and recorded them in
//! `tools/proof-kani/README.md` as F1 and F2.
//!
//! - **F1.** `wcet_stream_iteration_with_cost_model` added the `Stream` and `Reset` costs with a
//!   plain `+`. With overflow checks the sum panicked, and without them it wrapped, so the
//!   function returned a figure below the cost of the two instructions it had just charged.
//! - **F2.** No entry that takes `value_slot_bytes` validated it. Zero divided by zero at a
//!   `Call`, and a large width overflowed `stack_slots * value_slot_bytes`.
//!
//! # The contract these tests pin
//!
//! Every cost is a `u32` and every sum and product saturates. **A result of `u32::MAX` is
//! therefore not a bound.** It means that the true figure is `u32::MAX` or more, and a caller
//! must treat it as the absence of a bound. The compiler writes it to the module header, where it
//! is the overflow sentinel that `Vm::new` refuses. A slot width of zero is refused with a
//! `VerifyError` at every public entry that takes the width.
//!
//! # Coverage limits
//!
//! These are point tests over a few cost models and widths. They do not prove the absence of an
//! unchecked operation, which is what the Kani harnesses are for. The operand-stack offsets of
//! the memory walk are `i32` and now saturate, and no test here reaches that saturation, because
//! it needs a chunk of about two thousand million pushes.

use keleusma::bytecode::{BlockType, CostModel, Module, NOMINAL_COST_MODEL, Op};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::verify;

fn compile_src(src: &str) -> Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile")
}

fn stream_chunk(module: &Module) -> usize {
    module
        .chunks
        .iter()
        .position(|c| c.block_type == BlockType::Stream)
        .expect("a Stream chunk")
}

// --- F1, the time bound ---------------------------------------------------------------------

/// `Stream` and `Reset` each cost half the range, so their sum is exactly `2^32`.
fn half_range_envelope(op: &Op) -> u32 {
    match op {
        Op::Stream | Op::Reset => 0x8000_0000,
        _ => 1,
    }
}

/// `Stream` and `Reset` each cost the whole range, which is Kani's own counterexample.
fn full_range_envelope(op: &Op) -> u32 {
    match op {
        Op::Stream | Op::Reset => u32::MAX,
        _ => 1,
    }
}

fn model(op_cycles: fn(&Op) -> u32) -> CostModel {
    CostModel {
        value_slot_bytes: NOMINAL_COST_MODEL.value_slot_bytes,
        op_cycles,
        text_byte_cycles: NOMINAL_COST_MODEL.text_byte_cycles,
    }
}

#[test]
fn the_stream_envelope_saturates_where_it_wrapped_to_a_small_figure() {
    let module = compile_src("loop main(x: Word) -> Word { yield x }");
    let chunk = &module.chunks[stream_chunk(&module)];
    // Wrapping gave `0 + body`, a figure below the cost of `Stream` alone.
    let half =
        verify::wcet_stream_iteration_with_cost_model(chunk, &model(half_range_envelope), &[])
            .expect("a saturated figure, not a refusal");
    assert_eq!(half, u32::MAX);
    let full =
        verify::wcet_stream_iteration_with_cost_model(chunk, &model(full_range_envelope), &[])
            .expect("a saturated figure, not a refusal");
    assert_eq!(full, u32::MAX);
}

#[test]
fn a_figure_is_never_below_the_cost_of_one_instruction_it_charges() {
    // The property the wrap violated, over a sweep of envelope costs around the overflow.
    let module = compile_src("loop main(x: Word) -> Word { let y = x + 1; yield y }");
    let chunk = &module.chunks[stream_chunk(&module)];
    for cost in [
        1u32,
        0x7FFF_FFFF,
        0x8000_0000,
        0x8000_0001,
        u32::MAX - 1,
        u32::MAX,
    ] {
        ENVELOPE.with(|c| c.set(cost));
        let figure =
            verify::wcet_stream_iteration_with_cost_model(chunk, &model(swept_envelope), &[])
                .expect("a figure");
        assert!(
            figure >= cost,
            "an envelope cost of {cost} gave a figure of {figure}"
        );
        let stream = chunk
            .ops
            .iter()
            .position(|op| matches!(op, Op::Stream))
            .unwrap();
        let reset = chunk
            .ops
            .iter()
            .position(|op| matches!(op, Op::Reset))
            .unwrap();
        assert!(
            !chunk.ops[stream..reset]
                .iter()
                .any(|op| matches!(op, Op::If(_) | Op::Loop(_))),
            "the chunk is meant to be straight-line"
        );
        let exact = 2 * cost as u64 + (reset - stream - 1) as u64;
        assert_eq!(
            figure as u64,
            exact.min(u32::MAX as u64),
            "the figure for a straight-line chunk is the exact sum, saturated"
        );
    }
}

thread_local! {
    static ENVELOPE: std::cell::Cell<u32> = const { std::cell::Cell::new(1) };
}

fn swept_envelope(op: &Op) -> u32 {
    match op {
        Op::Stream | Op::Reset => ENVELOPE.with(|c| c.get()),
        _ => 1,
    }
}

#[test]
fn the_module_entry_saturates_as_the_chunk_entry_does() {
    let module = compile_src("loop main(x: Word) -> Word { yield x }");
    let per_chunk = verify::module_wcet_with_bounds(&module, &[], &model(full_range_envelope))
        .expect("figures");
    assert_eq!(per_chunk[stream_chunk(&module)], u32::MAX);
}

#[test]
fn a_saturated_time_bound_reaches_the_header_as_the_overflow_sentinel() {
    // Three nested loops of 3,000,000 passes cost far more than `u32::MAX` nominal cycles.
    let src = "loop main(n: Word) -> Word { \
               for i in 0..3000000 limit 3000000 { \
               for j in 0..3000000 limit 3000000 { \
               for k in 0..3000000 limit 3000000 { let w = n + 1; } } } \
               yield n }";
    let module = compile_src(src);
    let chunk = &module.chunks[stream_chunk(&module)];
    assert_eq!(
        verify::wcet_stream_iteration(chunk).expect("a figure"),
        u32::MAX
    );
    assert_eq!(
        module.wcet_cycles,
        u32::MAX,
        "the header carries the overflow sentinel for a saturated figure"
    );
    // The loader refuses the sentinel, so a saturated figure is never accepted as a bound.
    let arena = keleusma::Arena::with_capacity(1 << 16);
    assert!(keleusma::vm::Vm::new(module, &arena).is_err());
}

#[test]
fn a_saturated_time_bound_records_no_proof_witness() {
    use keleusma::compiler::{CompileOptions, compile_with_options};
    use keleusma::debug_meta::DebugRecordKind;
    let witnesses = |src: &str| -> Vec<String> {
        let program = parse(&tokenize(src).expect("lex")).expect("parse");
        let options = CompileOptions { emit_debug: true };
        let (module, _) =
            compile_with_options(&program, &keleusma::target::Target::host(), &options)
                .expect("compile");
        let chunk = &module.chunks[stream_chunk(&module)];
        let pool = chunk.debug_pool.as_ref().expect("a debug pool");
        pool.records
            .iter()
            .filter(|r| r.kind == DebugRecordKind::VerifierWitness)
            .filter_map(|r| r.operands.get(1))
            .map(|&i| pool.string_pool[i as usize].clone())
            .collect()
    };
    let bounded = witnesses("loop main(n: Word) -> Word { yield n }");
    assert!(
        bounded
            .iter()
            .any(|p| p == "wcet-per-iteration-bound-proven"),
        "the control records the witness: {bounded:?}"
    );
    let saturated = witnesses(
        "loop main(n: Word) -> Word { \
         for i in 0..3000000 limit 3000000 { \
         for j in 0..3000000 limit 3000000 { \
         for k in 0..3000000 limit 3000000 { let w = n + 1; } } } \
         yield n }",
    );
    assert!(
        !saturated
            .iter()
            .any(|p| p == "wcet-per-iteration-bound-proven"),
        "a saturated figure is not a proven bound: {saturated:?}"
    );
}

// --- F2, the slot width ---------------------------------------------------------------------

const CALLS: &str = "fn leaf(a: Word) -> Word { let b = a + 1; let c = b + 1; c }\n\
                     fn mid(a: Word) -> Word { let b = leaf(a); b + leaf(b) }\n\
                     loop main(x: Word) -> Word { let y = mid(x); yield y; x }";

#[test]
fn a_zero_slot_width_is_refused_at_every_public_entry() {
    let module = compile_src(CALLS);
    let stream = &module.chunks[stream_chunk(&module)];
    let func = module
        .chunks
        .iter()
        .find(|c| c.block_type == BlockType::Func)
        .expect("a Func chunk");
    let named = |r: Result<(), verify::VerifyError>, entry: &str| {
        let e = r.expect_err(&format!("{entry} accepted a slot width of zero"));
        assert!(
            e.message.contains("value_slot_bytes") && e.message.contains("zero"),
            "{entry}: {}",
            e.message
        );
    };
    named(
        verify::wcmu_stream_iteration_with_value_slot_bytes(stream, 0).map(|_| ()),
        "wcmu_stream_iteration_with_value_slot_bytes",
    );
    named(
        verify::wcmu_whole_chunk_with_value_slot_bytes(func, 0).map(|_| ()),
        "wcmu_whole_chunk_with_value_slot_bytes",
    );
    named(
        verify::module_wcmu_with_value_slot_bytes(&module, &[], 0).map(|_| ()),
        "module_wcmu_with_value_slot_bytes",
    );
    named(
        verify::module_wcmu_with_bounds(&module, &[], 0).map(|_| ()),
        "module_wcmu_with_bounds",
    );
    named(
        verify::verify_resource_bounds_with_natives_and_value_slot_bytes(
            &module,
            usize::MAX,
            &[],
            0,
        ),
        "verify_resource_bounds_with_natives_and_value_slot_bytes",
    );
    named(
        verify::verify_resource_bounds_with_bounds(&module, usize::MAX, &[], 0),
        "verify_resource_bounds_with_bounds",
    );
    let zero_width = CostModel {
        value_slot_bytes: 0,
        ..NOMINAL_COST_MODEL
    };
    named(
        verify::verify_resource_bounds_with_cost_model(&module, usize::MAX, &zero_width, &[]),
        "verify_resource_bounds_with_cost_model",
    );
}

#[test]
fn the_default_width_and_the_unit_width_are_still_accepted() {
    // The control for the refusals above, and the two widths the crate itself passes.
    let module = compile_src(CALLS);
    verify::module_wcmu(&module, &[]).expect("the default width");
    verify::module_wcmu_with_value_slot_bytes(&module, &[], 1).expect("the unit width");
    verify::module_runtime_footprint(&module, &[]).expect("the footprint, at the unit width");
}

#[test]
fn the_stack_bound_is_the_slot_count_times_the_width_saturated() {
    // The oracle is the slot count, taken at a width of one, multiplied in 64 bits. Every width
    // must give exactly that product, or `u32::MAX` where the product does not fit. A wrapped
    // product is below it, and a saturated callee whose caller divides the saturated figure back
    // into slots is below it as well.
    let module = compile_src(CALLS);
    let slots = verify::module_wcmu_with_value_slot_bytes(&module, &[], 1).expect("slots");
    assert!(slots.iter().all(|&(s, _)| s >= 1), "{slots:?}");
    let mut saturated = 0usize;
    for width in [
        1u32,
        32,
        1 << 16,
        1 << 30,
        (1 << 31) - 1,
        1 << 31,
        (1 << 31) + 1,
        u32::MAX - 1,
        u32::MAX,
    ] {
        let got = verify::module_wcmu_with_value_slot_bytes(&module, &[], width)
            .unwrap_or_else(|e| panic!("width {width}: {}", e.message));
        let with_bounds = verify::module_wcmu_with_bounds(&module, &[], width)
            .unwrap_or_else(|e| panic!("width {width}: {}", e.message));
        for (i, (&(slot_count, _), &(stack, _))) in slots.iter().zip(&got).enumerate() {
            let exact = slot_count as u64 * width as u64;
            let expected = exact.min(u32::MAX as u64) as u32;
            assert_eq!(
                stack, expected,
                "chunk {i} at width {width}: {slot_count} slots give {stack}"
            );
            assert_eq!(with_bounds[i].0, expected, "chunk {i} at width {width}");
            if exact > u32::MAX as u64 {
                saturated += 1;
            }
        }
    }
    assert!(
        saturated >= 6,
        "only {saturated} products exceeded the range, so the saturation was barely reached"
    );
}

#[test]
fn the_chunk_entries_saturate_at_a_large_width() {
    let module = compile_src(CALLS);
    let stream = &module.chunks[stream_chunk(&module)];
    let (stack, _) = verify::wcmu_stream_iteration_with_value_slot_bytes(stream, u32::MAX)
        .expect("a saturated figure, not a refusal");
    assert_eq!(stack, u32::MAX);
    for chunk in module
        .chunks
        .iter()
        .filter(|c| c.block_type == BlockType::Func)
    {
        let (stack, _) = verify::wcmu_whole_chunk_with_value_slot_bytes(chunk, u32::MAX)
            .expect("a saturated figure, not a refusal");
        assert_eq!(stack, u32::MAX, "{}", chunk.name);
    }
}

#[test]
fn a_saturated_memory_bound_admits_no_arena_below_its_own_size() {
    // A saturated component means at least `u32::MAX` bytes, so admission against a capacity
    // below that refuses the module. This is the existing contract and it is pinned so that the
    // saturation above cannot be read as a small figure by the admission check.
    let module = compile_src(CALLS);
    let err = verify::verify_resource_bounds_with_natives_and_value_slot_bytes(
        &module,
        u32::MAX as usize - 1,
        &[],
        u32::MAX,
    )
    .expect_err("a saturated bound was admitted into a smaller arena");
    assert!(
        err.message.contains("exceeds arena capacity"),
        "{}",
        err.message
    );
}

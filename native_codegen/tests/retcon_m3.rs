//! **R4.1 MILESTONE M3, the deterministic half: per-coroutine MEMORY.**
//!
//! M1 verified and split a `coro.id.retcon` fragment. M2 linked it and ran it.
//! M3 asks what a coroutine costs. That question has two halves with very
//! different evidential quality, and this file deliberately answers only one.
//!
//! # What is measured here, and what it covers
//!
//! **The frame the coroutine asks the arena for**, as a function of the live
//! state it carries across a suspension. Nothing here is timed.
//!
//! Per-coroutine memory is **not** a single number. It is split between two
//! places, and a figure that does not say which it means is not usable:
//!
//! * the **caller-provided buffer**, whose size is the first argument to
//!   `coro.id.retcon` and which the caller must supply whether or not it is
//!   filled;
//! * the **arena frame**, requested only when the live state overflows that
//!   buffer — M2 showed at run time that a 256-byte buffer yields `alloc=0`
//!   while an 8-byte one yields `alloc=1`.
//!
//! So a coroutine whose frame fits its buffer costs the buffer and nothing else,
//! and one that overflows costs the buffer **plus** the frame. The figures below
//! are the ARENA FRAME under a deliberately small buffer, which is the quantity
//! Workstream E needs for a native worst-case memory bound.
//!
//! # Why the timing half is not here
//!
//! `native_codegen/pending/README.md` records that two sessions share one machine
//! and that a full gate "saturates it". A wall-clock figure taken under that
//! contention is the confident-wrong-number failure this package keeps finding in
//! its own instruments. **M3 is therefore NOT complete**, and the roadmap says
//! which half is done.
use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::memory_buffer::MemoryBuffer;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetMachine};

fn host_machine() -> TargetMachine {
    Target::initialize_native(&InitializationConfig::default()).expect("init native target");
    let triple = TargetMachine::get_default_triple();
    Target::from_triple(&triple)
        .expect("target")
        .create_target_machine(
            &triple,
            "generic",
            "",
            OptimizationLevel::Default,
            RelocMode::PIC,
            CodeModel::Default,
        )
        .expect("target machine")
}

/// A retcon coroutine carrying `live` 64-bit values across its suspension.
///
/// The buffer is deliberately 8 bytes, just enough to hold the frame pointer, so
/// that the arena request is visible as soon as the live state needs more than
/// the buffer holds. With no live state at all the frame still fits, which is how
/// the crossover point is located rather than assumed.
fn ir_with_live_words(live: usize) -> String {
    let mut phis = String::new();
    let mut adds = String::new();
    let mut sink = String::new();
    for i in 0..live {
        phis.push_str(&format!(
            "  %p{i} = phi i64 [ {i}, %entry ], [ %q{i}, %resume ]\n"
        ));
        adds.push_str(&format!("  %q{i} = add i64 %p{i}, {}\n", i + 1));
        sink.push_str(&format!("  call void @kel_use(i64 %q{i})\n"));
    }
    format!(
        r#"
declare token @llvm.coro.id.retcon(i32, i32, ptr, ptr, ptr, ptr)
declare ptr @llvm.coro.begin(token, ptr)
declare i1 @llvm.coro.suspend.retcon.i1(...)
declare void @llvm.coro.end(ptr, i1, token)

declare ptr @kel_arena_alloc(i32)
declare void @kel_arena_free(ptr)
declare ptr @kel_coro_prototype(ptr, i1)
declare void @kel_yield(i32)
declare void @kel_use(i64)

define ptr @kel_stream(ptr %buffer, i32 %n) presplitcoroutine {{
entry:
  %id = call token @llvm.coro.id.retcon(i32 8, i32 8, ptr %buffer, ptr @kel_coro_prototype, ptr @kel_arena_alloc, ptr @kel_arena_free)
  %hdl = call ptr @llvm.coro.begin(token %id, ptr null)
  br label %loop

loop:
  %v = phi i32 [ %n, %entry ], [ %next, %resume ]
{phis}  call void @kel_yield(i32 %v)
  %unwind = call i1 (...) @llvm.coro.suspend.retcon.i1()
  br i1 %unwind, label %cleanup, label %resume

resume:
  %next = add i32 %v, 1
{adds}{sink}  br label %loop

cleanup:
  call void @llvm.coro.end(ptr %hdl, i1 0, token none)
  unreachable
}}
"#
    )
}

/// The arena frame size this coroutine requests, read from the split output.
///
/// **`None` means the frame FIT the caller's buffer**, so the coroutine costs
/// zero arena bytes. That is a legitimate result rather than an anomaly: an
/// 8-byte buffer holds a small frame, and the caller pays for it either way.
fn frame_bytes(live: usize) -> Option<u32> {
    let ctx = Context::create();
    let mut bytes = ir_with_live_words(live).as_bytes().to_vec();
    bytes.push(0);
    let buf = MemoryBuffer::create_from_memory_range(&bytes, "m3");
    let module = ctx
        .create_module_from_ir(buf)
        .unwrap_or_else(|e| panic!("live={live} did not parse: {}", e.to_string()));
    module
        .verify()
        .unwrap_or_else(|e| panic!("live={live} does not verify: {}", e.to_string()));

    let machine = host_machine();
    // The control that keeps the pipeline's success from being vacuous.
    assert!(
        module
            .run_passes(
                "definitely-not-a-real-pass",
                &machine,
                PassBuilderOptions::create()
            )
            .is_err(),
        "run_passes accepted a nonexistent pass, so its success proves nothing"
    );
    module
        .run_passes(
            "coro-early,coro-split,coro-cleanup",
            &machine,
            PassBuilderOptions::create(),
        )
        .unwrap_or_else(|e| panic!("live={live} pipeline failed: {}", e.to_string()));

    module
        .print_to_string()
        .to_string()
        .lines()
        .filter(|l| l.contains("@kel_arena_alloc(") && l.contains("call"))
        .find_map(|l| {
            let open = l.find("@kel_arena_alloc(i32 ")? + "@kel_arena_alloc(i32 ".len();
            let rest = &l[open..];
            let close = rest.find(')')?;
            rest[..close].trim().parse::<u32>().ok()
        })
}

/// **WHAT DOES ONE COROUTINE COST IN MEMORY, and does it scale with its state?**
///
/// A constant would mean the figure says nothing about a particular coroutine.
/// The point of the measurement is that it is a computable function of the
/// coroutine, recoverable from emitted code without running anything.
#[test]
fn m3_the_arena_frame_scales_with_live_state_and_is_statically_recoverable() {
    println!("\n================ R4.1 M3 (memory half): arena frame per coroutine");
    println!("  buffer configuration : 8 bytes, so every frame OVERFLOWS to the arena");
    println!("  figure covers        : the ARENA FRAME only, not the caller's buffer");
    println!(
        "  {:>10}  {:>12}  {:>10}",
        "live i64", "frame bytes", "delta"
    );

    // **ABSENCE IS A RESULT HERE, not a failure.** A first version panicked when
    // no arena request survived, on the reasoning that an 8-byte buffer must
    // always overflow. It does not: with no live state the frame fits the buffer
    // and the coroutine costs ZERO arena bytes. That is the overflow rule
    // working, and it locates the crossover where a coroutine starts costing
    // arena memory at all.
    let mut rows: Vec<(usize, u32)> = Vec::new();
    let mut prev: Option<u32> = None;
    for live in [0usize, 1, 2, 4, 8] {
        let size = frame_bytes(live).unwrap_or(0);
        let note = if size == 0 { "  (fits the buffer)" } else { "" };
        let delta = prev.map(|p| size as i64 - p as i64).unwrap_or(0);
        println!("  {live:>10}  {size:>12}  {delta:>10}{note}");
        prev = Some(size);
        rows.push((live, size));
    }
    let crossover = rows.iter().find(|(_, s)| *s > 0).map(|(l, s)| (*l, *s));
    match crossover {
        Some((l, s)) => {
            println!("  crossover: {l} live i64 is the first to reach the arena, at {s} bytes")
        }
        None => println!("  crossover: none reached the arena in this range"),
    }
    println!("================\n");

    // **It must GROW, or the figure is not per-coroutine.** A constant frame
    // would mean this measures the shape of the fragment rather than the
    // coroutine's state.
    let first = rows.first().expect("rows").1;
    let last = rows.last().expect("rows").1;
    assert!(
        last > first,
        "the frame did not grow with live state ({first} -> {last}), so this figure \
         is not a per-coroutine cost"
    );

    // Monotone, and every size a literal recoverable without executing anything —
    // which is what Workstream E needs as the input to a native memory bound.
    for w in rows.windows(2) {
        assert!(
            w[1].1 >= w[0].1,
            "frame shrank as live state grew: {:?} then {:?}",
            w[0],
            w[1]
        );
    }
}

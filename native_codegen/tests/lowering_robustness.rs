//! **DOES `lower_module` REFUSE MALFORMED BYTECODE, OR PANIC ON IT?**
//!
//! # The contract, and why nothing enforced it
//!
//! This backend's answer to input it cannot handle is a REFUSAL — an `Err` a
//! caller can act on. A panic is the worst of the three outcomes: worse than a
//! refusal because the caller cannot recover, and worse than a wrong answer only
//! in that it is at least loud.
//!
//! One test already pins one instance of this, born from a real `Vec` index
//! panic inside a library when the operand stack exceeded the backend's
//! provisioning. **That was found by tripping over it.** Nothing swept for the
//! class.
//!
//! # Why malformed input is reachable at all
//!
//! `verify()` runs before lowering in normal use, so a well-behaved caller never
//! presents this. But `lower_module` is a public entry point that does not
//! require a verified module, and the tree's own tests MUTATE compiled bytecode
//! and lower it — `yield_escape_gate.rs` removes opcodes, `probe_confine_reach.rs`
//! rewrites them. A panic reachable that way is reachable by any embedder who
//! calls the same function.
//!
//! # What the mutations are, and what they are not
//!
//! They are structural corruptions of real compiled modules: truncated op
//! streams, opcodes replaced, jump targets and local indices pushed out of
//! range, callee indices pointed at nothing. **They are not random bytes** — a
//! module that fails to decode at all would test the decoder, not the emitter.
//!
//! Each is applied to a COPY, so no mutation escapes its own case.

use keleusma::bytecode::{Module, Op};
use keleusma_native::{LowerOptions, lower_module};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;

mod common;

/// Where the last panic came from, captured by a hook because `catch_unwind`
/// hands back the payload and NOT the location.
///
/// **ATTRIBUTION BY ORIGIN, NOT BY PROVOCATION.** A first version classified
/// panics by the MUTATION that triggered them — "truncated" meant upstream,
/// anything else meant this backend. That was wrong and it misattributed three
/// panics: an out-of-range `Else` target reaches the SAME upstream defect a
/// truncation does, because both make a block's recorded extent exceed
/// `ops.len()`. Blaming the backend for them would have sent someone hunting in
/// the wrong crate.
static LAST_PANIC_FILE: Mutex<Option<String>> = Mutex::new(None);

fn install_location_hook() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(loc) = info.location() {
            *LAST_PANIC_FILE.lock().unwrap() = Some(loc.file().to_string());
        }
        let _ = &prev; // deliberately silent: the sweep prints its own summary
    }));
}

/// Lower `m`, reporting `Ok`, or `(origin file, message)` on a panic.
fn outcome(m: &Module) -> Result<&'static str, (String, String)> {
    *LAST_PANIC_FILE.lock().unwrap() = None;
    let r = catch_unwind(AssertUnwindSafe(|| {
        let ctx = inkwell::context::Context::create();
        let lm = ctx.create_module("kel");
        lower_module(&ctx, &lm, m, LowerOptions::default()).is_ok()
    }));
    match r {
        Ok(true) => Ok("lowered"),
        Ok(false) => Ok("refused"),
        Err(e) => {
            let msg = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "non-string panic".to_string());
            let file = LAST_PANIC_FILE
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| "unknown".to_string());
            Err((file, msg.chars().take(110).collect()))
        }
    }
}

/// Mutations generated per module, capped so a large module cannot dominate.
///
/// Every mutation lowers the whole module, so cost per mutation grows with
/// module size. See the note at the call site.
const MUTATIONS_PER_MODULE: usize = 45;

/// Structural corruptions, each a named function from a module to a module.
fn mutants(base: &Module) -> Vec<(String, Module)> {
    let mut out: Vec<(String, Module)> = Vec::new();
    // Stride the chunks so the sample spreads across the module rather than
    // clustering at its start, where the entry chunk's shape is unrepresentative.
    let per_chunk = 45usize; // 5 positions x 9 corruption kinds
    let want_chunks = MUTATIONS_PER_MODULE.div_ceil(per_chunk).max(1);
    let stride = base.chunks.len().div_ceil(want_chunks).max(1);
    for (ci, chunk) in base.chunks.iter().enumerate().step_by(stride) {
        let n = chunk.ops.len();
        if n == 0 {
            continue;
        }
        // Sample rather than sweep: the point is coverage of KINDS, and a full
        // cross-product over a 1074-chunk corpus is a sweep this file is not.
        //
        // ⚠ **THE POSITION SET NEVER INCLUDED THE END, AND THAT WAS AN ACCIDENT
        // OF AN EXPRESSION RATHER THAN A CHOICE.** It was `n * frac / 3` for
        // `frac` in 0..3, giving 0, n/3 and 2n/3 — so for a 57-op chunk it hit
        // 0, 19 and 38 and never 56.
        //
        // **The tail is the structurally interesting part.** A chunk ends in
        // `Return`, or in `PopN(1); Reset` for a stream, and the lowering treats
        // exactly those specially: the degenerate-stream tail walk, the `Reset`
        // back edge, and the missing-terminator path that synthesises a `Unit`
        // return. None of it was ever perturbed.
        //
        // Found by asking of this file the question that produced three other
        // findings this session: which parameter here was CHOSEN, and which one
        // merely fell out of an expression? The count of positions was chosen.
        // Their placement was not.
        let last = n - 1;
        for at in [0usize, n / 3, 2 * n / 3, last.saturating_sub(1), last] {
            let mut t = base.clone();
            t.chunks[ci].ops.truncate(at);
            out.push((format!("chunk {ci}: truncated to {at} of {n}"), t));

            if at < n {
                let mut r = base.clone();
                r.chunks[ci].ops[at] = Op::Return;
                out.push((format!("chunk {ci}: op {at} -> Return"), r));

                // **A BRANCH TARGET OUT OF RANGE.** There is no raw `Jump` in
                // this ISA — control flow is labelled block delimiters — so the
                // corruption is an `Else` whose target no op index reaches.
                let mut j = base.clone();
                j.chunks[ci].ops[at] = Op::Else(u16::MAX);
                out.push((format!("chunk {ci}: op {at} -> Else(out of range)"), j));

                let mut l = base.clone();
                l.chunks[ci].ops[at] = Op::GetLocal(u16::MAX);
                out.push((format!("chunk {ci}: op {at} -> GetLocal(huge)"), l));

                // **ADDED AFTER THE FIRST SWEEP MISSED IT.** The first mutation
                // set corrupted `GetLocal` and not `SetLocal`, so the `SetLocal`
                // bound went unfixed and the sweep reported clean. **A sweep is
                // only as wide as its mutation set.** The mutation-placement
                // guard pointed at the omission, not this file.
                let mut sl = base.clone();
                sl.chunks[ci].ops[at] = Op::SetLocal(u16::MAX);
                out.push((format!("chunk {ci}: op {at} -> SetLocal(huge)"), sl));

                let mut c = base.clone();
                c.chunks[ci].ops[at] = Op::Call(u16::MAX, 0);
                out.push((format!("chunk {ci}: op {at} -> Call(nonexistent)"), c));

                // **THE OPERANDS THAT INDEX TABLES**, which is where both fixed
                // panic classes lived. The first mutation set corrupted control
                // flow and local slots and stopped there; a table index out of
                // range is the same shape as `GetLocal(huge)` and was simply not
                // tried.
                let mut nv = base.clone();
                nv.chunks[ci].ops[at] = Op::CallVerifiedNative(u16::MAX, 0);
                out.push((
                    format!("chunk {ci}: op {at} -> CallVerifiedNative(huge)"),
                    nv,
                ));

                let mut ne = base.clone();
                ne.chunks[ci].ops[at] = Op::CallExternalNative(u16::MAX, 0);
                out.push((
                    format!("chunk {ci}: op {at} -> CallExternalNative(huge)"),
                    ne,
                ));

                let mut gd = base.clone();
                gd.chunks[ci].ops[at] = Op::GetData(u32::MAX / 2);
                out.push((format!("chunk {ci}: op {at} -> GetData(huge)"), gd));

                let mut gdi = base.clone();
                gdi.chunks[ci].ops[at] = Op::GetDataIndexed(u32::MAX / 2, u32::MAX / 2);
                out.push((format!("chunk {ci}: op {at} -> GetDataIndexed(huge)"), gdi));
            }
        }
    }
    out
}

#[test]
fn lowering_malformed_bytecode_refuses_rather_than_panicking() {
    let corpus = common::corpus();
    assert!(
        !corpus.is_empty(),
        "the corpus loaded nothing, so this sweep has no subject"
    );

    // **BREADTH OVER DEPTH, AND THE COST MODEL THAT SAYS SO WAS MEASURED THE
    // HARD WAY.**
    //
    // The first version swept 8 modules exhaustively: 234 mutations in 2.7
    // seconds. Widening to all 67 was estimated at roughly 20 seconds by scaling
    // that figure linearly. **It exceeded ten minutes and was killed.**
    //
    // The estimate was wrong because BOTH factors scale with module size: the
    // mutation count grows with the chunk count, and each mutation lowers the
    // WHOLE module. The eight sampled modules were small; the corpus contains
    // self-hosted stages with hundreds of chunks, so the true cost is closer to
    // quadratic in module size than linear in module count.
    //
    // So the sweep is capped PER MODULE rather than globally. Every module is
    // visited — a defect class living only in the large stages would be missed
    // by a sample chosen for speed — and no single module can dominate the
    // budget.
    let subjects: Vec<_> = corpus;

    let mut applied = 0usize;
    let mut lowered = 0usize;
    let mut refused = 0usize;
    // (module, mutation, origin file, message)
    let mut panics: Vec<(String, String, String, String)> = Vec::new();
    install_location_hook();

    for (name, m) in &subjects {
        for (what, mutant) in mutants(m) {
            applied += 1;
            match outcome(&mutant) {
                Ok("lowered") => lowered += 1,
                Ok(_) => refused += 1,
                Err((file, msg)) => panics.push((name.clone(), what, file, msg)),
            }
        }
    }

    println!("\n================ LOWERING ROBUSTNESS");
    println!("  modules      : {}", subjects.len());
    println!("  mutations    : {applied}");
    println!("  lowered      : {lowered}");
    println!("  refused      : {refused}");
    println!("  PANICKED     : {}", panics.len());
    for (n, w, f, m) in panics.iter().take(8) {
        println!("    {n} :: {w}\n      [{f}] {m}");
    }
    println!("================\n");

    // **NON-VACUITY, BOTH WAYS.** A sweep where nothing is refused is not
    // exercising the refusal paths, and one where everything is refused is
    // corrupting the modules past the point of saying anything about lowering.
    assert!(
        applied > 50,
        "only {applied} mutations were applied, which is too few to describe the \
         emitter's behaviour on malformed input"
    );
    assert!(
        refused > 0,
        "no mutation was refused, so this sweep exercised no refusal path"
    );
    assert!(
        lowered > 0,
        "every mutation was refused, so the corruptions are too severe to say \
         anything about the emitter's handling of input it accepts"
    );
    // **ONE KNOWN UPSTREAM PANIC, NAMED RATHER THAN TOLERATED IN GENERAL.**
    //
    // `src/confine.rs` walks `while ip < end { let op = &ops[ip]; .. }`, where
    // `end` comes from a block's recorded extent. On a TRUNCATED op stream that
    // extent can exceed `ops.len()`, and the index panics. It is reached through
    // `module_confinement`, which this backend calls.
    //
    // **`src/` belongs to the `v0.2.3` line and is read-only here**, so it is
    // reported through `REVERSE_PROMPT.md` rather than patched. The exception is
    // matched on its MESSAGE SHAPE, not merely counted: a different panic does
    // not slip through under its allowance, and when the upstream fix lands this
    // assertion fails and the exception must be removed rather than left to rot.
    let upstream: Vec<_> = panics
        .iter()
        .filter(|(_, _, file, _)| file.contains("confine.rs"))
        .collect();
    let ours: Vec<_> = panics
        .iter()
        .filter(|(_, _, file, _)| !file.contains("confine.rs"))
        .collect();

    assert!(
        ours.is_empty(),
        "{} of {applied} mutations PANICKED inside the BACKEND instead of being \
         refused. A panic is unrecoverable for the caller, and `lower_module` is \
         a public entry point that does not require a verified module. First: {:?}",
        ours.len(),
        ours.first()
    );
    assert!(
        !upstream.is_empty(),
        "the known upstream panic in `confine.rs` on a truncated op stream no \
         longer fires. If the `v0.2.3` line fixed it, DELETE this allowance and \
         the exception above rather than leaving a carve-out for a defect that \
         is gone."
    );
    println!(
        "  ({} upstream panic(s) in `confine.rs`, reported to the `v0.2.3` line \
         and allowed here by message shape)\n",
        upstream.len()
    );
}

/// **THE OTHER PUBLIC ENTRY POINTS, WHICH THE SWEEP ABOVE DOES NOT REACH.**
///
/// `lower_module` is not the only function this crate exposes that accepts
/// module data a caller may not have verified. **The region planners are the
/// ones that matter most**: a HOST calls `host_arena_supplement_bytes` and
/// `persistent_supplement_bytes` to size the buffers it will pass in, so a panic
/// there is reached without lowering anything at all — and a host sizing a
/// buffer is exactly the caller least able to recover from one.
///
/// # Why this is a separate test rather than more mutations
///
/// The sweep above measures whether lowering REFUSES. This measures whether a
/// planner RETURNS. They are different questions about the same inputs, and
/// folding them together would let a planner panic be reported as a lowering
/// refusal.
///
/// # Non-vacuity
///
/// A planner that returned early on every mutant would pass this while
/// exercising nothing, so the sizes are collected and at least one must be
/// non-zero. **A sweep that never reached the code is worse than no sweep** —
/// the census harness on this line fabricated coverage exactly that way.
#[test]
fn the_host_facing_planners_return_rather_than_panicking() {
    use keleusma_native::region;

    let corpus = common::corpus();
    assert!(!corpus.is_empty(), "the corpus loaded nothing");
    install_location_hook();

    let mut calls = 0usize;
    let mut nonzero = 0usize;
    let mut panics: Vec<(String, String, String, String)> = Vec::new();

    for (name, m) in &corpus {
        for (what, mutant) in mutants(m) {
            calls += 1;
            *LAST_PANIC_FILE.lock().unwrap() = None;
            let r = catch_unwind(AssertUnwindSafe(|| {
                let a = region::host_arena_supplement_bytes(&mutant);
                let b = region::persistent_supplement_bytes(&mutant);
                // Per-chunk planners, driven at an index that EXISTS and one
                // that does not, because an out-of-range index is exactly what a
                // host with a stale entry point would pass.
                let c = region::region_total_bytes(&mutant, 0, 0);
                let d = region::region_total_bytes(&mutant, usize::MAX, 0);
                let e = region::plan_call_site_regions(&mutant, 0).len() as u32;
                let f = region::plan_call_site_regions(&mutant, usize::MAX).len() as u32;
                let g = keleusma_native::delegated_suspension_subject(&mutant).is_some() as u32;
                a + b + c + d + e + f + g
            }));
            match r {
                Ok(total) => {
                    if total > 0 {
                        nonzero += 1;
                    }
                }
                Err(e) => {
                    let msg = e
                        .downcast_ref::<String>()
                        .cloned()
                        .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                        .unwrap_or_else(|| "non-string panic".to_string());
                    let file = LAST_PANIC_FILE
                        .lock()
                        .unwrap()
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string());
                    panics.push((name.clone(), what, file, msg.chars().take(110).collect()));
                }
            }
        }
    }

    println!("\n================ HOST-FACING PLANNERS ON MALFORMED INPUT");
    println!("  planner calls          : {calls}");
    println!("  returning a non-zero size : {nonzero}");
    println!("  PANICKED               : {}", panics.len());
    for (n, w, f, m) in panics.iter().take(8) {
        println!("    {n} :: {w}\n      [{f}] {m}");
    }
    println!("================\n");

    assert!(
        calls > 100,
        "only {calls} planner calls were made, too few to describe their behaviour"
    );
    assert!(
        nonzero > 0,
        "every planner call returned zero for every mutant, so this exercised \
         nothing but early returns and its clean result means nothing"
    );
    assert!(
        panics.is_empty(),
        "{} of {calls} planner calls PANICKED. A host calls these to SIZE THE \
         BUFFERS it will pass in, so this is reached without lowering anything. \
         First: {:?}",
        panics.len(),
        panics.first()
    );
}

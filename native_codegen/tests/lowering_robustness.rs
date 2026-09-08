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

mod common;

/// Lower `m`, reporting `Ok`, `Err`, or the panic message.
fn outcome(m: &Module) -> Result<&'static str, String> {
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
            Err(msg.chars().take(120).collect())
        }
    }
}

/// Structural corruptions, each a named function from a module to a module.
fn mutants(base: &Module) -> Vec<(String, Module)> {
    let mut out: Vec<(String, Module)> = Vec::new();
    for (ci, chunk) in base.chunks.iter().enumerate() {
        let n = chunk.ops.len();
        if n == 0 {
            continue;
        }
        // Sample rather than sweep: the point is coverage of KINDS, and a full
        // cross-product over a 1074-chunk corpus is a sweep this file is not.
        for &frac in &[0usize, 1, 2] {
            let at = n * frac / 3;

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

    // Bounded deliberately: enough modules for a range of shapes, few enough
    // that the sweep stays inside the everyday suite's budget.
    let subjects: Vec<_> = corpus.into_iter().take(8).collect();

    let mut applied = 0usize;
    let mut lowered = 0usize;
    let mut refused = 0usize;
    let mut panics: Vec<(String, String, String)> = Vec::new();

    for (name, m) in &subjects {
        for (what, mutant) in mutants(m) {
            applied += 1;
            match outcome(&mutant) {
                Ok("lowered") => lowered += 1,
                Ok(_) => refused += 1,
                Err(msg) => panics.push((name.clone(), what, msg)),
            }
        }
    }

    println!("\n================ LOWERING ROBUSTNESS");
    println!("  modules      : {}", subjects.len());
    println!("  mutations    : {applied}");
    println!("  lowered      : {lowered}");
    println!("  refused      : {refused}");
    println!("  PANICKED     : {}", panics.len());
    for (n, w, m) in panics.iter().take(12) {
        println!("    {n} :: {w}\n      {m}");
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
    let upstream: Vec<&(String, String, String)> = panics
        .iter()
        .filter(|(_, what, msg)| what.contains("truncated") && msg.contains("index out of bounds"))
        .collect();
    let ours: Vec<&(String, String, String)> = panics
        .iter()
        .filter(|p| !upstream.iter().any(|u| std::ptr::eq(*u, *p)))
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

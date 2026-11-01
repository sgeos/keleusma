//! **THE `Op::Reset` REGION-RETENTION PREMISE, CHECKED RATHER THAN ASSERTED.**
//!
//! The reference runtime resets the arena's ephemeral region at `Op::Reset`.
//! This backend emits nothing for it. The whole argument that the difference is
//! unobservable used to live in one sentence in the `Op::Reset` arm of
//! `src/lib.rs`:
//!
//! > every site has a fixed offset, so the next iteration overwrites exactly the
//! > bytes a reset would have reclaimed
//!
//! **That sentence over-claims, and this file is the measurement that shows it.**
//! An iteration that takes a branch skipping a construction site does NOT
//! overwrite it, and the previous iteration's bytes are still sitting there when
//! the stream suspends. `the_region_keeps_bytes_no_later_iteration_rewrote`
//! reads them back out of the host's buffer.
//!
//! # What actually makes the retention unobservable
//!
//! Two premises, neither of which is the sentence above:
//!
//! - **P1, provenance.** A site's bytes are reachable only through a pointer
//!   produced by that site's own `NewComposite`. No two sites share storage
//!   (`region_nonreuse.rs`) and no other op manufactures a region pointer.
//! - **P2, no pointer outlives its iteration.** Enumerated below, each with the
//!   mechanism that closes it.
//!
//! | route out of an iteration | mechanism | checked where |
//! |---|---|---|
//! | a local | every local is cleared at the back edge | `the_back_edge_clears_every_local`, here, with a control |
//! | an operand-stack entry | the back edge resets the depth to zero, which is the only depth the entry dispatch produces | indirectly, by `general_stream_sequence.rs` agreeing; no direct assertion exists |
//! | a spilled operand | the spill slice is addressed by depth, so depth zero abandons it | indirectly, as above |
//! | a value handed to the host at a `yield` | a composite that escapes its iteration is REFUSED | `the_escaping_composite_is_still_refused`, here, and `interproc_yield_escape.rs` |
//! | persistent storage | a composite data slot is REFUSED, because the access is one word wide | `private_slot_composite.rs` |
//!
//! **That last row was drafted as "a slot-homed composite is COPIED into
//! persistent bytes, not aliased", cited to `slot_homed_composites.rs`.** That
//! test measures the POPULATION of slot-homed composites and says nothing about
//! copy semantics. Reading the citation before filing it found the opposite of
//! what the row claimed: the store was an alias, and the route was a live defect
//! rather than a closed mechanism.
//!
//! **The two "indirectly" rows are stated rather than omitted.** They are the
//! routes this file does not close by direct assertion, and saying so is the
//! point: the recorded lesson on this line is that an instrument is blind to the
//! class next to the one that prompted it.
//!
//! # ⚠ WHAT THIS FILE FOUND, WHICH IS NOT WHAT IT WAS WRITTEN TO CHECK
//!
//! The differential leg below needed a subject with one extra local, purely as a
//! scaling control for the local-clearing count. **That subject diverged**: a
//! local written before a `yield` and read after it came back as zero natively
//! against the runtime's value, because the entry preamble zeroed the
//! non-parameter locals on EVERY call and a resumable stream is called once per
//! suspension. The initialisation now sits on the first-entry edge of the
//! dispatch.
//!
//! **No existing stream subject had a local live across a suspension**, which is
//! why the suite was green over it. The narrowest repro and the asymmetry that
//! makes the fix correct are pinned in `general_stream_sequence.rs`; the
//! class-level guard is `a_resumable_streams_entry_block_writes_only_the_parameter_slots`
//! below.
//!
//! # What this file cannot see
//!
//! - **Interprocedural retention.** Every subject here is single-chunk. A callee
//!   region is a disjoint block of the caller's, and a pointer returned from a
//!   callee is a separate recorded matter (`composite_return_aliasing.rs`).
//! - **A host that keeps a yielded pointer across a call.** That is the
//!   yield-escape hazard, refused rather than checked here.
//! - **Anything about the reference's own arena.** This measures the backend's
//!   buffer, and agreement with the runtime is measured by yielded values, not by
//!   comparing the two memories.

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::vm::required_persistent_capacity_for;
use keleusma_native::{LowerOptions, lower_module, module_refusals, region};

mod common;

/// A stream whose construction site is inside an `if`, so an iteration can skip
/// it. The yielded values are `Word`s, which keeps the differential simple and
/// keeps the retained bytes reachable ONLY through the site pointer.
const SUBJECT: &str = "struct P { a: Word, b: Word }\n\
                       loop main(t: Word) -> Word { \
                         let v = if t > 0 { let p = P { a: t + 7, b: t + 9 }; p.a } else { 0 }; \
                         let r = yield v; \
                         yield r + 1 }";

/// The same shape with one extra local, used as the scaling control for the
/// local-clearing count. Nothing else about it differs.
const SUBJECT_ONE_MORE_LOCAL: &str = "struct P { a: Word, b: Word }\n\
                       loop main(t: Word) -> Word { \
                         let extra = t + 1; \
                         let v = if t > 0 { let p = P { a: t + 7, b: t + 9 }; p.a } else { 0 }; \
                         let r = yield v; \
                         yield r + extra }";

/// The byte this file writes into the region before running. Chosen because it
/// is not a plausible program value: a zero-filled buffer cannot tell "never
/// written" from "written zero", which is the mistake this leg exists to avoid.
const POISON: u8 = 0xCD;

/// Run a general stream natively over a POISONED region, and hand back both the
/// yielded values and the region buffer.
///
/// A near-copy of `common::general_native_sequence`, which owns its buffer and
/// cannot expose it. The duplication is deliberate: the poison and the buffer
/// hand-back are the whole subject here, and pushing them into the shared helper
/// would change every caller's memory image to serve one test.
fn poisoned_native_run(src: &str, first: i64, replies: &[i64]) -> (Vec<i64>, Vec<u8>) {
    let m = common::build(src);
    let entry = m.entry_point.expect("entry point");
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("lower module");
    lm.verify().expect("LLVM module verification");
    common::maybe_optimize(&lm);
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");

    let sym = format!("kel_chunk_{entry}");
    let f = lm.get_function(&sym).expect("entry function");
    assert_eq!(
        f.count_params(),
        u32::from(m.chunks[entry].param_count) + 3,
        "a resumable stream must carry the three trailing pointers; the call below \
         names that signature by hand and cannot detect a change to it"
    );
    let callable = unsafe {
        ee.get_function::<unsafe extern "C" fn(i64, *mut u8, *mut u8, *mut u8) -> i64>(&sym)
    }
    .expect("entry symbol");

    let persistent =
        required_persistent_capacity_for(&m) + region::persistent_supplement_bytes(&m) as usize;
    let mut privs = vec![0u8; persistent + 64];
    let mut shared = vec![0u8; 4096];
    let mut buffer = vec![POISON; region::host_arena_supplement_bytes(&m) as usize + 4096];

    let mut out = Vec::new();
    let mut input = first;
    for &r in replies {
        out.push(unsafe {
            callable.call(
                input,
                shared.as_mut_ptr(),
                privs.as_mut_ptr(),
                buffer.as_mut_ptr(),
            )
        });
        input = r;
    }
    (out, buffer)
}

/// The one construction site of `SUBJECT`, taken from the backend's OWN planner.
///
/// Never written as a literal. A second computation of an offset the emitter
/// also computes is free to drift from it, which is the defect a hard-coded
/// "+3 pointers" already caused once in `declared_float_width.rs`.
fn site_bytes(src: &str, buffer: &[u8]) -> Vec<u8> {
    let m = common::build(src);
    let entry = m.entry_point.expect("entry point");
    let layout = region::plan_chunk_region(&m.chunks[entry]);
    assert_eq!(
        layout.sites.len(),
        1,
        "this file reads ONE site by index; the subject now plans {} and the read \
         below would be silently pointed at the wrong bytes",
        layout.sites.len()
    );
    let s = &layout.sites[0];
    let lo = s.offset as usize;
    buffer[lo..lo + s.size as usize].to_vec()
}

/// Two little-endian words out of a 16-byte flat body.
fn two_words(bytes: &[u8]) -> (i64, i64) {
    assert_eq!(bytes.len(), 16, "the subject's body is two Words");
    let w = |i: usize| i64::from_le_bytes(bytes[i..i + 8].try_into().unwrap());
    (w(0), w(8))
}

/// **LEG 1. The retained bytes are real, and three states tell them apart.**
///
/// Run A's second iteration skips the site; run B's builds it. Both start from
/// the same poisoned buffer and the same first iteration, so the only thing that
/// differs is whether the constructor ran again.
#[test]
fn the_region_keeps_bytes_no_later_iteration_rewrote() {
    // Iteration 1 has t = 5, so the site holds (12, 14).
    // Run A: the third call resumes with t = 0, so the `if` is false and the
    // site is not touched again.
    let (seq_a, buf_a) = poisoned_native_run(SUBJECT, 5, &[3, 0, 1]);
    // Run B: the third call resumes with t = 1, so the site is rebuilt as (8, 10).
    let (seq_b, buf_b) = poisoned_native_run(SUBJECT, 5, &[3, 1, 1]);

    let a = two_words(&site_bytes(SUBJECT, &buf_a));
    let b = two_words(&site_bytes(SUBJECT, &buf_b));

    // **Non-vacuity, across the WHOLE body rather than its first word.** If the
    // site were never written, both reads would be the poison and the comparison
    // below would still "pass" by matching nothing. Checking one word would also
    // miss a PARTIAL write, which is a different way for the retention argument
    // to be wrong: bytes a constructor never reached are not bytes an earlier
    // iteration owned.
    for (run, buf) in [("A", &buf_a), ("B", &buf_b)] {
        let body = site_bytes(SUBJECT, buf);
        assert!(
            !body.contains(&POISON),
            "run {run} left poison in the body at the construction site, so part of \
             it was never written: {body:?}"
        );
    }

    assert_eq!(
        a,
        (12, 14),
        "run A's site should still hold ITERATION ONE's body: the second iteration \
         skipped the constructor and nothing reset the region. seq={seq_a:?}"
    );
    assert_eq!(
        b,
        (8, 10),
        "run B's site should hold ITERATION TWO's body, which is what proves the \
         read in run A is measuring retention and not a site that is simply never \
         rewritten. seq={seq_b:?}"
    );
    assert_ne!(
        a, b,
        "the two runs must be distinguishable, or leg 1 discriminates nothing"
    );
}

/// **LEG 2. And none of it reaches the runtime's answer.**
///
/// The same two programs, compared against the reference yield by yield. The
/// helper refuses to run if the backend declines the subject, so this cannot
/// pass by asserting agreement about a program that never ran.
#[test]
fn the_retained_bytes_change_nothing_the_runtime_can_see() {
    common::assert_general_stream_agrees(SUBJECT, 5, &[3, 0, 1]);
    common::assert_general_stream_agrees(SUBJECT, 5, &[3, 1, 1]);
    common::assert_general_stream_agrees(SUBJECT_ONE_MORE_LOCAL, 5, &[3, 0, 1]);
}

/// The text of the entry chunk's function.
fn entry_ir(src: &str) -> String {
    let m = common::build(src);
    let entry = m.entry_point.expect("entry point");
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("lower module");
    let ir = lm.print_to_string().to_string();
    let head = format!("define i64 @kel_chunk_{entry}(");
    let start = ir.find(&head).expect("the entry function is emitted");
    let rest = &ir[start..];
    let end = rest.find("\n}").expect("a terminated function body");
    rest[..end].to_string()
}

/// The function's basic blocks as `(label, text)`, split on unindented labels.
fn blocks_of(ir: &str) -> Vec<(String, String)> {
    let mut blocks: Vec<(String, String)> = Vec::new();
    // The `define` line before the first label is its own entry, named so it
    // cannot be confused with the block LLVM calls `entry`.
    let mut name = String::from("<signature>");
    let mut body = String::new();
    for line in ir.lines() {
        let label = (!line.starts_with(' ') && !line.starts_with('\t'))
            .then(|| line.split(':').next().unwrap_or(""))
            .filter(|l| !l.is_empty() && l.chars().all(|c| c.is_alphanumeric() || c == '_'))
            .filter(|_| line.contains(':'));
        if let Some(l) = label {
            blocks.push((std::mem::take(&mut name), std::mem::take(&mut body)));
            name = l.to_string();
        }
        body.push_str(line);
        body.push('\n');
    }
    blocks.push((name, body));
    blocks
}

/// The block the back edge jumps FROM, identified structurally.
///
/// The loop top is the `default` label of the entry dispatch's `switch`, which is
/// where `Op::Reset` branches. Reading the label out of the dispatch rather than
/// guessing a name is what keeps this from breaking on a renumbering.
fn back_edge_block(ir: &str) -> String {
    let key = "switch i64 %state, label %";
    let at = ir
        .find(key)
        .expect("a general stream dispatches on its state");
    let dispatch_default: String = ir[at + key.len()..]
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();

    let blocks = blocks_of(ir);

    // The first-entry block branches to the loop top too, so the top is found
    // through the dispatch default and then through that block's own branch.
    // Matching the WHOLE terminator line matters: `br label %op1` is a prefix of
    // `br label %op19`, and a substring match found three back edges where there
    // is one.
    let terminator_to = |b: &str, t: &str| {
        b.lines()
            .map(str::trim)
            .any(|l| l == format!("br label %{t}"))
    };
    let init = blocks
        .iter()
        .find(|(n, _)| *n == dispatch_default)
        .expect("the dispatch default names a block");
    let top: String = init
        .1
        .lines()
        .map(str::trim)
        .find_map(|l| l.strip_prefix("br label %").map(str::to_string))
        .unwrap_or_else(|| dispatch_default.clone());

    let mut hits: Vec<String> = blocks
        .iter()
        .filter(|(n, _)| *n != dispatch_default && *n != top)
        .filter(|(_, b)| terminator_to(b, &top))
        .map(|(_, b)| b.clone())
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "exactly one block other than the first-entry block should branch to the \
         loop top: the back edge. Found {}",
        hits.len()
    );
    hits.pop().unwrap()
}

fn zero_stores_to_locals(block: &str) -> usize {
    block
        .lines()
        .filter(|l| l.trim_start().starts_with("store i64 0, ptr %l"))
        .count()
}

/// **LEG 3, route one. Every local is cleared at the back edge — with a control
/// that makes the count move.**
///
/// A bare "the block contains some zero stores" would pass against a lowering
/// that cleared one local and forgot the rest. The two subjects differ by
/// exactly one local, so the counts must differ by exactly one.
#[test]
fn the_back_edge_clears_every_local() {
    let m = common::build(SUBJECT);
    let entry = m.entry_point.expect("entry point");
    let locals = usize::from(m.chunks[entry].local_count);

    let m2 = common::build(SUBJECT_ONE_MORE_LOCAL);
    let entry2 = m2.entry_point.expect("entry point");
    let locals2 = usize::from(m2.chunks[entry2].local_count);
    assert_eq!(
        locals2,
        locals + 1,
        "the control subject must differ by exactly one local, or the difference \
         below tests nothing"
    );

    let cleared = zero_stores_to_locals(&back_edge_block(&entry_ir(SUBJECT)));
    let cleared2 = zero_stores_to_locals(&back_edge_block(&entry_ir(SUBJECT_ONE_MORE_LOCAL)));

    assert_eq!(
        cleared, locals,
        "the back edge must clear every local, not merely some"
    );
    assert_eq!(
        cleared2 - cleared,
        locals2 - locals,
        "one more local must produce exactly one more clear; a count that does not \
         move with the population is not measuring the population"
    );
}

/// **LEG 3, and the fact the old comment obscured: NO REGION RESET IS EMITTED.**
///
/// Recorded as an observed property rather than left as an implication of prose.
/// The back edge writes locals and the resume state, and touches no site byte.
#[test]
fn the_back_edge_resets_no_composite_bytes() {
    let block = back_edge_block(&entry_ir(SUBJECT));
    let stores: Vec<&str> = block
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("store "))
        .collect();
    assert!(
        !stores.is_empty(),
        "the back edge emits no stores at all, so this check is reading the wrong \
         block"
    );
    for s in &stores {
        assert!(
            s.contains(", ptr %l") || s.contains(", ptr %statep"),
            "the back edge stores somewhere other than a local or the resume state: \
             {s:?}. If that is a region clear, this backend has started paying for \
             one and the premise this file documents has changed."
        );
    }
}

/// **LEG 3, route four. A composite that leaves its iteration is still refused.**
///
/// The pair is the control: the subject here is accepted, so the refusal below is
/// about the escape and not about streams in general.
#[test]
fn the_escaping_composite_is_still_refused() {
    let accepted = module_refusals(&common::build(SUBJECT), LowerOptions::default());
    assert!(
        accepted.is_empty(),
        "the control must be ACCEPTED or the refusal below says nothing specific: \
         {accepted:?}"
    );

    let src = std::fs::read_to_string("../examples/scripts/13_telemetry_stream.kel")
        .expect("the corpus stream that yields a composite out of a bounded loop");
    let m = common::try_build(&src).expect("the corpus module compiles");
    let refusals = module_refusals(&m, LowerOptions::default());
    assert!(
        !refusals.is_empty(),
        "a composite yielded out of a user loop must still be refused; if this ever \
         passes, the pointer-escape route in this file's table has lost its \
         mechanism and the retention argument loses a leg"
    );
}

/// **THE GUARD CAST AGAINST THE CLASS, NOT AGAINST THE DEFECT.**
///
/// The defect was one unconditional write in the entry block: the non-parameter
/// locals were zeroed on every call, and a resumable stream is called once per
/// suspension, so a local live across a `yield` was wiped on the way back in.
/// The subject that found it yielded `[1, 3]` against the runtime's `[1, 108]`.
///
/// A test that counted zero-stores would be shaped by that instance. **The class
/// is "anything written unconditionally at entry"** -- every such write happens
/// again on each resume, so the entry block of a resumable stream may write the
/// parameter slots and nothing else. The runtime's resume writes slot 0, which is
/// what makes those the exception rather than an inconsistency.
///
/// Lives in this file rather than with the sequence differentials because it uses
/// the block machinery above, and because it is the same question those ask from
/// the other side: what survives an iteration boundary, and what must not.
#[test]
fn a_resumable_streams_entry_block_writes_only_the_parameter_slots() {
    let m = common::build(SUBJECT);
    let entry = m.entry_point.expect("entry point");
    let params = usize::from(m.chunks[entry].param_count);
    let locals = usize::from(m.chunks[entry].local_count);
    assert!(
        locals > params,
        "the subject must have non-parameter locals, or there is nothing this \
         check could catch"
    );

    let ir = entry_ir(SUBJECT);
    let blocks = blocks_of(&ir);
    let (_, entry_block) = blocks
        .iter()
        .find(|(n, _)| n == "entry")
        .expect("a function has an entry block");
    let stores: Vec<&str> = entry_block
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("store "))
        .collect();

    // Non-vacuity: the parameter store must be there, or the filter is reading
    // the wrong block and an empty list would "pass".
    assert!(
        !stores.is_empty(),
        "the entry block stores nothing at all, so this check is reading the wrong \
         block"
    );
    let allowed: Vec<String> = (0..params).map(|i| format!(", ptr %l{i},")).collect();
    for s in &stores {
        assert!(
            allowed.iter().any(|a| s.contains(a.as_str())),
            "a resumable stream's entry block writes {s:?}, which will be re-executed \
             on EVERY resume. If it is an initialisation, it belongs on the \
             first-entry edge of the dispatch; if it is state, it belongs in the \
             persistent region."
        );
    }

    // **The control.** A chunk entered once per call SHOULD carry the
    // initialisation in its entry block, which proves the check above is
    // discriminating between the two entry disciplines rather than passing
    // because no lowering ever writes at entry.
    // The entry point itself must hold the local, since `entry_ir` reads the
    // entry chunk: a helper function's locals are in a different body.
    let plain = entry_ir("fn main() -> Word { let b = 1; b + 1 }");
    let plain_blocks = blocks_of(&plain);
    let (_, plain_entry) = plain_blocks
        .iter()
        .find(|(n, _)| n == "entry")
        .expect("an entry block");
    assert!(
        plain_entry
            .lines()
            .map(str::trim)
            .any(|l| l.starts_with("store i64 0, ptr %l")),
        "a non-stream chunk should still initialise its locals at entry; if it does \
         not, the deferral was applied too widely"
    );
}

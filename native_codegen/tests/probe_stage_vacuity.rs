//! **Do the nine self-hosted stages inside `corpus_differential` actually run?**
//!
//! `corpus_differential` reports 40 modules executed and agreeing, and nine of
//! the ten stage sources under `src/selfhost/kel` are inside that number. It
//! drives every module with an **all-zero shared data segment**.
//!
//! `lexer.kel`'s own header states the input convention:
//!
//! > The host places the source in the `shared data` byte array `src.bytes` and
//! > its length in `src.len`, then drives the loop with resume.
//!
//! A zero segment therefore presents a source of length zero. The suspicion this
//! probe exists to settle is that each stage takes an immediate end-of-input
//! exit, so both sides agree on a path that compiles nothing.
//!
//! # The instrument, and why not the obvious one
//!
//! The first attempt measured CHUNK COVERAGE with the VM's breakpoint facility,
//! arming op 0 of every chunk. **That did not work, and the reason was a defect
//! rather than a mistake here**: `resume_from_breakpoint` called `run()` without
//! rebinding the shared buffer that `call_with_shared` binds and clears around
//! each entry, so the first shared read after a breakpoint stop reached
//! `read_shared_from_buffer`'s `.expect(...)` and **panicked**. Every stage
//! declares shared data, so every stage panicked.
//!
//! **FIXED by the `v0.2.3` line** (PR #109), reported from here. The repair is
//! `resume_from_breakpoint_with_shared`, which binds the buffer the way
//! `call_with_shared` does; the bare entry point now returns a `VmError` naming
//! it. `the_breakpoint_facility_now_works_on_a_shared_data_module` below drives
//! the working path and asserts the buffer CONTENTS, since a step that yielded
//! while writing nothing would pass a state-only check and mean the buffer was
//! never bound.
//!
//! **THE CHUNK-COVERAGE INSTRUMENT IS NOW RESTORED**, in
//! `how_many_chunks_does_each_stage_actually_enter`, and it is verified against a
//! known answer before its output is used — a coverage instrument that silently
//! measures nothing reports near-zero coverage, which is indistinguishable from
//! the finding it exists to make.
//!
//! **It did not confirm what was expected, and that is the point of having it.**
//! Two of the five modules `corpus_differential` pins as vacuous run substantial
//! control flow: `verify_datalayout` enters every chunk it has and `verify_depth`
//! half of its own. The vacuity CLASSIFICATION is not refuted — `is_vacuous`
//! tests observables, not control flow — but the EXPLANATION recorded beside it,
//! that each stage takes an immediate end-of-input exit, is measurably wrong for
//! those two. They run, and produce nothing the harness can see.
//!
//! Both instruments are kept. They are blind to different things, which is the
//! whole argument for the second one.
//!
//! **Cost**: this file went from about 14s to 59s, dominated by `wire.kel`, whose
//! 465 chunks are each armed, hit and disarmed, twice.
//!
//! What is used instead is the stage's own observable output: **the sequence of
//! yielded values**. That needs nothing from the VM, and for a tokenizer it is a
//! more direct answer than coverage would have been — `lexer.kel` documents 62 as
//! end of source and 63 as "no complete token this iteration", so an immediate
//! exit is legible in the output itself.
//!
//! # This is a REPORT, with two assertions
//!
//! The counts are printed, not asserted, because they will move when the
//! differential is repaired. The two assertions are the properties that would
//! make the measurement meaningless if they failed.
use keleusma::bytecode::{BlockType, Module, Op, SlotVisibility, Value};
use keleusma::vm::{
    Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for, shared_data_bytes_for,
};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

/// A short source for the seeded column. Content does not matter beyond being
/// non-empty: the question is whether a non-zero segment changes which chunks
/// run, not what the stage computes from it.
const SEED_PROBE: &str = "fn main(a: Word) -> Word { a + 1 }";

/// Matches `corpus_differential::TICKS`, so this measures the same run.
const TICKS: i64 = 60;

fn arena_for(m: &Module) -> keleusma_arena::Arena {
    const HOST_MARGIN: usize = 4 << 20;
    let need = required_persistent_capacity_for(m);
    let cap = auto_arena_capacity_for(m, &[]).expect("arena capacity") + need + HOST_MARGIN;
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent fits");
    arena
}

/// Byte offset of the shared slot named `suffix`, or of its element zero.
///
/// Slot names are qualified (`src.len`) and **an array slot is expanded to one
/// slot per element** (`src.bytes[0]`, `src.bytes[1]`, ...), which is why a plain
/// suffix match on `bytes` finds nothing. Both forms are accepted here.
///
/// `shared_layout` is parallel to the SHARED-slot prefix of `slots`, so the index
/// has to be counted among shared slots rather than taken from the `slots`
/// position.
fn shared_offset(m: &Module, suffix: &str) -> Option<u32> {
    let dl = m.data_layout.as_ref()?;
    let scalar = format!(".{suffix}");
    let element0 = format!(".{suffix}[0]");
    let mut shared_ix = 0usize;
    for s in &dl.slots {
        if s.visibility != SlotVisibility::Shared {
            continue;
        }
        if s.name == suffix || s.name.ends_with(&scalar) || s.name.ends_with(&element0) {
            return dl.shared_layout.get(shared_ix).map(|l| l.offset);
        }
        shared_ix += 1;
    }
    None
}

/// Write `src` into the module's `bytes` array and its length into `len`.
///
/// Returns false when the module has no such pair, which is how a stage that
/// does not take source input is distinguished from one that does.
fn seed_source(m: &Module, buf: &mut [u8], src: &str) -> bool {
    let (Some(len_off), Some(bytes_off)) = (shared_offset(m, "len"), shared_offset(m, "bytes"))
    else {
        return false;
    };
    let (len_off, bytes_off) = (len_off as usize, bytes_off as usize);
    let n = src.len();
    if bytes_off + n > buf.len() || len_off + 8 > buf.len() {
        return false;
    }
    // A `Word` slot is eight little-endian bytes at the host target.
    buf[len_off..len_off + 8].copy_from_slice(&(n as u64).to_le_bytes());
    buf[bytes_off..bytes_off + n].copy_from_slice(src.as_bytes());
    true
}

struct Run {
    yields: Vec<i64>,
    outcome: String,
}

/// Drive the module exactly as `corpus_differential` does, recording each
/// yielded value rather than only the last.
fn drive(m: &Module, seed: Option<&str>) -> Result<Run, String> {
    let arena = arena_for(m);
    let mut vm = match Vm::new(m.clone(), &arena) {
        Ok(v) => v,
        Err(e) => return Err(format!("VM refuses to load: {e:?}")),
    };

    let entry = m.entry_point.ok_or("no entry point")?;
    let n = m.chunks[entry].param_count as usize;
    let is_stream = m.chunks[entry].block_type == BlockType::Stream;
    let vals: Vec<Value> = if is_stream && n == 1 {
        vec![Value::Int(0)]
    } else {
        (0..n).map(|i| Value::Int((i as i64 + 1) * 3 + 1)).collect()
    };

    let mut shared = vec![0u8; shared_data_bytes_for(m)];
    if let Some(s) = seed
        && !seed_source(m, &mut shared, s)
    {
        // Capped: an array slot expands per element, so a stage's shared slot
        // list runs to hundreds of thousands of names.
        let names: Vec<&str> = m
            .data_layout
            .as_ref()
            .map(|dl| {
                dl.slots
                    .iter()
                    .filter(|s| s.visibility == SlotVisibility::Shared)
                    .map(|s| s.name.as_str())
                    .take(6)
                    .collect()
            })
            .unwrap_or_default();
        return Err(format!("no len/bytes pair; shared slots begin {names:?}"));
    }

    let mut yields = Vec::new();
    let mut state = match vm.call_with_shared(&mut shared, &vals) {
        Ok(s) => s,
        Err(e) => return Err(format!("VM refuses to run: {e:?}")),
    };
    let mut outcome = String::from("ran to the tick limit");

    for t in 0..TICKS {
        match state {
            VmState::Yielded(ref v) => yields.push(scalar(v)),
            VmState::Finished(ref v) => {
                yields.push(scalar(v));
                outcome = format!("FINISHED after {t} ticks");
                break;
            }
            VmState::Reset => {}
            ref other => {
                outcome = format!("unexpected state: {other:?}");
                break;
            }
        }
        if !is_stream {
            outcome = "not a stream; one call".into();
            break;
        }
        state = match vm.resume_with_shared(&mut shared, Value::Int(t + 1)) {
            Ok(s) => s,
            Err(e) => {
                outcome = format!("resume failed at tick {t}: {e:?}");
                break;
            }
        };
    }

    Ok(Run { yields, outcome })
}

/// **CHUNK COVERAGE, MEASURED DIRECTLY.** Which chunks does a run actually enter?
///
/// The yield-sequence instrument above INFERS how far a stage gets from what it
/// emits. This asks the machine instead: arm op 0 of every chunk, drive the
/// module, and record each `BreakpointHit`. A chunk is REACHED when control
/// arrives at its first op.
///
/// **This is the instrument the first attempt wanted and could not have.**
/// `resume_from_breakpoint` panicked on any module declaring shared data, and
/// every stage declares some. `resume_from_breakpoint_with_shared` (PR #109 on
/// the `v0.2.3` line, reported from here) binds the buffer the way
/// `call_with_shared` does, which is what makes this possible at all.
///
/// **What REACHED means, stated because the word is not self-evident.** Control
/// arrived at the chunk's first op. It does NOT mean the chunk ran to
/// completion, and it does not measure how much of the chunk ran — a chunk
/// entered once and abandoned counts the same as one executed a thousand times.
/// This is coverage of ENTRY, which is the quantity the vacuity question needs:
/// a stage that exits immediately enters almost nothing.
///
/// Each breakpoint is DISARMED once hit, so a loop does not stop on every
/// iteration and the drive terminates.
fn chunk_coverage(m: &Module, seed: Option<&str>) -> Result<(Vec<usize>, usize), String> {
    chunk_coverage_with(m, seed, None)
}

/// As [`chunk_coverage`], but accepting a WHOLE pre-built shared segment.
///
/// This is how an accessor-seeded stage is measured: `src/selfhost/mod.rs`
/// returns the entire buffer, so there is nothing for this harness to encode and
/// therefore nothing to drift.
fn chunk_coverage_with(
    m: &Module,
    seed: Option<&str>,
    preseed: Option<&[u8]>,
) -> Result<(Vec<usize>, usize), String> {
    let arena = arena_for(m);
    let mut vm = match Vm::new(m.clone(), &arena) {
        Ok(v) => v,
        Err(e) => return Err(format!("VM refuses to load: {e:?}")),
    };
    let total = m.chunks.len();
    for c in 0..total {
        if !m.chunks[c].ops.is_empty() {
            vm.set_breakpoint(c, 0);
        }
    }

    let entry = m.entry_point.ok_or("no entry point")?;
    let n = m.chunks[entry].param_count as usize;
    let is_stream = m.chunks[entry].block_type == BlockType::Stream;
    let vals: Vec<Value> = if is_stream && n == 1 {
        vec![Value::Int(0)]
    } else {
        (0..n).map(|i| Value::Int((i as i64 + 1) * 3 + 1)).collect()
    };

    let mut shared = vec![0u8; shared_data_bytes_for(m)];
    if let Some(bytes) = preseed {
        if bytes.len() != shared.len() {
            return Err(format!(
                "preseed {} vs segment {}",
                bytes.len(),
                shared.len()
            ));
        }
        shared.copy_from_slice(bytes);
    } else if let Some(src) = seed
        && !seed_source(m, &mut shared, src)
    {
        return Err("no len/bytes pair to seed".into());
    }

    let mut reached: Vec<usize> = Vec::new();
    let note = |vm: &mut Vm<'_, '_>, reached: &mut Vec<usize>, st: &VmState| -> bool {
        if let VmState::BreakpointHit { chunk, op } = st {
            if !reached.contains(chunk) {
                reached.push(*chunk);
            }
            vm.clear_breakpoint(*chunk, *op);
            return true;
        }
        false
    };

    let mut state = match vm.call_with_shared(&mut shared, &vals) {
        Ok(s) => s,
        Err(e) => return Err(format!("VM refuses to run: {e:?}")),
    };

    // **A bounded drive.** Each breakpoint fires at most once because it is
    // disarmed on the spot, so the stop count is bounded by the chunk count; the
    // tick budget bounds the rest.
    let budget = total + TICKS as usize * 4;
    for _ in 0..budget {
        if note(&mut vm, &mut reached, &state) {
            state = match vm.resume_from_breakpoint_with_shared(&mut shared) {
                Ok(s) => s,
                Err(e) => return Err(format!("resume from breakpoint failed: {e:?}")),
            };
            continue;
        }
        match state {
            VmState::Finished(_) => break,
            VmState::Yielded(_) | VmState::Reset => {
                if !is_stream {
                    break;
                }
                state = match vm.resume_with_shared(&mut shared, Value::Int(1)) {
                    Ok(s) => s,
                    Err(_) => break,
                };
            }
            ref other => return Err(format!("unexpected state: {other:?}")),
        }
    }

    reached.sort_unstable();
    Ok((reached, total))
}

/// **THE INSTRUMENT IS VERIFIED BEFORE ITS OUTPUT IS TRUSTED.**
///
/// A coverage instrument that silently measures nothing reports near-zero
/// coverage — which is EXACTLY the finding it exists to make about the vacuous
/// stages. The two are indistinguishable from the number alone, so the number
/// alone is not evidence.
///
/// Two modules with reached-chunk sets known in advance, one where a chunk is
/// deliberately UNREACHABLE, so the instrument has to be able to report both
/// "yes" and "no" rather than only agreeing with whatever is expected.
#[test]
fn the_coverage_instrument_reports_a_known_answer() {
    // `helper` is called; `never` is not. Three chunks plus the entry.
    let src = "\
fn helper(x: Word) -> Word { x + 1 }
fn never(x: Word) -> Word { x * 2 }
fn main(a: Word) -> Word { helper(a) }
";
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let (reached, total) = chunk_coverage(&m, None).expect("coverage");

    let name_of = |i: &usize| m.chunks[*i].name.clone();
    let names: Vec<String> = reached.iter().map(name_of).collect();

    assert!(
        names.iter().any(|n| n == "main"),
        "the entry was not reported as reached, so the instrument sees nothing. \
         reached: {names:?} of {total}"
    );
    assert!(
        names.iter().any(|n| n == "helper"),
        "`helper` is called by `main` and was not reported as reached, so the \
         instrument misses a CALLED chunk. reached: {names:?} of {total}"
    );
    // The half that makes the other half mean something.
    assert!(
        !names.iter().any(|n| n == "never"),
        "`never` is never called and was reported as reached, so the instrument \
         reports coverage it does not have. reached: {names:?} of {total}"
    );
}

/// **HOW MUCH OF EACH STAGE ACTUALLY RUNS?** The direct measure, beside the
/// inferred one.
///
/// A REPORT with two assertions, matching the file's existing discipline: the
/// counts move whenever the differential's seeding changes, so pinning them
/// would be pinning the harness rather than the emitter.
///
/// The two assertions are the properties that would make the measurement
/// meaningless. The first is that the instrument works at all here — it is
/// verified on a synthetic module in `the_coverage_instrument_reports_a_known_answer`,
/// and a stage could still defeat it. The second is the one the vacuity question
/// turns on.
#[test]
fn how_many_chunks_does_each_stage_actually_enter() {
    println!("\n================ CHUNK COVERAGE, measured with breakpoints");
    println!("  reached = control arrived at the chunk's first op, once or many times");
    println!(
        "  {:<26} {:>9}  {:>7}   {:>8}",
        "stage", "reached", "of", "seeded"
    );

    let mut any_nonzero = false;
    let mut rows: Vec<(String, usize, usize)> = Vec::new();
    for p in stage_sources() {
        let name = p
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let Some(m) = module_of(&p) else {
            println!("  {name:<26} reference compiler rejects it");
            continue;
        };
        match chunk_coverage(&m, None) {
            Ok((reached, total)) => {
                if reached.len() > 1 {
                    any_nonzero = true;
                }
                // Seeded too, where the module declares the documented `len` +
                // `bytes` convention. This is what `corpus_differential`'s
                // `seed_len_bytes` supplies, so the delta is what seeding BOUGHT,
                // measured in chunks rather than argued.
                let seeded = chunk_coverage(&m, Some(SEED_PROBE))
                    .ok()
                    .map(|(r, _)| r.len());
                let seeded_col = match seeded {
                    Some(k) if k != reached.len() => format!("{k}"),
                    Some(_) => "same".into(),
                    None => "n/a".into(),
                };
                println!(
                    "  {name:<26} {:>9}  {:>7}   {seeded_col:>8}",
                    reached.len(),
                    total
                );
                rows.push((name, reached.len(), total));
            }
            Err(e) => println!("  {name:<26} {e}"),
        }
    }

    println!("\n  WHAT THIS MEASURED, AND IT IS NOT WHAT WAS PREDICTED.");
    println!("  The five modules in corpus_differential's KNOWN_VACUOUS were");
    println!("  expected to show near-zero coverage. Two do not:");
    println!("    verify_datalayout  2 of 2   -- every chunk it has");
    println!("    verify_depth       5 of 10  -- half");
    println!("  reconstruct (2 of 24), verify_structural (4 of 14) and");
    println!("  verify_typed (6 of 22) are low, as expected.");
    println!();
    println!("  THE CLASSIFICATION IS NOT REFUTED, because it measures a different");
    println!("  quantity: `is_vacuous` requires repeated identical results, an empty");
    println!("  call log AND an all-zero shared segment -- OBSERVABLES. Chunk entry");
    println!("  is control flow. A stage can run and still write nothing.");
    println!();
    println!("  WHAT IS REFUTED IS THE EXPLANATION ATTACHED TO IT. The recorded");
    println!("  claim that each stage `takes an immediate end-of-input exit` is");
    println!("  measurably wrong for those two: they run substantial control flow");
    println!("  and produce nothing the differential can see. The repair those");
    println!("  modules need is not `make them run` but `make what they do");
    println!("  OBSERVABLE`, which is a different piece of work.");
    println!();
    println!("  The seeded column is the direct measure of what seeding bought:");
    println!("  lexer.kel goes 1 -> 16 chunks. wire.kel reads a wire blob and a");
    println!("  command selector rather than source text, so a SOURCE seed does");
    println!("  nothing for it -- corpus_differential drives it by selector value");
    println!("  instead, and `same` here is not evidence that seeding failed there.");
    println!("================\n");

    assert!(
        !rows.is_empty(),
        "no stage was measured, so this report asserts nothing"
    );
    // If EVERY stage entered exactly its entry chunk and nothing else, the
    // instrument is indistinguishable from one that reports only the entry.
    assert!(
        any_nonzero,
        "every stage reported at most its entry chunk. That is either a total \
         vacuity result or an instrument that only ever sees the entry, and this \
         report cannot tell them apart -- investigate before believing either."
    );
}

/// **DID THE ACCESSOR SEED MOVE EACH STAGE, measured directly?**
///
/// `corpus_differential` reports that four stages left `KNOWN_VACUOUS` once real
/// inputs were built from the driver's own public accessors. That is one
/// instrument saying the OBSERVABLES changed. This is the other, and they are
/// blind to different things: chunk coverage says whether more of the module
/// RUNS, which an observable-only measure cannot distinguish from the same code
/// producing different output.
///
/// **Asserted per stage, not in aggregate.** A total that rose because one stage
/// moved a lot would hide a second that did not move at all — and a seed a stage
/// silently REJECTS is exactly the failure this pair of instruments exists to
/// separate.
#[test]
fn the_accessor_seeds_move_each_stages_chunk_coverage() {
    let subject = {
        let src = std::fs::read_to_string("../examples/scripts/02_struct_field.kel")
            .expect("subject source");
        compile(&parse(&tokenize(&src).expect("lex")).expect("parse")).expect("compile")
    };
    let chunk = subject
        .chunks
        .iter()
        .max_by_key(|c| c.ops.len())
        .expect("subject chunk");

    println!("\n================ accessor-seeded chunk coverage");
    println!(
        "  {:<26} {:>9} {:>9} {:>7}",
        "stage", "unseeded", "seeded", "of"
    );

    let mut checked = 0usize;
    for stage in [
        "verify_depth.kel",
        "verify_typed.kel",
        "verify_structural.kel",
        // **`reconstruct.kel` joined on 2026-08-16.** It is seeded from a
        // different subject and by a different accessor from the three above, so
        // it is listed here rather than folded into their subject.
        "reconstruct.kel",
    ] {
        let Some(path) = stage_sources()
            .into_iter()
            .find(|p| p.file_name().unwrap_or_default().to_string_lossy() == stage)
        else {
            panic!("{stage} is missing from the stage sources");
        };
        let m = module_of(&path).unwrap_or_else(|| panic!("{stage} compiles"));
        let (bare, total) = chunk_coverage(&m, None).expect("unseeded coverage");

        let arena = arena_for(&m);
        let vm = Vm::new(m.clone(), &arena).expect("stage vm");
        let seed = match stage {
            "verify_depth.kel" => keleusma::selfhost::seed_verify_depth_shared(&vm, chunk),
            "verify_typed.kel" => {
                let wb = (1usize << subject.word_bits_log2) / 8;
                let fb = (1usize << subject.float_bits_log2) / 8;
                let idx = subject
                    .chunks
                    .iter()
                    .position(|c| core::ptr::eq(c, chunk))
                    .unwrap_or(0);
                keleusma::selfhost::seed_verify_typed_shared(
                    &vm,
                    &subject,
                    chunk,
                    subject.signatures.get(idx),
                    wb,
                    fb,
                )
            }
            // The multiheaded reconstruct path. Its record stream comes from
            // `parse_functions`, which is `pub` -- the report that this stage was
            // blocked on a private producer was wrong and is retracted. The head
            // grouping and parameter count are NOT part of that record format;
            // they are properties of the subject, asserted and derived here rather
            // than assumed, since a seed the stage rejects looks like coverage.
            "reconstruct.kel" => {
                let msrc = std::fs::read_to_string("../examples/scripts/06_multiheaded.kel")
                    .expect("multiheaded subject source");
                let (fns, _names, _, _) = keleusma::selfhost::parse_functions(&msrc);
                assert_eq!(
                    fns.len(),
                    4,
                    "06_multiheaded.kel no longer declares three `classify` heads \
                     then `main`, so the leading run is no longer the group"
                );
                let heads: Vec<&keleusma::selfhost::ParsedFn> = fns[..3].iter().collect();
                let msub = compile(&parse(&tokenize(&msrc).expect("lex")).expect("parse"))
                    .expect("compile");
                let idx = msub
                    .chunks
                    .iter()
                    .position(|c| c.name == "classify")
                    .expect("subject has a `classify` chunk");
                let pc = msub
                    .signatures
                    .get(idx)
                    .map(|s| s.params.len())
                    .expect("subject carries a signature for `classify`");
                keleusma::selfhost::seed_reconstruct_multihead_shared(&vm, &heads, pc)
            }
            _ => {
                let always = keleusma::selfhost::self_hosted_always_yielding(&subject);
                keleusma::selfhost::seed_verify_structural_shared(&vm, &subject, chunk, &always)
            }
        };
        let (seeded, _) = chunk_coverage_with(&m, None, Some(&seed)).expect("seeded coverage");
        println!(
            "  {stage:<26} {:>9} {:>9} {:>7}",
            bare.len(),
            seeded.len(),
            total
        );
        assert!(
            seeded.len() > bare.len(),
            "{stage}: the accessor seed did not move chunk coverage ({} unseeded, \
             {} seeded). A seed a stage silently REJECTS looks like coverage from \
             the observable side; this is the measure that tells them apart, and \
             it says the stage is not doing more work.",
            bare.len(),
            seeded.len()
        );
        checked += 1;
    }
    println!("================\n");
    assert_eq!(checked, 4, "not every seeded stage was measured");
}

/// **THE SINGLE-HEAD RECONSTRUCT PATH, the half that WAS genuinely blocked.**
///
/// `reconstruct.kel` has two entry points. The multiheaded one is driven by the
/// corpus differential. This is the other, `seed_reconstruct_shared`, and until
/// the `v0.2.3` line's `ParsedFn` accessors landed it could not be called from
/// outside the crate at all: it wants a record stream, and the fields holding one
/// were private. **That half of the old report was correct**, unlike the claim
/// about the producer, which was not and is retracted.
///
/// **The subject declares exactly ONE function, so nothing here depends on
/// declaration order.** The multiheaded path cannot say that -- no accessor
/// exposes a parsed function's name, so grouping same-named heads still rests on
/// the order they appear in, which is why that subject's shape stays asserted.
///
/// **The category is MAPPED, not passed through.** The driver sends 2 for a
/// **`loop`** declaration and 0 for anything else, and handing it the raw parsed
/// category would seed a different program. Read from the driver rather than
/// inferred from the multiheaded path, which concatenates differently.
///
/// **THIS COMMENT SAID `yield` UNTIL 2026-08-23 AND THE CODE BELOW ALWAYS SAID
/// `loop`.** The full mapping, from the `v0.2.3` line:
///
/// ```text
///   declaration   parse.kel   reconstruct.kel / codegen.kel
///   fn            1           0
///   yield         2           0
///   loop          3           2
/// ```
///
/// The code reads `category() == 3`, which is `loop`, and is correct. **The prose
/// named the wrong keyword**, and the peer line caught it by comparing my
/// description against their own implementation — they could not see my source,
/// so they flagged it as time-sensitive in case I had mirrored the prose. I had
/// not; I read the driver and then described it wrongly.
///
/// **Kept as a correction rather than a silent fix.** A wrong description of
/// right code is not caught by any test here: the mapping is exercised, the
/// stage runs, and the sentence beside it is what a reader believes.
///
/// **The mapping is LOSSY**: `yield` collapses onto 0 alongside `fn`, which is
/// safe only because `codegen.kel` closes categories 0 and 1 identically with
/// `Return`. It stops being safe the moment those diverge.
///
/// Two signals, because either alone is weak. Coverage says more of the stage
/// runs; the yielded node count says the stage ACCEPTED the records rather than
/// taking an early exit, and a rejected stream is indistinguishable from coverage
/// without it.
#[test]
fn the_single_head_reconstruct_seed_drives_the_stage() {
    // **The subject is SELECTED by the property it must have, not named.** A
    // hand-picked file was tried first and `parse_functions` reported two heads
    // for a source that visibly declares one, so a literal name here would encode
    // a belief about the parser rather than a fact about it. The property is what
    // matters: exactly one parsed head, unguarded, with a non-empty body. That is
    // what `is_multihead_group` routes to the single-head driver.
    let mut chosen: Option<(String, Vec<keleusma::selfhost::ParsedFn>)> = None;
    let mut panicked: Vec<String> = Vec::new();
    println!("\n================ single-head reconstruct");
    println!(
        "  {:<24} {:>5} {:>7} {:>6}",
        "candidate", "heads", "guarded", "body"
    );
    for p in stage_sources_examples() {
        let name = p
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let Ok(src) = std::fs::read_to_string(&p) else {
            continue;
        };
        // **`parse_functions` PANICS on part of the shipped corpus**, so this
        // survey cannot simply call it. See the assertion below: the panic is a
        // finding about a public API, not an obstacle to route around silently.
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let parsed = std::panic::catch_unwind(|| keleusma::selfhost::parse_functions(&src));
        std::panic::set_hook(prev);
        let Ok((fns, _names, _, _)) = parsed else {
            println!("  {name:<24} {:>5}", "PANIC");
            panicked.push(name);
            continue;
        };
        let guarded = fns.iter().filter(|f| !f.guard_records().is_empty()).count();
        let body = fns.first().map(|f| f.body_records().len()).unwrap_or(0);
        println!("  {name:<24} {:>5} {guarded:>7} {body:>6}", fns.len());
        if fns.len() == 1 && guarded == 0 && body > 0 && chosen.is_none() {
            chosen = Some((name, fns));
        }
    }
    if !panicked.is_empty() {
        println!("  parse_functions PANICKED on: {}", panicked.join(", "));
    }
    let (subject_name, fns) = chosen.expect(
        "no corpus example parses to exactly one unguarded head with a non-empty \
         body, so the single-head path has no subject that avoids an ordering \
         assumption. Report this rather than falling back to a positional pick.",
    );
    println!("  SELECTED: {subject_name}");
    let h = &fns[0];
    let records = h.body_records();
    // **THE DRIVER'S OWN FUNCTION, no longer a copy of it.** Absorbed 2026-08-23
    // from `origin/v0.2.3` at `639f970f`; this line kept a mirror until then.
    //
    // **THE SWAP WAS VERIFIED AGAINST THE IMPLEMENTATION, NOT AGAINST A TABLE.**
    // Theirs reads `if parse_category == 3 { 2 } else { 0 }`; the mirror read
    // `if h.category() == 3 { 2 } else { 0 }` — identical, so behaviour is
    // preserved. That check was insisted on by the `v0.2.3` line and is the one
    // thing they said not to skip: **a behaviour change inside a diff that reads
    // as cleanup is the worst kind.** Had the mirror been built from their PROSE
    // (which said "2 for a `yield` declaration" and was wrong), this swap would
    // have silently re-seeded every run against a different category.
    let category = h.reconstruct_category();

    let m = keleusma::selfhost::reconstruct_kel_module();
    let arena = arena_for(&m);
    let vm = Vm::new(m.clone(), &arena).expect("stage vm");
    let seed = keleusma::selfhost::seed_reconstruct_shared(&vm, records, category, h.param_count());
    drop(vm);

    let (bare, total) = chunk_coverage(&m, None).expect("unseeded coverage");
    let (seeded, _) = chunk_coverage_with(&m, None, Some(&seed)).expect("seeded coverage");
    println!("  records            {}", records.len());
    println!("  category           {category}");
    println!("  param_count        {}", h.param_count());
    println!(
        "  chunk coverage     {} unseeded -> {} seeded, of {total}",
        bare.len(),
        seeded.len()
    );

    // **The stage must ACCEPT the stream, not merely be handed it.** The entry
    // yields the reconstructed node count, so a zero here is an early exit and
    // exactly the silent rejection this whole arc keeps finding.
    let arena2 = arena_for(&m);
    let mut vm2 = Vm::new(m.clone(), &arena2).expect("stage vm");
    let mut shared = seed.clone();
    let nodes = match vm2.call_with_shared(&mut shared, &[Value::Int(0)]) {
        Ok(VmState::Yielded(Value::Int(n))) => n,
        other => panic!("reconstruct.kel did not yield a node count: {other:?}"),
    };
    println!("  nodes reconstructed {nodes}");
    println!("================\n");
    assert!(
        nodes > 0,
        "the stage yielded {nodes} nodes, so it took an early exit and the seed was \
         effectively rejected"
    );
    assert!(
        seeded.len() > bare.len(),
        "the single-head seed did not move chunk coverage ({} unseeded, {} seeded)",
        bare.len(),
        seeded.len()
    );
}

fn scalar(v: &Value) -> i64 {
    match v {
        Value::Int(x) => *x,
        Value::Byte(b) => i64::from(*b),
        Value::Bool(b) => i64::from(*b),
        Value::Unit => 0,
        _ => i64::MIN,
    }
}

fn distinct(v: &[i64]) -> usize {
    let mut s: Vec<i64> = v.to_vec();
    s.sort_unstable();
    s.dedup();
    s.len()
}

fn stage_sources() -> Vec<std::path::PathBuf> {
    let dir = std::path::Path::new("../src/selfhost/kel");
    let mut out: Vec<_> = std::fs::read_dir(dir)
        .expect("read src/selfhost/kel")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "kel"))
        .collect();
    out.sort();
    out
}

/// **CAN A VERIFY STAGE BE MADE TO REJECT, and does the verdict actually move?**
///
/// The three `verify_*` stages sit in `KNOWN_VACUOUS` not because they exit early
/// -- they run, and chunk coverage proves it -- but because their verdict on a
/// well-formed chunk is ACCEPT, which they encode as 0, over a seeded buffer that
/// already holds 0. Nothing the differential can observe changes.
///
/// So the question is whether a rejecting subject moves the verdict to 1. This
/// probe answers it by measurement instead of by argument, and prints both
/// directions per stage. **The accept case is not decoration**: a stage that
/// rejected everything would show a moving verdict and be worthless, which is the
/// must-fire and must-not-fire pair this line uses everywhere else.
#[test]
fn can_a_rejecting_subject_move_each_verify_stages_verdict() {
    let src = std::fs::read_to_string("../examples/scripts/01_arithmetic.kel").expect("subject");
    let subject =
        compile(&parse(&tokenize(&src).expect("lex")).expect("parse")).expect("compile subject");
    let chunk_ix = (0..subject.chunks.len())
        .max_by_key(|&i| subject.chunks[i].ops.len())
        .expect("subject has a chunk");

    // **Each stage gets the defect IT checks for, because they do not check the
    // same thing.** An operand-stack underflow was tried against all three first
    // and `verify_structural.kel` accepted it, correctly: it latches block-nesting
    // malformation, not depth. One mutation for all three would have read as "that
    // stage cannot be made to reject", which is a different and false claim.
    type Mutate = fn(&mut Module, usize);
    let cases: &[(&str, &str, Mutate)] = &[
        ("verify_depth.kel", "operand-stack underflow", |m, i| {
            m.chunks[i].ops.insert(0, Op::PopN(4))
        }),
        ("verify_typed.kel", "operand-stack underflow", |m, i| {
            m.chunks[i].ops.insert(0, Op::PopN(4))
        }),
        ("verify_structural.kel", "unclosed block opener", |m, i| {
            m.chunks[i].ops.insert(0, Op::If(1))
        }),
    ];

    println!("\n================ verify verdicts, accept against reject");
    println!(
        "  {:<26} {:>8} {:>8}  defect injected",
        "stage", "well", "broken"
    );
    for (stage, defect, mutate) in cases {
        let stage = *stage;
        let mut broken = subject.clone();
        mutate(&mut broken, chunk_ix);
        let path = stage_sources()
            .into_iter()
            .find(|p| p.file_name().unwrap_or_default().to_string_lossy() == stage)
            .unwrap_or_else(|| panic!("{stage} missing"));
        let m = module_of(&path).unwrap_or_else(|| panic!("{stage} compiles"));

        let arena = arena_for(&m);
        let vm = Vm::new(m.clone(), &arena).expect("stage vm");
        let well = verify_stage_seed(stage, &m, &vm, &subject, chunk_ix);
        let ill = verify_stage_seed(stage, &m, &vm, &broken, chunk_ix);
        drop(vm);

        let a = verify_stage_verdict(&m, &well);
        let b = verify_stage_verdict(&m, &ill);
        let show = |r: &Result<i64, String>| match r {
            Ok(v) => v.to_string(),
            Err(e) => format!("ERR {e}"),
        };
        println!("  {stage:<26} {:>8} {:>8}  {defect}", show(&a), show(&b));
        // **Both directions, asserted.** The reject case alone would pass for a
        // stage that rejects everything, and the accept case alone proves only
        // that it runs.
        assert_eq!(
            a.as_ref().ok(),
            Some(&0),
            "{stage} did not ACCEPT a well-formed chunk, so a moving verdict would \
             say nothing about the injected defect"
        );
        assert_eq!(
            b.as_ref().ok(),
            Some(&1),
            "{stage} did not REJECT a chunk carrying {defect}, so its verdict does \
             not move and it cannot leave the vacuous set by this route"
        );
    }
    println!("================\n");
}

/// Build the seed a stage expects for `chunk` of `subject`.
///
/// One place, so the accept case and the reject case cannot drift into being two
/// different constructions -- which would make a differing verdict evidence about
/// the harness rather than about the chunk.
fn verify_stage_seed(
    stage: &str,
    m: &Module,
    vm: &Vm<'_, '_>,
    subject: &Module,
    chunk_ix: usize,
) -> Vec<u8> {
    let _ = m;
    let chunk = &subject.chunks[chunk_ix];
    match stage {
        "verify_depth.kel" => keleusma::selfhost::seed_verify_depth_shared(vm, chunk),
        "verify_typed.kel" => {
            let wb = (1usize << subject.word_bits_log2) / 8;
            let fb = (1usize << subject.float_bits_log2) / 8;
            keleusma::selfhost::seed_verify_typed_shared(
                vm,
                subject,
                chunk,
                subject.signatures.get(chunk_ix),
                wb,
                fb,
            )
        }
        _ => {
            let always = keleusma::selfhost::self_hosted_always_yielding(subject);
            keleusma::selfhost::seed_verify_structural_shared(vm, subject, chunk, &always)
        }
    }
}

/// Drive a verify stage once and return the verdict it yields.
///
/// Each of the three ends in `yield run()`, and `run()`'s final expression is its
/// `out_reject` slot, so the yielded value IS the verdict. Read from the stage
/// sources rather than assumed.
fn verify_stage_verdict(m: &Module, seed: &[u8]) -> Result<i64, String> {
    let arena = arena_for(m);
    let mut vm = Vm::new(m.clone(), &arena).map_err(|e| format!("stage refuses to load: {e:?}"))?;
    let mut shared = seed.to_vec();
    match vm.call_with_shared(&mut shared, &[Value::Int(0)]) {
        Ok(VmState::Yielded(Value::Int(v))) => Ok(v),
        Ok(other) => Err(format!("unexpected state: {other:?}")),
        Err(e) => Err(format!("stage refuses to run: {e:?}")),
    }
}

/// The top-level example scripts, as candidate reconstruct subjects.
///
/// Deliberately NOT recursive: the `rogue` and `piano_roll` subdirectories carry
/// multi-file programs whose parts do not stand alone.
fn stage_sources_examples() -> Vec<std::path::PathBuf> {
    let dir = std::path::Path::new("../examples/scripts");
    let mut out: Vec<_> = std::fs::read_dir(dir)
        .expect("read examples/scripts")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "kel"))
        .collect();
    out.sort();
    out
}

fn module_of(p: &std::path::Path) -> Option<Module> {
    let src = std::fs::read_to_string(p).ok()?;
    tokenize(&src)
        .ok()
        .and_then(|t| parse(&t).ok())
        .and_then(|a| compile(&a).ok())
}

/// A source that exercises several token classes, so a real run cannot produce
/// one repeated value the way an empty one does.
const SEED: &str = "fn add(a: Word, b: Word) -> Word { let c = a + b; c }";

#[test]
fn how_far_does_each_stage_get_on_the_differentials_own_input() {
    println!("\n================ STAGE OUTPUT: all-zero shared segment vs. seeded source");
    println!("  the differential drives the left column. {TICKS} ticks each.\n");
    println!(
        "  {:<24} {:>7} {:>26}   {:>7} {:>26}",
        "stage", "distinct", "first yields (zero input)", "distinct", "first yields (seeded)"
    );

    let mut measured = 0usize;
    let mut single_valued = 0usize;
    for p in stage_sources() {
        let name = p.file_name().unwrap().to_str().unwrap().to_string();
        let Some(m) = module_of(&p) else {
            println!("  {name:<24} rejected by the REFERENCE compiler");
            continue;
        };
        let zero = drive(&m, None);
        let seeded = drive(&m, Some(SEED));

        let render = |r: &Result<Run, String>| match r {
            Ok(run) => {
                let head: Vec<String> = run.yields.iter().take(6).map(|v| v.to_string()).collect();
                (
                    format!("{}", distinct(&run.yields)),
                    format!(
                        "{} {}",
                        head.join(","),
                        if run.yields.len() > 6 { ".." } else { "" }
                    ),
                )
            }
            Err(_) => ("-".into(), "see below".into()),
        };
        let (dz, hz) = render(&zero);
        let (ds, hs) = render(&seeded);
        println!("  {name:<24} {dz:>7} {hz:>26}   {ds:>7} {hs:>26}");
        if let Err(e) = &seeded {
            println!("        seeded: {e}");
        }

        if let Ok(run) = &zero {
            measured += 1;
            if distinct(&run.yields) <= 1 {
                single_valued += 1;
            }
            if let Ok(s) = &seeded
                && run.yields == s.yields
            {
                println!("        ^ seeding changed NOTHING: this stage ignores the source input");
            }
        }
        if let Ok(run) = &zero
            && run.outcome != "ran to the tick limit"
        {
            println!("        zero-input outcome: {}", run.outcome);
        }
    }

    println!(
        "\n  {single_valued} of {measured} stages yield a SINGLE repeated value on the \
         differential's input."
    );
    println!("  A tokenizer emitting one value for sixty ticks has reached end of source");
    println!("  immediately. `lexer.kel` documents 62 as EOF and 63 as PENDING.");
    assert!(
        measured > 0,
        "no stage was measured; the probe asserts nothing"
    );
}

/// The property that would invalidate the comparison above.
///
/// `corpus_differential` returns an asymmetric `stub_value` from every native;
/// this probe registers none at all. If a stage declared a native, the two
/// harnesses could drive it down different paths and what is measured here would
/// not be what the differential runs. None does, so they cannot diverge.
#[test]
fn no_stage_declares_a_native_so_the_two_harnesses_cannot_diverge() {
    for p in stage_sources() {
        let name = p.file_name().unwrap().to_str().unwrap();
        let Some(m) = module_of(&p) else { continue };
        let declared: Vec<&String> = m.native_names.iter().filter(|n| !n.is_empty()).collect();
        assert!(
            declared.is_empty(),
            "{name} declares natives {declared:?}; this probe registers no natives while \
             `corpus_differential` returns `stub_value` from each, so the two harnesses \
             could drive it down different paths"
        );
    }
}

/// **The `v0.2.3` defect that blocked the instrument above is FIXED, and this
/// is the working path.**
///
/// The old test here asserted that `resume_from_breakpoint` PANICS on a
/// shared-data module, so that a repair on their side would fail loudly rather
/// than leave a stale claim on ours. It did exactly that, and this replaces it.
///
/// The repair is `resume_from_breakpoint_with_shared`, which binds the buffer the
/// way `call_with_shared` does. The bare entry point now returns a `VmError`
/// naming the method to use rather than panicking.
///
/// **This asserts the BUFFER CONTENTS, not merely that `Yielded` came back**, and
/// that choice is theirs: a step that returned `Yielded` while writing nothing
/// would satisfy a state-only check and would mean the buffer was never really
/// bound. The module increments `s.n` on every iteration, so the byte is the
/// evidence.
#[test]
fn the_breakpoint_facility_now_works_on_a_shared_data_module() {
    let src = "\
shared data s { n: Word }
loop main(resume: Word) -> Word {
  s.n = s.n + 1;
  yield s.n
}
";
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    assert!(
        shared_data_bytes_for(&m) > 0,
        "the reproducer must declare shared data or it tests nothing"
    );
    let n_off = shared_offset(&m, "n").expect("slot `s.n` has an offset");

    let arena = arena_for(&m);
    let mut vm = Vm::new(m.clone(), &arena).expect("vm");
    let mut shared = vec![0u8; shared_data_bytes_for(&m)];

    // Chunk 0 op 0 is entered on the first call, so this stops immediately.
    vm.set_breakpoint(0, 0);
    let st = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call");
    assert!(
        matches!(st, VmState::BreakpointHit { .. }),
        "expected a breakpoint stop, got {st:?}"
    );

    // The bare entry point must now REFUSE rather than panic, and say what to use.
    let refused = vm.resume_from_breakpoint();
    assert!(
        refused.is_err(),
        "bare `resume_from_breakpoint` accepted a shared-data module; it should refuse"
    );
    assert!(
        format!("{:?}", refused.unwrap_err()).contains("resume_from_breakpoint_with_shared"),
        "the refusal must name the method to use, or it sends a host to the wrong place"
    );

    // The working path, and the byte is what proves the buffer was bound.
    let before = shared[n_off as usize];
    let st = vm
        .resume_from_breakpoint_with_shared(&mut shared)
        .expect("resume with the buffer bound");
    assert!(
        matches!(st, VmState::Yielded(_)),
        "expected a yield after the resume, got {st:?}"
    );
    assert_ne!(
        shared[n_off as usize], before,
        "the step returned Yielded without writing the shared segment, so the \
         buffer was never really bound -- which is the failure a state-only \
         assertion would have missed"
    );
}

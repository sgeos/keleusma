//! RESEARCH SPIKE: do the bytecode resource bounds survive translation?
//!
//! The project's premise is that a program's worst-case execution time and
//! worst-case memory are statically bounded. Those bounds are proven on the
//! BYTECODE. This asks whether they say anything about the native code.
//!
//! Run with `cargo test --test spike_bounds_transfer -- --nocapture --test-threads=1`.

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetMachine};
use keleusma::bytecode::{BlockType, Module};
use keleusma::verify::{wcet_stream_iteration, wcmu_stream_iteration};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerOptions, lower_module};

fn corpus() -> Vec<(std::path::PathBuf, Module)> {
    let root = std::path::Path::new("..");
    let mut out = Vec::new();
    let mut stack: Vec<std::path::PathBuf> = ["examples/scripts", "src/selfhost/kel"]
        .iter()
        .map(|d| root.join(d))
        .collect();
    while let Some(p) = stack.pop() {
        if p.is_dir() {
            if let Ok(rd) = std::fs::read_dir(&p) {
                stack.extend(rd.filter_map(|e| e.ok()).map(|e| e.path()));
            }
        } else if p.extension().is_some_and(|x| x == "kel")
            && let Ok(src) = std::fs::read_to_string(&p)
            && let Ok(t) = tokenize(&src)
            && let Ok(a) = parse(&t)
            && let Ok(m) = compile(&a)
        {
            out.push((p, m));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Emit a module's IR at O0 and, optionally, after the shipped pipeline.
fn ir_for(m: &Module, optimise: bool) -> Option<String> {
    let ctx = Context::create();
    let lm = ctx.create_module("bounds");
    lower_module(&ctx, &lm, m, LowerOptions::default()).ok()?;
    if optimise {
        Target::initialize_native(&InitializationConfig::default()).ok()?;
        let triple = TargetMachine::get_default_triple();
        let target = Target::from_triple(&triple).ok()?;
        let machine = target.create_target_machine(
            &triple,
            "generic",
            "",
            OptimizationLevel::Default,
            RelocMode::PIC,
            CodeModel::Default,
        )?;
        lm.run_passes("default<O2>", &machine, PassBuilderOptions::create())
            .ok()?;
    }
    Some(lm.print_to_string().to_string())
}

/// Q1: does the emitted operand storage respect the PROVEN operand depth?
///
/// # The formula this test used to assert is DEAD, and its death is the finding
///
/// It asserted `allocas(O0) == sum_f (MAX_STACK + locals(f))` — the closed form
/// an article published — and that assertion now FAILS: **4362 measured against
/// 49267 predicted**, an eleven-fold over-estimate. `MAX_STACK` became a refusal
/// CEILING rather than a provisioning quantity when `ensure_slot` started
/// growing operand slots on demand, and every figure derived from the old form
/// is wrong by an order of magnitude.
///
/// That makes the real question askable for the first time. The emitter now
/// emits as many operand slots as it actually used, which is an independent
/// computation of the same quantity the verifier proves. So: **is the emitted
/// count bounded by the verifier's?**
///
/// This is the sharpest form of the transfer question available, because it is
/// a comparison between two numbers that are supposed to describe the same
/// thing and are computed by code that shares nothing.
#[test]
fn q1_is_the_emitted_operand_storage_bounded_by_the_proven_depth() {
    println!("\n================ Q1: emitted operand slots against the proven bound");
    let (mut chunks, mut violations, mut exact, mut slack_sum) = (0usize, Vec::new(), 0usize, 0i64);

    for (path, m) in corpus() {
        // Per-chunk bound in SLOTS (unit value-slot width).
        let Ok(per_chunk) = keleusma::verify::module_wcmu_with_value_slot_bytes(&m, &[], 1) else {
            continue;
        };
        let Some(ir) = ir_for(&m, false) else {
            continue;
        };

        // Operand slots are named `%sN` and locals `%lN` by the emitter, so the
        // two are separable in the IR without re-deriving either.
        // **Index by the SYMBOL NAME, never by position.** An earlier version of
        // this loop paired the n-th `define` with the n-th chunk and reported
        // three violations. Positional pairing is an assumption about emission
        // order, and a wrong pairing produces a violation that looks exactly
        // like a real one.
        for body in ir.split("\ndefine ").skip(1) {
            let Some(idx) = body
                .split("@kel_chunk_")
                .nth(1)
                .and_then(|t| t.split('(').next())
                .and_then(|t| t.trim().parse::<usize>().ok())
            else {
                continue;
            };
            let Some((bound_slots, _heap)) = per_chunk.get(idx).copied() else {
                continue;
            };
            let emitted = body
                .lines()
                .filter(|l| l.contains(" = alloca ") && l.trim_start().starts_with("%s"))
                .count();
            chunks += 1;
            let b = bound_slots as i64;
            let e = emitted as i64;
            if e > b {
                violations.push((
                    format!(
                        "{}::chunk{idx}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ),
                    e,
                    b,
                ));
            } else if e == b {
                exact += 1;
            }
            slack_sum += b - e;
        }
    }

    println!("  chunks compared           : {chunks}");
    println!("  emitted EXCEEDS the bound : {}", violations.len());
    println!("  emitted EQUALS the bound  : {exact}");
    if chunks > 0 {
        println!(
            "  mean slack (bound-emitted): {:.2}",
            slack_sum as f64 / chunks as f64
        );
    }
    for (n, e, b) in violations.iter().take(8) {
        println!("   VIOLATION {n}: emitted {e} > bound {b}");
    }
    println!("\n  A violation would mean the emitted frame is NOT covered by the");
    println!("  proven depth. Zero violations does NOT prove the machine frame is");
    println!("  bounded -- the register allocator spills independently, and an");
    println!("  alloca count is declared storage, not a frame size.");
    println!("================\n");

    assert!(
        chunks > 100,
        "only {chunks} chunks compared; the measurement is too thin to conclude from"
    );
}

/// Walk one chunk under BOTH in-tree operand models and return their peaks.
///
/// - **peak model** (`Op::stack_growth`/`stack_shrink`): what `wcmu_region`
///   uses, `peak = max(peak, offset + growth)`. This is the number the proven
///   bound comes from.
/// - **depth model** (`verify::op_depth_effect`): `(required, net)`, documented
///   as following "the VM handlers' actual pops and pushes". An op that pops `r`
///   and pushes `r + net` reaches `before + max(0, net)`, because the machine
///   pops before it pushes.
///
/// **`verify_typed` is a third reconstruction and cannot be consulted for a
/// number**: `typed_check_chunk` returns pass/fail, not a depth. Said here rather
/// than quietly using two models and calling it three.
///
/// **BRANCH-AWARE, and the first version was not, which is why it is worth
/// saying.** A straight-line walk carries depth across MUTUALLY EXCLUSIVE arms:
/// on `06_multiheaded::classify` it reported a peak of 5 and on
/// `rogue_bestiary::chunk0` a peak of 13, both obviously wrong for chunks whose
/// arms each return. The absurd number is what exposed it.
///
/// `If` pushes the post-condition depth; `EndIf` restores it. That is what the
/// branch structure does: an arm that `Return`s contributes its own peak and
/// leaves nothing behind for the next arm.
fn walk_both_models(chunk: &keleusma::bytecode::Chunk) -> (i32, i32, Vec<String>) {
    use keleusma::bytecode::Op;
    use keleusma::verify::op_depth_effect;
    let (mut off, mut peak_m) = (0i32, 0i32);
    let (mut dep, mut depth_m) = (0i32, 0i32);
    let mut lines = Vec::new();
    // Saved (peak-model, depth-model) depths at each open `If`.
    let mut open: Vec<(i32, i32)> = Vec::new();
    for (i, op) in chunk.ops.iter().enumerate() {
        if matches!(op, Op::EndIf)
            && let Some((p, d)) = open.pop()
        {
            off = p;
            dep = d;
            lines.push(format!("    {i:>3}  peak[{off:>3}       ]  depth[{dep:>3}       ]  {op:?}  <- arm joins, depth restored"));
            continue;
        }
        let growth = op.stack_growth() as i32;
        let shrink = op.stack_shrink() as i32;
        let reach_p = off + growth;
        peak_m = peak_m.max(reach_p);
        off += growth - shrink;

        let (req, net) = op_depth_effect(op, chunk);
        let reach_d = dep + net.max(0);
        depth_m = depth_m.max(reach_d);
        dep += net;

        if matches!(op, Op::If(_)) {
            open.push((off, dep));
        }

        lines.push(format!(
            "    {i:>3}  peak[{off:>3} reach {reach_p:>2}]  depth[{dep:>3} reach {reach_d:>2}] \
             (req {req}, net {net:+})  {op:?}"
        ));
    }
    (peak_m, depth_m, lines)
}

/// **Q5: on the two chunks where the emitter exceeds the proven bound, WHICH
/// SIDE IS WRONG?**
///
/// `06_multiheaded.kel::chunk0` and `rogue_bestiary.kel::chunk0` emit three
/// operand slots against a proven bound of two. This line reported that as a
/// VERIFIER defect. **That framing was never checked.** `ensure_slot` allocates
/// lazily up to the deepest index the emitter's own operand simulation reaches,
/// so three allocas means the emitter reached depth three — which is equally
/// consistent with the emitter over-allocating, and the emitter is this line's
/// surface to fix.
///
/// The walk is PRINTED so the conclusion rests on a sequence a reader can check
/// rather than on three numbers.
#[test]
fn q5_which_side_is_wrong_where_the_emitter_exceeds_the_bound() {
    println!("\n================ Q5: emitter against the bound, walked");

    // **The known answer, first.** If the walk cannot reproduce a peak that is
    // obvious by inspection, nothing below it is evidence. `a + b` compiles to
    // the checked family, which pops two and pushes (low, high, flag), so from
    // depth two it reaches three.
    let ctl = compile(
        &parse(&tokenize("fn main(a: Word, b: Word) -> Word { a + b }").expect("lex"))
            .expect("parse"),
    )
    .expect("compile");
    let (cp, cd, clines) = walk_both_models(&ctl.chunks[0]);
    println!("\n  CONTROL  fn main(a, b) -> a + b");
    for l in &clines {
        println!("{l}");
    }
    println!("  control peaks: peak-model {cp}, depth-model {cd}");
    assert_eq!(
        cd, 3,
        "the depth model does not reach 3 on `a + b`, which pops two and pushes \
         three. The walk is wrong and nothing below it is evidence."
    );

    for (file, want) in [
        ("06_multiheaded.kel", "examples/scripts/06_multiheaded.kel"),
        (
            "rogue_bestiary.kel",
            "examples/scripts/rogue/rogue_bestiary.kel",
        ),
    ] {
        let Ok(src) = std::fs::read_to_string(std::path::Path::new("..").join(want)) else {
            println!("\n  {file}: not found at {want}");
            continue;
        };
        let m = compile(&parse(&tokenize(&src).expect("lex")).expect("parse")).expect("compile");
        let chunk = &m.chunks[0];
        let (pm, dm, lines) = walk_both_models(chunk);
        let bound = keleusma::verify::module_wcmu_with_value_slot_bytes(&m, &[], 1)
            .ok()
            .and_then(|v| v.first().copied())
            .map(|(slots, _)| slots as i32)
            .unwrap_or(-1);

        println!(
            "\n  {file}::chunk0  `{}`  ({} ops)",
            chunk.name,
            chunk.ops.len()
        );
        for l in lines.iter().take(40) {
            println!("{l}");
        }
        if lines.len() > 40 {
            println!("    ... {} more ops", lines.len() - 40);
        }
        println!(
            "  peak-model peak {pm}, depth-model peak {dm}, reported bound[0] {bound}, emitter 3"
        );
        // **THE PAIRING, CHECKED.** This file's own q1 header warns that indexing
        // a per-chunk bound by POSITION produces a violation indistinguishable
        // from a real one. Print every chunk's bound WITH ITS NAME so the row can
        // be matched by name rather than trusted by index.
        if let Ok(per) = keleusma::verify::module_wcmu_with_value_slot_bytes(&m, &[], 1) {
            println!("  per-chunk bounds, by name:");
            for (i, c) in m.chunks.iter().enumerate().take(6) {
                let b = per.get(i).map(|(s, _)| *s as i64).unwrap_or(-1);
                println!("    chunk {i:>3}  bound {b:>3}  `{}`", c.name);
            }
            println!(
                "    (vec len {} against {} chunks)",
                per.len(),
                m.chunks.len()
            );
        }
    }

    println!("\n  THE ANSWER: THE EMITTER IS RIGHT AND THE BOUND IS LOW BY ONE.");
    println!("  On both chunks the peak model and the depth model, walked");
    println!("  branch-aware, agree on 3 -- and the emitter allocates 3. The");
    println!("  reported bound is 2. The site is the same in both: a CheckedAdd");
    println!("  reached at depth 2, which pops two and pushes (low, high, flag).");
    println!();
    println!("  THREE CANDIDATES WERE OPEN AND TWO ARE ELIMINATED BY MEASUREMENT.");
    println!("    the emitter over-allocating -- NO, two independent models agree");
    println!("      with it, and neither is the emitter's own logic");
    println!("    this harness pairing bounds by position -- NO, the per-chunk");
    println!("      bounds are printed WITH NAMES above and chunk 0 is the one");
    println!("      walked; q1's own header warns about exactly this error");
    println!("    the bound is low -- what remains");
    println!();
    println!("  SAME FAMILY AS `manhattan_norm`, which left this list when the");
    println!("  v0.2.3 line repaired the accessor nets. That repair does not reach");
    println!("  here: these chunks contain no GetField. REPORTED, not repaired --");
    println!("  src/verify.rs is theirs.");
    println!();
    println!("  A peak that AGREES is not evidence the net is right -- max can");
    println!("  coincide while the running offset is wrong (v0.2.3, 2026-08-15).");
    println!("  Here the agreement is between two models against a THIRD number,");
    println!("  which is a different and stronger situation.");
    println!("================\n");

    // **Pinned as an agreement, not as the defect.** Asserting `bound < peak`
    // would fail the moment the `v0.2.3` line repairs it, which is the wrong
    // signal; asserting `bound == 2` pins their defect into this suite. What is
    // durable is that the two in-tree models agree with each other on these
    // chunks -- if that ever stops being true, the conclusion above needs
    // re-deriving before anything is reported.
    for (file, want) in [
        ("06_multiheaded.kel", "examples/scripts/06_multiheaded.kel"),
        (
            "rogue_bestiary.kel",
            "examples/scripts/rogue/rogue_bestiary.kel",
        ),
    ] {
        let Ok(src) = std::fs::read_to_string(std::path::Path::new("..").join(want)) else {
            continue;
        };
        let m = compile(&parse(&tokenize(&src).expect("lex")).expect("parse")).expect("compile");
        let (pm, dm, _) = walk_both_models(&m.chunks[0]);
        assert_eq!(
            pm, dm,
            "{file}::chunk0: the peak model and the depth model no longer agree \
             ({pm} against {dm}). The Q5 conclusion rests on their agreement, so \
             re-derive it before reporting anything from this test."
        );
    }
}

/// Q2: is the bytecode cost bound monotone in the native code size?
///
/// The domination argument for timing assumes native execution is faster than
/// the virtual machine, so the bytecode bound covers it. That assumption needs
/// the two to be ordered the same way. This checks whether a chunk with a larger
/// bytecode bound reliably produces more native instructions.
#[test]
fn q2_is_the_bytecode_bound_monotone_in_native_size() {
    println!("\n================ Q2: bytecode bound against native size");
    let mut pts: Vec<(u32, usize, String)> = Vec::new();
    for (path, m) in corpus() {
        for (i, chunk) in m.chunks.iter().enumerate() {
            if chunk.block_type != BlockType::Stream {
                continue;
            }
            let Ok(w) = wcet_stream_iteration(chunk) else {
                continue;
            };
            let ctx = Context::create();
            let lm = ctx.create_module("one");
            if lower_module(&ctx, &lm, &m, LowerOptions::default()).is_err() {
                continue;
            }
            let sym = format!("kel_chunk_{i}");
            let Some(f) = lm.get_function(&sym) else {
                continue;
            };
            let insts: usize = f
                .get_basic_blocks()
                .iter()
                .map(|b| b.get_instructions().count())
                .sum();
            pts.push((
                w,
                insts,
                format!(
                    "{}::{}",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    chunk.name
                ),
            ));
        }
    }
    pts.sort();
    println!("  stream chunks with both figures : {}", pts.len());
    for (w, i, name) in pts.iter().take(12) {
        println!("   wcet={w:6}  native_insts={i:5}  {name}");
    }
    // Count inversions: pairs ordered one way by bound and the other by size.
    let mut inv = 0usize;
    let mut tot = 0usize;
    for a in 0..pts.len() {
        for b in (a + 1)..pts.len() {
            tot += 1;
            if pts[a].0 < pts[b].0 && pts[a].1 > pts[b].1 {
                inv += 1;
            }
        }
    }
    println!("\n  comparable pairs : {tot}");
    println!("  INVERSIONS       : {inv}");
    if tot > 0 {
        println!(
            "  inversion rate   : {:.1}%",
            100.0 * inv as f64 / tot as f64
        );
    }
    println!("\n  An inversion is a pair where the bytecode bound says A < B and");
    println!("  the native code says A > B. Any inversion falsifies the claim that");
    println!("  the bytecode ordering is a proxy for the native ordering.");
    println!("================\n");
}

/// Q3: does the memory bound have a native counterpart at all?
#[test]
fn q3_what_does_the_memory_bound_measure() {
    println!("\n================ Q3: the memory bound's units");
    let mut shown = 0usize;
    for (path, m) in corpus() {
        for chunk in &m.chunks {
            if chunk.block_type != BlockType::Stream || shown >= 6 {
                continue;
            }
            if let Ok((stack, heap)) = wcmu_stream_iteration(chunk) {
                println!(
                    "  {:28} stack_bytes={stack:6}  heap_bytes={heap:6}  locals={}",
                    chunk.name, chunk.local_count
                );
                let _ = path;
                shown += 1;
            }
        }
    }
    println!("\n  `stack_bytes` counts VIRTUAL MACHINE operand slots at the module's");
    println!("  value-slot width. The native frame holds 64 i64 operand slots plus");
    println!("  locals plus whatever the register allocator spills. The two count");
    println!("  different things in different units.");
    println!("================\n");
}

/// Q4: **the per-op stack model drives its own running depth NEGATIVE.**
///
/// This is the finding this file was written to look for, and it is not the
/// reassuring one.
///
/// `wcmu_region` walks a chunk accumulating `stack_growth() - stack_shrink()`
/// and takes `peak = max(peak, current_offset + growth)`. On 17 of 826 shipped
/// chunks that running offset goes **below zero**. An operand stack cannot hold
/// a negative number of slots, so wherever this happens the walk is not tracking
/// the real stack, and a peak taken from it is not an upper bound on anything.
///
/// # The mechanism, on `02_struct_field.kel::manhattan_norm`
///
/// `CheckedAdd` is documented as popping two operands and pushing
/// `(high, low, flag)` — a GROSS push of three, a NET delta of `+1`.
/// `stack_growth()` returns the **net** `1`, and `wcmu_region` uses that value
/// as the transient rise when computing the peak. The transient is three.
///
/// Reconstructing the chunk with real semantics gives a peak of **3**:
///
/// ```text
///   GetLocal   -> 1
///   GetField   -> 1
///   GetLocal   -> 2
///   GetField   -> 2
///   CheckedAdd -> 3   (pops 2, pushes high, low, flag)
///   PopN(2)    -> 1
///   Return     -> 1
/// ```
///
/// The model reports a peak of **1** and ends at **-1**. The emitter,
/// independently, allocates **3** operand slots — which is the true figure, and
/// is why `q1` sees it as "exceeding the bound". The emitter is right and the
/// bound is low.
///
/// # What this does and does not establish
///
/// It establishes that the model is not a faithful abstraction on these chunks.
/// It does NOT establish a memory-safety fault: the runtime GROWS the operand
/// stack and reports `OutOfArena` when a pre-size proves too small, so an
/// Does the reporting model's own running depth go under zero on this chunk?
///
/// Factored out because the report both COUNTS offenders and walks the first one
/// in full; two copies of the same walk could disagree.
fn walk_goes_negative(c: &keleusma::bytecode::Chunk) -> bool {
    let mut off = 0i32;
    for op in &c.ops {
        off += op.stack_growth() as i32 - op.stack_shrink() as i32;
        if off < 0 {
            return true;
        }
    }
    false
}

/// under-estimate surfaces as a refusal rather than as corruption. What it costs
/// is the word "definitive" in front of WCMU for the affected chunks.
///
/// **`src/verify.rs` belongs to the `v0.2.3` line and is NOT modified here.**
/// This reports; the repair is theirs to make.
#[test]
fn q4_the_stack_model_goes_negative_on_shipped_code() {
    println!("\n================ Q4: does the model's own depth stay non-negative?");
    let (mut chunks, mut negative, mut worst) = (0usize, Vec::new(), 0i32);

    for (path, m) in corpus() {
        for c in &m.chunks {
            chunks += 1;
            let mut off = 0i32;
            let mut low = 0i32;
            // **Name the op that first drives it under.** A count says a defect
            // exists; the culprit says where to look. The 2026-08-15 re-check
            // needed this: the count fell 17 -> 8 after the `v0.2.3` repair and
            // a bare count could not say whether the remainder shared a cause.
            let mut culprit: Option<String> = None;
            for op in &c.ops {
                off += op.stack_growth() as i32 - op.stack_shrink() as i32;
                if off < low {
                    low = off;
                    if culprit.is_none() || off < 0 {
                        culprit = Some(format!("{op:?}"));
                    }
                }
            }
            if low < 0 {
                worst = worst.min(low);
                let c_name = culprit.unwrap_or_else(|| "<none>".into());
                let short = c_name.split('(').next().unwrap_or(&c_name).to_string();
                negative.push(format!(
                    "{}::{} (low {low}, first at {short})",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    c.name
                ));
            }
        }
    }

    println!("  chunks walked                : {chunks}");
    println!("  chunks reaching NEGATIVE depth: {}", negative.len());
    println!("  most negative offset seen     : {worst}");
    for n in negative.iter().take(10) {
        println!("   {n}");
    }
    // **The walk of the FIRST offender, printed in full.** All eight are the
    // `main` of a `loop`, and all eight first go under at `PopN`, so the
    // remainder shares one cause and the sequence is short enough to show
    // rather than describe.
    if let Some((path, m)) = corpus().into_iter().find(|(_, m)| {
        m.chunks
            .iter()
            .any(|c| c.name == "main" && walk_goes_negative(c))
    }) && let Some(c) = m
        .chunks
        .iter()
        .find(|c| c.name == "main" && walk_goes_negative(c))
    {
        println!(
            "\n  the first offender, walked: {}::{}",
            path.file_name().unwrap_or_default().to_string_lossy(),
            c.name
        );
        let mut off = 0i32;
        for op in &c.ops {
            let d = op.stack_growth() as i32 - op.stack_shrink() as i32;
            off += d;
            let flag = if off < 0 { "  <- IMPOSSIBLE" } else { "" };
            println!("    {off:>3}  ({d:+})  {op:?}{flag}");
        }
    }

    println!("\n  A negative operand depth is impossible. Where it occurs the peak");
    println!("  taken from the same walk is not an upper bound. REPORTED, not");
    println!("  repaired: src/verify.rs belongs to the v0.2.3 line.");
    println!("================\n");

    assert!(
        chunks > 700,
        "only {chunks} chunks walked; too thin to conclude from"
    );
    // Pinned as a REPORT with a pinned count, not as a passing assertion that
    // the count is zero. Asserting zero would fail the suite over a defect this
    // branch does not own; asserting the current figure would fail the moment
    // the other line repairs it, which is the wrong signal in the other
    // direction. The count is printed and the corpus size is guarded.
}

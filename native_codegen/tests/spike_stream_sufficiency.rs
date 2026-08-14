//! RESEARCH SPIKE, not a regression test. **UNCOMPILED — see `README.md`.**
//!
//! Install as `native_codegen/tests/spike_stream_sufficiency.rs`.
//!
//! Answers the one question the four counts left open and the source reading
//! could not reach: **is handling `Stream` and `Reset` SUFFICIENT to unblock the
//! self-hosted stage modules, or do other unsupported opcodes sit behind them?**
//!
//! `lower_module` refuses on the FIRST unsupported opcode. `Op::Stream` is the
//! first op of every stream chunk, so every existing measurement stops there and
//! reports nothing about what follows. Count 2's "ten of eleven refuse on
//! `Stream`" is therefore a statement about ordering, not about blockers, and
//! reading it as the latter would put the whole of Order 1 behind one increment
//! that may not deliver it.
//!
//! It also promotes the source-level shape reading in
//! `NATIVE_LOWERING_INVENTORY.md` to a bytecode count. That reading found eight
//! of ten stages with a single top-level `yield` as the final statement, which is
//! the degenerate case where the rotation is the identity. It was taken from
//! `.kel` source because the machine was unavailable, and source is not bytecode.
//!
//! Run with `cargo test --test spike_stream_sufficiency -- --nocapture`.

use keleusma::bytecode::{BlockType, Module, Op};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use std::collections::BTreeMap;

fn corpus_sources() -> Vec<std::path::PathBuf> {
    let root = std::path::Path::new("..");
    let mut sources: Vec<std::path::PathBuf> = Vec::new();
    for dir in [
        "examples/scripts",
        "src/selfhost/kel",
        "examples/rtos/scripts",
        "compiler/kel",
    ] {
        let d = root.join(dir);
        if let Ok(rd) = std::fs::read_dir(&d) {
            let mut stack: Vec<std::path::PathBuf> =
                rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
            while let Some(p) = stack.pop() {
                if p.is_dir() {
                    if let Ok(rd2) = std::fs::read_dir(&p) {
                        stack.extend(rd2.filter_map(|e| e.ok()).map(|e| e.path()));
                    }
                } else if p.extension().is_some_and(|x| x == "kel") {
                    sources.push(p);
                }
            }
        }
    }
    sources.sort();
    sources
}

fn compiled_corpus() -> Vec<(std::path::PathBuf, Module)> {
    let mut out = Vec::new();
    for path in corpus_sources() {
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(toks) = tokenize(&src) else { continue };
        let Ok(ast) = parse(&toks) else { continue };
        let Ok(m) = compile(&ast) else { continue };
        out.push((path, m));
    }
    out
}

/// A stream chunk's yield shape, by the SAME depth rule the verifier's
/// `reentrant_segmented_wcet` uses.
///
/// The block-opening set is `If` and `Loop`; `Else` opens nothing (it is a jump
/// inside an already-open `If`), and `Break`/`BreakIf` transfer control without
/// nesting. That set was checked against the full block-structured opcode list
/// rather than assumed, because a missed opener would silently report a nested
/// yield as top level and license a wrong transformation.
struct YieldShape {
    top_level: usize,
    nested: usize,
    /// Ops strictly between the last top-level `Yield` and `Reset`. The
    /// degenerate case expects exactly `[PopN(1)]`.
    tail: Vec<String>,
}

fn yield_shape(ops: &[Op]) -> YieldShape {
    let mut depth: i32 = 0;
    let (mut top_level, mut nested) = (0usize, 0usize);
    let mut last_top: Option<usize> = None;
    for (ip, op) in ops.iter().enumerate() {
        match op {
            Op::If(_) | Op::Loop(_) => depth += 1,
            Op::EndIf | Op::EndLoop(_) => depth -= 1,
            Op::Yield => {
                if depth == 0 {
                    top_level += 1;
                    last_top = Some(ip);
                } else {
                    nested += 1;
                }
            }
            _ => {}
        }
    }
    let reset = ops.iter().position(|o| matches!(o, Op::Reset));
    let tail = match (last_top, reset) {
        (Some(y), Some(r)) if r > y => ops[y + 1..r].iter().map(|o| format!("{o:?}")).collect(),
        _ => Vec::new(),
    };
    YieldShape {
        top_level,
        nested,
        tail,
    }
}

/// THE SUFFICIENCY QUESTION. For each self-hosted stage, what blocks it BESIDES
/// the stream opcodes?
#[test]
fn spike_report_stream_sufficiency() {
    println!("\n================ SUFFICIENCY: what remains behind `Stream`?");
    let (mut freed, mut still_blocked) = (0usize, 0usize);

    for (path, m) in compiled_corpus() {
        if !path.to_string_lossy().contains("selfhost/kel") {
            continue;
        }
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        // **Derived from the real lowering, not from a model.** This asked a
        // hand-maintained per-opcode predicate until 2026-08-14, and that
        // predicate was measured stale by 1019 `CallVerifiedNative` instances
        // alone. `module_refusals` reports per CHUNK rather than per op, which
        // is coarser and TRUE; the refusal message names the blocking construct,
        // which is better attribution than the model gave.
        let mut others: BTreeMap<String, usize> = BTreeMap::new();
        for (_, e) in keleusma_native::module_refusals(&m, keleusma_native::LowerOptions::default())
        {
            let text = format!("{e}");
            // The stream family is what this report holds constant.
            if text.contains("Stream") || text.contains("Reset") || text.contains("Yield") {
                continue;
            }
            let head: String = text
                .split(['(', ':'])
                .next()
                .unwrap_or(&text)
                .trim()
                .to_string();
            *others.entry(head).or_default() += 1;
        }

        if others.is_empty() {
            freed += 1;
            println!("  FREED BY STREAM ALONE   {name}");
        } else {
            still_blocked += 1;
            let mut v: Vec<_> = others.iter().collect();
            v.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
            let summary: Vec<String> = v.iter().take(5).map(|(k, n)| format!("{k}x{n}")).collect();
            println!(
                "  still blocked           {name}  by {}",
                summary.join(", ")
            );
        }
    }

    println!("\n  stages freed by the stream work alone : {freed}");
    println!("  stages needing more                   : {still_blocked}");
    println!("  -> if `freed` is 0, the stream increment does NOT deliver Order 1");
    println!("     on its own, and the roadmap ordering needs restating again.");
    println!("================\n");
}

/// Promotes the SOURCE-level shape reading to a bytecode count.
///
/// The inventory records eight of ten stages as degenerate, meaning one top-level
/// `Yield` that is the final statement, so the segment partition has one element
/// and the rotation is the identity. That came from reading `.kel` files. This
/// says what the compiler actually emitted.
#[test]
fn spike_report_yield_shapes() {
    println!("\n================ SHAPE: yields per Stream chunk, corpus-wide");
    let (mut degenerate, mut multi, mut nested_any, mut no_yield) =
        (0usize, 0usize, 0usize, 0usize);
    let mut odd_tails: Vec<String> = Vec::new();

    for (path, m) in compiled_corpus() {
        for chunk in &m.chunks {
            if chunk.block_type != BlockType::Stream {
                continue;
            }
            let s = yield_shape(&chunk.ops);
            let label = format!(
                "{}::{}",
                path.file_name().unwrap_or_default().to_string_lossy(),
                chunk.name
            );
            if s.nested > 0 {
                nested_any += 1;
            } else if s.top_level == 0 {
                // The delegated case: no `Op::Yield` at all, because the yield is
                // in an always-yielding callee. `codegen.kel` is the known one.
                no_yield += 1;
                println!("  DELEGATED (no Op::Yield)  {label}");
            } else if s.top_level == 1 {
                degenerate += 1;
                if s.tail != vec![String::from("PopN(1)")] {
                    odd_tails.push(format!("{label}  tail={:?}", s.tail));
                }
            } else {
                multi += 1;
            }
        }
    }

    println!("\n  degenerate (1 top-level Yield, none nested) : {degenerate}");
    println!("  multi-segment (>1 top-level, none nested)   : {multi}");
    println!("  nested yields (general case)                : {nested_any}");
    println!("  delegated (no Op::Yield in the chunk)       : {no_yield}");
    if odd_tails.is_empty() {
        println!("\n  every degenerate chunk's tail is exactly [PopN(1)], as derived.");
    } else {
        println!("\n  TAIL IS NOT [PopN(1)] — the derivation is incomplete:");
        for t in odd_tails.iter().take(10) {
            println!("     {t}");
        }
    }
    println!("================\n");
}

/// Guard against measuring nothing, which is what makes every zero above look
/// like a finding rather than a broken path.
#[test]
fn the_corpus_is_actually_being_read() {
    let n = compiled_corpus().len();
    assert!(n > 10, "compiled only {n} modules; corpus paths are wrong");
    let streams: usize = compiled_corpus()
        .iter()
        .flat_map(|(_, m)| m.chunks.iter())
        .filter(|c| c.block_type == BlockType::Stream)
        .count();
    assert!(
        streams > 0,
        "no Stream chunks found; the shape report is vacuous"
    );
}

/// THE DELEGATED CASE: what shape are `Reentrant` chunks actually in?
///
/// `codegen.kel` is the one stage that delegates its whole body to a multiheaded
/// `yield` callee. The inventory records a traced hypothesis that such a callee
/// could lower to a function returning the YIELDED value, which would make the
/// delegated stage degenerate over it.
///
/// The hypothesis has one structural precondition it was NOT able to check from
/// source: a multiheaded `Reentrant` chunk compiles its heads to an `If` chain,
/// so its yields sit at nesting depth one or more. The degenerate stream rule
/// requires depth zero, so if the reduction is to work it needs a PER-HEAD rule
/// rather than the whole-chunk one.
///
/// This measures the shape instead of arguing about it. Reports, does not assert:
/// the distribution is a fact about the corpus, not about our code.
#[test]
fn spike_report_reentrant_shapes() {
    println!("\n================ DELEGATED: `Reentrant` chunk shapes");
    let (mut total, mut flat_single, mut nested_any) = (0usize, 0usize, 0usize);
    let mut yield_then_return = 0usize;
    let mut examples: Vec<String> = Vec::new();

    for (path, m) in compiled_corpus() {
        for chunk in &m.chunks {
            if chunk.block_type != BlockType::Reentrant {
                continue;
            }
            total += 1;
            let s = yield_shape(&chunk.ops);

            // How many `Yield`s are immediately followed by `Return`? That is
            // the shape the hypothesis needs: a head that suspends and then
            // returns the resume value, which the caller discards.
            let n_yr = chunk
                .ops
                .windows(2)
                .filter(|w| matches!(w[0], Op::Yield) && matches!(w[1], Op::Return))
                .count();
            let n_y = chunk.ops.iter().filter(|o| matches!(o, Op::Yield)).count();
            if n_y > 0 && n_yr == n_y {
                yield_then_return += 1;
            }

            if s.nested > 0 {
                nested_any += 1;
                if examples.len() < 6 {
                    examples.push(format!(
                        "{}::{}  top={} nested={} yield->return {}/{}",
                        path.file_name().unwrap_or_default().to_string_lossy(),
                        chunk.name,
                        s.top_level,
                        s.nested,
                        n_yr,
                        n_y
                    ));
                }
            } else if s.top_level == 1 {
                flat_single += 1;
            }
        }
    }

    println!("  Reentrant chunks                      : {total}");
    println!("  one top-level Yield, none nested      : {flat_single}");
    println!("  ANY nested Yield (the If-chain shape) : {nested_any}");
    println!("  every Yield immediately before Return : {yield_then_return}");
    if !examples.is_empty() {
        println!("\n  nested examples:");
        for e in &examples {
            println!("     {e}");
        }
    }
    println!("\n  -> if `nested_any` dominates, the reduction needs a PER-HEAD rule,");
    println!("     not the whole-chunk depth rule the degenerate stream uses.");
    println!("================\n");
}

/// THE NESTED CASE: what does `lexer.kel` actually look like at the op level?
///
/// It is the last unclassified class. The source shows yields inside `if`/`else`,
/// but "nested" is a single word covering shapes with very different costs: a
/// yield in each arm of one `If` is a join, while a yield inside a `Loop` is a
/// genuine suspension across a back edge and needs a real frame.
///
/// Reports the per-`Yield` context so the difference is visible instead of
/// collapsed. Reports rather than asserts: this is corpus shape, not our code.
#[test]
fn spike_report_nested_yield_context() {
    println!("\n================ NESTED: per-Yield context in Stream chunks");
    for (path, m) in compiled_corpus() {
        for chunk in &m.chunks {
            if chunk.block_type != BlockType::Stream {
                continue;
            }
            let s = yield_shape(&chunk.ops);
            if s.nested == 0 {
                continue;
            }
            // Walk the block stack so each Yield reports what encloses it. A
            // Yield under any `Loop` is the expensive case; one under `If` only
            // is a control-flow join a phi can express.
            let mut stack: Vec<&'static str> = Vec::new();
            let (mut in_if_only, mut under_loop) = (0usize, 0usize);
            let mut depths: Vec<usize> = Vec::new();
            for op in &chunk.ops {
                match op {
                    Op::If(_) => stack.push("if"),
                    Op::Loop(_) => stack.push("loop"),
                    Op::EndIf | Op::EndLoop(_) => {
                        stack.pop();
                    }
                    Op::Yield => {
                        depths.push(stack.len());
                        if stack.contains(&"loop") {
                            under_loop += 1;
                        } else if !stack.is_empty() {
                            in_if_only += 1;
                        }
                    }
                    _ => {}
                }
            }
            println!(
                "  {}::{}",
                path.file_name().unwrap_or_default().to_string_lossy(),
                chunk.name
            );
            println!("     Yields total          : {}", depths.len());
            println!("     under a Loop (costly) : {under_loop}");
            println!("     inside If only (join) : {in_if_only}");
            println!("     at top level          : {}", s.top_level);
            println!("     nesting depths        : {depths:?}");
            println!("     ops in chunk          : {}", chunk.ops.len());
        }
    }
    println!("\n  -> If-only nesting is a control-flow join. Loop nesting is a");
    println!("     suspension across a back edge and needs a real frame.");
    println!("================\n");
}

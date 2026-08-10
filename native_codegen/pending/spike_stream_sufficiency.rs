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

use inkwell::context::Context;
use keleusma::bytecode::{BlockType, Module, Op};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use std::collections::BTreeMap;

/// Every opcode the lowering handles today.
///
/// **This is a MODEL of the lowering, and a model can drift from what it
/// models.** `spike_corpus_coverage.rs` carries the same list, and a second copy
/// is a second thing to forget. It is duplicated rather than shared because these
/// are independent spikes, and the drift is made detectable rather than
/// prevented: `the_lowered_predicate_has_not_drifted` below asserts that every
/// chunk this predicate calls fully-supported is one `lower_chunk` actually
/// accepts. A silently stale copy would make every sufficiency figure here
/// optimistic, which is the direction that causes wasted work.
fn is_lowered(op: &Op) -> bool {
    matches!(
        op,
        Op::GetLocal(_)
            | Op::SetLocal(_)
            | Op::PopN(_)
            | Op::Dup
            | Op::Const(_)
            | Op::PushImmediate(_)
            | Op::CheckedAdd
            | Op::CheckedSub
            | Op::CheckedNeg
            | Op::CheckedMul(0)
            | Op::Div
            | Op::Mod
            | Op::CheckedDiv(0)
            | Op::CheckedMod
            | Op::CmpEq
            | Op::CmpNe
            | Op::CmpLt
            | Op::CmpGt
            | Op::CmpLe
            | Op::CmpGe
            | Op::Not
            | Op::BitAnd
            | Op::BitOr
            | Op::BitXor
            | Op::Shl
            | Op::Shr
            | Op::If(_)
            | Op::Else(_)
            | Op::EndIf
            | Op::Loop(_)
            | Op::EndLoop(_)
            | Op::Break(_)
            | Op::BreakIf(_)
            | Op::Return
            | Op::Trap(_)
            | Op::Call(_, _)
            | Op::WordToByte
            | Op::ByteToWord
            | Op::BoundsCheck(_)
            | Op::GetData(_)
            | Op::SetData(_)
            | Op::GetDataIndexed(..)
            | Op::SetDataIndexed(..)
    )
}

/// The three opcodes the stream work itself would add. Everything else that is
/// unsupported is a SEPARATE blocker, and separating them is the whole point.
fn is_stream_op(op: &Op) -> bool {
    matches!(op, Op::Stream | Op::Reset | Op::Yield)
}

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

        let mut others: BTreeMap<String, usize> = BTreeMap::new();
        for chunk in &m.chunks {
            for op in &chunk.ops {
                if !is_lowered(op) && !is_stream_op(op) {
                    let disc = format!("{op:?}");
                    let disc = disc.split('(').next().unwrap_or(&disc).to_string();
                    *others.entry(disc).or_default() += 1;
                }
            }
        }

        if others.is_empty() {
            freed += 1;
            println!("  FREED BY STREAM ALONE   {name}");
        } else {
            still_blocked += 1;
            let mut v: Vec<_> = others.iter().collect();
            v.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
            let summary: Vec<String> = v.iter().take(5).map(|(k, n)| format!("{k}x{n}")).collect();
            println!("  still blocked           {name}  by {}", summary.join(", "));
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
    let (mut degenerate, mut multi, mut nested_any, mut no_yield) = (0usize, 0usize, 0usize, 0usize);
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

/// CONTROL for `is_lowered`, which is a duplicated model of the real lowering.
///
/// Every chunk whose ops are all `is_lowered` must be one `lower_chunk` accepts.
/// If the predicate has drifted stale, this fires — and it fires in the direction
/// that matters, because a stale predicate makes the sufficiency report claim
/// more is freed than really is.
///
/// It deliberately does NOT assert the converse. `lower_chunk` may refuse a chunk
/// for a structural reason unrelated to any single opcode, and requiring
/// `is_lowered` to predict that would be asserting a stronger claim than the
/// predicate makes.
#[test]
fn the_lowered_predicate_has_not_drifted() {
    let mut checked = 0usize;
    for (path, m) in compiled_corpus() {
        for chunk in &m.chunks {
            if chunk.block_type == BlockType::Stream || !chunk.ops.iter().all(is_lowered) {
                continue;
            }
            let ctx = Context::create();
            let lm = ctx.create_module("drift");
            let r = keleusma_native::lower_chunk(
                &ctx,
                &lm,
                chunk,
                "probe",
                keleusma_native::LowerOptions::default(),
            );
            assert!(
                r.is_ok(),
                "`is_lowered` says every op of {}::{} is supported, but lower_chunk refused: {:?}\n\
                 The predicate has drifted from the lowering; every sufficiency figure \
                 in this file is optimistic until it is resynchronised.",
                path.display(),
                chunk.name,
                r.err()
            );
            checked += 1;
        }
    }
    assert!(
        checked > 5,
        "only {checked} chunks were fully supported, so this control proved almost nothing"
    );
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
    assert!(streams > 0, "no Stream chunks found; the shape report is vacuous");
}

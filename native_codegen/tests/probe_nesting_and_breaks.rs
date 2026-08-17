//! **TWO QUESTIONS THE `v0.2.3` LINE PUT TO THIS ONE, answered by measurement.**
//!
//! They retracted the claim that a windowed verifier is blocked by needing a
//! whole chunk's control-flow graph — the operator challenged it and was right,
//! the analysis being a fold over a well-nested bracket structure. Two things
//! remained genuinely unresolved, and they asked for this line's view:
//!
//! 1. **nesting depth has no static cap anywhere**, which a verifier written in
//!    Keleusma needs, since its own working set must be bounded;
//! 2. **the break fold assumes every break in a loop leaves the same stack
//!    depth**, which they had not confirmed and thought this line's depth pass
//!    might already know.
//!
//! A view is worth less than a walk, and this line already walks every chunk in
//! the corpus. So both are measured here.
//!
//! # WHAT A MEASURED MAXIMUM IS AND IS NOT
//!
//! **The figures below describe what the shipped corpus CONTAINS. They are not a
//! static cap and not a bound on what the language admits.** Nothing here stops a
//! program nesting deeper tomorrow. The distinction matters because question one
//! is precisely about whether a cap exists, and an observed maximum offered as an
//! answer to that would be a false safety property.
//!
//! What the measurement does settle is the SIZE of the gap: whether a Keleusma
//! verifier would need a deep stack for real inputs, or whether the shipped
//! corpus sits far below any plausible fixed allowance.
use keleusma::bytecode::{Module, Op};
use keleusma::verify::op_depth_effect;
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

/// Mirrors `corpus_differential::source_for`, so this walks the same corpus the
/// differential drives rather than a different one.
fn source_for(p: &std::path::Path) -> Option<String> {
    let src = std::fs::read_to_string(p).ok()?;
    let is_rtos = p.components().any(|c| c.as_os_str() == "rtos");
    let is_prelude = p.file_name().is_some_and(|n| n == "prelude.kel");
    if is_rtos && !is_prelude {
        let prelude = std::fs::read_to_string("../examples/rtos/scripts/prelude.kel").ok()?;
        return Some(format!("{prelude}\n{src}"));
    }
    Some(src)
}

fn sources() -> Vec<std::path::PathBuf> {
    let root = std::path::Path::new("..");
    let mut out = Vec::new();
    let mut stack: Vec<std::path::PathBuf> = [
        "examples/scripts",
        "src/selfhost/kel",
        "examples/rtos/scripts",
        "compiler/kel",
    ]
    .iter()
    .map(|d| root.join(d))
    .collect();
    while let Some(p) = stack.pop() {
        if p.is_dir() {
            if let Ok(rd) = std::fs::read_dir(&p) {
                stack.extend(rd.filter_map(|e| e.ok()).map(|e| e.path()));
            }
        } else if p.extension().is_some_and(|x| x == "kel") {
            out.push(p);
        }
    }
    out.sort();
    out
}

fn modules() -> Vec<(String, Module)> {
    let mut out = Vec::new();
    for p in sources() {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let Some(src) = source_for(&p) else { continue };
        let Some(m) = tokenize(&src)
            .ok()
            .and_then(|t| parse(&t).ok())
            .and_then(|a| compile(&a).ok())
        else {
            continue;
        };
        out.push((name, m));
    }
    out
}

/// What one chunk's walk found.
struct Walk {
    /// Deepest block nesting reached, counting `If` and `Loop` frames.
    max_nesting: usize,
    /// Loops whose breaks disagreed on operand depth, as
    /// `(loop opening index, the differing depths)`.
    disagreeing_loops: Vec<(usize, Vec<i32>)>,
    /// Loops that carried at least one break.
    loops_with_breaks: usize,
}

/// Walk one chunk, tracking block nesting and the operand depth at each break.
///
/// **The depth model is `verify::op_depth_effect`**, the pops-and-pushes model
/// the virtual machine's handlers follow, not the peak model. The question is
/// about the depth a break LEAVES, which is a net quantity.
fn walk(chunk: &keleusma::bytecode::Chunk) -> Walk {
    // Open blocks: (is_loop, saved depth, break depths seen, opening index).
    let mut frames: Vec<(bool, i32, Vec<i32>, usize)> = Vec::new();
    let mut depth = 0i32;
    let mut max_nesting = 0usize;
    let mut disagreeing_loops = Vec::new();
    let mut loops_with_breaks = 0usize;

    for (i, op) in chunk.ops.iter().enumerate() {
        // **`EndIf` RESTORES, it does not accumulate.** Mirrors
        // `spike_bounds_transfer::walk_both_models`, which is the walker this
        // line already validated against both models. A first version of this
        // file invented its own handling -- restoring AND then applying the
        // op's effect -- and reported 365 of 386 loops disagreeing. Competing
        // with a validated walker is how that happens.
        if matches!(op, Op::EndIf) {
            if let Some((_, saved, _, _)) = frames.pop() {
                depth = saved;
            }
            continue;
        }
        if let Op::EndLoop(_) = op {
            if let Some((_, saved, breaks, at)) = frames.pop() {
                if !breaks.is_empty() {
                    loops_with_breaks += 1;
                    let mut distinct = breaks.clone();
                    distinct.sort_unstable();
                    distinct.dedup();
                    if distinct.len() > 1 {
                        disagreeing_loops.push((at, distinct));
                    }
                }
                depth = saved;
            }
            continue;
        }

        let (_req, net) = op_depth_effect(op, chunk);
        depth += net;

        // **The depth a break LEAVES is the depth AFTER its own effect.**
        // `Op::Break` only sets the instruction pointer and unwinds nothing, so
        // what it leaves is what is on the stack at that point; `BreakIf` pops
        // its condition first, which `net` already accounts for.
        if matches!(op, Op::Break(_) | Op::BreakIf(_))
            && let Some(f) = frames.iter_mut().rev().find(|f| f.0)
        {
            f.2.push(depth);
        }
        if matches!(op, Op::If(_)) {
            frames.push((false, depth, Vec::new(), i));
            max_nesting = max_nesting.max(frames.len());
        }
        if matches!(op, Op::Loop(_)) {
            frames.push((true, depth, Vec::new(), i));
            max_nesting = max_nesting.max(frames.len());
        }
    }

    Walk {
        max_nesting,
        disagreeing_loops,
        loops_with_breaks,
    }
}

/// **ANSWER TO BOTH QUESTIONS, over the whole corpus.**
#[test]
fn how_deep_does_nesting_go_and_do_breaks_agree_on_depth() {
    let mods = modules();
    assert!(
        mods.len() > 40,
        "only {} modules compiled; too thin a corpus to answer from",
        mods.len()
    );

    let mut chunks = 0usize;
    let mut deepest = (0usize, String::new());
    let mut hist: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
    let mut loops_with_breaks = 0usize;
    let mut disagreements: Vec<String> = Vec::new();

    for (name, m) in &mods {
        for c in &m.chunks {
            chunks += 1;
            let w = walk(c);
            *hist.entry(w.max_nesting).or_default() += 1;
            if w.max_nesting > deepest.0 {
                deepest = (w.max_nesting, format!("{name}::{}", c.name));
            }
            loops_with_breaks += w.loops_with_breaks;
            for (at, depths) in w.disagreeing_loops {
                disagreements.push(format!(
                    "     {name}::{} loop at op {at}: break depths {depths:?}",
                    c.name
                ));
            }
        }
    }

    println!("\n================ nesting depth and break depths, whole corpus");
    println!("  modules compiled            : {}", mods.len());
    println!("  chunks walked               : {chunks}");
    println!(
        "  DEEPEST NESTING OBSERVED    : {} in {}",
        deepest.0, deepest.1
    );
    println!("  loops carrying a break      : {loops_with_breaks}");
    println!("  loops whose breaks DISAGREE : {}", disagreements.len());
    for d in disagreements.iter().take(20) {
        println!("{d}");
    }
    println!("\n  distribution of per-chunk maximum nesting:");
    for (d, n) in &hist {
        println!("    depth {d:>2}: {n:>5} chunk(s)");
    }
    println!(
        "\n  THIS IS WHAT THE SHIPPED CORPUS CONTAINS, NOT A STATIC CAP.\n  \
         Nothing here bounds what the language admits; a program may nest\n  \
         deeper tomorrow. What it settles is the SIZE of the gap."
    );
    println!("================\n");

    // The property that would make the report meaningless: a walk that never
    // entered a block would report depth 0 everywhere and prove nothing.
    assert!(
        deepest.0 > 0,
        "no chunk reached nesting depth 1, so the walk is not tracking blocks"
    );
    assert!(
        loops_with_breaks > 100,
        "only {loops_with_breaks} loops carried a break; too few to conclude the \
         break-depth property from"
    );
}

/// **THE MUST-FIRE CONTROL, and without it the zero above is worthless.**
///
/// `how_deep_does_nesting_go_and_do_breaks_agree_on_depth` reports that no loop
/// in the corpus has breaks leaving different depths. A detector that can never
/// fire reports exactly the same thing. This hands the walker a chunk whose two
/// breaks leave deliberately different depths and requires it to notice.
///
/// The chunk is assembled by hand rather than compiled, because the compiler does
/// not emit unbalanced breaks — which is the very property being tested, and
/// therefore cannot be used to test it.
#[test]
fn the_break_depth_detector_actually_fires() {
    use keleusma::bytecode::Chunk;

    // loop { push; break; push; push; break; } -- the two breaks leave 1 and 3.
    let c = Chunk {
        name: "synthetic_unbalanced".into(),
        ops: vec![
            Op::Loop(9),
            Op::PushImmediate(1),
            Op::Break(9),
            Op::PushImmediate(1),
            Op::PushImmediate(1),
            Op::Break(9),
            Op::EndLoop(0),
        ],
        constants: Vec::new(),
        struct_templates: Vec::new(),
        local_count: 0,
        param_count: 0,
        block_type: keleusma::bytecode::BlockType::Func,
        param_types: Vec::new(),
        debug_pool: None,
    };

    let w = walk(&c);
    println!(
        "\n  control: synthetic loop, disagreeing loops = {}",
        w.disagreeing_loops.len()
    );
    for (at, d) in &w.disagreeing_loops {
        println!("  control: loop at op {at} break depths {d:?}");
    }
    assert_eq!(
        w.disagreeing_loops.len(),
        1,
        "the detector did not notice two breaks leaving different depths, so the \
         zero it reports over the corpus asserts nothing"
    );
}

//! RESEARCH SPIKE: do the preconditions of the stream-rotation hypothesis hold?
//!
//! The inventory records a hypothesis that most `Stream` chunks need no
//! coroutine frame, because `Reset` clears every local and therefore no state
//! crosses a suspension. It also records three preconditions, none of which was
//! checked when the hypothesis was written. Two are statically checkable and are
//! checked here. The third, that the rotation preserves observable order, is a
//! program-equivalence claim and is NOT addressed.
//!
//! # The approximation, stated up front
//!
//! The checks walk the LINEAR instruction stream, not the control-flow graph.
//! For "is there a `Reset` between consecutive `Yield`s" that is conservative in
//! the useful direction: a `Reset` appearing textually between two yields may be
//! branched around, so a chunk this reports as safe COULD be unsafe. A chunk it
//! reports as unsafe is unsafe. Any use of this beyond triage needs the graph.

use keleusma::bytecode::{BlockType, Op};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

fn corpus() -> Vec<std::path::PathBuf> {
    let root = std::path::Path::new("..");
    let mut v = Vec::new();
    for d in [
        "examples/scripts",
        "src/selfhost/kel",
        "examples/rtos/scripts",
        "compiler/kel",
    ] {
        let mut stack = vec![root.join(d)];
        while let Some(p) = stack.pop() {
            if p.is_dir() {
                if let Ok(rd) = std::fs::read_dir(&p) {
                    stack.extend(rd.filter_map(|e| e.ok()).map(|e| e.path()));
                }
            } else if p.extension().is_some_and(|x| x == "kel") {
                v.push(p);
            }
        }
    }
    v.sort();
    v
}

#[test]
fn spike_report_rotation_preconditions() {
    let mut streams = 0usize;
    // Precondition 1: a Reset separates every pair of consecutive Yields, so no
    // local written after one yield can be read by the next.
    let mut p1_ok = 0usize;
    let mut p1_bad = 0usize;
    // Precondition 3: no path leaves the body between a Yield and a Reset. The
    // linear proxy is a Return or Trap appearing after a Yield with no
    // intervening Reset.
    let mut p3_ok = 0usize;
    let mut p3_bad = 0usize;
    let mut both = 0usize;

    for p in corpus() {
        let Ok(t) = std::fs::read_to_string(&p) else {
            continue;
        };
        let Ok(tk) = tokenize(&t) else { continue };
        let Ok(a) = parse(&tk) else { continue };
        let Ok(m) = compile(&a) else { continue };
        for c in &m.chunks {
            if c.block_type != BlockType::Stream {
                continue;
            }
            streams += 1;
            let mut seen_yield = false;
            let mut p1 = true;
            let mut p3 = true;
            for op in &c.ops {
                match op {
                    Op::Yield => {
                        if seen_yield {
                            // A second yield with no reset since the first.
                            p1 = false;
                        }
                        seen_yield = true;
                    }
                    Op::Reset => seen_yield = false,
                    Op::Return | Op::Trap(_) if seen_yield => p3 = false,
                    _ => {}
                }
            }
            if p1 {
                p1_ok += 1;
            } else {
                p1_bad += 1;
            }
            if p3 {
                p3_ok += 1;
            } else {
                p3_bad += 1;
            }
            if p1 && p3 {
                both += 1;
            }
        }
    }

    println!("\n================ SPIKE: stream-rotation preconditions");
    println!("  Stream chunks                                  {streams}");
    println!("  P1 a Reset separates consecutive Yields   ok={p1_ok} violated={p1_bad}");
    println!("  P3 no Return/Trap between Yield and Reset ok={p3_ok} violated={p3_bad}");
    println!("  BOTH hold (rotation candidate)                 {both}");
    println!(
        "  needs a real coroutine frame                   {}",
        streams - both
    );
    println!("  NOTE: linear-stream approximation; a textually intervening Reset");
    println!("        may be branched around, so 'ok' is necessary and not sufficient.");
    println!("================\n");

    assert!(
        streams > 5,
        "measured almost nothing; corpus paths are probably wrong"
    );
}

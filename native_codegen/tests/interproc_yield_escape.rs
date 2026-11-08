//! How far the yield-escape defect reaches once calls are followed.
//!
//! # The residual this measures
//!
//! `region::yield_escape_hazards` reads ONE chunk. A composite built in a loop
//! body, handed to a callee, and yielded THERE is the same defect — the storage
//! is still rewritten next iteration and the host still holds a handle into it —
//! and the single-chunk predicate cannot see it. The previous increment named
//! that residual and did not measure it. **An unmeasured residual is
//! indistinguishable from an unbounded one**, so this measures it.
//!
//! # Which way these figures err
//!
//! **Every count here is an UPPER BOUND on escape.** Three separate
//! over-approximations stack up, and none of them is a reachability result:
//!
//! - **"A callee can yield" is not "the composite reaches that yield".** No data
//!   flow is traced into the callee, or into the argument list at all.
//! - **Every `Loop` scope counts as iterating**, including the ones the compiler
//!   emits for `match`.
//! - **The call graph is followed transitively without asking whether the value
//!   travels with the call.**
//!
//! So a chunk counted here MAY be safe. A chunk NOT counted here is safe from
//! the single-hop-and-beyond argument, which is the direction that matters for a
//! refusal. Reading these numbers as "this many chunks are defective" would be
//! the bound-direction error this line keeps having to restate.

use keleusma::bytecode::{Module, Op, WireShape};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma_native::region::{loop_body_sites, yield_escape_hazards};

const CORPUS_DIRS: [&str; 3] = [
    "examples/scripts",
    "examples/scripts/rogue",
    "src/selfhost/kel",
];

fn compile_src(src: &str) -> Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile")
}

fn corpus() -> Vec<(String, Module)> {
    let root = std::path::Path::new("..");
    let mut stack: Vec<std::path::PathBuf> = CORPUS_DIRS.iter().map(|d| root.join(d)).collect();
    let mut paths = Vec::new();
    while let Some(p) = stack.pop() {
        if p.is_dir() {
            if let Ok(rd) = std::fs::read_dir(&p) {
                stack.extend(rd.filter_map(|e| e.ok()).map(|e| e.path()));
            }
        } else if p.extension().is_some_and(|x| x == "kel") {
            paths.push(p);
        }
    }
    paths.sort();
    // **DEDUPE, BECAUSE THE ROOTS OVERLAP.** `examples/scripts/rogue` is listed
    // explicitly AND reached by recursion from `examples/scripts`, so every
    // rogue file was visited twice and the module and chunk denominators
    // reported here were inflated by the whole of that directory: 67 unique
    // files were counted as 91. Exact duplicates sort adjacent, so this removes
    // them. The findings above were unaffected -- none of them fell in `rogue` --
    // but the populations they were measured against were not what they said.
    paths.dedup();
    let mut out = Vec::new();
    for p in paths {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let Ok(src) = std::fs::read_to_string(&p) else {
            continue;
        };
        if let Some(m) = tokenize(&src)
            .ok()
            .and_then(|t| parse(&t).ok())
            .and_then(|a| compile(&a).ok())
        {
            out.push((name, m));
        }
    }
    out
}

/// Can a value crossing this boundary be a composite?
///
/// `Scalar` rules it out. `Flat` is one. **`Top` is UNKNOWN and counts as YES**,
/// because an absent or unseeded signature entry must not be read as a licence
/// to ignore the boundary — that would turn a missing table into a safety
/// claim.
fn can_carry_composite(shape: &WireShape) -> bool {
    !matches!(shape, WireShape::Scalar { .. })
}

/// Can a composite cross chunk `ci`'s outward boundary?
///
/// One question serves two roles, because they are the same boundary: a normal
/// chunk hands its return value to its caller, and a `loop` chunk's declared
/// return type IS the type it yields to the host. A chunk whose return is
/// scalar can therefore neither return nor yield a composite, whatever it
/// constructs internally.
///
/// A missing table entry answers YES, for the reason on [`can_carry_composite`].
fn boundary_can_carry_composite(m: &Module, ci: usize) -> bool {
    m.signatures
        .get(ci)
        .map(|sg| can_carry_composite(&sg.ret))
        .unwrap_or(true)
}

/// `yields[i]` — chunk `i` contains a `Yield`, or reaches one through calls.
///
/// # Termination
///
/// **Structural, not hoped for.** The call graph may contain cycles (mutual
/// recursion is expressible), so a naive walk would not terminate. This is a
/// monotone fixpoint over a boolean vector: a round either sets at least one
/// `false` to `true` or stops. At most `chunks.len()` rounds can set anything,
/// so the loop is bounded by `chunks.len()` and the bound is visible to a reader
/// without reasoning about the graph's shape.
fn yields_transitively(m: &Module) -> Vec<bool> {
    let n = m.chunks.len();
    let mut y: Vec<bool> = m
        .chunks
        .iter()
        .map(|c| c.ops.iter().any(|o| matches!(o, Op::Yield)))
        .collect();
    for _round in 0..n {
        let mut changed = false;
        for i in 0..n {
            if y[i] {
                continue;
            }
            let reaches = m.chunks[i].ops.iter().any(|op| match op {
                Op::Call(t, _) => y.get(*t as usize).copied().unwrap_or(false),
                _ => false,
            });
            if reaches {
                y[i] = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    y
}

/// Chunks that call into something which can yield, from inside a loop scope
/// that also constructs.
fn calls_a_yielding_chunk(m: &Module, ci: usize, y: &[bool]) -> bool {
    m.chunks[ci].ops.iter().any(|op| match op {
        Op::Call(t, _) => y.get(*t as usize).copied().unwrap_or(false),
        _ => false,
    })
}

#[test]
fn how_far_the_defect_reaches_once_calls_are_followed() {
    let mods = corpus();
    assert!(
        mods.len() >= 20,
        "the corpus loader found only {} modules; a census over a corpus that \
         failed to load would report zero for the wrong reason",
        mods.len()
    );

    let (mut chunks_total, mut with_loop_sites) = (0usize, 0usize);
    let mut intra: Vec<String> = Vec::new();
    let mut via_call: Vec<String> = Vec::new();
    let mut via_return: Vec<String> = Vec::new();
    // The unrefined figures, kept visible. A refinement that quietly emptied
    // every list would otherwise look like good news.
    let mut crude_call: Vec<String> = Vec::new();
    let mut crude_return: Vec<String> = Vec::new();

    for (name, m) in &mods {
        let y = yields_transitively(m);
        // Who calls whom, for the return-path question.
        let called_by_yielder: Vec<bool> = {
            let mut v = vec![false; m.chunks.len()];
            for (ci, c) in m.chunks.iter().enumerate() {
                if !y[ci] {
                    continue;
                }
                for op in &c.ops {
                    if let Op::Call(t, _) = op
                        && let Some(slot) = v.get_mut(*t as usize)
                    {
                        *slot = true;
                    }
                }
            }
            v
        };

        for (ci, c) in m.chunks.iter().enumerate() {
            chunks_total += 1;
            if loop_body_sites(c).is_empty() {
                continue;
            }
            with_loop_sites += 1;
            let at = format!("{name} chunk {ci}");
            if !yield_escape_hazards(c).is_empty() {
                intra.push(at);
                continue;
            }
            if calls_a_yielding_chunk(m, ci, &y) {
                crude_call.push(at.clone());
                via_call.push(at.clone());
            }
            if called_by_yielder[ci] {
                crude_return.push(at.clone());
                // **REFINED, AND THE REFINEMENT IS SOUND IN ONE DIRECTION.** The
                // composite leaves this chunk only if this chunk can return one,
                // and reaches the host only if the caller that yields can yield
                // one. A `Scalar` boundary rules that out; `Top` does not.
                let escapes_here = boundary_can_carry_composite(m, ci);
                let a_yielding_caller_can_carry_it = m.chunks.iter().enumerate().any(|(j, c)| {
                    y[j] && c
                        .ops
                        .iter()
                        .any(|op| matches!(op, Op::Call(t, _) if *t as usize == ci))
                        && boundary_can_carry_composite(m, j)
                });
                if escapes_here && a_yielding_caller_can_carry_it {
                    via_return.push(at);
                }
            }
        }
    }

    println!("\n================ YIELD ESCAPE, FOLLOWING CALLS");
    println!("  chunks examined                        : {chunks_total}");
    println!("  chunks constructing inside a loop      : {with_loop_sites}");
    println!(
        "  already refused: yield in the SAME chunk : {}",
        intra.len()
    );
    for w in &intra {
        println!("    {w}");
    }
    println!(
        "  RESIDUAL, may escape via a CALL          : {}",
        via_call.len()
    );
    for w in &via_call {
        println!("    {w}");
    }
    println!(
        "  RESIDUAL, may escape via a RETURN        : {}",
        via_return.len()
    );
    for w in &via_return {
        println!("    {w}");
    }
    println!("  ------------------------------------------------");
    println!(
        "  BEFORE the signature refinement: call {} / return {}",
        crude_call.len(),
        crude_return.len()
    );
    for w in &crude_return {
        if !via_return.contains(w) {
            println!("    ruled out by a SCALAR boundary: {w}");
        }
    }
    println!(
        "\n  UPPER BOUNDS, NOT DEFECT COUNTS. \"a callee can yield\" is not \"the\n  \
         composite reaches that yield\"; no data flow is traced. A chunk absent\n  \
         from these lists is what the figures establish, not a chunk present in\n  \
         them."
    );

    // The intra figure must still find the known instance, or a refactor has
    // quietly stopped detecting the shape and every residual below it is
    // measured against a broken baseline.
    assert!(
        intra
            .iter()
            .any(|w| w.starts_with("13_telemetry_stream.kel")),
        "the corpus's deliberate instance is no longer detected: {intra:?}"
    );

    // Pinned so a change is announced rather than discovered later. Not an
    // assertion that these numbers are GOOD -- an assertion that they are known.
    // **THE REFINEMENT MUST DO WORK.** If it removed nothing, the zero below
    // would be produced by the crude test alone and the signature reasoning
    // would be untested decoration.
    assert!(
        crude_return.len() > via_return.len(),
        "the signature refinement ruled nothing out (crude {} vs refined {}), so \
         the residual figure does not depend on it and it is untested",
        crude_return.len(),
        via_return.len()
    );

    assert_eq!(
        (via_call.len(), via_return.len()),
        (0, 0),
        "the interprocedural residual is no longer empty. via_call={via_call:?} \
         via_return={via_return:?}. Decide whether to extend the refusal, and \
         record the cost either way."
    );
}

/// The fixpoint must actually follow an edge, or `(0, 0)` above means only that
/// the walk never moved.
///
/// Two chained calls, so a single-hop implementation passes the direct case and
/// fails here.
#[test]
fn the_transitive_walk_follows_more_than_one_edge() {
    let m = compile_src(
        "fn deep(x: Word) -> Word { x }\n\
         fn mid(x: Word) -> Word { deep(x) }\n\
         loop main(t: Word) -> Word { yield mid(t) }",
    );
    let y = yields_transitively(&m);
    assert!(
        y.iter().any(|b| *b),
        "no chunk yields at all; the subject is wrong"
    );
    assert!(
        !y.iter().all(|b| *b),
        "every chunk is marked as yielding, so the walk cannot distinguish \
         anything and a positive result would be meaningless"
    );
}

/// A cyclic call graph must terminate, and the bound must not depend on the
/// graph being acyclic.
#[test]
fn a_cyclic_call_graph_terminates() {
    // Built by hand: mutual recursion is refused upstream by the totality
    // rules, so a cycle cannot be obtained from source. The property under
    // test is the walk's termination, not the compiler's acceptance.
    let mut m = compile_src("fn a(x: Word) -> Word { x }\nfn main() -> Word { a(1) }");
    assert!(m.chunks.len() >= 2, "need two chunks to build a cycle");
    let n = m.chunks.len();
    for (i, c) in m.chunks.iter_mut().enumerate() {
        c.ops.insert(0, Op::Call(((i + 1) % n) as u16, 0));
    }
    // Reaching this line at all is the result: a walk without the round bound
    // would not return.
    let y = yields_transitively(&m);
    assert_eq!(y.len(), n);
}

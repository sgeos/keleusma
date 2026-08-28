//! Do the corpus's structurally awkward cases concentrate in a few modules?
//!
//! # The claim being tested, which this line published
//!
//! Three independent investigations converged on `14_frame_log.kel::main` op 24,
//! and the handoff recorded that this **"suggests the corpus's awkward cases
//! cluster."** That is a hypothesis stated as a finding, and it changes
//! behaviour: if true it justifies searching a few modules first; if false it
//! sends future work confidently to the wrong place.
//!
//! **Three convergences are weak evidence either way.** They are equally
//! consistent with a corpus where the properties are common and this line simply
//! asked three questions that all touch composites. **Distinguishing those
//! requires counting.**
//!
//! # Selection by attention, acknowledged
//!
//! Modules investigated often appear often. `14_frame_log.kel` and
//! `12_sensor_window.kel` are prominent in this line's notes partly because they
//! were examined repeatedly. **The properties below are chosen for reasons the
//! backend has independently of those two modules**, and the distribution is
//! reported over every module rather than sampled from memory.

use keleusma::bytecode::{Module, NewCompositeOperand, Op};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::verify::op_depth_effect;
use keleusma_native::{LowerOptions, module_refusals};

const CORPUS_DIRS: [&str; 4] = [
    "../examples/scripts",
    "../src/selfhost/kel",
    "../examples/rtos/scripts",
    "../compiler/kel",
];

fn corpus() -> Vec<(String, Module)> {
    let mut stack: Vec<std::path::PathBuf> =
        CORPUS_DIRS.iter().map(std::path::PathBuf::from).collect();
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

/// Producer op index for each live operand-stack slot, before `upto`.
///
/// **`op_depth_effect`, not the peak model.** The latter is a transient reach
/// and a net; a shadow stack built on it desynchronises at every pop-and-push
/// instruction, which this line did once already.
fn producers_before(c: &keleusma::bytecode::Chunk, upto: usize) -> Vec<usize> {
    let mut stack: Vec<usize> = Vec::new();
    for (i, op) in c.ops[..upto].iter().enumerate() {
        let (required, delta) = op_depth_effect(op, c);
        for _ in 0..required.max(0) {
            stack.pop();
        }
        for _ in 0..(required + delta).max(0) {
            stack.push(i);
        }
    }
    stack
}

/// The six properties, each mattering to the backend for its own reason.
///
/// # ⚠ EVERY NAME HERE WAS CHECKED AGAINST ITS BODY, AFTER ONE FAILED
///
/// The third was named **"yields a composite"** and implemented as *a chunk
/// containing both a `Yield` and a `NewComposite`* — co-occurrence, not the
/// claim. It counted `14_frame_log.kel`, whose entry is
/// `loop main(tick: Word) -> Word` and which yields a **Word**, so that module
/// was reported as holding four properties when it holds three.
///
/// **The instrument built to correct an attention-driven claim had a proxy that
/// overclaimed.** An instrument is not exempt from the scrutiny applied to the
/// claims it measures.
///
/// The other five were re-read against their implementations:
///
/// - *constructs in a break scope* — **renamed.** `Op::Loop` is a break-scope
///   marker the compiler also emits for `match`, so "inside a loop" asserted
///   more than the body checks. The body is unchanged; only the name was wrong.
/// - *stores a composite to a slot* — matches: a `SetData` whose stored value is
///   produced by a `Flat` construction.
/// - *returns a composite* — matches: the value live at the last `Return` is
///   produced by a `Flat` construction.
/// - *packs a multi-write local* — matches: a construction one of whose operands
///   is a read of a local the chunk writes more than once.
/// - *is refused by the backend* — matches: the backend reports a refusal.
const PROPERTY_NAMES: [&str; 6] = [
    "constructs in a break scope",  // cross-iteration slot reuse
    "stores a composite to a slot", // persistence versus temporary placement
    "yields a composite",           // escape to the host
    "returns a composite",          // caller-region ABI
    "packs a multi-write local",    // operand-width certification
    "is refused by the backend",    // the coverage frontier
];

fn properties(m: &Module) -> [bool; 6] {
    let mut p = [false; 6];

    for c in &m.chunks {
        // 1: a Flat construction inside any Loop scope.
        let mut depth = 0usize;
        for op in &c.ops {
            match op {
                Op::Loop(_) => depth += 1,
                Op::EndLoop(_) => depth = depth.saturating_sub(1),
                Op::NewComposite(NewCompositeOperand::Flat { .. }) if depth > 0 => p[0] = true,
                _ => {}
            }
        }

        // 2 and 5: walks that need operand provenance.
        let writes = |idx: u16| {
            c.ops
                .iter()
                .filter(|o| matches!(o, Op::SetLocal(n) if *n == idx))
                .count()
        };
        for (i, op) in c.ops.iter().enumerate() {
            match op {
                Op::SetData(_) => {
                    if let Some(&prod) = producers_before(c, i).last()
                        && matches!(
                            c.ops.get(prod),
                            Some(Op::NewComposite(NewCompositeOperand::Flat { .. }))
                        )
                    {
                        p[1] = true;
                    }
                }
                Op::NewComposite(NewCompositeOperand::Flat { count, .. }) => {
                    let st = producers_before(c, i);
                    let n = *count as usize;
                    if st.len() >= n {
                        for &prod in &st[st.len() - n..] {
                            if let Some(Op::GetLocal(idx)) = c.ops.get(prod)
                                && writes(*idx) > 1
                            {
                                p[4] = true;
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // 3: a chunk that yields, whose YIELDED TYPE is a composite.
        //
        // **For a `loop` chunk the declared return type IS what it yields**, so
        // the signature's shape distinguishes a composite yield from a `Word`
        // one. The earlier version tested co-occurrence of `Yield` and
        // `NewComposite` and counted a module that yields a `Word`.
        if c.ops.iter().any(|o| matches!(o, Op::Yield)) {
            let ci = m.chunks.iter().position(|x| std::ptr::eq(x, c));
            let yields_composite = ci
                .and_then(|i| m.signatures.get(i))
                .is_some_and(|sg| matches!(sg.ret, keleusma::bytecode::WireShape::Flat { .. }));
            if yields_composite {
                p[2] = true;
            }
        }

        // 4: a construction whose value is the returned one.
        if let Some(ret) = c.ops.iter().rposition(|o| matches!(o, Op::Return))
            && let Some(&prod) = producers_before(c, ret).last()
            && matches!(
                c.ops.get(prod),
                Some(Op::NewComposite(NewCompositeOperand::Flat { .. }))
            )
        {
            p[3] = true;
        }
    }

    p[5] = !module_refusals(m, LowerOptions::default()).is_empty();
    p
}

#[test]
fn how_the_awkward_properties_are_distributed() {
    let corpus = corpus();
    assert!(
        corpus.len() > 50,
        "only {} modules compiled; a distribution over a corpus that failed to \
         load would be flat for the wrong reason",
        corpus.len()
    );

    let mut per_property = [0usize; 6];
    let mut histogram = [0usize; 7];
    let mut carriers: Vec<(String, usize, [bool; 6])> = Vec::new();

    for (name, m) in &corpus {
        let p = properties(m);
        let n = p.iter().filter(|b| **b).count();
        histogram[n] += 1;
        for (i, held) in p.iter().enumerate() {
            if *held {
                per_property[i] += 1;
            }
        }
        if n >= 2 {
            carriers.push((name.clone(), n, p));
        }
    }
    carriers.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    let total = corpus.len();
    println!("\n================ AWKWARD PROPERTIES, FOUR-ROOT CORPUS ({total} modules)");
    for (i, n) in PROPERTY_NAMES.iter().enumerate() {
        println!("  {n:<30} {} of {total}", per_property[i]);
    }
    println!("  ------------------------------------------------");
    println!("  modules by number of properties held:");
    for (n, count) in histogram.iter().enumerate() {
        if *count > 0 {
            println!(
                "    {n} propert{}: {count} module(s)",
                if n == 1 { "y " } else { "ies" }
            );
        }
    }
    println!("  ------------------------------------------------");
    println!("  modules holding two or more:");
    for (name, n, p) in &carriers {
        let which: Vec<&str> = PROPERTY_NAMES
            .iter()
            .enumerate()
            .filter(|(i, _)| p[*i])
            .map(|(_, n)| *n)
            .collect();
        println!("    {name:<28} {n}  {which:?}");
    }
    println!("================\n");

    // **NON-VACUITY: the distribution must be able to discriminate.** If every
    // property were held by every module, or by none, the histogram would be
    // degenerate and could not support any statement about concentration.
    let discriminating = per_property.iter().any(|c| *c > 0 && *c < total);
    assert!(
        discriminating,
        "no property is held by some modules and not others, so this distribution \
         cannot support or refute a clustering claim: {per_property:?}"
    );
    assert!(
        histogram[0] > 0,
        "every module holds at least one property, which would make 'awkward' \
         mean nothing"
    );
}

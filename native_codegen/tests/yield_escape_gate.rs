//! The cost, and the fireability, of refusing the one shape that miscompiles
//! silently.
//!
//! # What is being closed
//!
//! `region::plan_chunk_region` gives each construction site ONE offset for the
//! life of the chunk, so a site in a loop body rewrites the same bytes every
//! iteration. For a composite the host receives by `yield` that is unsound: the
//! value is an arena handle, an overwrite in place advances no epoch, `resolve`
//! succeeds, and the host reads the NEXT iteration's bytes having asked for
//! this one's. `docs/proofs/COMPOSITE_REGION_REUSE.md` §4.1.1 establishes it
//! against the runtime.
//!
//! **A silently wrong value cannot be left to a runtime guard, because there is
//! no runtime guard it trips.** The disposition taken here is to REFUSE the
//! shape at compile time.
//!
//! # Why refusal rather than a better placement
//!
//! Not reusing the slot means one region per iteration, which is unbounded in
//! the iteration count and so gives up the bounded-memory property the whole
//! subproject exists to provide. Making `resolve` fail instead would need the
//! epoch to advance on an in-place overwrite, and epoch semantics live in
//! `src/vm.rs` and the arena, which this line may read and must not edit.
//! Refusal is the disposition available here, and it converts a silent wrong
//! value into a loud one.
//!
//! # Why refusal is sound under a WRONG verdict, which is the design tension
//!
//! The recorded objection to consuming a confinement verdict in the planner is
//! that a wrong verdict would then miscompile. That objection does not reach
//! this gate: `yield_escape_hazards` over-approximates in one direction only,
//! and its result is used to REFUSE rather than to place. A verdict that is
//! wrong in the permissive direction refuses a program that would have been
//! fine — loud and recoverable — and placement still consumes nothing.

use keleusma::bytecode::{Module, Op};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma_native::region::yield_escape_hazards;
use keleusma_native::{LowerOptions, module_refusals};

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

/// The program from the proof's §4.1, verbatim.
///
/// **THE WHOLE GATE RESTS ON THIS TEST.** A refusal that cannot fire is
/// indistinguishable from no refusal at all, and this line has shipped several
/// guards that passed while unable to fail. This one names the exact program the
/// proof used to establish the defect and requires the predicate to see it.
const SECTION_4_1: &str = "struct P { a: Word, b: Word, c: Word }\n\
                           loop main(t: Word) -> P {\n\
                             let xs = [1, 2];\n\
                             for x in xs { let _ = yield P { a: x, b: x, c: x }; }\n\
                             let _ = yield P { a: 0, b: 0, c: 0 };\n\
                             P { a: 9, b: 9, c: 9 }\n\
                           }";

#[test]
fn the_program_the_proof_used_to_establish_the_defect_is_flagged() {
    let m = compile_src(SECTION_4_1);
    let hazards: Vec<_> = m.chunks.iter().flat_map(yield_escape_hazards).collect();
    assert!(
        !hazards.is_empty(),
        "§4.1 compiles to a loop-body composite yielded to the host, which is the \
         defect this predicate exists to see. Detecting nothing here means the \
         gate is decorative."
    );
    // The flagged site must really be a construction, not an arbitrary op index
    // the predicate happened to emit. An earlier guard on this line asserted a
    // count over a value it never validated and passed for weeks on garbage.
    for h in &hazards {
        let c = m
            .chunks
            .iter()
            .find(|c| {
                yield_escape_hazards(c)
                    .iter()
                    .any(|g| g.site_op == h.site_op && g.yield_op == h.yield_op)
            })
            .expect("the hazard came from some chunk");
        assert!(
            matches!(c.ops[h.site_op], Op::NewComposite(_)),
            "site_op {} is {:?}, not a construction",
            h.site_op,
            c.ops[h.site_op]
        );
        assert!(
            matches!(c.ops[h.yield_op], Op::Yield),
            "yield_op {} is {:?}, not a Yield",
            h.yield_op,
            c.ops[h.yield_op]
        );
        assert!(
            h.scope.0 <= h.site_op && h.site_op < h.scope.1,
            "the site must lie inside the scope it was reported under"
        );
    }
}

/// The predicate must SEPARATE the hazardous shape from its nearest benign
/// neighbours, or it is just "this module has a loop".
#[test]
fn the_nearest_benign_neighbours_are_not_flagged() {
    // A loop that constructs but never yields: the value cannot reach the host
    // mid-loop, so the overwrite is unobservable.
    let no_yield = compile_src(
        "struct P { a: Word, b: Word }\n\
         fn main() -> Word {\n\
           let xs = [1, 2];\n\
           for x in xs { let _p = P { a: x, b: x }; }\n\
           0\n\
         }",
    );
    // **NOT VACUOUS.** A source the compiler optimised down to no construction,
    // or that never entered a loop scope, would satisfy the assertion below
    // while testing nothing. This line has shipped guards that passed because
    // their subject was empty, so the subject is checked first.
    assert!(
        has_a_site_inside_a_loop(&no_yield),
        "the benign case must actually construct inside a loop, or 'not flagged' \
         is true for the wrong reason"
    );
    assert!(
        no_yield
            .chunks
            .iter()
            .all(|c| yield_escape_hazards(c).is_empty()),
        "a loop that constructs but never yields has nothing to overwrite behind \
         the host's back"
    );

    // A yield of a composite built OUTSIDE any loop: one site, written once.
    let no_loop = compile_src(
        "struct P { a: Word, b: Word }\n\
         loop main(t: Word) -> P { yield P { a: t, b: t } }",
    );
    assert!(
        no_loop
            .chunks
            .iter()
            .any(|c| c.ops.iter().any(|o| matches!(o, Op::Yield))),
        "the no-loop case must actually yield, or it excludes the hazard for the \
         wrong reason"
    );
    assert!(
        no_loop
            .chunks
            .iter()
            .all(|c| yield_escape_hazards(c).is_empty()),
        "a construction outside every loop scope is written once and is not the \
         cross-iteration defect"
    );
}

/// Does some `NewComposite` sit between a `Loop` and its `EndLoop`?
///
/// Used only to prove a negative test is not vacuous, so it is deliberately
/// cruder than the predicate under test: nesting depth does not matter here,
/// only that the compiler really put a construction inside a loop scope.
fn has_a_site_inside_a_loop(m: &Module) -> bool {
    m.chunks.iter().any(|c| {
        let mut depth = 0usize;
        c.ops.iter().any(|op| {
            match op {
                Op::Loop(_) => depth += 1,
                Op::EndLoop(_) => depth = depth.saturating_sub(1),
                Op::NewComposite(_) if depth > 0 => return true,
                _ => {}
            }
            false
        })
    })
}

/// What refusing costs, measured against what the backend already refuses.
///
/// **THE RECORDED PREMISE WAS WRONG.** The handoff and the obligation document
/// both said the defect was latent because "no corpus module has the shape".
/// The corpus has one: `13_telemetry_stream.kel` was written to carry it and
/// says so in its header. So latency has to be explained by the backend, and it
/// is — that module is refused for an unrelated missing opcode.
///
/// The number that matters is therefore not "how many chunks are flagged" but
/// "how many chunks the gate refuses that the backend would otherwise have
/// lowered". That is what this measures, and it is what makes the gate free.
#[test]
fn the_gate_refuses_nothing_the_backend_would_otherwise_have_lowered() {
    let mods = corpus();
    assert!(
        mods.len() >= 20,
        "the corpus loader found only {} modules; a census over a corpus that \
         failed to load would report zero for the wrong reason",
        mods.len()
    );

    let mut chunks_total = 0usize;
    let mut flagged: Vec<String> = Vec::new();
    let mut flagged_but_already_refused: Vec<String> = Vec::new();
    let mut newly_refused: Vec<String> = Vec::new();

    for (name, m) in &mods {
        let refused_syms: Vec<String> = module_refusals(m, LowerOptions::default())
            .into_iter()
            .map(|(s, _)| s)
            .collect();
        let module_is_refused = !refused_syms.is_empty();
        for (ci, c) in m.chunks.iter().enumerate() {
            chunks_total += 1;
            if yield_escape_hazards(c).is_empty() {
                continue;
            }
            let at = format!("{name} chunk {ci}");
            flagged.push(at.clone());
            if module_is_refused {
                flagged_but_already_refused.push(at);
            } else {
                newly_refused.push(at);
            }
        }
    }

    println!("\n================ YIELD-ESCAPE GATE: WHAT IT COSTS");
    println!("  modules loaded                    : {}", mods.len());
    println!("  chunks examined                   : {chunks_total}");
    println!("  chunks carrying the shape         : {}", flagged.len());
    for w in &flagged {
        println!("    {w}");
    }
    println!(
        "  ...of those, in modules the backend\n  \
         ALREADY refuses for another reason : {}",
        flagged_but_already_refused.len()
    );
    println!(
        "  NEWLY refused by this gate        : {}",
        newly_refused.len()
    );
    println!(
        "\n  INTERPROCEDURAL CASES ARE NOT IN THIS COUNT. A composite built in a\n  \
         loop body, returned, and yielded by the caller is a hazard this\n  \
         single-chunk predicate cannot see."
    );

    // The shape IS present. Asserting it is present guards against a future
    // refactor that quietly stops seeing it and then reports a comfortable zero.
    assert!(
        flagged
            .iter()
            .any(|w| w.starts_with("13_telemetry_stream.kel")),
        "the corpus's deliberate instance of the shape is no longer detected: \
         {flagged:?}"
    );

    assert!(
        newly_refused.is_empty(),
        "the gate now refuses chunks the backend would otherwise have lowered: \
         {newly_refused:?}. That is a real coverage cost and must be a decision \
         rather than a surprise."
    );
}

/// **WHY THE DEFECT HAS NOT BITTEN, ASKED OF THE BACKEND RATHER THAN ASSUMED.**
///
/// The corpus DOES contain the escaping shape — `13_telemetry_stream.kel` was
/// written to carry it, and says so in its own header. So the recorded reason
/// for the defect being latent ("no corpus module has the shape") is false, and
/// the real reason has to come from the backend.
///
/// This test asks it. Whatever the answer is, it is recorded here rather than
/// inferred, because "it does not bite" and "it does not bite FOR THE REASON I
/// think" are different facts and only the second one survives a change
/// elsewhere.
#[test]
fn what_the_backend_does_with_the_module_that_carries_the_shape() {
    let src = std::fs::read_to_string("../examples/scripts/13_telemetry_stream.kel")
        .expect("the flagged corpus module is readable");
    let m = compile_src(&src);

    let hazards: Vec<_> = m
        .chunks
        .iter()
        .enumerate()
        .flat_map(|(ci, c)| yield_escape_hazards(c).into_iter().map(move |h| (ci, h)))
        .collect();
    assert!(
        !hazards.is_empty(),
        "this module is the corpus instance of the shape; a predicate that stops \
         seeing it has stopped working"
    );

    let refusals = module_refusals(&m, LowerOptions::default());
    println!("\n================ 13_telemetry_stream.kel");
    println!("  hazardous sites : {}", hazards.len());
    for (ci, h) in &hazards {
        println!(
            "    chunk {ci} site op {} yielded at op {}",
            h.site_op, h.yield_op
        );
    }
    println!("  backend refusals: {}", refusals.len());
    for (sym, e) in &refusals {
        println!("    {sym}: {e}");
    }

    assert!(
        !refusals.is_empty(),
        "THE BACKEND ACCEPTS THE MODULE THAT CARRIES THE DEFECT. If this fires, \
         the silent-wrong-value case is not latent at all and the gate is not an \
         improvement in principle but a fix for a live defect."
    );
}

/// **CAN THE REFUSAL ACTUALLY FIRE THROUGH `lower_module`?**
///
/// Every chunk that can carry the shape is a `loop` chunk, and a `loop` chunk
/// opens with `Op::Stream`, which this backend refuses. So the yield-escape
/// refusal is SHADOWED today: the module is rejected before lowering reaches
/// the construction site.
///
/// That is worth an explicit test rather than a footnote, because a guard that
/// cannot fire is indistinguishable from no guard, and this line has shipped
/// several of those. What is asserted here is the shadowing itself.
///
/// **THIS TEST IS A TRIPWIRE.** It fails on the day `Stream` is lowered. Whoever
/// lands `Stream` must then confirm that the yield-escape refusal fires in its
/// place — because at that moment it stops being a precaution and becomes the
/// only thing standing between the corpus and a silently wrong value.
#[test]
fn the_yield_escape_refusal_is_shadowed_by_the_missing_stream_opcode() {
    let m = compile_src(SECTION_4_1);
    let refusals = module_refusals(&m, LowerOptions::default());
    let text: Vec<String> = refusals.iter().map(|(s, e)| format!("{s}: {e}")).collect();
    println!("\n================ §4.1 REFUSALS TODAY\n  {text:?}");
    assert!(
        text.iter().any(|t| t.contains("Stream")),
        "the §4.1 module is no longer refused for Stream: {text:?}. If Stream now \
         lowers, check that the yield-escape refusal fires in its place -- that is \
         the whole point of this tripwire."
    );
}

/// The refusal fires on the shape once the shadowing refusal is removed.
///
/// The `Stream` op is replaced in compiled bytecode with an op the backend
/// lowers, so the chunk reaches its construction site. This is a MUTATION of a
/// real module rather than a hand-built one, so the ops around the site are
/// whatever the compiler really emits.
///
/// Without this, the gate's only evidence would be a predicate returning a
/// non-empty vector, which says nothing about whether `lower_module` consults
/// it.
#[test]
fn with_the_shadow_removed_the_lowering_refuses_the_shape() {
    let mut m = compile_src(SECTION_4_1);
    let before: usize = m.chunks.iter().map(|c| c.ops.len()).sum();
    for c in m.chunks.iter_mut() {
        c.ops.retain(|op| !matches!(op, Op::Stream));
    }
    let after: usize = m.chunks.iter().map(|c| c.ops.len()).sum();
    assert_eq!(
        before - after,
        1,
        "expected exactly one Stream to shadow the gate; removed {}",
        before - after
    );
    // Branch structure here is by LABEL (`Loop`/`EndLoop`/`Break` all carry a
    // label operand and are matched on it), not by op index, so dropping one op
    // does not invalidate the targets. The construction site also precedes the
    // `Yield`, so lowering reaches the gate before any Yield refusal.
    let site_before_yield = m.chunks.iter().any(|c| {
        let site = c.ops.iter().position(|o| matches!(o, Op::NewComposite(_)));
        let y = c.ops.iter().position(|o| matches!(o, Op::Yield));
        matches!((site, y), (Some(a), Some(b)) if a < b)
    });
    assert!(
        site_before_yield,
        "the mutation only proves the gate fires if lowering reaches the site \
         before any Yield refusal"
    );

    let refusals = module_refusals(&m, LowerOptions::default());
    let text: Vec<String> = refusals.iter().map(|(s, e)| format!("{s}: {e}")).collect();
    println!("\n================ §4.1 REFUSALS WITH THE SHADOW REMOVED\n  {text:?}");
    assert!(
        text.iter().any(|t| t.contains("yielded at op")),
        "with Stream out of the way the lowering must refuse the yield-escaping \
         composite, and it did not: {text:?}"
    );
}

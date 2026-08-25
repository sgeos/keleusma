//! **DOES ANY SHIPPED PROGRAM BUILD A COMPOSITE INSIDE AN ITERATING LOOP?**
//!
//! Measured 2026-08-23, corpus-wide: **NO**. That single fact decides whether a
//! confinement analysis is worth building, because it is the population such an
//! analysis would serve.
//!
//! # Why the zero matters
//!
//! Theorem B1 of the composite-region-reuse proof licenses reusing one slot
//! across loop iterations for a CONFINED site. **With no composite constructed
//! in any iterating loop, B1 has no subject** — and the backend's soundness
//! obligation, to stop reusing slots of UNCONFINED sites, has nothing here to
//! miscompile. It is LATENT, not live.
//!
//! # `Op::Loop` IS A BREAK-SCOPE MARKER, NOT AN ITERATION MARKER
//!
//! **This is the whole difficulty and it produced a wrong answer first.** The
//! compiler emits `Op::Loop` for real loops, for `match`, and for multi-clause
//! dispatch. Counting every `Loop` scope as a loop body gave **196 of 208 sites
//! surviving a confinement test — 94%, flattering and wrong.** A `match` body
//! runs ONCE, so its sites are never reused and confinement cannot apply.
//!
//! What gave it away was not the 94%: it was **zero of 407 bodies containing a
//! call**, which is implausible and meant the population was not what it claimed.
//!
//! # A `Break` belongs to the scope whose EXIT it targets
//!
//! A second, subtler error: treating ANY `Break` in a body as evidence the scope
//! runs once. **That misreads a real loop containing a `match`**, which inherits
//! the match's breaks. Measured discriminator:
//!
//! ```text
//!   real for-in   Loop(30) at [11], EndLoop(12) at [29], NO Break targeting 30
//!   match         Loop(23) at [2],  Break(23) at [10] and [19], EndLoop(3)
//! ```
//!
//! Attribution by target is what this file uses. The numbers did not move when it
//! was corrected, so **the zero is robust to the refinement rather than an
//! artefact of a stricter reading** — but it would not have been safe to assume.
//!
//! # This census reads the OTHER LINE'S corpus too
//!
//! It walks `src/selfhost/kel` as well as the example directories, so additions
//! by the `v0.2.3` line move it. That is deliberate: the `v0.3.X` operator has
//! asked that line to land examples exercising this shape, and when one arrives
//! **this census should stop reporting zero.**
use keleusma::bytecode::{Module, NewCompositeOperand, Op};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

const CORPUS_DIRS: [&str; 3] = [
    "examples/scripts",
    "examples/scripts/rogue",
    "src/selfhost/kel",
];

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

/// `(body_start, body_end, exit_target)` for each `Loop` scope.
///
/// **A `Break` BELONGS TO THE SCOPE WHOSE EXIT IT TARGETS.** Measured: a `match`
/// emits `Loop(a)` with every arm ending `Break(a)` — same operand. A real
/// `for` body carries no `Break` targeting its own exit. Treating ANY `Break`
/// anywhere in the body as evidence the scope runs once is WRONG: a real loop
/// containing a `match` inherits the match's breaks and would be misread as a
/// once-executor. That error made the first run of this measurement report zero
/// over a stricter population than it claimed.
fn loop_scopes(ops: &[Op]) -> Vec<(usize, usize, u16)> {
    let mut open: Vec<(usize, u16)> = Vec::new();
    let mut out = Vec::new();
    for (i, op) in ops.iter().enumerate() {
        match op {
            Op::Loop(a) => open.push((i, *a)),
            Op::EndLoop(_) => {
                if let Some((s, a)) = open.pop() {
                    out.push((s + 1, i, a));
                }
            }
            _ => {}
        }
    }
    out
}

#[test]
fn how_many_loop_body_sites_survive_a_crude_confinement_test() {
    let mods = corpus();
    let (mut modules, mut loops, mut sites) = (0usize, 0usize, 0usize);
    let mut amb = 0usize;
    let (mut nobreak_bodies, mut nobreak_sites) = (0usize, 0usize);
    // Disqualification counts. A site can be hit by several; each is counted.
    let (mut d_yield, mut d_setlocal, mut d_call, mut d_native) = (0usize, 0usize, 0usize, 0usize);
    let mut confined = 0usize;
    let mut confined_where: Vec<String> = Vec::new();

    for (name, m) in &mods {
        modules += 1;
        for (ci, c) in m.chunks.iter().enumerate() {
            for (lo, hi, exit) in loop_scopes(&c.ops) {
                loops += 1;
                let body = &c.ops[lo..hi];
                let n_sites = body
                    .iter()
                    .filter(|o| matches!(o, Op::NewComposite(NewCompositeOperand::Flat { .. })))
                    .count();
                let breaks_me = body.iter().any(|o| matches!(o, Op::Break(a) if *a == exit));
                if !breaks_me {
                    nobreak_bodies += 1;
                    nobreak_sites += n_sites;
                }
                if n_sites == 0 {
                    continue;
                }
                // **`Op::Loop` IS A BREAK-SCOPE MARKER, NOT AN ITERATION MARKER.**
                // The compiler emits it for real loops, for `match`, AND for
                // multi-clause dispatch. A `match` ends every arm in `Break` and
                // runs ONCE, so its sites are not reused and confinement is
                // irrelevant to them. Measured: a real `for` body carries NO
                // Break; a match body carries one per arm.
                if breaks_me {
                    amb += n_sites;
                    continue;
                }
                sites += n_sites;
                let has_yield = body.iter().any(|o| matches!(o, Op::Yield));
                let has_setlocal = body.iter().any(|o| matches!(o, Op::SetLocal(_)));
                let has_call = body.iter().any(|o| matches!(o, Op::Call(_, _)));
                let has_native = body.iter().any(|o| {
                    matches!(
                        o,
                        Op::CallVerifiedNative(_, _) | Op::CallExternalNative(_, _)
                    )
                });
                if has_yield {
                    d_yield += n_sites;
                }
                if has_setlocal {
                    d_setlocal += n_sites;
                }
                if has_call {
                    d_call += n_sites;
                }
                if has_native {
                    d_native += n_sites;
                }
                if !(has_yield || has_setlocal || has_call || has_native) {
                    confined += n_sites;
                    confined_where
                        .push(format!("{name} chunk {ci} [{lo},{hi})  {n_sites} site(s)"));
                }
            }
        }
    }

    println!("\n================ CRUDE CONFINEMENT SURVIVAL");
    println!("  modules compiled                       : {modules}");
    println!("  Loop scopes walked (ALL kinds)          : {loops}");
    println!("  of those, bodies with NO Break (REAL loops): {nobreak_bodies}");
    println!("  composite sites in those real loops    : {nobreak_sites}");
    println!("  sites in bodies with NO Break (ITERATING) : {sites}");
    println!(
        "  sites in bodies WITH Break (match/dispatch,\n                or a real loop containing break) -- AMBIGUOUS : {amb}"
    );
    println!("  ------------------------------------------------");
    println!("  SURVIVE the crude test (confined)      : {confined}");
    println!("  ------------------------------------------------");
    println!("  disqualified by Yield                  : {d_yield}");
    println!("  disqualified by SetLocal               : {d_setlocal}");
    println!("  disqualified by Call (callee may Return a composite) : {d_call}");
    println!("  disqualified by a native call          : {d_native}");
    println!("\n  Counts overlap: one site may be hit by several.");
    // **THE TWO SIDES OF THIS REPORT ARE NOT THE SAME KIND OF FACT, and reading
    // them as though they were is the error the `v0.2.3` line measured on their
    // own analysis on 2026-08-24.**
    //
    // `confined` is SOUND to trust: a site reaches it only when the body carries
    // none of the escaping opcodes, so no route exists to miss.
    //
    // **EVERY DISQUALIFIER COUNT IS AN UPPER BOUND ON ESCAPE, NOT A RESULT.**
    // They are SYNTACTIC -- "this body contains an `Op::Call`" -- and say nothing
    // about whether the composite ever REACHES that call. A site disqualified
    // here may be perfectly confined.
    //
    // **MEASURED ON THEIR SIDE, WHICH IS WHY THIS IS NOT A HYPOTHETICAL.** Adding
    // real callee summaries moved `examples/scripts` from 17 confined / 12
    // escapes / 4 cannot-establish to 23 / 10 / 0. The four cannot-establish
    // closing was the aim. **The two escapes that also went away were WRONG
    // rather than unestablished** -- with no summary a call's return is assumed
    // to alias every argument, so a composite passed to a callee and then
    // returned was reported as escaping through a route that does not exist.
    //
    // **A CONSERVATIVE DEFAULT HIDES FALSE POSITIVES EXACTLY AS WELL AS IT HIDES
    // GAPS, and there is no third value to record it in.** `Escapes` and
    // `Confined` are both confident answers; one of theirs was simply wrong, and
    // nothing in the corpus said so.
    println!("\n  `confined` IS SOUND. Every disqualifier is an UPPER BOUND on escape,");
    println!("  syntactic rather than a reachability result: a disqualified site may");
    println!("  in fact be confined. `sites - d_call` is therefore a LOWER bound.");
    // **THE ISOLATION FIGURE, AND IT IS THE ONE THE DESIGN NEEDS.** Until
    // absorption 7 every site tripped BOTH `SetLocal` and `Call`, so the census
    // could not say whether a confinement analysis needed a callee summary or
    // only local-store handling -- the two requirements were never separated by
    // any subject. `15_pixel_blend.kel` separates them.
    println!(
        "  sites blocked WITHOUT a call in the body : {} (of {sites})",
        sites.saturating_sub(d_call)
    );
    for w in confined_where.iter().take(12) {
        println!("    survivor: {w}");
    }
    println!("================\n");
    // **THE WALK MUST REACH SOMETHING**, or a zero above means nothing.
    assert!(
        loops > 100,
        "only {loops} Loop scopes walked; the corpus walk is not reaching the \
         modules and every figure here is an artefact of that"
    );
    assert!(
        nobreak_bodies > 0,
        "no genuinely-iterating loop found at all. Either the corpus lost its \
         for-in loops or the Break-attribution discriminator has broken; \
         establish WHICH before reading the site count as a result"
    );
    // **THE ZERO IS GONE, AND THAT WAS THE GUARD WORKING.** It fired on
    // absorption naming the three scripts the `v0.2.3` line landed on operator
    // direction. The verdict is REWRITTEN to the new state; the assertion is not
    // deleted.
    //
    // **WHAT THE NEW STATE MEANS.** Theorem B1/B1r now has subjects, and the
    // native planner's slot reuse is unsound for any of them that is not
    // confined. **It is NOT yet live**: all three are refused by the backend
    // before the differential runs — `13_telemetry_stream` on
    // `UnsupportedOp("Stream")`, the other two on an unknown packed width. The
    // yield route is gated behind Workstream B by construction, since `yield`
    // exists only inside a `Stream` this backend does not lower.
    assert!(
        nobreak_sites > 0,
        "THE SUBJECTS ARE GONE. Composite sites inside iterating loops went to \
         zero, which is how this file read before 2026-08-24. Either the corpus \
         lost 12_sensor_window/13_telemetry_stream/14_frame_log, or the \
         Break-attribution discriminator has broken and is misreading real loops \
         as once-executors. Establish WHICH -- the second would make every figure \
         here an artefact."
    );
    // **ZERO SURVIVORS IS THE DESIGN INPUT, NOT A DISAPPOINTMENT.** A crude
    // "any Escapes opcode in the body" predicate admits NOTHING even with
    // subjects present. Every site trips `SetLocal`, because a `let` inside a
    // loop body is a store.
    //
    // **SUPERSEDED, 2026-08-24 (absorption 7): "THE ANALYSIS NEEDS BOTH FEATURES
    // ON DAY ONE OR IT RETURNS NOTHING" IS NO LONGER TRUE.** That conclusion was
    // drawn when all three subjects tripped BOTH `SetLocal` and `Call`, so the
    // two requirements were inseparable and either alone bought nothing.
    //
    // **`15_pixel_blend.kel` separates them.** It is the call-free confined shape
    // this line asked the `v0.2.3` line for, and with it the counts are
    // `sites = 4`, `d_setlocal = 4`, `d_call = 3`. **At least one site is blocked
    // by `SetLocal` ALONE.** So an analysis implementing only Theorem B1r's
    // boundary-dead store handling -- no callee summary at all -- would admit
    // something rather than nothing, which makes the callee summary a SECOND
    // increment instead of a precondition.
    //
    // **"AT LEAST ONE", NOT "ONE", AND THE DIFFERENCE IS THE POINT.** `d_call` is
    // syntactic: it counts bodies CONTAINING a call, not composites that reach
    // one. So `sites - d_call` is a LOWER bound on what B1r alone admits, and
    // `d_call` is an UPPER bound on what needs a summary.
    //
    // **WHAT THE `v0.2.3` MEASUREMENT DOES AND DOES NOT SHOW, since the careless
    // citation is tempting and they corrected it here.** Their escape count fell
    // 12 to 10 when callee summaries landed, and those two were false rather than
    // unestablished. That establishes the assumption "a call in the body means an
    // escape" is LOOSE in a corpus both lines share. **It does NOT establish that
    // any particular one of the four subjects counted here is misfiled** -- their
    // scan is flat over `examples/scripts`, this one is over composite sites
    // inside iterating loops across four directories. Cite it for the former.
    //
    // **Their analysis, not this crude test, is the authority on which sites are
    // confined** -- subject to the shared blind spot recorded below.
    //
    // **THE SEQUENCING CHANGES; THE SOUNDNESS DOES NOT.** A site admitted on
    // B1r alone is still only as sound as B1r, and the planner's reuse remains
    // unsound for any site that is not confined.
    //
    // **WHEN THE ANALYSIS LANDS THIS SHOULD RISE.** A survivor count that stays
    // at zero after the predicate exists means the predicate is not admitting
    // anything, which is news of a different kind.
    assert_eq!(
        confined, 0,
        "A SITE NOW SURVIVES THE CRUDE TEST. Either the corpus gained a loop-body \
         composite with no store and no call — the `v0.2.3` line said it was \
         adding exactly that, to isolate the SetLocal requirement from the callee \
         summary — or this census stopped counting a disqualifier. Check the \
         breakdown before treating it as progress."
    );

    // **THE ISOLATION IS THE RESULT OF ABSORPTION 7 AND IT IS PINNED HERE.**
    //
    // Without at least one call-free site, `SetLocal` and `Call` are confounded
    // and the census cannot tell the two requirements apart -- which is exactly
    // the state that produced the superseded "needs both on day one" reading
    // above. This assertion is what stops that reading from coming back
    // unnoticed if the corpus loses `15_pixel_blend.kel`.
    //
    // Stated as `> 0` rather than `== 1` deliberately, and the second reason is
    // stronger than the first. **One**: the useful property is that the
    // requirements are SEPARATED, and pinning the exact count would make every
    // future call-free subject a failure. **Two**: `sites - d_call` is a LOWER
    // BOUND, not a count. Asserting equality would be asserting a syntactic
    // over-approximation as if it were a reachability result, which is precisely
    // the reading the `v0.2.3` line's false escapes disprove.
    assert!(
        sites > d_call,
        "EVERY site in an iterating loop again has a Call in its body \
         ({sites} sites, {d_call} with a call), so `SetLocal` and `Call` are \
         confounded once more and this census can no longer say which \
         requirement a confinement analysis needs first. The corpus has probably \
         lost `15_pixel_blend.kel`. Restore a call-free subject before reading \
         the breakdown as a design input."
    );
}

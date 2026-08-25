#![cfg(all(feature = "compile", feature = "verify"))]
//! What the confinement predicate actually answers, over the example corpus.
//!
//! # The measurement this replaces
//!
//! The `v0.3.0` line ran a crude any-`Escapes`-opcode test over the three
//! per-iteration composite sites the corpus held after `12` through `14`
//! landed. **Zero survived** — one disqualified by `Yield`, three by
//! `SetLocal`, three by `Call`. A predicate lacking either the boundary-dead
//! rule or a callee summary admits nothing at all, which is sound and
//! worthless.
//!
//! Both of the crude test's negatives turn out to be answerable without a
//! callee summary, and the reason is dataflow rather than a stronger rule:
//!
//! - **`SetLocal`** is answered by liveness. The slot a per-iteration `let`
//!   writes is rewritten before the next read, so the write does not carry the
//!   region past the boundary.
//! - **`Call`** is answered by not being in the way. `scale(raw[i])` passes a
//!   `Word`, not a composite, so the call never touches the site's region. The
//!   crude test saw the opcode; this one follows the value.
//!
//! # What is NOT established here
//!
//! A composite genuinely passed to a Keleusma call still yields
//! `CannotEstablish`, because no callee summary exists. That is the next
//! increment, and the corpus counts below are where its effect will show.

use keleusma::bytecode::{Chunk, Module, Op};
use keleusma::confine::{Confinement, Reason, Scope, SiteVerdict, chunk_confinement};
use std::path::PathBuf;

fn scripts_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/scripts")
}

fn compile_file(path: &PathBuf) -> Option<Module> {
    let src = std::fs::read_to_string(path).ok()?;
    let tokens = keleusma::lexer::tokenize(&src).ok()?;
    let mut ast = keleusma::parser::parse(&tokens).ok()?;
    keleusma::typecheck::check(&mut ast).ok()?;
    let ast = keleusma::monomorphize::monomorphize(ast);
    keleusma::compiler::compile(&ast).ok()
}

fn module_of(script: &str) -> Module {
    let path = scripts_dir().join(script);
    compile_file(&path).unwrap_or_else(|| panic!("{script} compiles"))
}

/// Every site the analysis judges against an ITERATION, across a module.
fn iteration_sites(module: &Module) -> Vec<(&Chunk, SiteVerdict)> {
    module
        .chunks
        .iter()
        .flat_map(|c| {
            chunk_confinement(c)
                .into_iter()
                .filter(|v| matches!(v.scope, Scope::Iteration { .. }))
                .map(move |v| (c, v))
        })
        .collect()
}

/// The one per-iteration site a corpus script contains.
fn sole_iteration_site(script: &str) -> SiteVerdict {
    let module = module_of(script);
    let sites = iteration_sites(&module);
    assert_eq!(
        sites.len(),
        1,
        "{script} is expected to hold exactly one composite site inside an \
         iterating loop; it holds {}. If the script changed, the expectation \
         below is measuring something other than what it names.",
        sites.len()
    );
    sites[0].1
}

/// THE ISOLATE. A per-iteration composite with no call in its body, so the
/// boundary-dead rule alone must admit it.
///
/// The script exists for this assertion. A verdict of `CannotEstablish` here
/// is a failure, not a conservative answer: nothing about this site is
/// unestablishable with the rule the analysis already implements.
#[test]
fn the_isolate_is_confined() {
    let v = sole_iteration_site("15_pixel_blend.kel");
    assert_eq!(
        (v.verdict, v.reason),
        (Confinement::Confined, Reason::None),
        "15_pixel_blend.kel's per-iteration composite must be confined"
    );
}

/// A call in the body does not by itself disqualify a site, because the
/// analysis follows the value rather than the opcode. `scale(raw[i])` takes a
/// `Word`.
#[test]
fn a_scalar_call_in_the_body_does_not_disqualify_the_site() {
    let v = sole_iteration_site("12_sensor_window.kel");
    assert_eq!(
        (v.verdict, v.reason),
        (Confinement::Confined, Reason::None),
        "the call in 12_sensor_window.kel's loop body passes a Word, so it \
         cannot carry the composite anywhere"
    );
}

/// A composite copied into a data slot does not alias the ephemeral body, so
/// the site stays confined. Treating the copy as an alias would be sound and
/// would make the corpus measure nothing.
#[test]
fn a_composite_copied_to_a_data_slot_stays_confined() {
    let v = sole_iteration_site("14_frame_log.kel");
    assert_eq!(
        (v.verdict, v.reason),
        (Confinement::Confined, Reason::None),
        "SetData copies bytes into the persistent region; no handle is stored"
    );
}

/// The negative case, and what makes the three positives above evidence rather
/// than a constant function. The host holds the handle after the yield.
#[test]
fn a_yielded_per_iteration_composite_is_refused() {
    let v = sole_iteration_site("13_telemetry_stream.kel");
    assert_eq!(
        v.verdict,
        Confinement::Escapes,
        "a yielded composite demonstrably outlives its iteration"
    );
    assert!(
        matches!(v.reason, Reason::Yielded { .. }),
        "and the refusal must name the yield rather than some other route: \
         {:?}",
        v.reason
    );
}

/// Dispatch scopes are not iterations. `Op::Loop` marks both, and every
/// `if`/`match` arm result in the corpus sits inside one.
///
/// Measured 2026-08-23: 30 of the corpus's composite sites sat inside a `Loop`
/// region and every one was an arm result. If the discriminator stopped
/// discriminating, those 30 would appear here as per-iteration sites and the
/// analysis would be answering a question nobody asked.
#[test]
fn dispatch_scopes_do_not_appear_as_iterations() {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(scripts_dir())
        .expect("examples/scripts is readable")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "kel"))
        .collect();
    paths.sort();

    let mut iterating = Vec::new();
    for path in &paths {
        let Some(module) = compile_file(path) else {
            continue;
        };
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        // Every site the walk calls an iteration must sit in a scope whose exit
        // is NOT an unconditional break to itself.
        //
        // The chunk comes from the site's own analysis rather than from
        // searching the module for one with a `Loop` at that address: several
        // chunks can carry a loop at the same address, and a search would let
        // this check pass against a chunk other than the one it names.
        for (chunk, v) in iteration_sites(&module) {
            let Scope::Iteration { loop_ip } = v.scope else {
                unreachable!("iteration_sites filters to iteration scopes")
            };
            let Some(Op::Loop(exit)) = chunk.ops.get(loop_ip) else {
                panic!("{name}: the recorded loop address is not a Loop opcode")
            };
            let end = (*exit as usize).min(chunk.ops.len());
            assert!(
                !chunk.ops[loop_ip + 1..end]
                    .iter()
                    .any(|o| matches!(o, Op::Break(t) if t == exit)),
                "{name} chunk {} site at ip {} was filed as an ITERATION, but \
                 its scope leaves by an unconditional break to its own exit, \
                 which makes it DISPATCH. The discriminator has stopped \
                 discriminating.",
                chunk.name,
                v.ip
            );
            iterating.push((name.clone(), v));
        }
    }

    assert_eq!(
        iterating.len(),
        4,
        "the corpus holds four per-iteration composite sites, one per script \
         12 through 15. A different number means either a script changed or \
         dispatch scopes are being counted: {iterating:?}"
    );
}

/// The corpus-wide counts, recorded so a later reader can tell an analysis
/// that improved from one that did not.
///
/// **This is a MEASUREMENT, not an invariant.** It is expected to move, and the
/// direction that matters is `CannotEstablish` falling as `Confined` rises. It
/// is pinned exactly so that a change is deliberate and explained rather than
/// unnoticed.
///
/// Scanned FLAT, not recursively: `examples/scripts` also holds `piano_roll/`
/// and `rogue/` subdirectories with 34 further scripts, and a recursive scan
/// gives 251 sites instead of 33. A bare site count is meaningless without its
/// scan rule, and this repository already has one unreproducible figure that
/// differs by exactly this.
#[test]
fn the_corpus_verdict_counts_are_recorded() {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(scripts_dir())
        .expect("examples/scripts is readable")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "kel"))
        .collect();
    paths.sort();

    let (mut sites, mut confined, mut escapes, mut cannot) = (0, 0, 0, 0);
    let mut raw = 0usize;
    for path in &paths {
        let Some(module) = compile_file(path) else {
            continue;
        };
        for chunk in &module.chunks {
            raw += chunk
                .ops
                .iter()
                .filter(|o| matches!(o, Op::NewComposite(_)))
                .count();
            for v in chunk_confinement(chunk) {
                sites += 1;
                match v.verdict {
                    Confinement::Confined => confined += 1,
                    Confinement::Escapes => escapes += 1,
                    Confinement::CannotEstablish => cannot += 1,
                }
            }
        }
    }

    assert_eq!(
        sites, raw,
        "every NewComposite must be judged. A site the walk drops would be \
         silently unmeasured rather than reported."
    );
    assert_eq!(
        (sites, confined, escapes, cannot),
        (33, 17, 12, 4),
        "the corpus verdict counts moved. If the analysis improved, update \
         this and say what moved; if a script changed, say that. Do not update \
         it without reading which of the three columns moved."
    );
}

//! **CAN THE CONFINEMENT REFUSAL ACTUALLY FIRE?** It adds nothing on the corpus,
//! which is the acceptance criterion holding and is also exactly the shape of a
//! guard that cannot fire. This asks which source shapes reach it.
mod common;
use keleusma::confine::{Confinement, Scope, module_confinement};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

fn build(src: &str) -> Option<keleusma::bytecode::Module> {
    tokenize(src)
        .ok()
        .and_then(|t| parse(&t).ok())
        .and_then(|a| compile(&a).ok())
}

#[test]
fn which_shapes_reach_the_confinement_refusal() {
    let cases: &[(&str, &str)] = &[
        (
            "return a composite from inside a loop",
            "fn f() -> [Word; 2] { let ch = [0, 1, 2]; for c in ch { return [c, c]; } [0, 0] }\nfn main(a: Word, b: Word) -> Word { let r = f(); r[0] + b }",
        ),
        (
            "helper returns a loop-built composite; caller returns it on",
            "fn mk() -> [Word; 2] { let ch = [0, 1]; for c in ch { return [c, c]; } [0, 0] }\nfn g() -> [Word; 2] { mk() }\nfn main(a: Word, b: Word) -> Word { let r = g(); r[0] + b }",
        ),
        (
            "composite built in a loop and returned after it",
            "fn f() -> [Word; 2] { let ch = [0, 1]; let t = [0, 0]; for c in ch { let p = [c, c]; } t }\nfn main(a: Word, b: Word) -> Word { let r = f(); r[0] + b }",
        ),
        (
            "control: built and consumed inside the same iteration",
            "fn main(a: Word, b: Word) -> Word { let ch = [0, 1]; let t = 0; for c in ch { let p = [c, b]; t = t + p[0]; } t }",
        ),
    ];
    println!("\n================ WHICH SHAPES REACH THE CONFINEMENT REFUSAL");
    let mut compiled = 0usize;
    for (name, src) in cases {
        let Some(m) = build(src) else {
            println!("  {name}\n    reference compiler refuses");
            continue;
        };
        compiled += 1;
        let verdicts = module_confinement(&m);
        let mut iter_declines = 0usize;
        for (ci, per) in verdicts.iter().enumerate() {
            for v in per {
                if matches!(v.scope, Scope::Iteration { .. })
                    && !matches!(v.verdict, Confinement::Confined)
                {
                    iter_declines += 1;
                    println!(
                        "  {name}\n    chunk {} ({}) op {} -> {:?} {:?}",
                        ci, m.chunks[ci].name, v.ip, v.verdict, v.reason
                    );
                }
            }
        }
        let refusals =
            keleusma_native::module_refusals(&m, keleusma_native::LowerOptions::default());
        let why = refusals
            .first()
            .map(|(c, e)| format!("{c}: {e}"))
            .unwrap_or_else(|| "LOWERS".into());
        println!("  {name}\n    iteration-scope declines: {iter_declines}   backend: {why}");
    }
    println!("================\n");
    assert!(
        compiled > 0,
        "nothing compiled; this probe measures the harness"
    );
}

/// **THE GUARD'S REACH, PROVED BY MUTATING REAL BYTECODE.**
///
/// The confinement refusal adds nothing on the corpus, which is the acceptance
/// criterion holding and is also indistinguishable from a guard that cannot
/// fire. Measured above: it cannot, FROM SOURCE. The reference compiler refuses
/// an early return inside a loop and refuses reassignment, so `yield` is the
/// only route by which a loop-built composite escapes its iteration, and every
/// chunk carrying that shape is refused for `Stream` before the placement is
/// reached.
///
/// **That is the accidental protection the obligation names, and it expires the
/// day `Stream` lowers.** So the guard is proved reachable the way the typed
/// verifier's conformance corpus proves its own: by mutating a real module
/// rather than by writing a source program the language will not accept.
///
/// `Op::Stream` and `Op::Reset` are replaced by `Op::PopN(0)`, which is a no-op
/// and preserves every op index, so the site addresses the analysis reports stay
/// valid.
#[test]
fn the_confinement_refusal_fires_once_the_stream_refusal_is_out_of_the_way() {
    use keleusma::bytecode::Op;

    let path = common::corpus_sources()
        .into_iter()
        .find(|p| {
            p.file_name()
                .is_some_and(|n| n == "13_telemetry_stream.kel")
        })
        .expect("13_telemetry_stream.kel is the module written to carry the escaping shape");
    let src = std::fs::read_to_string(&path).expect("read");
    let mut m = build(&src).expect("compiles");

    // Confirm the premise before mutating: today this module is refused for
    // Stream, NOT for confinement.
    let before = keleusma_native::module_refusals(&m, keleusma_native::LowerOptions::default());
    let before_text = before
        .first()
        .map(|(c, e)| format!("{c}: {e}"))
        .unwrap_or_else(|| "LOWERS".into());
    assert!(
        before_text.contains("Stream"),
        "premise gone: this module is no longer refused for Stream, it says {before_text}. \
         The mutation below is then testing something else"
    );

    let mut replaced = 0usize;
    for chunk in &mut m.chunks {
        for op in &mut chunk.ops {
            if matches!(op, Op::Stream | Op::Reset) {
                *op = Op::PopN(0);
                replaced += 1;
            }
        }
    }
    assert!(
        replaced > 0,
        "no Stream or Reset found; the mutation applied nothing"
    );

    let after = keleusma_native::module_refusals(&m, keleusma_native::LowerOptions::default());
    let after_text = after
        .first()
        .map(|(c, e)| format!("{c}: {e}"))
        .unwrap_or_else(|| "LOWERS".into());

    println!("\n================ THE GUARD'S REACH");
    println!("  Stream/Reset ops replaced : {replaced}");
    println!("  before the mutation       : {before_text}");
    println!("  after the mutation        : {after_text}");
    println!("================\n");

    // **WHAT THIS ASSERTS, AND WHAT IT DELIBERATELY DOES NOT.**
    //
    // With `Stream` out of the way the site is still refused -- but by the
    // PRE-EXISTING syntactic check, which sits ahead of the confinement one by
    // design, because the confinement verdict may only ADD refusals and never
    // remove one. So this does not assert which check fires, only that the
    // hazard is refused. Asserting the confinement message here would fail the
    // moment the ordering did its job.
    assert!(
        after_text.contains("op 24"),
        "with the Stream refusal out of the way the escaping site is no longer \
         refused at all. It said: {after_text}"
    );

    // The analysis's own answer for that site, asserted directly. This is the
    // input the lowering consumes, and it is what would fire if the syntactic
    // check ahead of it ever stopped covering the shape.
    let verdicts = module_confinement(&m);
    let mut escaping_at_iteration = 0usize;
    for per in &verdicts {
        for v in per {
            if matches!(v.scope, Scope::Iteration { .. })
                && !matches!(v.verdict, Confinement::Confined)
            {
                escaping_at_iteration += 1;
            }
        }
    }
    assert!(
        escaping_at_iteration > 0,
        "the analysis reports every site in this module confined to its \
         iteration, so the verdict the lowering consumes would license exactly \
         the reuse the obligation is about"
    );

    // **REACH, PROVED BY A MUTATION RECORDED IN THE SESSION RATHER THAN HERE.**
    // Disabling the syntactic check makes the confinement refusal fire on this
    // same site, naming it as `Escapes its iteration (Yielded { ip: 25 })`. That
    // is what establishes the new check is not a guard that cannot fire. It is
    // not a permanent test because a test that disables a neighbouring guard to
    // observe its successor would have to keep them both, and the ordering
    // between them is the safety property.
}

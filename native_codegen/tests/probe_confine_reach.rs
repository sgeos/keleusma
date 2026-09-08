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

/// **THE GUARD'S REACH — NO LONGER A SIMULATION. THE DAY ARRIVED.**
///
/// This test used to MUTATE real bytecode, replacing `Op::Stream` and `Op::Reset`
/// with no-ops, because every chunk carrying an escaping composite was refused
/// for `Stream` before the placement was reached. Its own header said that
/// accidental protection *"expires the day `Stream` lowers"*.
///
/// **It has expired.** General `Op::Stream` lowering landed, and
/// `13_telemetry_stream.kel` is now refused by the escape check itself, naming
/// the site. The mutation is therefore removed rather than kept: a simulation
/// whose premise has come true is no longer evidence about anything, and the
/// premise assertion it opened with is what caught the change.
///
/// The refusal is observed through `module_refusals`, which is the boundary a
/// consumer meets, and not by calling the analysis directly — a predicate that
/// reports an escape while the emitter lowers the chunk anyway would satisfy a
/// direct test and ship a wrong module.
#[test]
fn the_confinement_refusal_fires_on_the_real_module_with_no_mutation() {
    let path = common::corpus_sources()
        .into_iter()
        .find(|p| {
            p.file_name()
                .is_some_and(|n| n == "13_telemetry_stream.kel")
        })
        .expect("13_telemetry_stream.kel is the module written to carry the escaping shape");
    let src = std::fs::read_to_string(&path).expect("read");
    let m = build(&src).expect("compiles");

    let refusals = keleusma_native::module_refusals(&m, keleusma_native::LowerOptions::default());
    let text = refusals
        .iter()
        .map(|(c, e)| format!("{c}: {e}"))
        .collect::<Vec<_>>()
        .join(" | ");

    println!("\n================ THE GUARD'S REACH, UNMUTATED");
    println!("  refusals: {text}");
    println!("================\n");

    // **NOT REFUSED FOR `Stream`.** Stated as its own assertion because the
    // whole point of the change is that the shadowing refusal is gone; if it
    // came back, the test below would still pass for the wrong reason.
    assert!(
        !text.contains("does not yet support opcode Stream"),
        "this module is refused for Stream again, so the escape refusal is \
         shadowed once more and this test is not measuring it: {text}"
    );
    assert!(
        text.contains("op 24"),
        "the escaping site is no longer refused. It said: {text}"
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

    // **WHICH CHECK FIRES IS DELIBERATELY NOT ASSERTED.** The pre-existing
    // syntactic check sits ahead of the confinement one by design, because the
    // confinement verdict may only ADD refusals and never remove one. Naming the
    // confinement message here would fail the moment that ordering did its job.
}

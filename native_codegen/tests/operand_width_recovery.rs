//! What actually makes the last two composite sites unlowerable.
//!
//! The backend refuses `12_sensor_window.kel::main` op 23 and
//! `14_frame_log.kel::main` op 24 for an **unknown operand width**. That was the
//! condition; this file establishes the CAUSE, which is a different thing, and
//! pins it so the next attempt does not start from the same guesses.
//!
//! # The cause, measured
//!
//! The unknown operand is **the first of three**, and in both modules it is a
//! `GetLocal` of the `for` loop's induction variable. A local's width is trusted
//! only when the chunk writes it **at most once**, and an induction variable is
//! written twice — once at initialisation and once by the loop's increment.
//!
//! **That rule is deliberate and it is sound.** The width pass is a linear scan
//! and cannot see a back edge, so a local rewritten in a loop body would be read
//! at the width of whichever write appears earlier in the text and packed wrongly
//! on every iteration after the first. Trusting it would mispack silently.
//!
//! # Two hypotheses were tested and refuted before this one
//!
//! **The `Boxed` composite form** — refuted earlier by measurement: the corpus
//! contains zero non-`Flat` composites.
//!
//! **The `Call` result** — refuted here. The instruction immediately before both
//! refused sites IS a `Call`, and both callees declare `ret = Scalar`, which made
//! it the obvious candidate. Seeding call results from `Module::signatures` was
//! implemented, and the refusal did not move: coverage stayed at 1070 of 1074.
//! **The adjacent instruction was not the producer of the offending operand.**
//!
//! # What would actually lift it
//!
//! A fixpoint over local widths rather than a linear scan. The increment's width
//! depends on the very local being analysed, so a single pass cannot settle it;
//! a monotone dataflow analysis can, since each local moves at most from
//! undefined to a concrete width to unknown. That is a separate increment with
//! real mispack risk, and it is not attempted here.

use keleusma::bytecode::{Module, Op};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma_native::{LowerOptions, module_refusals};

fn load(name: &str) -> Module {
    let src = std::fs::read_to_string(format!("../examples/scripts/{name}"))
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    compile(&parse(&tokenize(&src).expect("lex")).expect("parse")).expect("compile")
}

fn compile_src(src: &str) -> Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile")
}

/// How many times the chunk writes local `idx`.
fn writes_to(c: &keleusma::bytecode::Chunk, idx: u16) -> usize {
    c.ops
        .iter()
        .filter(|o| matches!(o, Op::SetLocal(n) if *n == idx))
        .count()
}

/// The refused operand is a local the chunk writes more than once.
///
/// **This is the finding, asserted rather than printed.** The refusal message
/// names which operand is unknown; this walks back to the instruction that
/// pushed it and shows that instruction is a `GetLocal` of a multiply-written
/// local — which is exactly the case the width rule declines to trust.
#[test]
fn the_unknown_operand_is_a_local_the_chunk_writes_more_than_once() {
    for (name, site) in [
        ("12_sensor_window.kel", 23usize),
        ("14_frame_log.kel", 24usize),
    ] {
        let m = load(name);

        let refusals = module_refusals(&m, LowerOptions::default());
        let text: Vec<String> = refusals.iter().map(|(s, e)| format!("{s}: {e}")).collect();
        println!("\n================ {name}\n  {text:?}");
        assert!(
            text.iter().any(|t| t.contains("unknown packed")),
            "{name} is no longer refused for an unknown operand width, so this \
             file's subject has changed: {text:?}"
        );
        // The refusal must name WHICH operand, or the walk below is guesswork.
        assert!(
            text.iter().any(|t| t.contains("operand 1 of 3")),
            "{name}: expected the FIRST of three operands to be the unknown one, \
             which is what the walk below assumes: {text:?}"
        );

        let c = m
            .chunks
            .iter()
            .find(|c| matches!(c.ops.get(site), Some(Op::NewComposite(_))))
            .unwrap_or_else(|| panic!("{name}: no composite site at op {site}"));

        // **A REAL STACK SIMULATION, NOT A WINDOW HEURISTIC.** The first
        // attempt took "the nearest preceding `GetLocal`" and picked the loop
        // CONDITION's read rather than the operand, then reported a write count
        // for the wrong local. The stack effects are published by the
        // instruction set itself, so the producer of each operand is derivable
        // rather than guessable.
        let mut producers: Vec<usize> = Vec::new();
        for (i, op) in c.ops[..site].iter().enumerate() {
            for _ in 0..op.stack_shrink() {
                producers.pop();
            }
            for _ in 0..op.stack_growth() {
                producers.push(i);
            }
        }
        let Op::NewComposite(_) = &c.ops[site] else {
            unreachable!("checked above")
        };
        assert!(
            producers.len() >= 3,
            "{name}: only {} operand(s) on the stack at the site, so the three \
             this site packs cannot all be accounted for",
            producers.len()
        );
        // Operand 1 of 3 is the DEEPEST of the three, so it is three from the top.
        let at = producers[producers.len() - 3];
        let first_local = match &c.ops[at] {
            Op::GetLocal(n) => *n,
            other => panic!(
                "{name}: operand 1 is produced at op {at} by {other:?}, not a local \
                 read. The recorded cause is about a LOCAL's width, so if this is \
                 not a GetLocal the explanation in this file is wrong."
            ),
        };
        println!("  operand 1 is produced at op {at} by GetLocal({first_local})");

        let n_writes = writes_to(c, first_local);
        println!(
            "  operand 1 comes from GetLocal({first_local}), written {n_writes} time(s) in the chunk"
        );
        assert!(
            n_writes > 1,
            "{name}: local {first_local} is written {n_writes} time(s). The recorded \
             cause is that the width rule declines a local written more than once; \
             if it is written at most once the cause is something else and this \
             file's explanation is wrong."
        );
    }
}

/// The rule's other side, so "written more than once" is shown to be the
/// discriminator rather than merely true of the failing cases.
///
/// A composite packing a local written EXACTLY once must lower. Without this the
/// test above would be satisfied by a backend that refused every composite.
#[test]
fn a_composite_packing_a_singly_written_local_is_not_refused() {
    let m = compile_src(
        "struct P { a: Word, b: Word }\n\
         fn main() -> Word {\n\
           let x = 7;\n\
           let p = P { a: x, b: 2 };\n\
           p.a + p.b\n\
         }",
    );
    let c = &m.chunks[0];
    let packed = c
        .ops
        .iter()
        .filter_map(|o| match o {
            Op::GetLocal(n) => Some(*n),
            _ => None,
        })
        .find(|n| writes_to(c, *n) == 1);
    assert!(
        packed.is_some(),
        "this subject must read a singly-written local, or it does not exercise \
         the other side of the rule"
    );
    assert!(
        c.ops.iter().any(|o| matches!(o, Op::NewComposite(_))),
        "this subject must construct a composite"
    );

    let refusals = module_refusals(&m, LowerOptions::default());
    let text: Vec<String> = refusals.iter().map(|(s, e)| format!("{s}: {e}")).collect();
    assert!(
        !text.iter().any(|t| t.contains("unknown packed")),
        "a composite packing a singly-written local was refused for an unknown \
         width, so the discriminator is not the write count after all: {text:?}"
    );
}

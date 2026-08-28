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
//! **The `Call` result** — refuted here AS THE CAUSE OF THESE TWO REFUSALS. The
//! instruction immediately before both sites IS a `Call`, and both callees
//! declare `ret = Scalar`, which made it the obvious candidate. Seeding call
//! results from `Module::signatures` was implemented and the refusal did not
//! move. **The adjacent instruction was not the producer of the offending
//! operand.**
//!
//! **That seeding is nonetheless a real fix for a real gap, and it now ships**
//! with `module_source_differential.rs` behind it: a composite packing a call
//! result was refused for an unknown width and now lowers and agrees with the
//! reference. It simply does not lift THESE two sites. Adjacency is not
//! provenance in either direction — the neighbouring instruction was neither the
//! cause here nor innocent everywhere.
//!
//! # What lifted it, and why no fixpoint was needed
//!
//! **"Cannot see a back edge" only matters when the writes DISAGREE.** If every
//! write to a local stores the same width, that is the width whichever write
//! reached the read. The two writes here are a `Const` and an arithmetic result
//! slot, and `push_triple` pushes that slot at a LITERAL word width regardless
//! of its operands — so neither write depends on the local being analysed and
//! **the circularity that would have forced a fixpoint does not exist**. An
//! earlier draft of this plan assumed one was required; reading the arm removed
//! the requirement.
//!
//! Both modules now lower, and both now execute and agree with the reference
//! under the corpus differential.

use keleusma::bytecode::{Module, Op};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::verify::op_depth_effect;
use keleusma_native::{LowerOptions, module_refusals};

fn load(name: &str) -> Module {
    let src = std::fs::read_to_string(format!("../examples/scripts/{name}"))
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    compile(&parse(&tokenize(&src).expect("lex")).expect("parse")).expect("compile")
}

fn compile_src(src: &str) -> Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile")
}

/// The producer of every operand-stack slot live just before `upto`.
///
/// Each entry is `(op index, which push of that instruction)`. The second half
/// matters: `CheckedAdd` pushes low, high and flag, and only the low slot is the
/// arithmetic result.
///
/// # ⚠ THIS USES `op_depth_effect`, NOT `stack_growth`/`stack_shrink`
///
/// **An earlier version of this walk used the latter and was WRONG.** Those two
/// are the operand-stack PEAK model — a transient reach and a net — and their
/// own documentation says so outright: *"Not a pop count... For true pop and
/// push counts use `op_depth_effect`."* Under the peak model `CheckedAdd`
/// reports growth 1 and shrink 0, when it in fact pops two and pushes three, so
/// a shadow stack driven by it desynchronises at every pop-and-push
/// instruction.
///
/// **The repository had already recorded this exact mistake** — `text_size` made
/// it and was fixed — and this line then made it again. The wrong walk
/// attributed a loop increment's stored value to a `GetLocal` rather than to the
/// arithmetic that produced it.
fn producer_stack_before(c: &keleusma::bytecode::Chunk, upto: usize) -> Vec<(usize, u32)> {
    let mut stack: Vec<(usize, u32)> = Vec::new();
    for (i, op) in c.ops[..upto].iter().enumerate() {
        let (required, delta) = op_depth_effect(op, c);
        let pops = required.max(0) as usize;
        let pushes = (required + delta).max(0) as usize;
        for _ in 0..pops {
            stack.pop();
        }
        for k in 0..pushes {
            stack.push((i, k as u32));
        }
    }
    stack
}

/// How many times the chunk writes local `idx`.
fn writes_to(c: &keleusma::bytecode::Chunk, idx: u16) -> usize {
    c.ops
        .iter()
        .filter(|o| matches!(o, Op::SetLocal(n) if *n == idx))
        .count()
}

/// Both modules now lower, and the local at the refused operand is still one the
/// chunk writes more than once — so it is the certification that admits them.
///
/// **This test FAILS without the certification**, which is what distinguishes it
/// from a test that passes for unrelated reasons. Before it, both were refused
/// with "operand 1 of 3 ... unknown packed width".
#[test]
fn the_two_modules_lower_and_their_operand_is_still_a_multiply_written_local() {
    for (name, site, local) in [
        ("12_sensor_window.kel", 23usize, 1u16),
        ("14_frame_log.kel", 24usize, 2u16),
    ] {
        let m = load(name);

        let refusals = module_refusals(&m, LowerOptions::default());
        let text: Vec<String> = refusals.iter().map(|(s, e)| format!("{s}: {e}")).collect();
        println!("\n================ {name}\n  refusals: {text:?}");
        assert!(
            !text.iter().any(|t| t.contains("unknown packed")),
            "{name} is still refused for an unknown operand width: {text:?}"
        );

        let c = m
            .chunks
            .iter()
            .find(|c| matches!(c.ops.get(site), Some(Op::NewComposite(_))))
            .unwrap_or_else(|| panic!("{name}: no composite site at op {site}"));
        let producers = producer_stack_before(c, site);
        let (at, _) = producers[producers.len() - 3];
        assert!(
            matches!(&c.ops[at], Op::GetLocal(n) if *n == local),
            "{name}: operand 1 is produced at op {at} by {:?}, not GetLocal({local}); \
             the explanation in this file is about that local",
            c.ops[at]
        );
        let n_writes = writes_to(c, local);
        assert!(
            n_writes > 1,
            "{name}: local {local} is written {n_writes} time(s). If it is written \
             at most once, the OLD rule already admitted it and the certification \
             is not what changed anything here."
        );
        println!("  operand 1 is GetLocal({local}), written {n_writes} times, and it lowers");
    }
}

/// **WHY THE REFUSING CASE IS NOT EXERCISED FROM SOURCE, recorded rather than
/// glossed over.**
///
/// The obvious negative subject — a loop variable bound from an array element,
/// whose width is a property of the operand rather than of the instruction — does
/// NOT exercise the certification at all. The compiler writes that binding
/// **once**, so the pre-existing "at most one write" rule already trusts it from
/// the width the tracker records for `GetIndex`. It lowered before this change
/// and it lowers now.
///
/// Every multiply-written local found in the shipped corpus, and in every source
/// form tried here, is a loop counter written from a constant and an arithmetic
/// result. **So no program this line can currently write reaches the refusing
/// path**, and it is unit-tested directly in `src/lib.rs` instead. That is a
/// fact about the compiler's output, not a reason to leave the path untested —
/// and asserting a refusal here would have been asserting something false.
#[test]
fn an_element_bound_loop_variable_is_written_once_and_was_never_the_blocked_case() {
    let m = compile_src(
        "struct P { a: Word, b: Word }\n\
         fn main() -> Word {\n\
           let xs = [4, 9];\n\
           for x in xs { let p = P { a: x, b: 1 }; }\n\
           0\n\
         }",
    );
    let c = &m.chunks[0];
    // The binding the loop introduces, as distinct from the hidden counter.
    let single_write_locals: Vec<u16> = (0..c.local_count)
        .filter(|n| writes_to(c, *n) == 1)
        .collect();
    let multi_write_locals: Vec<u16> = (0..c.local_count)
        .filter(|n| writes_to(c, *n) > 1)
        .collect();
    println!(
        "\n================ element-bound loop\n  written once: {single_write_locals:?}  \
         written more than once: {multi_write_locals:?}"
    );
    assert!(
        !single_write_locals.is_empty(),
        "the element binding must be written exactly once, which is what makes \
         this NOT a case the certification decides"
    );
    let text: Vec<String> = module_refusals(&m, LowerOptions::default())
        .iter()
        .map(|(s, e)| format!("{s}: {e}"))
        .collect();
    assert!(
        !text.iter().any(|t| t.contains("unknown packed")),
        "this program was never blocked on an unknown width: {text:?}"
    );
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

/// The instructions that WRITE the multiply-written local, which is what any
/// attempt to certify its width has to classify.
///
/// **Re-derived with the corrected walk.** The peak-model version reported the
/// increment's stored value as coming from a `GetLocal`; it comes from the
/// arithmetic, whose result slot carries a width fixed by the instruction rather
/// than by its operands. That difference is the whole basis for certifying the
/// local, so getting it wrong would have sunk the certification or, worse,
/// justified it for the wrong reason.
#[test]
fn what_writes_the_multiply_written_local() {
    for (name, local) in [("12_sensor_window.kel", 1u16), ("14_frame_log.kel", 2u16)] {
        let m = load(name);
        let c = m
            .chunks
            .iter()
            .find(|c| writes_to(c, local) > 1)
            .unwrap_or_else(|| panic!("{name}: no chunk writes local {local} twice"));
        println!("\n================ {name}: writes to local {local}");
        let mut kinds: Vec<String> = Vec::new();
        for (i, op) in c.ops.iter().enumerate() {
            if !matches!(op, Op::SetLocal(n) if *n == local) {
                continue;
            }
            let st = producer_stack_before(c, i);
            match st.last() {
                Some(&(pi, k)) => {
                    println!("    op {i:>3} <- push {k} of op {pi} {:?}", c.ops[pi]);
                    kinds.push(format!("{:?}#{k}", c.ops[pi]));
                }
                None => panic!("{name}: SetLocal at op {i} with an empty operand stack"),
            }
        }
        assert_eq!(
            kinds.len(),
            2,
            "{name}: expected exactly two writes to local {local}, found {kinds:?}"
        );
        // The point of the classification: one write is a constant and the other
        // is an arithmetic RESULT slot, and both of those carry a width that is a
        // property of the instruction rather than of its operands.
        assert!(
            kinds.iter().any(|k| k.starts_with("Const")),
            "{name}: expected one write from a constant, got {kinds:?}"
        );
        assert!(
            kinds
                .iter()
                .any(|k| k.starts_with("Checked") && k.ends_with("#0")),
            "{name}: expected one write from an arithmetic result slot (push 0 of a \
             Checked op), got {kinds:?}. If the increment's value comes from \
             somewhere else, certifying this local's width needs a different \
             argument."
        );
    }
}

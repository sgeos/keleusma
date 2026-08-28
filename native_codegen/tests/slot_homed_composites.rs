//! Is any composite in the corpus slot-homed?
//!
//! # The half-corrected sentence this finishes
//!
//! `region.rs` carried *"0 of 239 construction sites are slot-homed and 239 are
//! temporaries"*. The previous increment established that **239 had no producer**
//! and replaced it with a measured **256 over the four-root corpus's 69
//! modules** — and **deliberately did not restate the other half**, because
//! correcting a denominator does not license the numerator.
//!
//! **A sentence with one verified half and one carried half is harder to use
//! than one that is wholly stale**, because the verified half lends the rest
//! credibility. So the numerator is measured here.
//!
//! # It is not only bookkeeping
//!
//! `region.rs` places composite bodies at fixed offsets from a CHUNK's region, on
//! the premise that they are temporaries rather than program state. **A
//! slot-homed composite would not fit that model** — it outlives the chunk and
//! survives `Reset`.
//!
//! # Two methods, one population
//!
//! A cross-check is **different methods over the same population**; the same
//! method over different populations is not one, as this line published and had
//! to correct. The two here share no evidence:
//!
//! 1. **A producer walk** over the instruction stream: a `NewComposite` whose
//!    value is consumed by `SetData`.
//! 2. **The module's own descriptors**: `private_composite_layout` and
//!    `persistent_composite_bytes`, which the compiler fills when it bakes a slot.

use keleusma::bytecode::{Module, NewCompositeOperand, Op, SlotVisibility};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::verify::op_depth_effect;
use keleusma_native::region;

const CORPUS_DIRS: [&str; 4] = [
    "../examples/scripts",
    "../src/selfhost/kel",
    "../examples/rtos/scripts",
    "../compiler/kel",
];

/// A program that DOES bake a composite into a private slot.
///
/// Borrowed from `spike_composite_shape.rs`, whose own control it is. **Without
/// it, "no slot-homed composites in the corpus" would be indistinguishable from
/// "this walk cannot see one".**
const SLOT_HOMED_CONTROL: &str = "\
struct P { x: Word, y: Word }
private data st { p: P }
fn build(a: Word) -> Word {
    st.p = P { x: a, y: a + 1 };
    st.p.y
}
loop main(r: Word) -> Word { yield build(r) }
";

fn compile_src(src: &str) -> Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile")
}

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

/// METHOD 1 — sites whose value is stored into a data slot, by visibility.
///
/// **Driven by `op_depth_effect`**, whose contract is true pop and push counts.
/// `stack_growth`/`stack_shrink` are the operand-stack PEAK model and their own
/// documentation says they are not pop counts; a shadow stack built on them
/// mis-attributed a stored value earlier in this session.
fn sites_stored_into_slots(m: &Module) -> (usize, usize) {
    let (mut private, mut shared) = (0usize, 0usize);
    for c in &m.chunks {
        let mut stack: Vec<usize> = Vec::new();
        for (i, op) in c.ops.iter().enumerate() {
            if let Op::SetData(slot) = op
                && let Some(&producer) = stack.last()
                && matches!(
                    c.ops.get(producer),
                    Some(Op::NewComposite(NewCompositeOperand::Flat { .. }))
                )
            {
                let vis = m
                    .data_layout
                    .as_ref()
                    .and_then(|dl| dl.slots.get(*slot as usize))
                    .map(|s| s.visibility);
                match vis {
                    Some(SlotVisibility::Private) => private += 1,
                    _ => shared += 1,
                }
            }
            let (required, delta) = op_depth_effect(op, c);
            for _ in 0..required.max(0) {
                stack.pop();
            }
            for _ in 0..(required + delta).max(0) {
                stack.push(i);
            }
        }
    }
    (private, shared)
}

/// METHOD 2 — what the module's own descriptors say.
fn descriptors_say_slot_homed(m: &Module) -> bool {
    let baked = m
        .data_layout
        .as_ref()
        .is_some_and(|dl| !dl.private_composite_layout.is_empty());
    baked || m.persistent_composite_bytes > 0
}

/// **THE MUST-FIRE CONTROL.** Both methods must report the control program as
/// slot-homed, or a zero over the corpus says nothing.
#[test]
fn both_methods_see_a_composite_that_really_is_slot_homed() {
    let m = compile_src(SLOT_HOMED_CONTROL);
    let (private, shared) = sites_stored_into_slots(&m);
    let descriptors = descriptors_say_slot_homed(&m);
    println!("\n================ CONTROL: A BAKED PRIVATE COMPOSITE SLOT");
    println!("  producer walk : {private} private, {shared} shared");
    println!("  descriptors   : slot-homed = {descriptors}");
    println!(
        "  persistent_composite_bytes = {}",
        m.persistent_composite_bytes
    );
    println!("================\n");
    assert!(
        private > 0,
        "the producer walk cannot see a composite stored into a private slot, so \
         a zero over the corpus would be meaningless"
    );
    assert!(
        descriptors,
        "the module descriptors do not report the control as slot-homed, so they \
         cannot report the corpus either"
    );
}

/// The corpus, by both methods.
///
/// **THE EXPECTATION WRITTEN BEFORE MEASURING WAS ZERO, AND IT WAS WRONG.** One
/// corpus composite reaches a private slot, and both methods name the same
/// module. The prediction was cheap only because it was recorded first.
#[test]
fn exactly_one_corpus_composite_reaches_a_private_slot() {
    let corpus = corpus();
    assert!(
        corpus.len() > 50,
        "only {} modules compiled; a zero over a corpus that failed to load would \
         be true for the wrong reason",
        corpus.len()
    );

    let (mut priv_total, mut shared_total) = (0usize, 0usize);
    let mut by_descriptor: Vec<String> = Vec::new();
    let mut by_walk: Vec<String> = Vec::new();
    for (name, m) in &corpus {
        let (p, s) = sites_stored_into_slots(m);
        if p > 0 {
            by_walk.push(format!("{name} ({p} private)"));
        }
        priv_total += p;
        shared_total += s;
        if descriptors_say_slot_homed(m) {
            by_descriptor.push(name.clone());
        }
    }

    println!("\n================ SLOT-HOMED COMPOSITES, FOUR-ROOT CORPUS");
    println!("  modules                              : {}", corpus.len());
    println!("  method 1, producer walk, PRIVATE slots: {priv_total} {by_walk:?}");
    println!("  method 1, producer walk, SHARED slots : {shared_total}");
    println!(
        "  method 2, module descriptors          : {} {by_descriptor:?}",
        by_descriptor.len()
    );
    println!(
        "\n  PRIVATE AND SHARED ARE NOT THE SAME CLAIM. The sentence being checked\n  \
         is about private, arena-resident slots that survive `Reset`; a composite\n  \
         written to a host-visible slot is a different fact and is counted apart.\n================\n"
    );

    // **THE METHODS MUST AGREE, and a disagreement would be the finding rather
    // than a tie to break.** They do: one site, one module, the same module.
    assert_eq!(
        (priv_total, by_descriptor.len()),
        (1, 1),
        "walk says {priv_total} private site(s) {by_walk:?}, descriptors say {} \
         module(s) {by_descriptor:?}. If these ever differ, report both rather \
         than preferring one.",
        by_descriptor.len()
    );
    assert!(
        by_walk.iter().any(|w| w.starts_with("14_frame_log.kel"))
            && by_descriptor.iter().any(|d| d == "14_frame_log.kel"),
        "both methods named `14_frame_log.kel` when this was written; they now \
         name {by_walk:?} and {by_descriptor:?}"
    );
    assert_eq!(
        shared_total, 0,
        "a composite now reaches a HOST-VISIBLE slot, which is a different claim \
         from the private one this file is about and needs its own reasoning"
    );
}

/// **DOES THE PLANNER ALSO PLACE THE SITE THAT REACHES A SLOT?**
///
/// This is what decides whether the finding disturbs `region.rs`'s model. That
/// pass places every `Flat` construction at a fixed offset from the CHUNK's
/// region, on the premise that composites are temporaries. If the site whose
/// value reaches a private slot is ALSO given a chunk-region placement, then the
/// construction is a temporary that is subsequently COPIED into the slot, and the
/// model holds. If it were not placed, the body would live only in the slot and a
/// chunk-relative placement would be the wrong model for it.
#[test]
fn the_slot_reaching_site_is_still_placed_as_a_temporary() {
    let m = corpus()
        .into_iter()
        .find(|(n, _)| n == "14_frame_log.kel")
        .map(|(_, m)| m)
        .expect("the module the two methods named");

    let mut found = false;
    for c in &m.chunks {
        let placed: Vec<usize> = region::plan_chunk_region(c)
            .sites
            .iter()
            .map(|s| s.op_index)
            .collect();
        let mut stack: Vec<usize> = Vec::new();
        for (i, op) in c.ops.iter().enumerate() {
            if let Op::SetData(_) = op
                && let Some(&producer) = stack.last()
                && matches!(
                    c.ops.get(producer),
                    Some(Op::NewComposite(NewCompositeOperand::Flat { .. }))
                )
            {
                found = true;
                println!("\n================ THE SLOT-REACHING SITE");
                println!(
                    "  chunk {} op {producer} stores into a slot at op {i}",
                    c.name
                );
                println!(
                    "  the planner places op {producer}: {}",
                    placed.contains(&producer)
                );
                println!(
                    "\n  PLACED means the construction is a TEMPORARY that is then copied\n  \
                     into the slot, which is exactly what `region.rs` assumes. It does\n  \
                     NOT mean the body lives in the slot.\n================\n"
                );
                assert!(
                    placed.contains(&producer),
                    "the site reaching a private slot is NOT placed in the chunk region, \
                     so its body does not live where `region.rs` assumes and that pass's \
                     premise needs revisiting"
                );
            }
            let (required, delta) = op_depth_effect(op, c);
            for _ in 0..required.max(0) {
                stack.pop();
            }
            for _ in 0..(required + delta).max(0) {
                stack.push(i);
            }
        }
    }
    assert!(
        found,
        "no slot-reaching construction found in the module the other tests name, \
         so this measures nothing"
    );
}

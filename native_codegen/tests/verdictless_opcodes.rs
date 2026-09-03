//! **VERDICTS FOR THE OPCODES THAT HAVE NONE.**
//!
//! Two censuses run in this package and neither can classify every opcode. The
//! ISA lowering census reads the corpus and reports `Reset` as UNPROVEN, because
//! the corpus emits it only inside chunks that refuse on something else. The
//! backend support census drives hand-built probes and reports `Reset` as NEVER
//! VISITED, because its probe emits the opcode in a position the lowering steps
//! over.
//!
//! **An absent verdict is not a negative verdict.** A reader who sees "63 of 66"
//! will infer that three opcodes are unsupported, and that inference is wrong
//! here. This file supplies the missing verdict by driving the backend rather
//! than by reading it.
//!
//! # Why `Reset` never reaches opcode dispatch
//!
//! The backend recognises a degenerate stream by SHAPE, not by walking its
//! opcodes: `Stream ; <body> ; Yield ; PopN(1) ; Reset`, in which `Stream` and
//! `Reset` lower to nothing. So a module can contain `Reset`, lower cleanly, and
//! still never mark that instruction as visited. The census instruments the
//! dispatch, so it sees an absence.
//!
//! **That is a property of where the instrument sits, not of the backend's
//! support.** These tests state it as a measured pair rather than as one word.
//!
//! # What a pass here does NOT mean
//!
//! That the emitted code is correct. `lower_module` returning success is a fact
//! about the compiler, not about the program. Correctness is the differential's
//! job.
use keleusma::bytecode::Module;
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerOptions, module_lowered_op_indices, module_refusals};
use std::path::Path;

mod common;

/// The RTOS scripts need their prelude prepended, exactly as the ISA census does.
fn source_for(p: &Path) -> Option<String> {
    let src = std::fs::read_to_string(p).ok()?;
    let is_rtos = p.components().any(|c| c.as_os_str() == "rtos");
    let is_prelude = p.file_name().is_some_and(|n| n == "prelude.kel");
    if is_rtos && !is_prelude {
        let prelude = std::fs::read_to_string("../examples/rtos/scripts/prelude.kel").ok()?;
        return Some(format!("{prelude}\n{src}"));
    }
    Some(src)
}

fn compile_path(p: &Path) -> Option<Module> {
    let src = source_for(p)?;
    let toks = tokenize(&src).ok()?;
    let ast = parse(&toks).ok()?;
    compile(&ast).ok()
}

/// Is `opcode` present in the module's instruction stream at all?
fn emits(m: &Module, opcode: &str) -> bool {
    m.chunks
        .iter()
        .any(|c| c.ops.iter().any(|o| format!("{o:?}").starts_with(opcode)))
}

/// Does the lowering actually REACH `opcode`? Distinct from `emits`: an opcode
/// consumed by a shape match is emitted and never visited.
fn visits(m: &Module, opcode: &str) -> bool {
    let (_, seen_per_chunk) = module_lowered_op_indices(m, LowerOptions::default());
    m.chunks.iter().enumerate().any(|(ci, c)| {
        let Some(seen) = seen_per_chunk.get(ci).and_then(|v| v.as_ref()) else {
            return false;
        };
        c.ops
            .iter()
            .enumerate()
            .any(|(i, o)| format!("{o:?}").starts_with(opcode) && seen.contains(&i))
    })
}

/// Every corpus module that emits `opcode`, partitioned by what the backend did.
struct Partition {
    accepted: Vec<String>,
    refused: Vec<(String, String)>,
    visited_anywhere: bool,
}

fn partition_corpus_on(opcode: &str) -> Partition {
    let mut out = Partition {
        accepted: Vec::new(),
        refused: Vec::new(),
        visited_anywhere: false,
    };
    for p in common::corpus_sources() {
        let Some(m) = compile_path(&p) else { continue };
        if !emits(&m, opcode) {
            continue;
        }
        let name = p.display().to_string();
        if visits(&m, opcode) {
            out.visited_anywhere = true;
        }
        let refusals = module_refusals(&m, LowerOptions::default());
        match refusals.first() {
            None => out.accepted.push(name),
            Some((chunk, err)) => out.refused.push((name, format!("{chunk}: {err:?}"))),
        }
    }
    out
}

#[test]
fn reset_is_accepted_in_modules_that_lower_yet_never_reaches_opcode_dispatch() {
    let part = partition_corpus_on("Reset");

    // REACH FIRST. If no corpus module emits Reset, everything below is vacuous
    // and would pass while measuring nothing.
    let total = part.accepted.len() + part.refused.len();
    assert!(
        total > 0,
        "no corpus module emits Reset, so this test proves nothing about it"
    );

    // THE VERDICT THE CENSUSES COULD NOT GIVE. Reset does not block lowering.
    assert!(
        !part.accepted.is_empty(),
        "Reset is emitted by {total} corpus modules and EVERY ONE is refused; \
         that would make it genuinely unsupported rather than merely unvisited. \
         Refusals: {:?}",
        part.refused
    );

    // AND WHY THE CENSUSES REPORT AN ABSENCE. It is consumed by the degenerate
    // stream shape match, which lowers it to nothing, so dispatch never sees it.
    assert!(
        !part.visited_anywhere,
        "Reset was VISITED by the lowering in some module. That contradicts the \
         recorded explanation that it is handled by shape recognition, and this \
         file's premise would need rewriting rather than the assertion relaxing."
    );

    println!("\n================ RESET, THE OPCODE WITH NO VERDICT");
    println!("  corpus modules emitting Reset : {total}");
    println!("  of those, the backend ACCEPTS : {}", part.accepted.len());
    println!("  of those, the backend REFUSES : {}", part.refused.len());
    println!(
        "  lowering ever VISITS Reset    : {}",
        part.visited_anywhere
    );
    println!(
        "\n  VERDICT: accepted in modules that lower, and never dispatched.\n  \
         The censuses report an absence because they instrument the opcode walk\n  \
         and this opcode is consumed by a SHAPE match that emits nothing for it.\n  \
         An absent verdict was not a negative verdict.\n\
         ================\n"
    );
}

#[test]
fn is_struct_has_no_corpus_witness_and_that_is_the_finding() {
    let part = partition_corpus_on("IsStruct");
    let total = part.accepted.len() + part.refused.len();

    // This test RECORDS a reachability fact. It is written to fail if the fact
    // changes, because a new witness would mean a verdict is newly available and
    // the census commentary would be stale.
    assert_eq!(
        total, 0,
        "IsStruct now has {total} corpus witnesses, so a verdict is available \
         that the census still reports as absent. Accepted: {:?}, refused: {:?}",
        part.accepted, part.refused
    );

    println!(
        "\n  IsStruct: 0 corpus witnesses. No verdict is available from this\n  \
         population, and none is claimed. This is a reachability finding, not\n  \
         a support finding.\n"
    );
}

/// **THE CONTROL THAT KEEPS THE `Reset` TEST FROM BEING VACUOUS.**
///
/// The interesting assertion above is that the lowering never VISITS `Reset`. If
/// `visits` were simply broken and always returned false, that assertion would
/// pass while measuring nothing, and the file would read as evidence while being
/// evidence of nothing. This is not hypothetical: earlier in this line's history
/// a reach test asserted a classifier's verdict and stayed green under a mutation
/// that mis-classified its input.
///
/// So prove the instrument responds: an opcode the backend demonstrably dispatches
/// must come back visited.
#[test]
fn the_visit_instrument_reports_true_for_an_opcode_that_is_dispatched() {
    let mut visited_any = false;
    let mut emitted_any = false;
    for p in common::corpus_sources() {
        let Some(m) = compile_path(&p) else { continue };
        if emits(&m, "Add") {
            emitted_any = true;
            if visits(&m, "Add") {
                visited_any = true;
                break;
            }
        }
    }
    assert!(
        emitted_any,
        "no corpus module emits Add; the control cannot run"
    );
    assert!(
        visited_any,
        "the visit instrument never reported true for Add, an opcode the backend \
         lowers. It is therefore incapable of reporting true at all, and the \
         'Reset is never visited' result above measures nothing."
    );
}

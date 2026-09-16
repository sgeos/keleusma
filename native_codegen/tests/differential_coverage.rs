//! **LOWERED IS NOT EXECUTED, AND THAT IS WHERE A DEFECT HIDES.**
//!
//! # The gap this closes
//!
//! Every other coverage instrument here measures LOWERING.
//! `isa_coverage_census` asks which opcodes nothing has ever lowered;
//! `spike_corpus_coverage` reports opcode instances lowering;
//! `opcode_denominator` classifies all 66 and says in its own header that a
//! `Lowered` verdict *"does not mean the emitted code is correct"*.
//!
//! **None asks which opcodes are lowered but never differentially EXECUTED.**
//! That is exactly where `Fixed % Fixed` sat: it lowered without refusal, inside
//! an ordinary module, and was wrong only once something ran both
//! implementations and compared them.
//!
//! # The measurement
//!
//! The corpus differential classifies its modules as **executed and agreeing**,
//! **agreed but vacuous**, or **exempt**. Intersecting that with per-module
//! opcode sets gives the opcodes for which the corpus supplies no executed
//! witness.
//!
//! **Four do: `BitAnd`, `BitOr`, `BitXor`, `Shr`** — all only in `wire.kel`,
//! which the virtual machine refuses to resume, so it is compared by the FAULT
//! observable rather than by a result.
//!
//! # ⚠ "COVERED ELSEWHERE" IS CHECKED HERE, NOT ASSERTED
//!
//! Those four ARE driven, by `scalar_operator_matrix.rs`. **A claim of coverage
//! elsewhere is precisely what this session has repeatedly found to be false**, so
//! it is not asserted here — **the witnesses are DRIVEN in this file**, virtual
//! machine against native, and the agreement is the evidence.
//!
//! An earlier attempt checked instead that the other file *contained* the witness
//! text. It failed: the matrix GENERATES its expressions from an operator list
//! rather than spelling them, so the check was coupling to a formatting accident.
//! Driving them here is stronger and depends on nothing but this file.
//!
//! It also revises what that matrix is for. Built to find type-dispatch defects,
//! it turns out to be the **sole** result-comparing evidence for four opcodes,
//! which raises the cost of ever trimming it.
//!
//! # What this does NOT claim
//!
//! **That an executed module exercises every opcode it contains.** It does not; a
//! branch may never be taken. The claim is the weaker, checkable one: an opcode
//! appearing only in never-executed modules has no result-comparing evidence from
//! the corpus at all.
//!
//! **That `fault-compared` means no evidence.** A fault IS an observable and the
//! differential compares it. The four orphans lack RESULT comparison, not all
//! comparison.

use std::collections::{BTreeMap, BTreeSet};

mod common;

/// Modules the corpus differential does not execute, transcribed from its own
/// report, with the class it assigns.
///
/// **Not a reimplementation of its exemption logic.** A private copy of a
/// canonical walk already cost this package once, reporting five rtos scripts as
/// compiler failures when a missing prelude was the cause. The counts below are
/// asserted against the differential's own totals, which is what licenses this
/// list.
const NOT_EXECUTED: &[(&str, &str)] = &[
    ("11_signed.kel", "the VM would not load it"),
    ("13_telemetry_stream.kel", "the backend refuses it"),
    ("faulty.kel", "faults; compared by the FAULT observable"),
    ("prelude.kel", "no runnable entry point"),
    (
        "rogue_ai_boss.kel",
        "composite entry; hand-written differential",
    ),
    (
        "rogue_ai_hunter.kel",
        "composite entry; hand-written differential",
    ),
    (
        "rogue_ai_tracker.kel",
        "composite entry; hand-written differential",
    ),
    (
        "rogue_dungen.kel",
        "faults; compared by the FAULT observable",
    ),
    ("verify_datalayout.kel", "agreed but vacuous"),
    ("wire.kel", "faults; compared by the FAULT observable"),
];

/// What the differential reports. A change here means the classification moved
/// and this file's population is stale.
const EXECUTED_MODULES: usize = 63;
const NOT_EXECUTED_MODULES: usize = 11;

/// `(opcode, witness source, file that drives it)` for every opcode the corpus
/// gives no executed witness.
///
/// Each row is verified three ways below. A row is not a claim; it is a chain.
const WITNESSES_ELSEWHERE: &[(&str, &str, &str)] = &[
    (
        "BitAnd",
        "fn main(a: Word, b: Word) -> Word { a band b }",
        "scalar_operator_matrix.rs",
    ),
    (
        "BitOr",
        "fn main(a: Word, b: Word) -> Word { a bor b }",
        "scalar_operator_matrix.rs",
    ),
    (
        "BitXor",
        "fn main(a: Word, b: Word) -> Word { a bxor b }",
        "scalar_operator_matrix.rs",
    ),
    (
        "Shr",
        "fn main(a: Word, b: Word) -> Word { a asr b }",
        "scalar_operator_matrix.rs",
    ),
];

fn op_names(m: &keleusma::bytecode::Module) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for c in &m.chunks {
        for op in &c.ops {
            let s = format!("{op:?}");
            out.insert(s.split(['(', ' ', '{']).next().unwrap_or(&s).to_string());
        }
    }
    out
}

/// Opcodes witnessed inside an executed module, and those only outside one.
fn partition() -> (
    BTreeSet<String>,
    BTreeMap<String, Vec<String>>,
    usize,
    usize,
) {
    let mut executed_witness = BTreeSet::new();
    let mut elsewhere: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let (mut n_exec, mut n_not) = (0, 0);
    for p in common::corpus_sources() {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let Ok(src) = std::fs::read_to_string(&p) else {
            continue;
        };
        let is_rtos = p.components().any(|c| c.as_os_str() == "rtos");
        let is_prelude = name == "prelude.kel";
        let src = if is_rtos && !is_prelude {
            match std::fs::read_to_string("../examples/rtos/scripts/prelude.kel") {
                Ok(pr) => format!("{pr}\n{src}"),
                Err(_) => src,
            }
        } else {
            src
        };
        let Some(m) = common::try_build(&src) else {
            continue;
        };
        let skipped = NOT_EXECUTED.iter().find(|(n, _)| *n == name);
        if skipped.is_some() {
            n_not += 1;
        } else {
            n_exec += 1;
        }
        for op in op_names(&m) {
            match skipped {
                None => {
                    executed_witness.insert(op);
                }
                Some((_, class)) => elsewhere
                    .entry(op)
                    .or_default()
                    .push(format!("{name} ({class})")),
            }
        }
    }
    (executed_witness, elsewhere, n_exec, n_not)
}

#[test]
fn the_opcodes_without_a_corpus_witness_are_exactly_the_recorded_ones() {
    let (witnessed, elsewhere, n_exec, n_not) = partition();

    assert_eq!(
        (n_exec, n_not),
        (EXECUTED_MODULES, NOT_EXECUTED_MODULES),
        "the executed/not-executed split is now ({n_exec}, {n_not}), not \
         ({EXECUTED_MODULES}, {NOT_EXECUTED_MODULES}). This file's list is \
         transcribed from the differential's own report; when that report moves, \
         re-transcribe it rather than adjusting these numbers."
    );
    assert!(
        witnessed.len() >= 50,
        "only {} opcodes have an executed witness; a broken probe rather than a \
         shrunken corpus",
        witnessed.len()
    );

    let orphans: BTreeSet<&str> = elsewhere
        .keys()
        .filter(|k| !witnessed.contains(*k))
        .map(String::as_str)
        .collect();
    let recorded: BTreeSet<&str> = WITNESSES_ELSEWHERE.iter().map(|(o, _, _)| *o).collect();

    let appeared: Vec<&&str> = orphans.difference(&recorded).collect();
    let vanished: Vec<&&str> = recorded.difference(&orphans).collect();

    assert!(
        appeared.is_empty(),
        "{} opcode(s) newly have NO executed witness in the corpus: {appeared:?}. \
         Either the corpus lost the module that witnessed them, or an opcode \
         arrived. Say which, and record where its differential evidence now comes \
         from — an opcode with none has no result comparison behind it at all.",
        appeared.len()
    );
    assert!(
        vanished.is_empty(),
        "{} opcode(s) now HAVE an executed corpus witness and no longer need an \
         external one: {vanished:?}. Good — delete their rows, so the record does \
         not keep pointing elsewhere for evidence the corpus now supplies.",
        vanished.len()
    );
}

/// **The chain, and the last link is EXECUTION rather than a citation.**
#[test]
fn every_external_witness_emits_its_opcode_and_agrees_with_the_reference() {
    assert!(
        !WITNESSES_ELSEWHERE.is_empty(),
        "no external witnesses recorded; if every opcode gained a corpus witness \
         this test should be deleted rather than left passing vacuously"
    );
    for (opcode, src, driver) in WITNESSES_ELSEWHERE {
        // Link 1: it compiles.
        let m = common::try_build(src)
            .unwrap_or_else(|| panic!("the witness for `{opcode}` no longer compiles: {src}"));
        // Link 2: it emits the opcode it is claimed to witness.
        assert!(
            op_names(&m).contains(*opcode),
            "the witness for `{opcode}` compiles but no longer emits it: {src}. \
             The claim that this opcode has evidence rests on this."
        );
        // Link 3: **the two implementations agree on it.** Driven here, so the
        // evidence does not depend on another file's shape. The recorded driver
        // covers the same ground across the whole type-by-operator space; this is
        // the standing witness for an opcode the CORPUS cannot supply.
        //
        // ⚠ **THE DRIVER NAME IS REPORTED, NOT MERELY RECORDED.** It sat in a
        // COMMENT as `{driver}`, which reads like an interpolation and is not
        // one, leaving the binding unused — a `-D warnings` failure that stood in
        // the tree across four commits while the state table called the backend
        // green. A field carried in a table and never read is a claim nothing
        // checks; naming it in the failure is what makes it load-bearing.
        let (vm, native) =
            common::vm_and_native_two_arg(src, 0x0F0F_0F0F_0F0F_0F0F, 0x00FF_00FF_00FF_00FF);
        assert_eq!(
            vm, native,
            "`{opcode}` DIVERGES: the reference gives {vm}, native gives {native}. \
             This opcode has no result-comparing witness in the corpus, so this \
             test is the only thing that would have caught it. `{driver}` covers \
             the same ground more broadly and is where to look next."
        );
    }
}

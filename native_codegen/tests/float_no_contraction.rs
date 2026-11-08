//! **DOES THE BACKEND EMIT THE FIVE OPERATIONS THE EQUIVALENCE ARGUMENT COVERS,
//! AND NOTHING ELSE?**
//!
//! The differential oracle compares this backend against the reference virtual
//! machine, and the two do not compute the same way. The backend lowers float
//! arithmetic **natively at the declared width**, because LLVM has the type. The
//! reference computes in a **wider** type and narrows.
//!
//! # Why they may be compared at all
//!
//! Widen-compute-narrow agrees with native arithmetic **only** under the
//! innocuous-double-rounding condition: the intermediate must carry at least
//! `2p + 2` significand bits for a target precision `p`. Computing in `binary64`
//! and rounding once per operation to `binary32` needs 50 and has 53, so it is
//! **equivalent** to native `binary32` arithmetic, with a margin of 3.
//!
//! **That equivalence covers exactly five operations**: addition, subtraction,
//! multiplication, division and square root. It does **not** cover a fused
//! multiply-add, whose whole purpose is to round ONCE where the theorem assumes
//! TWICE, and it does not cover transcendentals.
//!
//! # So this file pins the precondition rather than the conclusion
//!
//! If the backend ever fuses a multiply and an add where the reference performs
//! two rounded operations, **the differential stops being a proof and becomes a
//! coincidence** — and it would stay green on most inputs, which is the worst
//! shape a defect can take here.
//!
//! **The agreement itself is measured elsewhere**, by `float_differential.rs`
//! and `entry_abi_float.rs`. Establishing that the equivalence argument APPLIES
//! is what happens here.
//!
//! # ⚠ THE OBVIOUS INSTRUMENT IS THE WRONG ONE, AND IT READS GREEN
//!
//! The first version of this file looked for `@llvm.fma` in the emitted IR and
//! for fast-math flags after an O2 pipeline. **It passed, and it was measuring
//! nothing.** Fusion is not an IR transform. `default<O2>` leaves `fmul` and
//! `fadd` exactly as they are even when both carry `contract`; the fusion
//! happens in CODEGEN, and the first place it is visible is the machine
//! instruction. An IR-level search finds no FMA on a module that will certainly
//! fuse, which is a false negative that looks like a clean result.
//!
//! **The mutation is what exposed that.** Granting `contract` changed the IR and
//! produced no FMA, so either the pipeline does not fuse or the instrument
//! cannot see it. Only checking assembly distinguishes those.
//!
//! Raised by the `v0.2.3` line on 2026-09-01 while agreeing the acceptance
//! criterion for the arithmetic width. Recorded in
//! `docs/decisions/FLOAT_LADDER.md`.

mod common;

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::memory_buffer::MemoryBuffer;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerOptions, lower_chunk};
use std::hint::black_box;

/// The shape a compiler contracts. A multiply feeding an add is the ONLY pattern
/// that becomes an FMA, so a probe without it could not detect contraction
/// however permissive the flags were.
const MUL_THEN_ADD: &str = "\
fn main(w: Word) -> Word {
  let f = w as Float;
  let m = f * 2.5;
  let s = m + 1.5;
  s as Word
}
";

/// Fast-math flag spellings LLVM prints on a floating-point instruction.
/// `contract` is the one that licenses fusion; the others are listed because
/// `fast` implies all of them and a partial set is just as much a licence.
const FAST_MATH_FLAGS: [&str; 8] = [
    "fast", "nnan", "ninf", "nsz", "arcp", "contract", "afn", "reassoc",
];

/// Float instruction mnemonics whose flags matter.
const FLOAT_OPS: [&str; 6] = ["fmul", "fadd", "fsub", "fdiv", "frem", "fneg"];

/// Fused multiply-add mnemonics across the targets this may run on. `vfmadd132sd`
/// and friends contain `fmadd`, so the x86 family is covered by the same needle.
const FUSED_MNEMONICS: [&str; 4] = ["fmadd", "fmsub", "fmla", "fmls"];

fn lower_to_ir(src: &str) -> String {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_chunk(
        &ctx,
        &lm,
        &m.chunks[0],
        "kel_entry",
        LowerOptions::default(),
    )
    .expect("the float lowering must accept a multiply feeding an add");
    lm.verify().expect("LLVM module verification");
    lm.print_to_string().to_string()
}

fn host_machine() -> TargetMachine {
    Target::initialize_native(&InitializationConfig::default()).expect("init native target");
    let triple = TargetMachine::get_default_triple();
    Target::from_triple(&triple)
        .expect("target")
        .create_target_machine(
            &triple,
            "generic",
            "",
            OptimizationLevel::Default,
            RelocMode::PIC,
            CodeModel::Default,
        )
        .expect("target machine")
}

/// Takes IR TEXT through O2 and CODEGEN, and returns the assembly.
///
/// Working from text rather than from a live module is what lets the identical
/// path be applied to what the backend emits and to a mutated copy of it.
fn assemble(ir: &str) -> String {
    let ctx = Context::create();
    // The buffer must be NUL-terminated; inkwell asserts on it rather than
    // returning an error.
    let mut bytes = ir.as_bytes().to_vec();
    bytes.push(0);
    let buf = MemoryBuffer::create_from_memory_range(&bytes, "probe");
    let lm = ctx
        .create_module_from_ir(buf)
        .expect("the IR under test must parse");

    let machine = host_machine();
    lm.run_passes("default<O2>", &machine, PassBuilderOptions::create())
        .expect("O2 pipeline");
    lm.verify().expect("IR still valid AFTER the O2 pipeline");

    let asm = machine
        .write_to_memory_buffer(&lm, FileType::Assembly)
        .expect("assembly emission");
    String::from_utf8_lossy(asm.as_slice()).into_owned()
}

/// The mutation: grant the float instructions the `contract` flag and change
/// nothing else.
///
/// **This perturbs the LOWERING, not the probe.** A mutation that changed the
/// Keleusma source would prove nothing, because the guard is invariant to which
/// program is compiled — a distinction this line has recorded getting wrong.
fn licence_contraction(ir: &str) -> String {
    // **Anchor on `= `, because the backend NAMES its values after the
    // mnemonic.** A bare `"fmul "` also matches the `%fmul` on the left of the
    // assignment and yields `%fmul contract = ...`, which does not parse. The
    // first draft did exactly that and LLVM rejected it.
    ir.replace("= fmul ", "= fmul contract ")
        .replace("= fadd ", "= fadd contract ")
}

/// Lines carrying both a float mnemonic and a fast-math flag.
fn flagged_float_lines(ir: &str) -> Vec<String> {
    // **Compare WHOLE TOKENS, and do not try to slice a span.** The first draft
    // anchored on the first occurrence of the mnemonic and bounded the span on
    // `" float"`. Both were wrong: the backend names its values after the
    // mnemonic, so `%fmul` matched first, and the default float type prints as
    // `double`, so the bound never fired and the span ran to end of line. It
    // happened to give the right answer, which is the least useful way to be
    // wrong.
    //
    // A whole-token test cannot confuse a value named `%fast` with the flag
    // `fast`, because the token carries its sigil.
    ir.lines()
        .filter(|l| {
            let has_op = l.split_whitespace().any(|w| FLOAT_OPS.contains(&w));
            let has_flag = l.split_whitespace().any(|w| FAST_MATH_FLAGS.contains(&w));
            has_op && has_flag
        })
        .map(|l| l.trim().to_string())
        .collect()
}

fn fused_lines(asm: &str) -> Vec<String> {
    asm.lines()
        .filter(|l| FUSED_MNEMONICS.iter().any(|m| l.contains(m)))
        .map(|l| l.trim().to_string())
        .collect()
}

/// **NON-VACUITY, and it must come first.** Every assertion below is of the form
/// "the output does not contain X", and all of them hold trivially for an empty
/// module, a refused lowering, or a probe that produced no float arithmetic.
#[test]
fn the_probe_actually_emits_a_multiply_feeding_an_add() {
    let ir = lower_to_ir(MUL_THEN_ADD);
    assert!(
        ir.contains("fmul"),
        "the probe emitted no float multiply, so nothing below is testing \
         contraction:\n{ir}"
    );
    assert!(
        ir.contains("fadd"),
        "the probe emitted no float add, so nothing below is testing \
         contraction:\n{ir}"
    );

    let asm = assemble(&ir);
    assert!(
        asm.contains("kel_entry"),
        "no code was emitted for the entry symbol, so the assembly checks below \
         are vacuous:\n{asm}"
    );
}

/// The protection that actually operates: without the flag, no downstream stage
/// is permitted to fuse. Pinning the flag is stronger than pinning one probe's
/// output, because it holds for every shape the backend can emit.
#[test]
fn no_float_instruction_carries_a_fast_math_flag() {
    let ir = lower_to_ir(MUL_THEN_ADD);
    let flagged = flagged_float_lines(&ir);
    assert!(
        flagged.is_empty(),
        "{} float instruction(s) carry a fast-math flag. `contract` licenses \
         fusion into an FMA and `fast` implies it. The lines:\n  {}",
        flagged.len(),
        flagged.join("\n  ")
    );
}

/// The consequence, read at the level where fusion is actually visible.
#[test]
fn the_emitted_machine_code_contains_no_fused_multiply_add() {
    let asm = assemble(&lower_to_ir(MUL_THEN_ADD));
    let fused = fused_lines(&asm);
    assert!(
        fused.is_empty(),
        "the backend's machine code fuses a multiply and an add, which rounds \
         ONCE across two operations. The reference rounds twice, so the \
         differential would disagree only on inputs where the fused and unfused \
         results differ — and agree everywhere else, which is why this is pinned \
         rather than left to the oracle. The lines:\n  {}",
        fused.join("\n  ")
    );
}

/// **THE MUTATION, AND WITHOUT IT THE ABSENCE ABOVE MEANS NOTHING.**
///
/// Take the backend's own IR, add the `contract` flag, change nothing else, and
/// run the SAME path. If an FMA appears in the assembly, then this host does
/// fuse this shape and the flag is exactly what withholds it — which is what
/// makes the guard above a real one.
///
/// **A failure here is not a backend defect.** It says the guard is not
/// demonstrated on this host's LLVM and target, and the guard should then be
/// believed less rather than the backend suspected.
#[test]
fn licensing_contraction_does_produce_a_fused_multiply_add() {
    let clean = lower_to_ir(MUL_THEN_ADD);
    let licensed = licence_contraction(&clean);
    assert_ne!(
        clean, licensed,
        "the mutation changed nothing, so it did not perturb the lowering"
    );
    assert!(
        !flagged_float_lines(&licensed).is_empty(),
        "the mutation produced no flagged float instruction, so it did not \
         licence anything:\n{licensed}"
    );

    let fused = fused_lines(&assemble(&licensed));
    assert!(
        !fused.is_empty(),
        "granting `contract` produced no fused multiply-add on this host, so the \
         absence of one in the unmutated output is not evidence that the flag is \
         what withholds it. The guard in this file is UNDEMONSTRATED here — \
         treat it as weaker, and do not treat the backend as broken."
    );
}

/// **CONTROL, must-fire.** Without it, [`flagged_float_lines`] could return an
/// empty vector because its token logic is broken rather than because the IR is
/// clean, and the guard above would pass while measuring nothing.
#[test]
fn the_flag_detector_fires_on_a_flagged_instruction_and_not_a_clean_one() {
    let synthetic = "  %s = fmul contract double %m, %n\n  %t = fadd double %a, %b\n";
    let flagged = flagged_float_lines(synthetic);
    assert_eq!(
        flagged.len(),
        1,
        "the detector must find exactly the contracted line and not the clean \
         one; it found {flagged:?}"
    );
    assert!(flagged[0].contains("contract"));
}

/// **CONTROL, must-fire.** The same argument for the assembly reader.
#[test]
fn the_fusion_detector_fires_on_a_fused_instruction_and_not_a_clean_one() {
    let synthetic = "\tfmadd\td0, d1, d2, d3\n\tfmul\td0, d1, d2\n";
    let fused = fused_lines(synthetic);
    assert_eq!(
        fused.len(),
        1,
        "the detector must find exactly the fused line and not the plain \
         multiply; it found {fused:?}"
    );
}

// ── The OTHER half of the oracle ───────────────────────────────────────────────
//
// Everything above concerns the BACKEND, whose float arithmetic LLVM generates.
// The differential's other side is the reference virtual machine, which is
// ordinary Rust compiled by rustc and running in this same process. **If rustc
// ever contracts a multiply feeding an add inside the runtime's arithmetic, the
// reference stops rounding per operation** — and the oracle would then be
// comparing a fused reference against an unfused backend while every test at
// equal widths stayed green.
//
// Measured by the `v0.2.3` line on 2026-09-01 from generated code, at `-O` and
// at `-C opt-level=3 -C target-cpu=native`, on `aarch64-apple-darwin`: zero
// fused instructions for a plain `a * b + c` at either width, and one for an
// explicit `mul_add` as a must-fire control. **On aarch64 the fused
// multiply-add is baseline rather than a target feature**, so a null result
// cannot mean the instruction was unavailable — the compiler had it, used it
// where asked, and withheld it otherwise.
//
// **Assembly inspection established that fact and is a poor instrument for
// keeping it true.** The witnesses below pin it in an ordinary test. Both come
// from the `v0.2.3` line and both were re-derived here rather than taken on
// report; the second is **2 ulps** apart, where their message said one.

/// Operands where the fused and unfused results genuinely differ, with the
/// distance recorded so a mistyped constant is caught rather than silently
/// weakening the witness.
const CONTRACTION_WITNESSES: [(f64, f64, f64, i64); 2] = [
    (
        -3.523344703336752,
        -6.9830165215099615,
        3.0186894607970753,
        1,
    ),
    (
        -1.3270863267522834,
        -8.602891528507621,
        -8.185739733122698,
        2,
    ),
];

/// **The reference side of the oracle rounds after every operation.**
///
/// Fails the moment a toolchain starts contracting, which is exactly when the
/// differential would stop meaning what it claims.
#[test]
fn the_reference_toolchain_does_not_contract_a_multiply_feeding_an_add() {
    for (a, b, c, expected_ulps) in CONTRACTION_WITNESSES {
        // **The operands MUST go through a black box.** Without it the compiler
        // constant-folds the expression and this pins its constant evaluator
        // rather than its code generation. Constant folding does not contract
        // either, **so the test would still pass, and it would pass without
        // testing the thing**. Caution supplied by the `v0.2.3` line.
        let two_step = black_box(a) * black_box(b) + black_box(c);
        let fused = black_box(a).mul_add(black_box(b), black_box(c));

        assert_ne!(
            two_step, fused,
            "a two-step multiply-then-add agrees with an explicit fused \
             multiply-add at ({a}, {b}, {c}). Either the toolchain now contracts \
             — in which case the reference side of every float differential on \
             this line has stopped rounding per operation — or these operands no \
             longer distinguish the two."
        );

        // Non-vacuity in the other direction: a mistyped constant would still
        // satisfy the inequality above while testing nothing near the boundary.
        let ulps = (two_step.to_bits() as i64 - fused.to_bits() as i64).abs();
        assert_eq!(
            ulps, expected_ulps,
            "the witness at ({a}, {b}, {c}) is {ulps} ulps from the fused result \
             rather than {expected_ulps}; a constant has drifted and this is no \
             longer the witness that was verified"
        );
    }
}

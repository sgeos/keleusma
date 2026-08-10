// ---------------------------------------------------------------------------
// PREPARED WHILE ANOTHER SESSION'S GATE HELD THE MACHINE. Not yet compiled.
// Append to native_codegen/tests/differential.rs.
//
// WHY THIS EXISTS, and it is not tidiness.
//
// `V0_4_0_NATIVE_CODEGEN.md`'s Out of scope list says "JIT compilation. V0.4.0
// is AOT only." Every case in this file runs the JIT at `OptimizationLevel::None`
// — the configuration the architecture EXCLUDES from the deliverable — while the
// shipped shape is AOT at `default<O2>`, covered end to end by exactly one test
// in `aot_linkage.rs`.
//
// That gap is not hypothetical. The unwritten-local control in this file PASSED
// against the unfixed lowering: an uninitialised `alloca`, loaded immediately at
// O0, read zero and matched the expected value by accident. **At O2 LLVM does
// not leave `undef` alone — it propagates it and deletes branches on the
// assumption it may take any convenient value.** The same defect that was
// invisible at O0 can produce actively wrong control flow at O2. The coverage
// gap already concealed a real defect from the control written to catch it.
//
// TWO DIMENSIONS, DELIBERATELY SEPARATED. An earlier note in the inventory
// recommended "an AOT-and-O2 arm", which conflated them:
//
//   1. OPTIMISATION LEVEL — where undef/poison exploitation lives. Closed here,
//      cheaply, by running the middle end before executing. No linker per case.
//   2. DELIVERY SHAPE — platform calling convention, external symbol emission,
//      real linkage. Already covered by `aot_linkage.rs` for representative
//      programs, and expensive per case (link plus subprocess).
//
// Dimension 1 carries the soundness risk and costs almost nothing. Dimension 2
// carries integration risk and is adequately sampled. Closing 1 across the whole
// corpus and sampling 2 is the right split; running every case through a linker
// would buy little for a large cost.
// ---------------------------------------------------------------------------

/// Lower `src`, run the REAL optimisation pipeline over it, then JIT and call.
///
/// The distinction from [`native_result`] is one line — `run_passes` — and it is
/// the line that matters. `default<O2>` is the same pipeline `aot_linkage.rs`
/// uses to emit shipped objects, so a disagreement here is a disagreement the
/// deliverable would exhibit.
fn native_result_o2(src: &str, args: &[i64]) -> i64 {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    check_word_width(m.word_bits_log2).expect("word width");

    let ctx = Context::create();
    let lm = ctx.create_module("kel_o2");
    lower_chunk(
        &ctx,
        &lm,
        &m.chunks[0],
        "kel_entry",
        LowerOptions::default(),
    )
    .expect("lower");
    lm.verify().expect("LLVM module verification");

    // The middle end. Without this the test is just `native_result` again.
    Target::initialize_native(&InitializationConfig::default()).expect("init native target");
    let triple = TargetMachine::get_default_triple();
    let target = Target::from_triple(&triple).expect("target from triple");
    let machine = target
        .create_target_machine(
            &triple,
            // "generic"/"" matches `aot_linkage.rs` exactly. The host-CPU
            // accessors return `LLVMString` rather than `&str` and would not
            // compile as first drafted; more importantly, matching the shipped
            // emitter's settings is the point of this arm.
            "generic",
            "",
            OptimizationLevel::Default,
            RelocMode::PIC,
            CodeModel::Default,
        )
        .expect("target machine");
    lm.run_passes("default<O2>", &machine, PassBuilderOptions::create())
        .expect("O2 pipeline");

    // Re-verify AFTER optimisation. A pass that miscompiles malformed-but-
    // accepted IR shows up here rather than as a wrong answer, and this is the
    // only place in the suite that checks the post-optimisation module.
    lm.verify().expect("LLVM module verification after O2");

    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::Default)
        .expect("jit");
    match args.len() {
        1 => {
            let f = unsafe { ee.get_function::<unsafe extern "C" fn(i64) -> i64>("kel_entry") }
                .expect("symbol");
            unsafe { f.call(args[0]) }
        }
        2 => {
            let f =
                unsafe { ee.get_function::<unsafe extern "C" fn(i64, i64) -> i64>("kel_entry") }
                    .expect("symbol");
            unsafe { f.call(args[0], args[1])
            }
        }
        n => panic!("harness does not drive {n}-argument entry points"),
    }
}

/// The O2 arm of the differential oracle.
///
/// Deliberately reuses cases that already pass at O0. The question is not
/// whether the lowering is right — O0 answers that — but whether it SURVIVES the
/// pipeline that ships. A case passing at O0 and failing here is exactly the
/// class the architecture's AOT-only scope makes load-bearing.
///
/// Inputs distinguish branch paths, per this file's own rule: `maxi(2, 3)` takes
/// the else path and proved nothing about the then path, which is how the first
/// lowering defect survived.
#[test]
fn the_optimised_pipeline_agrees_with_the_vm() {
    let cases: &[(&str, &[i64])] = &[
        // Branch, both directions.
        ("fn main(a: Word, b: Word) -> Word { if a > b { a } else { b } }", &[9, 4]),
        ("fn main(a: Word, b: Word) -> Word { if a > b { a } else { b } }", &[2, 3]),
        // Checked multiply, whose triple O2 will fold hard.
        ("fn main(a: Word, b: Word) -> Word { a * b }", &[7, 6]),
        // Cross-function call, which O2 will inline.
        ("fn helper(x: Word) -> Word { x + 1 }\n\
          fn main(a: Word, b: Word) -> Word { helper(a) + b }", &[41, 1]),
        // Wrapping corner. Verified against `wrapping_addition_agrees_with_the_vm`
        // that `a + b` WRAPS rather than trapping here, so this case tests the
        // triple's low word rather than failing for an unrelated reason.
        ("fn main(a: Word, b: Word) -> Word { a + b }", &[i64::MAX, 1]),
        // Division, where the divisor substitution guard must not be optimised
        // away as unreachable.
        ("fn main(a: Word, b: Word) -> Word { a / b }", &[i64::MIN, -1]),
    ];

    for (src, args) in cases {
        let vm = vm_result(src, args);
        let o0 = native_result(src, args);
        let o2 = native_result_o2(src, args);
        assert_eq!(
            vm, o0,
            "O0 disagrees with the VM (pre-existing defect, not an O2 issue)\nsrc: {src}\nargs: {args:?}"
        );
        assert_eq!(
            vm, o2,
            "THE OPTIMISED PIPELINE DISAGREES WITH THE VM. This is the shipped \
             configuration; O0 agreeing is not sufficient.\nsrc: {src}\nargs: {args:?}"
        );
    }
}

/// MUST-FIRE evidence for the arm itself.
///
/// An arm that merely re-runs passing cases at a second optimisation level can
/// look like coverage while being unable to fail differently from the O0 arm.
/// This pins the one property that is genuinely O2-only: the module must still
/// verify AFTER the middle end has run.
///
/// It fires if a pass ever produces IR that LLVM's own verifier rejects, which
/// no O0 test can observe because no O0 test runs a pass.
#[test]
fn the_module_still_verifies_after_the_optimisation_pipeline() {
    // Exercises locals, a branch, a call and the checked triple together, so the
    // post-pipeline module is not trivially small.
    let src = "fn helper(x: Word) -> Word { x + 1 }\n\
               fn main(a: Word, b: Word) -> Word { \
                 if a > b { helper(a) * b } else { helper(b) + a } }";
    // Reaching the assertion at all means both verifies passed inside the
    // helper; the value check is a bonus rather than the point.
    let vm = vm_result(src, &[9, 4]);
    let o2 = native_result_o2(src, &[9, 4]);
    assert_eq!(vm, o2);
}

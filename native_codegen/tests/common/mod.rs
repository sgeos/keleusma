//! Shared test support: run the shipping middle end on demand.
//!
//! # Why this exists
//!
//! Every differential on this line creates its JIT at
//! `OptimizationLevel::None`. That is a CODEGEN setting; `mem2reg` and the rest
//! of the middle end are a pass pipeline and do not run from it. Undefined
//! behaviour in emitted IR is invisible at `-O0` and appears at `-O2`.
//!
//! `corpus_differential` gained a `KEL_OPTIMIZE` hook first, and covering only
//! that one left the HAND-WRITTEN differentials unoptimised — including
//! `composite_return_aliasing`, which pins the composite-return aliasing defect,
//! the only genuine codegen defect this line has found. Region aliasing is
//! exactly the sort of thing an optimiser reasons about, so leaving that case at
//! `-O0` was the wrong one to leave.
//!
//! # Deliberately opt-in
//!
//! The default stays `None` so the everyday suite keeps its current meaning and
//! runtime, and the optimised run is a separate, explicit pass over the same
//! tests. Setting `KEL_OPTIMIZE` turns it on everywhere at once.

/// Run `default<O2>` over `lm` when `KEL_OPTIMIZE` is set, otherwise do nothing.
///
/// Call it AFTER `lower_module` and `verify`, and BEFORE creating the execution
/// engine. Verifying first keeps a pre-existing IR defect distinguishable from
/// one the optimiser introduces.
#[allow(dead_code)]
pub fn maybe_optimize(lm: &inkwell::module::Module<'_>) {
    if std::env::var("KEL_OPTIMIZE").is_err() {
        return;
    }
    use inkwell::OptimizationLevel;
    use inkwell::passes::PassBuilderOptions;
    use inkwell::targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetMachine};

    Target::initialize_native(&InitializationConfig::default()).expect("init native target");
    let triple = TargetMachine::get_default_triple();
    let machine = Target::from_triple(&triple)
        .expect("target")
        .create_target_machine(
            &triple,
            "generic",
            "",
            OptimizationLevel::Default,
            RelocMode::PIC,
            CodeModel::Default,
        )
        .expect("target machine");
    lm.run_passes("default<O2>", &machine, PassBuilderOptions::create())
        .expect("O2 pipeline");
    // A module that verified before the pipeline and not after it is the
    // finding this whole exercise is looking for, so it fails loudly here
    // rather than surfacing later as a wrong value.
    lm.verify().expect("IR still valid AFTER the O2 pipeline");
}

/// The corpus roots every sweep on this line is derived from, relative to
/// `native_codegen/`.
///
/// Kept beside [`corpus_sources`] so the roots and the walk cannot drift apart.
/// `corpus_fingerprint.rs` pins the CONTENT of these directories; this pins the
/// POPULATION read out of them.
pub const CORPUS_ROOTS: [&str; 4] = [
    "../examples/scripts",
    "../src/selfhost/kel",
    "../examples/rtos/scripts",
    "../compiler/kel",
];

/// **THE canonical corpus enumeration. One copy, so sweeps cannot disagree.**
///
/// # Why this is shared rather than repeated
///
/// Five defects on this line took the same shape: a measurement enumerated a
/// **narrower population than the thing it described**, then reported the
/// difference as a property of the subjects. A non-recursive walk saw 35 modules
/// where its consumers saw 74; a fingerprint covered three roots where consumers
/// read four; a directory listed explicitly *and* reached by recursion was
/// counted twice.
///
/// `corpus_fingerprint.rs` closed the neighbouring hole — the corpus content —
/// and its own header states the argument for this one: *"A habit is not a
/// check."* Keeping the walk in one place makes divergence impossible for callers
/// that use it, the same move that made two mutation censuses agree by
/// construction rather than by comparison.
///
/// **This eliminates the class for CALLERS OF THIS FUNCTION only.** A test still
/// carrying its own walk remains exposed.
///
/// # What it does and does not do
///
/// Enumerates `.kel` files recursively under [`CORPUS_ROOTS`], sorted and
/// deduplicated — the dedup matters because listing a root and also reaching it
/// by recursion is one of the five defects above. **It does not LOAD them.**
/// Loading is separate and some sources need a prelude prepended; unifying the
/// walk must not disturb that.
#[allow(dead_code)]
pub fn corpus_sources() -> Vec<std::path::PathBuf> {
    let mut out: Vec<std::path::PathBuf> = Vec::new();
    for dir in CORPUS_ROOTS {
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut stack: Vec<std::path::PathBuf> = rd.flatten().map(|e| e.path()).collect();
        while let Some(p) = stack.pop() {
            if p.is_dir() {
                if let Ok(rd2) = std::fs::read_dir(&p) {
                    stack.extend(rd2.flatten().map(|e| e.path()));
                }
            } else if p.extension().is_some_and(|x| x == "kel") {
                out.push(p);
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// `(vm, native)` for a two-argument entry, driving the trailing pointers when
/// the module builds composites or declares data slots.
///
/// # Why this lives here rather than in the file that first needed it
///
/// It began inside `composite_return_aliasing.rs`. A second file needing the
/// same harness has exactly two options, and one of them is the failure this
/// package keeps finding: a COPY that drifts from its original, after which two
/// tests answer the same question differently and neither says so. The other is
/// one definition, which is this. `composite_return_aliasing.rs` delegates here
/// rather than keeping a private twin.
///
/// The parameter count is read off the lowered entry and asserted before the
/// call, because a wrong signature is undefined behaviour that surfaces as a
/// segmentation fault inside JIT-compiled code with no usable stack.
#[allow(dead_code)]
pub fn vm_and_native_two_arg(src: &str, a: i64, b: i64) -> (i64, i64) {
    use inkwell::OptimizationLevel;
    use inkwell::context::Context;
    use keleusma::bytecode::Value;
    use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
    use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    assert!(
        keleusma_native::module_refusals(&m, keleusma_native::LowerOptions::default()).is_empty(),
        "the case must LOWER for the comparison to mean anything"
    );

    let need = required_persistent_capacity_for(&m);
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena") + need + (1 << 20);
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent");
    let mut vm = Vm::new(m.clone(), &arena).expect("vm");
    let vv = match vm.call(&[Value::Int(a), Value::Int(b)]).expect("vm run") {
        VmState::Finished(Value::Int(v)) | VmState::Yielded(Value::Int(v)) => v,
        other => panic!("unexpected VM outcome: {other:?}"),
    };

    let ctx = Context::create();
    let lm = ctx.create_module("k");
    keleusma_native::lower_module(&ctx, &lm, &m, keleusma_native::LowerOptions::default())
        .expect("lower");
    maybe_optimize(&lm);
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");
    let entry = m.entry_point.expect("entry");
    let sym = format!("kel_chunk_{entry}");
    let np = lm.get_function(&sym).expect("entry fn").count_params();

    let n_region: usize = m
        .chunks
        .iter()
        .map(|c| keleusma_native::region::plan_chunk_region(c).bytes as usize)
        .sum();
    let mut region = vec![0u64; n_region.div_ceil(8) + 4];
    let mut shared = vec![0u8; 64];
    let mut privs = vec![0u64; 8];

    let nv = match np {
        2 => {
            let f = unsafe { ee.get_function::<unsafe extern "C" fn(i64, i64) -> i64>(&sym) }
                .expect("symbol");
            unsafe { f.call(a, b) }
        }
        5 => {
            let f = unsafe {
                ee.get_function::<unsafe extern "C" fn(i64, i64, *mut u8, *mut u8, *mut u8) -> i64>(
                    &sym,
                )
            }
            .expect("symbol");
            unsafe {
                f.call(
                    a,
                    b,
                    shared.as_mut_ptr(),
                    privs.as_mut_ptr() as *mut u8,
                    region.as_mut_ptr() as *mut u8,
                )
            }
        }
        n => panic!("entry takes {n} parameters; this harness drives 2 or 5"),
    };
    (vv, nv)
}

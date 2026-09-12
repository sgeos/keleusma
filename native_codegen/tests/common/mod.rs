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

/// Install the backend's published private-data image into a host buffer.
///
/// **The host's obligation, exercised rather than described.** If these helpers
/// did not call it, the differential would compare a backend that never applies
/// a declared initializer against a runtime that always does — and would pass,
/// because every existing subject writes its slots before reading them.
#[allow(dead_code)]
pub fn install_private_init(m: &keleusma::bytecode::Module, privs: &mut [u64]) {
    let bytes: &mut [u8] =
        unsafe { core::slice::from_raw_parts_mut(privs.as_mut_ptr() as *mut u8, privs.len() * 8) };
    install_private_init_bytes(m, bytes);
}

#[allow(dead_code)]
pub fn install_private_init_bytes(m: &keleusma::bytecode::Module, privs: &mut [u8]) {
    if let Some(image) = keleusma_native::region::private_init_image(m) {
        let n = image.len().min(privs.len());
        privs[..n].copy_from_slice(&image[..n]);
    }
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

    // ⚠ **EVERY BUFFER IS SIZED FROM THE PUBLISHED CONTRACT, AND CARRIES A
    // CANARY.**
    //
    // These were literals: `vec![0u64; 8]` for the private region and a
    // per-chunk sum for the composite region. The corpus harness had the same
    // defect and was repaired hours earlier; **this one was left and produced a
    // SIGSEGV within the day**, when the indexed composite path became the first
    // subject here to write into the persistent pool. Under the narrow
    // configuration it passed, and in isolation it passed — a literal-sized
    // buffer fails by corrupting whatever is next to it, which is not a stable
    // observable.
    //
    // The region figure is TRANSITIVE: a call site receives a disjoint block of
    // the caller's region, so a per-chunk sum under-counts a module whose entry
    // calls anything.
    const CANARY: u64 = 0xDEAD_BEEF_FEED_FACE;
    let n_region = keleusma_native::region::host_arena_supplement_bytes(&m) as usize;
    let mut region = vec![0u64; n_region.div_ceil(8) + 4];
    let region_canary_at = region.len() - 1;
    region[region_canary_at] = CANARY;
    let mut shared = vec![0u8; keleusma::vm::shared_data_bytes_for(&m).max(64) + 8];
    // **THE PRIVATE REGION IS INSTALLED, NOT ZEROED.** A private scalar slot
    // carries its declared initializer in the module's `private_init` table and
    // the runtime applies it at load; there is no native load step, so the host
    // installs the published image. A harness that zeroed this buffer reported
    // `1` where the reference reported `8`.
    let n_priv = (required_persistent_capacity_for(&m)
        + keleusma_native::region::persistent_supplement_bytes(&m) as usize)
        .div_ceil(8);
    let mut privs = vec![0u64; n_priv + 1];
    privs[n_priv] = CANARY;
    install_private_init(&m, &mut privs[..n_priv]);

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
    // **The canaries, checked rather than merely placed.** A buffer sized from a
    // literal fails by corrupting its neighbour, which is not a stable
    // observable; these turn that into an assertion at the boundary of what the
    // backend is entitled to touch.
    assert_eq!(
        privs[n_priv], CANARY,
        "the lowering wrote past the {n_priv}-word private region a host is told \
         to allocate"
    );
    assert_eq!(
        region[region_canary_at], CANARY,
        "the lowering wrote past the {n_region}-byte composite region"
    );
    (vv, nv)
}

// ---------------------------------------------------------------------------
// THE WITNESS REGISTRY, AND WHY IT IS HERE RATHER THAN COPIED INTO EACH TEST
// ---------------------------------------------------------------------------
//
// Absorption 51 turned TWELVE tests red at once across FIVE files, because each
// carried its own copy of the same `Op::Len` witness source. One upstream
// improvement invalidated every copy simultaneously, and each copy had to be
// found and disposed of separately.
//
// **That coupling is this line's own design fault and it has now rotted three
// times** -- `Op::Call`, `Op::IsStruct`, and now `Op::Len`. The witness text is
// a single definition from here on, so the next fold invalidates ONE location.
//
// The tests themselves stay numerous; that is not the problem. What must not be
// duplicated is the definition of what is being witnessed.

/// The `if`-EXPRESSION for-in source. **This no longer emits `Op::Len`** -- the
/// fold is recorded in `docs/decisions/OP_LEN_PRODUCER_CENSUS.md` -- and it is
/// kept because several tests assert exactly that absence.
#[allow(dead_code)]
pub const IF_SOURCE: &str = "\
fn f(c: bool) -> Word {
  let a = [1, 2];
  let b = [3, 4];
  for x in if c { a } else { b } { let _d = x; }
  0
}
fn main() -> Word { f(true) }
";

/// The same shape with BOTH ARMS THE SAME LENGTH.
#[allow(dead_code)]
pub const IF_SOURCE_EQUAL_LENGTHS: &str = "\
fn f(c: bool) -> Word {
  let a = [1, 2];
  let b = [9, 9];
  for x in if c { a } else { b } { let _d = x; }
  0
}
fn main() -> Word { f(true) }
";

/// The ordinary for-in. The control that keeps every claim about the `if` form
/// from being a claim about for-in in general.
#[allow(dead_code)]
pub const PLAIN_SOURCE: &str = "\
fn f() -> Word {
  let a = [1, 2];
  for x in a { let _d = x; }
  0
}
fn main() -> Word { f() }
";

/// The former `Op::IsStruct` witness, kept as a control: it must still compile
/// and still MEAN `a + b`.
#[allow(dead_code)]
pub const IS_STRUCT_SOURCE: &str = "\
struct P { a: Word, b: Word }
fn g(P { a, b }) -> Word { a + b }
fn main() -> Word { g(P { a: 1, b: 2 }) }
";

/// Compile a source that is expected to compile.
#[allow(dead_code)]
pub fn build(src: &str) -> keleusma::bytecode::Module {
    keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(src).expect("lex")).expect("parse"),
    )
    .expect("compile")
}

/// Compile a source that may legitimately fail at any stage.
#[allow(dead_code)]
pub fn try_build(src: &str) -> Option<keleusma::bytecode::Module> {
    keleusma::lexer::tokenize(src)
        .ok()
        .and_then(|t| keleusma::parser::parse(&t).ok())
        .and_then(|a| keleusma::compiler::compile(&a).ok())
}

/// Does any chunk carry an opcode whose debug name begins with `name`?
///
/// Prefix matching on the debug form covers both the nullary `Len` and the
/// parameterised forms such as `IsStruct(0)` without naming their payloads.
#[allow(dead_code)]
pub fn emits(m: &keleusma::bytecode::Module, name: &str) -> bool {
    m.chunks
        .iter()
        .any(|c| c.ops.iter().any(|o| format!("{o:?}").starts_with(name)))
}

/// The corpus roots, **widened to four after this loader reproduced the exact
/// defect `corpus_fingerprint.rs` was built to prevent.**
///
/// The first version here listed three, copied from `remaining_refusals`. But
/// `corpus_fingerprint` watches FOUR, because the censuses that publish figures
/// read four — `spike_corpus_coverage`, `isa_lowering_census` and
/// `bound_transfer` all include `examples/rtos/scripts` and `compiler/kel`.
///
/// **That guard's own header records this defect at three granularities**: a
/// pinned value whose input was an unwatched directory scan, then a scan of three
/// named directories where the loaders recurse, then a guard covering three roots
/// where the consumers read four. **Each time the watched population was narrower
/// than the one that mattered, and each time the narrow scan returned a
/// well-formed answer.** This is the fourth occurrence, committed by a loader
/// written in the same session that read the warning.
///
/// A sweep looking for an opcode wants the WIDEST population available: a
/// negative over a narrow corpus is a weaker negative, and nothing here is made
/// worse by reading more files. Recursion reaches `examples/scripts/rogue` and
/// `examples/scripts/piano_roll`; listing a subdirectory as well is what made
/// three other tests count one directory twice.
#[allow(dead_code)]
pub const CORPUS_DIRS: [&str; 4] = [
    "examples/scripts",
    "src/selfhost/kel",
    "examples/rtos/scripts",
    "compiler/kel",
];

/// Every `.kel` file under the corpus roots that COMPILES, as `(file name, module)`.
///
/// A file that does not compile is skipped rather than reported: several corpus
/// files are stage sources needing a prelude, and they are not this loader's
/// subject. Callers that need a population floor must assert one.
#[allow(dead_code)]
pub fn corpus() -> Vec<(String, keleusma::bytecode::Module)> {
    let root = std::path::Path::new("..");
    let mut stack: Vec<std::path::PathBuf> = CORPUS_DIRS.iter().map(|d| root.join(d)).collect();
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
        if let Some(m) = try_build(&src) {
            out.push((name, m));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// GENERAL (RESUMABLE) STREAM DIFFERENTIAL
//
// A resumable stream RETURNS at each `yield` and re-enters at the same point,
// with its locals and its resume state in the arena. Calling it repeatedly must
// therefore reproduce exactly the sequence the virtual machine yields.
//
// **WHY THIS LIVES HERE RATHER THAN IN ONE TEST FILE.** The driver names the
// lowered function's signature by hand, and a harness that names a signature
// cannot see it change. Extending the arena pointers to stream chunks once
// changed the DEGENERATE chunks' signature too, and ten tests kept passing on
// the calling convention's good manners — garbage read from registers the callee
// never touched. The `count_params` assertion below is the guard against that,
// and it belongs in ONE place rather than being re-derived by each file that
// drives a stream.
//
// **THE ARENA IS THE INSTANCE.** Nothing here is global, so the buffers built
// per call belong to this stream and to no other; two arenas would be two
// independent streams.
// ---------------------------------------------------------------------------

/// Every value the virtual machine yields, bounded by the reply count.
#[allow(dead_code)]
pub fn general_vm_sequence(src: &str, first: i64, replies: &[i64]) -> Vec<i64> {
    use keleusma::bytecode::Value;
    use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};

    let m = build(src);
    let need = required_persistent_capacity_for(&m);
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena capacity") + need + (64 << 10);
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent region");

    let mut vm = Vm::new(m, &arena).expect("vm");
    let mut out = Vec::new();
    let mut st = vm.call(&[Value::Int(first)]).expect("vm run");
    while out.len() < replies.len() {
        match st {
            VmState::Yielded(Value::Int(v)) => {
                out.push(v);
                let r = replies[out.len() - 1];
                st = vm.resume(Value::Int(r)).expect("resume");
            }
            // The runtime reports the rewind as a leg of its own; the native
            // driver collapses it, so it contributes no yielded value here and
            // is offered the SAME reply as the suspension it follows.
            VmState::Reset => {
                let r = replies[out.len().saturating_sub(1)];
                st = vm.resume(Value::Int(r)).expect("resume after reset");
            }
            other => panic!("a stream produced {other:?}"),
        }
    }
    out
}

/// The same chunk as native code: one call per suspension, arena-resident state.
#[allow(dead_code)]
pub fn general_native_sequence(src: &str, first: i64, replies: &[i64]) -> Vec<i64> {
    use inkwell::OptimizationLevel;
    use inkwell::context::Context;
    use keleusma::vm::required_persistent_capacity_for;
    use keleusma_native::{LowerOptions, lower_module};

    let m = build(src);
    let entry = m.entry_point.expect("entry point");
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("lower module");
    lm.verify().expect("LLVM module verification");
    maybe_optimize(&lm);
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");

    let sym = format!("kel_chunk_{entry}");
    let f = lm.get_function(&sym).expect("entry function");
    assert_eq!(
        f.count_params(),
        u32::from(m.chunks[entry].param_count) + 3,
        "a resumable stream must carry the three trailing pointers; the call below \
         names that signature by hand and cannot detect a change to it"
    );
    let callable = unsafe {
        ee.get_function::<unsafe extern "C" fn(i64, *mut u8, *mut u8, *mut u8) -> i64>(&sym)
    }
    .expect("entry symbol");

    // Persistent carries the resume state, which is why it is sized with the
    // backend's supplement and not with the runtime's figure alone.
    let persistent = required_persistent_capacity_for(&m)
        + keleusma_native::region::persistent_supplement_bytes(&m) as usize;
    let mut privs = vec![0u8; persistent + 64];
    install_private_init_bytes(&m, &mut privs);
    let mut shared = vec![0u8; 4096];
    let mut region =
        vec![0u8; keleusma_native::region::host_arena_supplement_bytes(&m) as usize + 4096];

    let mut out = Vec::new();
    let mut input = first;
    for &r in replies {
        out.push(unsafe {
            callable.call(
                input,
                shared.as_mut_ptr(),
                privs.as_mut_ptr(),
                region.as_mut_ptr(),
            )
        });
        input = r;
    }
    out
}

/// Assert a resumable stream's whole yielded sequence matches the runtime's.
///
/// Refuses to be pointed at a program the backend declines: asserting agreement
/// about a module that never ran is the failure mode this whole file exists to
/// prevent.
#[allow(dead_code)]
pub fn assert_general_stream_agrees(src: &str, first: i64, replies: &[i64]) {
    use keleusma_native::{LowerOptions, module_refusals};

    let refusals = module_refusals(&build(src), LowerOptions::default());
    assert!(
        refusals.is_empty(),
        "the backend refuses {src:?}, so this would be asserting agreement about a \
         program it never ran: {refusals:?}"
    );
    let vm = general_vm_sequence(src, first, replies);
    let nat = general_native_sequence(src, first, replies);
    assert_eq!(
        nat, vm,
        "YIELD SEQUENCE differs for {src:?}\n  native={nat:?}\n  vm    ={vm:?}"
    );
}

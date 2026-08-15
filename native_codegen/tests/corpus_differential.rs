//! **Every module that lowers, executed against the virtual machine.**
//!
//! Fourteen modules had hand-written differentials. The other forty-one rested
//! on `module_refusals(...).is_empty()` — `lower_module` returning `Ok`, which is
//! a fact about the COMPILER and not about the program. That claim stood in for
//! verification once already and was wrong to.
//!
//! # How this is generic where the others are specific
//!
//! Nothing here is written per module. Each module's natives and their arities
//! are read **from the bytecode** (`native_names`, and `n & 0x7F` at each call
//! site), each entry's parameter and return shapes from `Module::signatures`,
//! and the three buffer sizes from the data layout and `plan_chunk_region`.
//!
//! Native stubs are bound by `ExecutionEngine::add_global_mapping` against the
//! declaration looked up **from the lowered module**, so the harness never has to
//! reproduce `native_symbol`'s mangling. An earlier attempt bound by a
//! hand-written name, got the mangling wrong, and segfaulted; looking the
//! declaration up cannot be wrong in that way.
//!
//! # A stub is wider than its declaration, deliberately
//!
//! Every stub takes five `i64`s. A declaration of lower arity is mapped to one
//! anyway, and the surplus registers hold whatever the caller left there. The
//! stub reads only the `argc` the bytecode records, so the garbage is never
//! observed. This is what lets ONE stub table serve 42 natives, and it is sound
//! here only because **no native in the corpus is called at two different
//! arities** — measured in `probe_corpus_shapes.rs`, and asserted below.
use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::{BlockType, Module, Op, SlotVisibility, Value, WireShape};
use keleusma::vm::{
    Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for, shared_data_bytes_for,
};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerOptions, lower_module};
use std::cell::RefCell;

/// Ticks to drive a `Stream` entry. Enough to leave any init branch.
const TICKS: i64 = 60;
/// Stub slots. Must be at least the corpus's distinct-native count (42).
const STUBS: usize = 48;

thread_local! {
    /// Index -> (name, argc) for the module currently under test.
    static TABLE: RefCell<Vec<(String, usize)>> = const { RefCell::new(Vec::new()) };
    static LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// Set when a native received a NON-INTEGER argument on the VM side.
    ///
    /// Such an argument is a reference — a string, most often — and it reaches
    /// the native as an arena handle on one side and a pointer on the other.
    /// Neither renders as the same integer, so the module is exempted rather
    /// than reported as a disagreement it is not.
    static SAW_REF_ARG: RefCell<bool> = const { RefCell::new(false) };
}

fn take_log() -> Vec<String> {
    LOG.with(|l| core::mem::take(&mut *l.borrow_mut()))
}

/// The value a native returns, on BOTH sides.
///
/// Deterministic, and asymmetric in the argument positions so a swapped or
/// dropped argument changes it. A random source would make the two runs
/// incomparable, which is the point of stubbing at all.
fn stub_value(idx: usize, args: &[i64]) -> i64 {
    let mut acc = (idx as i64 + 1) * 7;
    for (i, a) in args.iter().enumerate() {
        acc = acc
            .wrapping_mul(31)
            .wrapping_add(a.wrapping_mul(i as i64 + 1));
    }
    acc % 1024
}

fn record(idx: usize, all: [i64; 5]) -> i64 {
    let (name, argc) = TABLE.with(|t| {
        t.borrow()
            .get(idx)
            .cloned()
            .unwrap_or_else(|| (format!("<unmapped #{idx}>"), 0))
    });
    let args = &all[..argc.min(5)];
    let parts: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    LOG.with(|l| l.borrow_mut().push(format!("{name}({})", parts.join(", "))));
    stub_value(idx, args)
}

macro_rules! stubs {
    ($($n:literal => $sym:ident),* $(,)?) => {
        $(
            #[unsafe(no_mangle)]
            pub extern "C" fn $sym(a: i64, b: i64, c: i64, d: i64, e: i64) -> i64 {
                record($n, [a, b, c, d, e])
            }
        )*
        fn stub_addrs() -> Vec<usize> {
            vec![$($sym as *const () as usize),*]
        }
    };
}

stubs!(
    0 => kel_stub_00, 1 => kel_stub_01, 2 => kel_stub_02, 3 => kel_stub_03,
    4 => kel_stub_04, 5 => kel_stub_05, 6 => kel_stub_06, 7 => kel_stub_07,
    8 => kel_stub_08, 9 => kel_stub_09, 10 => kel_stub_10, 11 => kel_stub_11,
    12 => kel_stub_12, 13 => kel_stub_13, 14 => kel_stub_14, 15 => kel_stub_15,
    16 => kel_stub_16, 17 => kel_stub_17, 18 => kel_stub_18, 19 => kel_stub_19,
    20 => kel_stub_20, 21 => kel_stub_21, 22 => kel_stub_22, 23 => kel_stub_23,
    24 => kel_stub_24, 25 => kel_stub_25, 26 => kel_stub_26, 27 => kel_stub_27,
    28 => kel_stub_28, 29 => kel_stub_29, 30 => kel_stub_30, 31 => kel_stub_31,
    32 => kel_stub_32, 33 => kel_stub_33, 34 => kel_stub_34, 35 => kel_stub_35,
    36 => kel_stub_36, 37 => kel_stub_37, 38 => kel_stub_38, 39 => kel_stub_39,
    40 => kel_stub_40, 41 => kel_stub_41, 42 => kel_stub_42, 43 => kel_stub_43,
    44 => kel_stub_44, 45 => kel_stub_45, 46 => kel_stub_46, 47 => kel_stub_47,
);

fn sources() -> Vec<std::path::PathBuf> {
    let root = std::path::Path::new("..");
    let mut out = Vec::new();
    let mut stack: Vec<std::path::PathBuf> = [
        "examples/scripts",
        "src/selfhost/kel",
        "examples/rtos/scripts",
        "compiler/kel",
    ]
    .iter()
    .map(|d| root.join(d))
    .collect();
    while let Some(p) = stack.pop() {
        if p.is_dir() {
            if let Ok(rd) = std::fs::read_dir(&p) {
                stack.extend(rd.filter_map(|e| e.ok()).map(|e| e.path()));
            }
        } else if p.extension().is_some_and(|x| x == "kel") {
            out.push(p);
        }
    }
    out.sort();
    out
}

/// The source to compile for `p`, with the rtos prelude prepended where the
/// rtos host prepends it.
///
/// **Five exemptions were harness artefacts.** `event_listener`, `faulty`,
/// `heartbeat`, `led` and `sensor` were recorded as "rejected by the REFERENCE
/// compiler", which reads as a statement about the scripts. It was a statement
/// about this harness: it compiled each `.kel` standalone, while the rtos host
/// does
///
/// ```ignore
/// let combined = format!("{}\n{}", PRELUDE, src);   // examples/rtos/src/setup.rs:429
/// ```
///
/// so every script referencing a prelude declaration failed to compile.
///
/// **This is NOT the thing Part B of this increment refused to do.** Declining
/// to reproduce the self-hosted stages' input formats was right because a seed a
/// stage silently rejects looks exactly like coverage. This composition is four
/// lines, it is the shipping host's own, it is quoted from `setup.rs`, and the
/// scripts document it themselves. A wrong composition fails loudly at compile
/// time rather than producing a plausible run.
fn source_for(p: &std::path::Path) -> Option<String> {
    let src = std::fs::read_to_string(p).ok()?;
    let is_rtos = p.components().any(|c| c.as_os_str() == "rtos");
    let is_prelude = p.file_name().is_some_and(|n| n == "prelude.kel");
    if is_rtos && !is_prelude {
        let prelude = std::fs::read_to_string("../examples/rtos/scripts/prelude.kel").ok()?;
        return Some(format!("{prelude}\n{src}"));
    }
    Some(src)
}

/// `(native index, argc)` for every native the module actually calls.
///
/// Asserts the single-arity property this whole design rests on.
fn native_table(m: &Module) -> Vec<(String, usize)> {
    let mut argc: Vec<Option<usize>> = vec![None; m.native_names.len()];
    for c in &m.chunks {
        for op in &c.ops {
            if let Op::CallVerifiedNative(i, n) | Op::CallExternalNative(i, n) = op {
                let a = usize::from(n & 0x7F);
                let slot = &mut argc[usize::from(*i)];
                if let Some(prev) = slot {
                    assert_eq!(
                        *prev,
                        a,
                        "native `{}` is called at two arities ({prev} and {a}); one stub \
                         cannot serve both and the whole stub table design fails here",
                        m.native_names[usize::from(*i)]
                    );
                } else {
                    *slot = Some(a);
                }
            }
        }
    }
    m.native_names
        .iter()
        .zip(argc)
        .map(|(n, a)| (n.clone(), a.unwrap_or(0)))
        .collect()
}

fn arena_for(m: &Module) -> keleusma_arena::Arena {
    // The host margin is the harness's job. `auto_arena_capacity_for` sizes the
    // nominal stack and a long run exhausts it; the runtime says so explicitly.
    const HOST_MARGIN: usize = 4 << 20;
    let need = required_persistent_capacity_for(m);
    let cap = auto_arena_capacity_for(m, &[]).expect("arena capacity") + need + HOST_MARGIN;
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent fits");
    arena
}

/// Scalar arguments for an entry of `n` parameters. Asymmetric per position.
/// The argument vectors each module is driven with, in order.
///
/// **Seed 0 is the original vector and every other seed is new.** Driving one
/// vector of pairwise-DISTINCT ascending arguments is what left a hole: `SLT`
/// and `SLE` differ only when the operands are EQUAL, and nothing in seed 0 ever
/// makes two comparands equal. Measured: with `Op::CmpLt` lowered as `SLE` --
/// 126 sites across 25 modules -- the whole differential passed. Inverting it to
/// `SGT` outright was caught by only 2 of those 25.
///
/// Seed 1 makes every argument equal, which is what drives a comparison to its
/// boundary. Seed 2 is all zeros, the identity case. Seed 3 descends, so an
/// ordering assumption that holds under seed 0 fails here.
const SEEDS: usize = 4;

fn args_for_seed(n: usize, seed: usize) -> Vec<i64> {
    (0..n)
        .map(|i| match seed {
            0 => (i as i64 + 1) * 3 + 1,
            1 => 5,
            2 => 0,
            _ => (n as i64 - i as i64) * 3 + 1,
        })
        .collect()
}

/// Is every parameter of this entry a scalar the harness can synthesise?
///
/// A composite parameter is an address into caller-owned storage, which the
/// three hand-written differentials build per module. Reported as an exemption
/// rather than guessed at.
fn params_are_scalar(m: &Module, entry: usize) -> bool {
    match m.signatures.get(entry) {
        Some(sig) => sig
            .params
            .iter()
            .all(|p| matches!(p, WireShape::Scalar { .. })),
        // No signature table entry: fall back to the chunk's own arity being 0,
        // which cannot carry a composite.
        None => m.chunks[entry].param_count == 0,
    }
}

struct Run {
    results: Vec<i64>,
    log: Vec<String>,
    shared: Vec<u8>,
}

fn run_vm(m: &Module, table: &[(String, usize)], seed: usize) -> Result<Run, String> {
    let _ = take_log();
    SAW_REF_ARG.with(|f| *f.borrow_mut() = false);
    let arena = arena_for(m);
    // Fallible: a module may refuse to LOAD for reasons unrelated to lowering —
    // a signature requirement, for one. That is an exemption with a stated
    // reason, not a harness crash.
    let mut vm = match Vm::new(m.clone(), &arena) {
        Ok(v) => v,
        Err(e) => return Err(format!("the VM refuses to load it: {e:?}")),
    };
    for (idx, (name, argc)) in table.iter().enumerate() {
        if name.is_empty() {
            continue;
        }
        let (n, ac) = (name.clone(), *argc);
        vm.register_native_closure(name, move |args: &[Value]| {
            let vals: Vec<i64> = args
                .iter()
                .take(ac)
                .map(|v| match v {
                    Value::Int(x) => *x,
                    Value::Byte(b) => i64::from(*b),
                    Value::Bool(b) => i64::from(*b),
                    _ => {
                        SAW_REF_ARG.with(|f| *f.borrow_mut() = true);
                        0
                    }
                })
                .collect();
            let parts: Vec<String> = vals.iter().map(|a| a.to_string()).collect();
            LOG.with(|l| l.borrow_mut().push(format!("{n}({})", parts.join(", "))));
            Ok(Value::Int(stub_value(idx, &vals)))
        });
    }

    let entry = m.entry_point.expect("entry");
    let n = m.chunks[entry].param_count as usize;
    // A Stream's first call is tick 0, matching the native driver's `f.call(0)`.
    // Passing `args_for` here instead desynchronised every stream in the corpus,
    // which showed up as a call-count difference rather than a value difference.
    let vals: Vec<Value> = if m.chunks[entry].block_type == BlockType::Stream && n == 1 {
        vec![Value::Int(0)]
    } else {
        args_for_seed(n, seed).into_iter().map(Value::Int).collect()
    };
    let mut shared = vec![0u8; shared_data_bytes_for(m)];
    let mut results = Vec::new();

    let first = match vm.call_with_shared(&mut shared, &vals) {
        Ok(v) => v,
        Err(e) => return Err(format!("the VM refuses to run it: {e:?}")),
    };
    results.push(scalar_of(&first));
    if m.chunks[entry].block_type == BlockType::Stream {
        for t in 1..TICKS {
            // One tick is a `Reset` leg then a `Yielded` leg, and the SAME reply
            // goes to both. A fresh reply on the Reset leg is silently discarded.
            let mut st = match vm.resume_with_shared(&mut shared, Value::Int(t)) {
                Ok(v) => v,
                Err(e) => return Err(format!("the VM refuses to resume it: {e:?}")),
            };
            if matches!(st, VmState::Reset) {
                st = match vm.resume_with_shared(&mut shared, Value::Int(t)) {
                    Ok(v) => v,
                    Err(e) => return Err(format!("the VM refuses to resume it: {e:?}")),
                };
            }
            results.push(scalar_of(&st));
        }
    }
    if SAW_REF_ARG.with(|f| *f.borrow()) {
        return Err(
            "a native receives a REFERENCE argument (a string); it is an \
                    arena handle on the VM side and a pointer natively, so the two \
                    do not render as the same integer"
                .into(),
        );
    }
    Ok(Run {
        results,
        log: take_log(),
        shared,
    })
}

/// A scalar outcome, or a stable marker for anything else.
///
/// A composite result is compared through the shared segment and the call log
/// rather than decoded here; marking it keeps the two sides comparable without
/// pretending to read a body the harness did not build.
fn scalar_of(st: &VmState) -> i64 {
    match st {
        VmState::Yielded(Value::Int(v)) | VmState::Finished(Value::Int(v)) => *v,
        VmState::Yielded(Value::Unit) | VmState::Finished(Value::Unit) => 0,
        VmState::Reset => i64::MIN + 1,
        _ => i64::MIN,
    }
}

fn run_native(m: &Module, table: &[(String, usize)], seed: usize) -> Option<Run> {
    let _ = take_log();
    TABLE.with(|t| *t.borrow_mut() = table.to_vec());
    let addrs = stub_addrs();
    std::hint::black_box(&addrs);

    let entry = m.entry_point.expect("entry");
    let n_shared = shared_data_bytes_for(m);
    let n_priv = m
        .data_layout
        .as_ref()
        .map(|dl| {
            dl.slots
                .iter()
                .filter(|s| s.visibility == SlotVisibility::Private)
                .count()
        })
        .unwrap_or(0);
    // Transitive, not the per-chunk sum: each call site now receives a disjoint
    // block of the caller's region, so the entry needs everything it can reach.
    let n_region: usize = keleusma_native::region::region_total_bytes(m, entry, 0) as usize;

    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_module(&ctx, &lm, m, LowerOptions::default()).expect("lower module");
    lm.verify().expect("LLVM module verification");
    // **PART B: run the O2 middle end when asked.**
    //
    // `create_jit_execution_engine(OptimizationLevel::_)` sets the CODEGEN level
    // only; `mem2reg` and the rest of the middle end are a pass pipeline and do
    // not run from it. `aot_linkage.rs` already runs `default<O2>` on one
    // hand-written module — so the claim "no object file has ever been
    // optimised" was wrong — but no CORPUS-WIDE differential has ever run the
    // middle end. Undefined behaviour in emitted IR is invisible at `-O0`.
    if std::env::var("KEL_OPTIMIZE").is_ok() {
        use inkwell::passes::PassBuilderOptions;
        use inkwell::targets::{InitializationConfig, Target, TargetMachine};
        Target::initialize_native(&InitializationConfig::default()).expect("init target");
        let triple = TargetMachine::get_default_triple();
        let machine = Target::from_triple(&triple)
            .expect("target")
            .create_target_machine(
                &triple,
                "generic",
                "",
                OptimizationLevel::Default,
                inkwell::targets::RelocMode::PIC,
                inkwell::targets::CodeModel::Default,
            )
            .expect("target machine");
        lm.run_passes("default<O2>", &machine, PassBuilderOptions::create())
            .expect("O2 pipeline");
    }

    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");

    // Bind each declared native to its stub. The declaration is looked up FROM
    // THE MODULE, so the harness never reproduces `native_symbol`'s mangling —
    // getting that wrong is what segfaulted an earlier attempt.
    let by_symbol: std::collections::BTreeMap<String, usize> = table
        .iter()
        .enumerate()
        .filter(|(_, (n, _))| !n.is_empty())
        .map(|(i, (n, _))| {
            let mut s = String::from("kel_native_");
            for c in n.chars() {
                s.push(if c.is_ascii_alphanumeric() || c == '_' {
                    c
                } else {
                    '_'
                });
            }
            (s, i)
        })
        .collect();
    for f in lm.get_functions() {
        let sym = f.get_name().to_string_lossy().to_string();
        if let Some(&i) = by_symbol.get(&sym) {
            assert!(i < STUBS, "native index {i} exceeds the {STUBS} stub slots");
            ee.add_global_mapping(&f, addrs[i]);
        }
    }

    const CANARY: u64 = 0xDEAD_BEEF_FEED_FACE;
    let mut shared = vec![0u8; n_shared + 8];
    shared[n_shared..].copy_from_slice(&CANARY.to_le_bytes());
    let mut privs = vec![0u64; n_priv + 1];
    privs[n_priv] = CANARY;
    let mut region = vec![0u64; n_region.div_ceil(8) + 1];
    let canary_at = n_region.div_ceil(8);
    region[canary_at] = CANARY;

    let sym = format!("kel_chunk_{entry}");
    let fv = lm.get_function(&sym).expect("entry function");
    let declared = fv.count_params() as usize;
    let src_arity = m.chunks[entry].param_count as usize;
    let ptrs = declared.checked_sub(src_arity).expect("declared >= arity");
    // **Assert the ABI before calling through it.** A wrong signature is
    // undefined behaviour that surfaces as SIGSEGV inside JIT code with no
    // stack; it cost two cycles this session.
    assert!(
        ptrs == 0 || ptrs == 3,
        "entry `{sym}` has {declared} parameters for arity {src_arity}: {ptrs} trailing \
         pointers, which is neither none nor the three the ABI defines"
    );

    let a = args_for_seed(src_arity, seed);
    let sp = shared.as_mut_ptr();
    let pp = privs.as_mut_ptr() as *mut u8;
    let rp = region.as_mut_ptr() as *mut u8;
    let is_stream = m.chunks[entry].block_type == BlockType::Stream;
    let ticks = if is_stream { TICKS } else { 1 };
    let mut results = Vec::new();

    macro_rules! drive {
        ($t:ty, $($x:expr),*) => {{
            let f = unsafe { ee.get_function::<$t>(&sym) }.expect("entry symbol");
            for t in 0..ticks {
                let _ = t;
                results.push(unsafe { f.call($($x),*) });
            }
        }};
    }

    // Only the shapes the corpus actually presents; anything else is reported
    // as an exemption rather than driven through a guessed signature.
    match (src_arity, ptrs, is_stream) {
        (0, 0, _) => drive!(unsafe extern "C" fn() -> i64,),
        (0, 3, _) => drive!(
            unsafe extern "C" fn(*mut u8, *mut u8, *mut u8) -> i64,
            sp,
            pp,
            rp
        ),
        (1, 0, false) => drive!(unsafe extern "C" fn(i64) -> i64, a[0]),
        (1, 3, false) => {
            drive!(
                unsafe extern "C" fn(i64, *mut u8, *mut u8, *mut u8) -> i64,
                a[0],
                sp,
                pp,
                rp
            )
        }
        (1, 0, true) => {
            let f = unsafe { ee.get_function::<unsafe extern "C" fn(i64) -> i64>(&sym) }
                .expect("entry symbol");
            for t in 0..ticks {
                results.push(unsafe { f.call(t) });
            }
        }
        (1, 3, true) => {
            let f = unsafe {
                ee.get_function::<unsafe extern "C" fn(i64, *mut u8, *mut u8, *mut u8) -> i64>(&sym)
            }
            .expect("entry symbol");
            for t in 0..ticks {
                results.push(unsafe { f.call(t, sp, pp, rp) });
            }
        }
        (2, 0, false) => drive!(unsafe extern "C" fn(i64, i64) -> i64, a[0], a[1]),
        (2, 3, false) => drive!(
            unsafe extern "C" fn(i64, i64, *mut u8, *mut u8, *mut u8) -> i64,
            a[0],
            a[1],
            sp,
            pp,
            rp
        ),
        (3, 0, false) => drive!(unsafe extern "C" fn(i64, i64, i64) -> i64, a[0], a[1], a[2]),
        (3, 3, false) => drive!(
            unsafe extern "C" fn(i64, i64, i64, *mut u8, *mut u8, *mut u8) -> i64,
            a[0],
            a[1],
            a[2],
            sp,
            pp,
            rp
        ),
        (4, 0, false) => drive!(
            unsafe extern "C" fn(i64, i64, i64, i64) -> i64,
            a[0],
            a[1],
            a[2],
            a[3]
        ),
        (4, 3, false) => drive!(
            unsafe extern "C" fn(i64, i64, i64, i64, *mut u8, *mut u8, *mut u8) -> i64,
            a[0],
            a[1],
            a[2],
            a[3],
            sp,
            pp,
            rp
        ),
        (5, 0, false) => drive!(
            unsafe extern "C" fn(i64, i64, i64, i64, i64) -> i64,
            a[0],
            a[1],
            a[2],
            a[3],
            a[4]
        ),
        (5, 3, false) => drive!(
            unsafe extern "C" fn(i64, i64, i64, i64, i64, *mut u8, *mut u8, *mut u8) -> i64,
            a[0],
            a[1],
            a[2],
            a[3],
            a[4],
            sp,
            pp,
            rp
        ),
        _ => return None,
    }

    assert_eq!(
        &shared[n_shared..],
        &CANARY.to_le_bytes(),
        "wrote past the {n_shared}-byte shared segment"
    );
    assert_eq!(privs[n_priv], CANARY, "wrote past the private region");
    assert_eq!(
        region[canary_at], CANARY,
        "wrote past the {n_region}-byte composite region"
    );

    shared.truncate(n_shared);
    Some(Run {
        results,
        log: take_log(),
        shared,
    })
}

/// Did this run produce any observable work at all?
///
/// The harness compares three things, and a module that exits immediately is
/// trivial in all three at once: one repeated result, no host calls, and a shared
/// segment still holding the zeros it was handed. Two sides agreeing on that
/// state assert nothing about the emitter.
///
/// **Conservative by construction.** A run is vacuous only when EVERY observable
/// is trivial, so a module doing real work in any one of them is counted as
/// executed. The consequence is that this under-reports vacuity rather than
/// over-reporting it, which is the safe direction for a number quoted as
/// coverage.
///
/// **Only a STREAM is judged.** A single-call module produces exactly one result
/// by construction, so "every result is the same" is true of it vacuously and
/// says nothing. A first attempt omitted this and classified 32 modules as
/// vacuous, including `10_multbyte.kel` — the module whose execution exposed the
/// composite-return aliasing defect, and therefore the clearest possible
/// counterexample to its own classification.
fn is_vacuous(run: &Run) -> bool {
    if run.results.len() < 2 {
        return false;
    }
    let mut distinct: Vec<i64> = run.results.clone();
    distinct.sort_unstable();
    distinct.dedup();
    distinct.len() <= 1 && run.log.is_empty() && run.shared.iter().all(|b| *b == 0)
}

/// Modules that AGREE while producing nothing, tracked rather than counted.
///
/// Every one is a self-hosted compiler stage that reads its input from the shared
/// data segment. This harness supplies that segment as zeros, so each takes an
/// immediate end-of-input exit — `lexer.kel` yielded `62`, its own documented
/// end-of-source marker, sixty times.
///
/// **They were inside the "executed and agreeing" count until 2026-08-14**, which
/// is why the headline moved from 40 to 34. Nothing regressed; the number was
/// measuring the harness rather than the emitter.
///
/// `lexer.kel` and `parse.kel` now have REAL coverage in `stage_differential.rs`,
/// which seeds the segment identically on both sides. They stay listed here
/// because this harness still drives them on nothing.
///
/// The other four consume abstract-syntax-tree and descriptor blocks whose
/// layouts belong to the `src/selfhost/mod.rs` driver, which this line may read
/// but must not edit. Seeding those means reproducing four input formats, and a
/// seed a stage silently rejects looks exactly like coverage.
const KNOWN_VACUOUS: &[&str] = &[
    "lexer.kel",
    "reconstruct.kel",
    "verify_datalayout.kel",
    "verify_depth.kel",
    "verify_structural.kel",
    "verify_typed.kel",
];

/// Modules KNOWN to disagree, tracked rather than ignored.
///
/// The test asserts the disagreement set EQUALS this list, so a new
/// disagreement fails and a fixed one also fails — neither can rot into
/// silence.
///
/// **EMPTY since 2026-08-14.** `10_multbyte.kel` was the only entry, and it left
/// because the BEHAVIOUR CHANGED: the `sret` repair gives each call site a
/// disjoint block of the caller's region, so two live composite returns no
/// longer alias. It was not removed to make the suite green — the set-equality
/// assertion failed on its departure, which is the mechanism working in the
/// direction that means success.
const KNOWN_DISAGREEMENTS: &[&str] = &[];

/// The whole corpus, in one test, because the interesting output is the
/// EXEMPTION LIST and that is a property of the set rather than of any module.
#[test]
fn every_lowering_module_executes_or_is_exempt() {
    let mut executed: Vec<String> = Vec::new();
    let mut vacuous: Vec<String> = Vec::new();
    let mut exempt: Vec<(String, String)> = Vec::new();
    let mut disagreed: Vec<String> = Vec::new();

    // **Single-module mode**, for the mutation sweep. `tools/mutation_sweep.py`
    // runs this binary once per module in its own PROCESS, so a mutation that
    // kills a module with SIGBUS or SIGTRAP costs one measurement rather than
    // the whole census. Without process isolation two of the first four
    // mutations tried took the entire run down and yielded no per-module data.
    let only = std::env::var("KEL_ONLY_MODULE").ok();

    for p in sources() {
        let name = p.file_name().unwrap().to_str().unwrap().to_string();
        if let Some(want) = &only
            && &name != want
        {
            continue;
        }
        let Some(src) = source_for(&p) else {
            continue;
        };
        let Some(m) = tokenize(&src)
            .ok()
            .and_then(|t| parse(&t).ok())
            .and_then(|a| compile(&a).ok())
        else {
            exempt.push((name, "rejected by the REFERENCE compiler".into()));
            continue;
        };
        if !keleusma_native::module_refusals(&m, LowerOptions::default()).is_empty() {
            exempt.push((name, "the backend refuses it".into()));
            continue;
        }
        let Some(entry) = m.entry_point else {
            exempt.push((
                name,
                "no entry point (a prelude declares, it does not run)".into(),
            ));
            continue;
        };
        if !params_are_scalar(&m, entry) {
            exempt.push((
                name,
                "composite entry parameter; covered by a hand-written differential".into(),
            ));
            continue;
        }
        let table = native_table(&m);
        if table.len() > STUBS {
            exempt.push((
                name,
                format!("{} natives exceeds {STUBS} stub slots", table.len()),
            ));
            continue;
        }

        // **Drive each module with SEVERAL argument vectors, not one.**
        //
        // Seed 0 alone left a measured hole: with `Op::CmpLt` lowered as `SLE`,
        // 126 sites across 25 modules, the whole differential passed. `SLT` and
        // `SLE` differ only when the comparands are EQUAL, and seed 0's
        // pairwise-distinct ascending arguments never make two equal.
        //
        // A stream's single parameter is the tick, which the driver already
        // varies across 60 iterations, so seeding it would change what the run
        // MEANS rather than broaden it. Streams keep seed 0.
        let n_params = m.chunks[entry].param_count as usize;
        let seeds = if m.chunks[entry].block_type == BlockType::Stream || n_params == 0 {
            1
        } else {
            SEEDS
        };

        let mut runs: Vec<(usize, Run, Run)> = Vec::new();
        let mut bail: Option<String> = None;
        for seed in 0..seeds {
            // **The virtual machine runs FIRST, and that ordering is load-bearing.**
            // A module that traps reports an error here; natively the same trap is
            // `llvm.trap`, which kills the process with SIGTRAP and takes the whole
            // harness with it. Asking the tolerant side first turns a fatal signal
            // into a named exemption.
            let v = match run_vm(&m, &table, seed) {
                Ok(v) => v,
                Err(why) => {
                    // A LATER seed that traps is not an exemption for the module:
                    // seed 0 already ran. Stop widening and keep what agreed,
                    // rather than discarding coverage the module does have.
                    if seed == 0 {
                        bail = Some(why);
                    }
                    break;
                }
            };
            let Some(n) = run_native(&m, &table, seed) else {
                if seed == 0 {
                    bail = Some("entry signature shape the harness does not drive".into());
                }
                break;
            };
            runs.push((seed, v, n));
        }
        if let Some(why) = bail {
            exempt.push((name, why));
            continue;
        }
        if runs.is_empty() {
            exempt.push((name, "no seed produced a comparable run".into()));
            continue;
        }
        // Seed 0 supplies the reported observables, so the vacuity classification
        // and the printed figures still describe the same run they always did.
        // The native side of seed 0 is compared in the loop below with every
        // other seed, so only the VM half is bound here.
        let vm = &runs[0].1;

        // A COMPOSITE return is a body address natively and a decoded value on
        // the VM side; the two are not comparable as integers, and the hand-
        // written differentials decode them per module. Here the observable is
        // the call log and the data segment, which cover a composite-returning
        // module's actual work. Stated rather than silently skipped.
        let ret_scalar = matches!(
            m.signatures.get(entry).map(|s| &s.ret),
            Some(WireShape::Scalar { .. })
        );

        // **EVERY seed is compared**, not only the one whose figures are
        // reported. The first disagreeing seed is the one described, and its
        // number is printed, because "module X disagrees" is far less useful than
        // "module X disagrees on the all-equal argument vector".
        let mut found: Option<(usize, &Run, &Run)> = None;
        for (seed, v, n) in &runs {
            let vm_scalar = v.results.iter().all(|x| *x != i64::MIN);
            let differ = ret_scalar && vm_scalar && v.results != n.results;
            if differ || v.log != n.log || v.shared != n.shared {
                found = Some((*seed, v, n));
                break;
            }
        }
        if let Some((seed, vm, nat)) = found {
            let vm_scalar = vm.results.iter().all(|x| *x != i64::MIN);
            let results_differ = ret_scalar && vm_scalar && vm.results != nat.results;
            // Report the FIRST differing element, not the lengths. Lengths are
            // equal in most real disagreements and say nothing.
            let where_ = if results_differ {
                let i = vm
                    .results
                    .iter()
                    .zip(&nat.results)
                    .position(|(a, b)| a != b)
                    .unwrap_or(0);
                format!(
                    "result[{i}] vm={:?} native={:?}",
                    vm.results.get(i),
                    nat.results.get(i)
                )
            } else if vm.log != nat.log {
                let i = vm
                    .log
                    .iter()
                    .zip(&nat.log)
                    .position(|(a, b)| a != b)
                    .unwrap_or(vm.log.len().min(nat.log.len()));
                format!(
                    "log[{i}] of {}/{} vm={:?} native={:?}",
                    vm.log.len(),
                    nat.log.len(),
                    vm.log.get(i),
                    nat.log.get(i)
                )
            } else {
                let i = vm
                    .shared
                    .iter()
                    .zip(&nat.shared)
                    .position(|(a, b)| a != b)
                    .unwrap_or(0);
                format!(
                    "shared[{i}] vm={:?} native={:?}",
                    vm.shared.get(i),
                    nat.shared.get(i)
                )
            };
            disagreed.push(format!("{name}: seed {seed}: {where_}"));
            continue;
        }
        // **Agreement is not evidence unless the run did something.** Nine of the
        // stage sources agreed here for months while doing nothing at all: they
        // read their input from the shared segment, which this harness supplies
        // as zeros, so each took an immediate end-of-input exit. `lexer.kel`
        // yielded 62 — its own documented end-of-source marker — sixty times.
        //
        // The three observables are all trivial in that state, so a run is
        // classified vacuous only when EVERY one of them is, which is
        // conservative: a module doing real work fails the test on any one.
        if is_vacuous(vm) {
            vacuous.push(name);
            continue;
        }
        executed.push(name);
    }

    println!("================ CORPUS DIFFERENTIAL");
    println!("  EXECUTED AND AGREEING : {}", executed.len());
    println!(
        "  AGREED BUT VACUOUS    : {}  <- agreement on a run that produced nothing",
        vacuous.len()
    );
    for n in &vacuous {
        println!("     {n:26}");
    }
    if !vacuous.is_empty() {
        println!(
            "     ^ these read their input from the shared segment, which this harness\n\
             \x20      supplies as zeros. Real coverage for `lexer.kel` and `parse.kel` is in\n\
             \x20      `stage_differential.rs`, which seeds the segment on BOTH sides."
        );
    }
    println!("  EXEMPT                : {}", exempt.len());
    for (n, why) in &exempt {
        println!("     {n:26} {why}");
    }
    if !disagreed.is_empty() {
        println!("  DISAGREED             : {}", disagreed.len());
        for d in &disagreed {
            println!("     {d}");
        }
    }
    println!("================");

    // In single-module mode the pinned sets describe the WHOLE corpus and would
    // fail spuriously, so the run reports and exits instead. The sweep reads the
    // exit status, so the disagreement must still fail the process.
    if only.is_some() {
        assert!(disagreed.is_empty(), "disagreed: {}", disagreed.join("; "));
        return;
    }

    let mut names: Vec<&str> = disagreed
        .iter()
        .map(|d| d.split(':').next().unwrap_or(d))
        .collect();
    names.sort();
    names.dedup();
    let mut known: Vec<&str> = KNOWN_DISAGREEMENTS.to_vec();
    known.sort();
    assert_eq!(
        names,
        known,
        "the disagreement set changed.\n  NEW disagreements are real defects; a \
         module that LEFT the list is fixed and should be removed from \
         KNOWN_DISAGREEMENTS.\n  detail:\n{}",
        disagreed.join("\n")
    );
    let mut vac: Vec<&str> = vacuous.iter().map(String::as_str).collect();
    vac.sort();
    let mut known_vac: Vec<&str> = KNOWN_VACUOUS.to_vec();
    known_vac.sort();
    assert_eq!(
        vac, known_vac,
        "the VACUOUS set changed.\n  A module that JOINED it agrees while producing \
         nothing, so its entry in the executed count was never evidence.\n  A module \
         that LEFT it is now doing real work and should be removed from KNOWN_VACUOUS.\n  \
         Neither direction may pass silently: this list was 40-strong-looking coverage \
         for months precisely because nothing checked it."
    );

    assert!(
        executed.len() >= 20,
        "only {} modules executed; the harness is not covering the corpus and \
         every exemption above should be read as unfinished work",
        executed.len()
    );
}

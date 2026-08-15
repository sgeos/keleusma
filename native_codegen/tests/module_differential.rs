//! A WHOLE `piano_roll` module, executed natively and on the virtual machine.
//!
//! # Why this file exists, and what it replaces
//!
//! The family was previously "verified" by `module_refusals(...).is_empty()` —
//! that is, by `lower_module` returning `Ok`. That is a claim about the
//! COMPILER, not about the program: it says the lowering did not refuse, and
//! says nothing whatever about whether the code it emitted does what the virtual
//! machine does. Every construct had its own differential; the assembled module
//! had none.
//!
//! # The oracle, which is three things and not one
//!
//! For each of N ticks, both sides must agree on:
//!
//! 1. **the native call sequence** — `(name, args)` in order, and the decoded
//!    text for a string argument. These modules are almost entirely side
//!    effects, so this is the program's actual output.
//! 2. **the per-tick return value**.
//! 3. **the shared data segment**, byte for byte, at the end.
//!
//! Any one alone is weak. A lowering that dropped every native call still
//! returns the right integer and leaves the right bytes. A lowering that wrote a
//! data slot at the wrong offset returns the right integer and makes the right
//! calls. The `data state` block in these modules compiles to SHARED slots, so
//! (3) is the one that catches an addressing error.
//!
//! # Driving a degenerate stream
//!
//! `f(a), f(r1), f(r2)` reproduces the whole virtual-machine sequence with no
//! distinguished first call, so the native side calls the entry once per tick.
//! The virtual machine does `call_with_shared` once and `resume_with_shared`
//! thereafter, which is the same sequence expressed the other way round.
//!
//! # The entry is NOT chunk 0
//!
//! It is `kel_chunk_2`, `kel_chunk_21`, `kel_chunk_24`, `kel_chunk_4` and so on
//! across the family — chunk 0 for only two of the ten. Use `m.entry_point`.
use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::{Module, SlotVisibility, Value};
use keleusma::vm::{
    Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for, shared_data_bytes_for,
};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerOptions, lower_module};
use std::cell::RefCell;

mod common;

/// How many ticks to drive. `piano_roll_7` has section onsets at 256, 512, 768,
/// 1024, 1280, 1536 and 1792 and wraps at 2048, so a run must cross 2048 to
/// exercise the loop-boundary reset rather than only the init block. A shorter
/// run would pass while leaving every section-onset branch untaken.
const TICKS: i64 = 2100;

thread_local! {
    static LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn note(entry: String) {
    LOG.with(|l| l.borrow_mut().push(entry));
}

fn take_log() -> Vec<String> {
    LOG.with(|l| core::mem::take(&mut *l.borrow_mut()))
}

/// Decode the `{ i64 len, [n+1 x i8] bytes }` a static string global holds.
///
/// # Safety
/// `p` must be an address the lowering produced for a static string.
unsafe fn decode_static_str(p: i64) -> String {
    let base = p as *const u8;
    let len = unsafe { core::ptr::read_unaligned(base as *const i64) } as usize;
    let bytes = unsafe { core::slice::from_raw_parts(base.add(8), len) };
    String::from_utf8_lossy(bytes).into_owned()
}

// The fifteen `host::*` natives the family calls, exported under the names
// `native_symbol` mangles them to. Arities are DERIVED from the bytecode
// (`n & 0x7F` at every call site), not read off the source by eye; each of the
// fifteen has exactly one arity across all ten modules.
/// One rendering, used by BOTH sides. Comparing renderings rather than raw
/// operands is not laziness: a marshalled `String` and a pointer to a constant
/// global are not comparable as operands, and rendering is where the two become
/// the same question.
fn render(name: &str, args: &[String]) -> String {
    format!("{name}({})", args.join(", "))
}

macro_rules! host_native {
    ($sym:ident, $name:literal, $($arg:ident),+) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $sym($($arg: i64),+) -> i64 {
            note(render($name, &[$($arg.to_string()),+]));
            0
        }
    };
}

host_native!(kel_native_host__play, "host::play", a, b);
host_native!(kel_native_host__silence, "host::silence", a);
host_native!(kel_native_host__set_enable, "host::set_enable", a, b);
host_native!(kel_native_host__set_waveform, "host::set_waveform", a, b);
host_native!(kel_native_host__set_duty, "host::set_duty", a, b);
host_native!(kel_native_host__set_adsr, "host::set_adsr", a, b, c, d, e);
host_native!(kel_native_host__set_volume, "host::set_volume", a, b, c);
host_native!(kel_native_host__set_vibrato, "host::set_vibrato", a, b, c);
host_native!(kel_native_host__set_lpf, "host::set_lpf", a, b);
host_native!(kel_native_host__set_retrigger, "host::set_retrigger", a, b);
host_native!(kel_native_host__set_detune, "host::set_detune", a, b);
host_native!(kel_native_host__set_velocity, "host::set_velocity", a, b);
host_native!(
    kel_native_host__set_master_volume,
    "host::set_master_volume",
    a
);
host_native!(kel_native_host__set_bpm, "host::set_bpm", a);

/// The one string-taking native, which cannot use the macro: its argument is an
/// address rather than a value, and only the DECODED text is comparable with
/// what the virtual machine's marshalling hands its side.
#[unsafe(no_mangle)]
pub extern "C" fn kel_native_host__song_name(p: i64) -> i64 {
    let s = unsafe { decode_static_str(p) };
    note(render("host::song_name", &[format!("{s:?}")]));
    0
}

/// Register all fifteen through the arity-agnostic entry point.
///
/// `register_fn` marshals up to **four** arguments and `host::set_adsr` takes
/// five, so the typed helper cannot express the numeric natives at all.
///
/// `host::song_name` nonetheless goes through `register_fn`, and the split is
/// not arbitrary. The raw path hands the closure whatever the operand stack
/// holds, which for a string is a `KStr` — an ARENA HANDLE, rendering as
/// `KStr(KString(ArenaHandle { ptr: ... }))`, a value that differs between runs
/// and carries no text. The marshalling layer is what resolves a handle to its
/// contents, so the string native has to use it. Two paths, because the two
/// natives genuinely differ.
fn register_all(vm: &mut Vm<'_, '_>) {
    const NAMES: &[&str] = &[
        "host::play",
        "host::silence",
        "host::set_enable",
        "host::set_waveform",
        "host::set_duty",
        "host::set_adsr",
        "host::set_volume",
        "host::set_vibrato",
        "host::set_lpf",
        "host::set_retrigger",
        "host::set_detune",
        "host::set_velocity",
        "host::set_master_volume",
        "host::set_bpm",
        "host::song_name",
    ];
    for name in NAMES {
        if *name == "host::song_name" {
            continue;
        }
        let n = name.to_string();
        vm.register_native_closure(name, move |args: &[Value]| {
            let rendered: Vec<String> = args
                .iter()
                .map(|v| match v {
                    Value::Int(x) => x.to_string(),
                    Value::StaticStr(s) => format!("{s:?}"),
                    other => format!("{other:?}"),
                })
                .collect();
            note(render(&n, &rendered));
            Ok(Value::Int(0))
        });
    }
    vm.register_fn("host::song_name", |s: String| -> i64 {
        note(render("host::song_name", &[format!("{s:?}")]));
        0
    });
}

fn compile_module(path: &std::path::Path) -> Module {
    let src = std::fs::read_to_string(path).expect("read source");
    compile(&parse(&tokenize(&src).expect("lex")).expect("parse")).expect("compile")
}

/// Arena covering the operand stack, frames AND the persistent region.
///
/// `auto_arena_capacity_for` omits the persistent region, and a module with
/// private slots then computes an address inside a zero-length region and dies
/// with SIGBUS — which reads as a code-generation fault and is not one.
/// **Plus a host-side margin, which is required rather than defensive.**
/// `auto_arena_capacity_for` sizes the nominal stack; driving these modules for
/// thousands of ticks exhausted it on eight of the ten with
/// `OutOfArena("arena exhausted while growing the operand stack")`, and the
/// runtime's own message directs the host to add a margin. That is a property of
/// the virtual machine's harness, NOT of the lowering — the native side has a
/// fixed frame and never touches this arena.
fn arena_for(m: &Module) -> keleusma_arena::Arena {
    const HOST_MARGIN: usize = 1 << 20;
    let need = required_persistent_capacity_for(m);
    let cap = auto_arena_capacity_for(m, &[]).expect("arena capacity") + need + HOST_MARGIN;
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent fits");
    arena
}

fn private_slot_count(m: &Module) -> usize {
    m.data_layout
        .as_ref()
        .map(|dl| {
            dl.slots
                .iter()
                .filter(|s| s.visibility == SlotVisibility::Private)
                .count()
        })
        .unwrap_or(0)
}

/// Region bytes for the whole module, as the SUM over chunks.
///
/// `plan_chunk_region` places each chunk's sites from offset zero, so two chunks
/// name the same offsets. Summing over-allocates rather than assuming the
/// caller may overlap them: an over-allocation is safe, and if the emitter ever
/// does need disjoint per-chunk regions this is already large enough. The
/// canary below is what would catch a write beyond it.
fn region_bytes(m: &Module) -> usize {
    m.chunks
        .iter()
        .map(|c| keleusma_native::region::plan_chunk_region(c).bytes as usize)
        .sum()
}

/// Run the module on the virtual machine for `TICKS` ticks.
///
/// Returns `(per-tick results, call log, final shared bytes)`.
fn run_vm(m: &Module) -> (Vec<i64>, Vec<String>, Vec<u8>) {
    let _ = take_log();
    let n_shared = shared_data_bytes_for(m);
    let arena = arena_for(m);
    let mut vm = Vm::new(m.clone(), &arena).expect("vm");
    register_all(&mut vm);

    let mut shared = vec![0u8; n_shared];
    let mut out = Vec::new();
    let first = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("vm call");
    out.push(state_value(first));
    for t in 1..TICKS {
        // **One tick is a `Reset` leg followed by a `Yielded` leg**, and the
        // SAME reply must go to both. The `PopN(1); Reset` tail is walked by the
        // resume after the yield, so the runtime hands back `Reset` before the
        // next iteration's value. Supplying a fresh reply on the `Reset` leg
        // would silently discard it and desynchronise the two sides — the
        // existing `yield_sequence.rs` records exactly this.
        //
        // The native side has no counterpart to the Reset leg: the degenerate
        // lowering turns the whole envelope into one call per tick. That
        // asymmetry is the transformation being verified, not a defect.
        let mut st = vm
            .resume_with_shared(&mut shared, Value::Int(t))
            .expect("vm resume");
        if matches!(st, VmState::Reset) {
            st = vm
                .resume_with_shared(&mut shared, Value::Int(t))
                .expect("vm resume after reset");
        }
        out.push(state_value(st));
    }
    (out, take_log(), shared)
}

fn state_value(st: VmState) -> i64 {
    match st {
        VmState::Yielded(Value::Int(v)) | VmState::Finished(Value::Int(v)) => v,
        VmState::Yielded(Value::Unit) | VmState::Finished(Value::Unit) => 0,
        other => panic!("unexpected VM outcome: {other:?}"),
    }
}

/// Lower the module, JIT it, and drive the entry once per tick.
fn run_native(m: &Module) -> (Vec<i64>, Vec<String>, Vec<u8>) {
    let _ = take_log();
    let entry = m.entry_point.expect("entry point");
    let n_shared = shared_data_bytes_for(m);
    let n_priv = private_slot_count(m);
    let n_region = region_bytes(m);

    // Force the exported host symbols to be retained. Without a use the test
    // binary drops them, the execution engine then resolves the declaration to
    // nothing and jumps to it, and the result is SIGSEGV rather than a failed
    // assertion. This cost a cycle in `native_calls.rs` and another here.
    std::hint::black_box((
        kel_native_host__play as *const (),
        kel_native_host__silence as *const (),
        kel_native_host__set_enable as *const (),
        kel_native_host__set_waveform as *const (),
        kel_native_host__set_duty as *const (),
        kel_native_host__set_adsr as *const (),
        kel_native_host__set_volume as *const (),
        kel_native_host__set_vibrato as *const (),
        kel_native_host__set_lpf as *const (),
        kel_native_host__set_retrigger as *const (),
        kel_native_host__set_detune as *const (),
        kel_native_host__set_velocity as *const (),
        kel_native_host__set_master_volume as *const (),
        kel_native_host__set_bpm as *const (),
        kel_native_host__song_name as *const (),
    ));

    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_module(&ctx, &lm, m, LowerOptions::default()).expect("lower module");
    lm.verify().expect("LLVM module verification");
    common::maybe_optimize(&lm);
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");

    // A canary word past the end of each caller-provided buffer. A UNIFORM
    // offset error inside a region is value-invariant — reads and writes shift
    // together, so every round trip returns the same answer — and is invisible
    // to a value comparison. It is observable only as a write outside the
    // buffer.
    const CANARY: u64 = 0xDEAD_BEEF_FEED_FACE;
    let mut shared = vec![0u8; n_shared + 8];
    shared[n_shared..].copy_from_slice(&CANARY.to_le_bytes());
    // Word-allocated: the private region must be aligned to the private slot
    // width, and a `Vec<u8>` promises one byte of alignment.
    let mut privs = vec![0u64; n_priv + 1];
    privs[n_priv] = CANARY;
    let mut region = vec![0u64; n_region.div_ceil(8) + 1];
    let region_canary_at = n_region.div_ceil(8);
    region[region_canary_at] = CANARY;

    let sym = format!("kel_chunk_{entry}");
    // Assert the ABI before calling through it. A mismatch between the emitted
    // signature and the one this harness declares is undefined behaviour that
    // manifests as SIGSEGV inside JIT-compiled code, with no stack and no
    // indication of which side is wrong.
    let declared = lm
        .get_function(&sym)
        .expect("entry function")
        .count_params();
    assert_eq!(
        declared, 4,
        "the entry `{sym}` takes {declared} parameters; this harness passes one \
         tick plus the three trailing pointers"
    );
    let f = unsafe {
        ee.get_function::<unsafe extern "C" fn(i64, *mut u8, *mut u8, *mut u8) -> i64>(&sym)
    }
    .expect("entry symbol");

    let mut out = Vec::new();
    for t in 0..TICKS {
        out.push(unsafe {
            f.call(
                t,
                shared.as_mut_ptr(),
                privs.as_mut_ptr() as *mut u8,
                region.as_mut_ptr() as *mut u8,
            )
        });
    }

    assert_eq!(
        &shared[n_shared..],
        &CANARY.to_le_bytes(),
        "the lowering wrote past the {n_shared}-byte shared segment"
    );
    assert_eq!(
        privs[n_priv], CANARY,
        "the lowering wrote past the {n_priv} private slots the module declares"
    );
    assert_eq!(
        region[region_canary_at], CANARY,
        "the lowering wrote past the {n_region}-byte composite region"
    );

    shared.truncate(n_shared);
    (out, take_log(), shared)
}

fn assert_module_agrees(name: &str) {
    let path = std::path::Path::new("../examples/scripts/piano_roll").join(name);
    let m = compile_module(&path);

    let (nres, nlog, nshared) = run_native(&m);
    let (vres, vlog, vshared) = run_vm(&m);

    assert!(
        !nlog.is_empty(),
        "{name}: the call log is empty over {TICKS} ticks, so this test asserts \
         nothing about what the module DOES"
    );

    // Compare the call sequence first: it is the program's actual output, and a
    // disagreement here explains a value disagreement rather than the reverse.
    if vlog != nlog {
        let at = vlog
            .iter()
            .zip(nlog.iter())
            .position(|(a, b)| a != b)
            .unwrap_or(vlog.len().min(nlog.len()));
        panic!(
            "{name}: native call sequence diverges at index {at} of \
             {}/{} entries\n  vm     = {:?}\n  native = {:?}",
            vlog.len(),
            nlog.len(),
            vlog.get(at),
            nlog.get(at)
        );
    }
    assert_eq!(vres, nres, "{name}: per-tick return values disagree");

    // **Guard against the data-segment comparison being vacuous.** Two
    // all-zero buffers compare equal, so if nothing ever wrote a slot this
    // assertion would pass while checking nothing. These modules keep
    // `state.init` and per-channel indices, so a run that leaves the segment
    // untouched means the harness never executed the body.
    assert!(
        vshared.iter().any(|&b| b != 0),
        "{name}: the shared data segment is entirely zero after {TICKS} ticks, \
         so comparing it asserts nothing"
    );
    assert_eq!(
        vshared, nshared,
        "{name}: the shared data segment disagrees after {TICKS} ticks; a slot \
         written at the wrong offset returns the right value and makes the \
         right calls, so only this comparison sees it"
    );
}

macro_rules! module_test {
    ($fn_name:ident, $file:literal) => {
        #[test]
        fn $fn_name() {
            assert_module_agrees($file);
        }
    };
}

module_test!(piano_roll_0_agrees_with_the_vm, "piano_roll_0.kel");
module_test!(piano_roll_1_agrees_with_the_vm, "piano_roll_1.kel");
module_test!(piano_roll_2_agrees_with_the_vm, "piano_roll_2.kel");
module_test!(piano_roll_3_agrees_with_the_vm, "piano_roll_3.kel");
module_test!(piano_roll_4_agrees_with_the_vm, "piano_roll_4.kel");
module_test!(piano_roll_5_agrees_with_the_vm, "piano_roll_5.kel");
module_test!(piano_roll_6_agrees_with_the_vm, "piano_roll_6.kel");
module_test!(piano_roll_7_agrees_with_the_vm, "piano_roll_7.kel");
module_test!(piano_roll_8_agrees_with_the_vm, "piano_roll_8.kel");
module_test!(piano_roll_9_agrees_with_the_vm, "piano_roll_9.kel");

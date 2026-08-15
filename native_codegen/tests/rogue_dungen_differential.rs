//! `rogue_dungen.kel`, EXECUTED against the virtual machine.
//!
//! # Why this module needed a SOURCE change, not a backend one
//!
//! `random_in_room` builds a tuple from two `host::rng_range` results:
//!
//! ```text
//!   OP29 CallVerifiedNative(0, 2) -> SetLocal(5)
//!   OP36 CallVerifiedNative(0, 2) -> SetLocal(6)
//!   OP38 GetLocal(5) ; OP39 GetLocal(6)
//!   OP40 NewComposite(Flat { kind: Tuple, count: 2, byte_size: 64 })
//! ```
//!
//! The emitter now consults `Module::native_return_shapes`, but an undeclared
//! native records `WireShape::Top`, so its result still has **no width** and
//! `NewComposite` cannot pack it. That refusal is the fail-closed path working,
//! not a gap.
//!
//! The fix is to declare what the native already is:
//! `use host::rng_range(Word, Word) -> Word`, the form `examples/rtos/scripts/prelude.kel`
//! already uses throughout. **Declaring a type is not inventing an ABI** — an
//! earlier note here treated it as though it were, which was over-cautious.
//!
//! # The oracle
//!
//! `rogue_dungen` is a dungeon generator: its entire output is the sequence of
//! host calls that place rooms, corridors, monsters and items, plus the map
//! state it leaves behind. So both sides compare the **native call sequence**,
//! the **return value**, and the **shared data segment byte for byte**.
//!
//! `rng_range` is a deterministic stub rather than a random source, so the two
//! runs are comparable at all. That is a property of the harness; the module
//! sees an ordinary native either way.
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

thread_local! {
    static LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn note(s: String) {
    LOG.with(|l| l.borrow_mut().push(s));
}

fn take_log() -> Vec<String> {
    LOG.with(|l| core::mem::take(&mut *l.borrow_mut()))
}

/// Deterministic stand-in for a random source, identical on both sides.
///
/// Asymmetric in its two arguments on purpose: a swapped pair changes the
/// answer, and every room placement downstream with it.
fn rng(lo: i64, hi: i64) -> i64 {
    let span = (hi - lo).max(1);
    lo + ((lo * 7 + hi * 13 + 5) % span).abs()
}

macro_rules! host_native {
    ($sym:ident, $name:literal, $($arg:ident),*) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $sym($($arg: i64),*) -> i64 {
            let parts: Vec<String> = vec![$($arg.to_string()),*];
            note(format!("{}({})", $name, parts.join(", ")));
            0
        }
    };
}

host_native!(kel_native_host__clear_floor, "host::clear_floor",);
host_native!(kel_native_host__map_set, "host::map_set", a, b, c);
host_native!(kel_native_host__place_exit, "host::place_exit", a, b);
host_native!(kel_native_host__place_player, "host::place_player", a, b);
host_native!(kel_native_host__place_stairs, "host::place_stairs", a, b);
host_native!(kel_native_host__spawn_item, "host::spawn_item", a, b, c, d);
host_native!(
    kel_native_host__spawn_monster,
    "host::spawn_monster",
    a,
    b,
    c
);

/// The one native whose RESULT is consumed, so it both logs and computes.
#[unsafe(no_mangle)]
pub extern "C" fn kel_native_host__rng_range(lo: i64, hi: i64) -> i64 {
    note(format!("host::rng_range({lo}, {hi})"));
    rng(lo, hi)
}

fn register_all(vm: &mut Vm<'_, '_>) {
    for name in [
        "host::clear_floor",
        "host::map_set",
        "host::place_exit",
        "host::place_player",
        "host::place_stairs",
        "host::spawn_item",
        "host::spawn_monster",
    ] {
        let n = name.to_string();
        vm.register_native_closure(name, move |args: &[Value]| {
            let rendered: Vec<String> = args
                .iter()
                .map(|v| match v {
                    Value::Int(x) => x.to_string(),
                    other => format!("{other:?}"),
                })
                .collect();
            note(format!("{}({})", n, rendered.join(", ")));
            Ok(Value::Int(0))
        });
    }
    vm.register_native_closure("host::rng_range", |args: &[Value]| {
        let g = |i: usize| match args.get(i) {
            Some(Value::Int(x)) => *x,
            other => panic!("rng_range got {other:?}"),
        };
        let (lo, hi) = (g(0), g(1));
        note(format!("host::rng_range({lo}, {hi})"));
        Ok(Value::Int(rng(lo, hi)))
    });
}

fn module() -> Module {
    let src = std::fs::read_to_string("../examples/scripts/rogue/rogue_dungen.kel")
        .expect("read rogue_dungen.kel");
    compile(&parse(&tokenize(&src).expect("lex")).expect("parse")).expect("compile")
}

fn run_vm(m: &Module, floor: i64) -> (i64, Vec<String>, Vec<u8>) {
    let _ = take_log();
    const HOST_MARGIN: usize = 1 << 20;
    let need = required_persistent_capacity_for(m);
    let cap = auto_arena_capacity_for(m, &[]).expect("arena") + need + HOST_MARGIN;
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent fits");
    let mut vm = Vm::new(m.clone(), &arena).expect("vm");
    register_all(&mut vm);
    let mut shared = vec![0u8; shared_data_bytes_for(m)];
    let out = match vm
        .call_with_shared(&mut shared, &[Value::Int(floor)])
        .expect("vm run")
    {
        VmState::Finished(Value::Int(v)) | VmState::Yielded(Value::Int(v)) => v,
        VmState::Finished(Value::Unit) | VmState::Yielded(Value::Unit) => 0,
        other => panic!("unexpected VM outcome: {other:?}"),
    };
    (out, take_log(), shared)
}

fn run_native(m: &Module, floor: i64) -> (i64, Vec<String>, Vec<u8>) {
    let _ = take_log();
    // Retain the exported symbols; without a use the binary drops them and the
    // engine resolves the declaration to nothing and jumps to it.
    std::hint::black_box((
        kel_native_host__clear_floor as *const (),
        kel_native_host__map_set as *const (),
        kel_native_host__place_exit as *const (),
        kel_native_host__place_player as *const (),
        kel_native_host__place_stairs as *const (),
        kel_native_host__spawn_item as *const (),
        kel_native_host__spawn_monster as *const (),
        kel_native_host__rng_range as *const (),
    ));

    let entry = m.entry_point.expect("entry point");
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
    // **Transitive**, not the per-chunk sum. Each call site now receives a
    // disjoint block of the caller's region, so the entry needs everything it
    // can reach. The per-chunk sum under-counts and the canary would catch it.
    let n_region: usize = keleusma_native::region::region_total_bytes(m, entry, 0) as usize;

    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_module(&ctx, &lm, m, LowerOptions::default()).expect("lower module");
    lm.verify().expect("LLVM module verification");
    common::maybe_optimize(&lm);
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");

    const CANARY: u64 = 0xDEAD_BEEF_FEED_FACE;
    let mut shared = vec![0u8; n_shared + 8];
    shared[n_shared..].copy_from_slice(&CANARY.to_le_bytes());
    let mut privs = vec![0u64; n_priv + 1];
    privs[n_priv] = CANARY;
    let mut region = vec![0u64; n_region.div_ceil(8) + 1];
    let canary_at = n_region.div_ceil(8);
    region[canary_at] = CANARY;

    let sym = format!("kel_chunk_{entry}");
    let declared = lm.get_function(&sym).expect("entry fn").count_params();
    assert_eq!(
        declared, 4,
        "entry `{sym}` takes {declared} parameters; this harness passes the \
         floor plus the three trailing pointers"
    );
    let f = unsafe {
        ee.get_function::<unsafe extern "C" fn(i64, *mut u8, *mut u8, *mut u8) -> i64>(&sym)
    }
    .expect("entry symbol");
    let out = unsafe {
        f.call(
            floor,
            shared.as_mut_ptr(),
            privs.as_mut_ptr() as *mut u8,
            region.as_mut_ptr() as *mut u8,
        )
    };

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
    (out, take_log(), shared)
}

fn assert_agrees(floor: i64) {
    let m = module();
    assert!(
        keleusma_native::module_refusals(&m, LowerOptions::default()).is_empty(),
        "rogue_dungen must lower; if this fails the `use host::rng_range(Word, Word) -> Word` \
         signature was lost from the source and the native result has no width again"
    );

    let (nr, nlog, nshared) = run_native(&m, floor);
    let (vr, vlog, vshared) = run_vm(&m, floor);

    if vlog != nlog {
        let at = vlog
            .iter()
            .zip(nlog.iter())
            .position(|(a, b)| a != b)
            .unwrap_or(vlog.len().min(nlog.len()));
        panic!(
            "floor {floor}: the host call sequence diverges at index {at} of {}/{}\n  vm     = {:?}\n  native = {:?}",
            vlog.len(),
            nlog.len(),
            vlog.get(at),
            nlog.get(at)
        );
    }
    assert_eq!(vr, nr, "floor {floor}: return value disagrees");
    assert_eq!(
        vshared, nshared,
        "floor {floor}: the shared data segment disagrees; a slot written at the \
         wrong offset makes the right calls and returns the right value, so only \
         this comparison sees it"
    );

    // Vacuity guards. A generator that placed nothing would compare equal on
    // two empty logs and two zero segments.
    assert!(
        nlog.len() > 20,
        "only {} host calls logged; the generator cannot have run",
        nlog.len()
    );
    assert!(
        vshared.iter().any(|&b| b != 0),
        "the shared segment is entirely zero, so comparing it asserts nothing"
    );
}

#[test]
fn rogue_dungen_agrees_with_the_vm_on_floor_1() {
    assert_agrees(1);
}

#[test]
fn rogue_dungen_agrees_with_the_vm_on_a_deeper_floor() {
    assert_agrees(7);
}

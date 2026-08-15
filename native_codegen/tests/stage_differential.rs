//! **The self-hosted stages, driven on REAL input, native against the VM.**
//!
//! # Why this file exists
//!
//! `corpus_differential` counts nine of the ten stage sources among its
//! "executed and agreeing" modules. `probe_stage_vacuity` shows what that
//! agreement is worth: it drives every module with an **all-zero shared data
//! segment**, and `lexer.kel` documents that the host places the source in
//! `src.bytes` with its length in `src.len`. A zero segment is a source of
//! length zero, so the lexer yields `62` — its own end-of-source marker —
//! **sixty times**, and eight other stages yield a single repeated value.
//!
//! Both sides agreed. They agreed on doing nothing.
//!
//! # What is different here
//!
//! The shared segment is **seeded before the call, identically on both sides**,
//! and the segment is just bytes, so the same seeding function serves the VM and
//! the JIT. Under seeding the lexer produces ten distinct token codes instead of
//! one, which is the difference between exercising the emitter and not.
//!
//! # Why only two stages
//!
//! Each stage consumes a **different** shared block, and only `lexer.kel` takes
//! source text. `parse.kel` takes `toks.len` and `toks.packed[]`, which is the
//! lexer's own output — `lexer.kel`'s header states that "the lexer's output
//! stream IS the parser's input stream and the two stages can be composed with no
//! host-side adapter", so chaining them invents no format.
//!
//! The remaining eight consume abstract-syntax-tree and descriptor blocks whose
//! layouts are the `src/selfhost/mod.rs` driver's private business. Seeding those
//! would mean reproducing ten input formats from a file this line may read but
//! must not edit, and a seed the stage silently rejects would look exactly like
//! coverage. They are reported as still-vacuous rather than guessed at.
use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::{Module, SlotVisibility, Value};
use keleusma::vm::{
    Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for, shared_data_bytes_for,
};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerOptions, lower_module};

/// Enough iterations to scan the whole seed and reach end of source. One byte is
/// consumed per iteration, so this must exceed the seed length.
const TICKS: i64 = 400;

/// Real Keleusma source, chosen to span token classes: keywords, identifiers,
/// an integer literal, a two-byte operator, a line comment, and punctuation.
/// A seed of one class would agree for a reason nearly as thin as no seed.
const SEED: &str = "\
// a comment, which the scanner must skip
fn add(a: Word, b: Word) -> Word {
  let c = a + b;
  if c >= 10 { c } else { c * 2 }
}
";

fn arena_for(m: &Module) -> keleusma_arena::Arena {
    const HOST_MARGIN: usize = 4 << 20;
    let need = required_persistent_capacity_for(m);
    let cap = auto_arena_capacity_for(m, &[]).expect("arena capacity") + need + HOST_MARGIN;
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent fits");
    arena
}

/// Byte offset of the shared slot named `suffix`, or of its element zero.
///
/// An array slot is expanded to one slot per element (`src.bytes[0]`), which is
/// why a plain match on `bytes` finds nothing. `shared_layout` is parallel to the
/// SHARED-slot prefix of `slots`, so the index is counted among shared slots.
fn shared_offset(m: &Module, suffix: &str) -> Option<u32> {
    let dl = m.data_layout.as_ref()?;
    let scalar = format!(".{suffix}");
    let element0 = format!(".{suffix}[0]");
    let mut shared_ix = 0usize;
    for s in &dl.slots {
        if s.visibility != SlotVisibility::Shared {
            continue;
        }
        if s.name == suffix || s.name.ends_with(&scalar) || s.name.ends_with(&element0) {
            return dl.shared_layout.get(shared_ix).map(|l| l.offset);
        }
        shared_ix += 1;
    }
    None
}

/// Write `count` into the `len`-style slot and `body` into the array slot.
///
/// Operates on raw bytes, which is exactly why one function can seed both sides:
/// the VM and the JIT are handed the same buffer contents.
fn seed(m: &Module, buf: &mut [u8], len_slot: &str, array_slot: &str, body: &[i64], word: usize) {
    let len_off = shared_offset(m, len_slot).expect("len slot") as usize;
    let arr_off = shared_offset(m, array_slot).expect("array slot") as usize;
    assert!(
        len_off + 8 <= buf.len() && arr_off + body.len() * word <= buf.len(),
        "seed does not fit the {}-byte shared segment",
        buf.len()
    );
    buf[len_off..len_off + 8].copy_from_slice(&(body.len() as u64).to_le_bytes());
    for (i, v) in body.iter().enumerate() {
        let at = arr_off + i * word;
        match word {
            1 => buf[at] = *v as u8,
            8 => buf[at..at + 8].copy_from_slice(&v.to_le_bytes()),
            other => panic!("unsupported slot width {other}"),
        }
    }
}

struct Outcome {
    yields: Vec<i64>,
    shared: Vec<u8>,
}

fn run_vm(m: &Module, seeded: &[u8]) -> Outcome {
    let arena = arena_for(m);
    let mut vm = Vm::new(m.clone(), &arena).expect("vm loads");
    let mut shared = seeded.to_vec();
    let mut yields = Vec::new();

    // The first call is tick 0, matching the native driver's `f.call(0)`.
    let first = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("vm first call");
    yields.push(scalar_of(&first));
    for t in 1..TICKS {
        // **One tick is a `Reset` leg then a `Yielded` leg**, and the SAME reply
        // goes to both. Pushing only on the Yielded leg gives half as many
        // entries as the native side has calls, which presents as a length
        // mismatch rather than as a value difference.
        let mut st = vm
            .resume_with_shared(&mut shared, Value::Int(t))
            .expect("vm resume");
        if matches!(st, VmState::Reset) {
            st = vm
                .resume_with_shared(&mut shared, Value::Int(t))
                .expect("vm resume after reset");
        }
        yields.push(scalar_of(&st));
    }
    Outcome { yields, shared }
}

/// A scalar outcome, or a stable marker for anything else, so the two sides stay
/// index-aligned rather than silently shortening one of them.
fn scalar_of(st: &VmState) -> i64 {
    match st {
        VmState::Yielded(v) | VmState::Finished(v) => scalar(v),
        VmState::Reset => i64::MIN + 1,
        other => panic!("unexpected VM state: {other:?}"),
    }
}

fn scalar(v: &Value) -> i64 {
    match v {
        Value::Int(x) => *x,
        Value::Byte(b) => i64::from(*b),
        Value::Bool(b) => i64::from(*b),
        Value::Unit => 0,
        other => panic!("non-scalar yield {other:?}"),
    }
}

fn run_native(m: &Module, seeded: &[u8]) -> Outcome {
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
    let n_region = keleusma_native::region::region_total_bytes(m, entry, 0) as usize;

    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_module(&ctx, &lm, m, LowerOptions::default()).expect("lower module");
    lm.verify().expect("LLVM module verification");
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");

    const CANARY: u64 = 0xDEAD_BEEF_FEED_FACE;
    let mut shared = vec![0u8; n_shared + 8];
    shared[..n_shared].copy_from_slice(seeded);
    shared[n_shared..].copy_from_slice(&CANARY.to_le_bytes());
    let mut privs = vec![0u64; n_priv + 1];
    privs[n_priv] = CANARY;
    let mut region = vec![0u64; n_region.div_ceil(8) + 1];
    let canary_at = n_region.div_ceil(8);
    region[canary_at] = CANARY;

    let sym = format!("kel_chunk_{entry}");
    let fv = lm.get_function(&sym).expect("entry function");
    // **Assert the ABI before calling through it.** A wrong signature is
    // undefined behaviour that surfaces as SIGSEGV inside JIT code with no stack.
    assert_eq!(
        fv.count_params(),
        4,
        "entry `{sym}` takes {} parameters; this driver passes the tick plus the \
         three trailing pointers",
        fv.count_params()
    );
    let f = unsafe {
        ee.get_function::<unsafe extern "C" fn(i64, *mut u8, *mut u8, *mut u8) -> i64>(&sym)
    }
    .expect("entry symbol");

    let mut yields = Vec::new();
    for t in 0..TICKS {
        yields.push(unsafe {
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
        "wrote past the {n_shared}-byte shared segment"
    );
    assert_eq!(privs[n_priv], CANARY, "wrote past the private region");
    assert_eq!(
        region[canary_at], CANARY,
        "wrote past the {n_region}-byte composite region"
    );

    shared.truncate(n_shared);
    Outcome { yields, shared }
}

fn module_of(path: &str) -> Module {
    let src = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    compile(&parse(&tokenize(&src).expect("lex")).expect("parse")).expect("compile")
}

/// Compare, with the vacuity guard that is the entire point of this file.
fn compare(name: &str, m: &Module, seeded: &[u8], want_distinct: usize) {
    // The VM runs FIRST. A trapping module reports an error there; natively the
    // same trap is `llvm.trap`, which kills the process with SIGTRAP.
    let vm = run_vm(m, seeded);
    let nat = run_native(m, seeded);

    let mut d: Vec<i64> = vm.yields.clone();
    d.sort_unstable();
    d.dedup();
    assert!(
        d.len() >= want_distinct,
        "{name}: the seeded run produced only {} distinct yields ({d:?}); at least \
         {want_distinct} were expected. THIS IS THE VACUITY GUARD — a run that emits one \
         repeated value agrees for no reason, which is the defect this file exists to \
         close. Either the seed did not reach the stage or the stage exited early.",
        d.len()
    );

    if vm.yields != nat.yields {
        let at = vm
            .yields
            .iter()
            .zip(nat.yields.iter())
            .position(|(a, b)| a != b)
            .unwrap_or(vm.yields.len().min(nat.yields.len()));
        panic!(
            "{name}: the yield sequences diverge at tick {at} of {}/{}\n  vm     = {:?}\n  native = {:?}",
            vm.yields.len(),
            nat.yields.len(),
            vm.yields.get(at..at.saturating_add(8).min(vm.yields.len())),
            nat.yields
                .get(at..at.saturating_add(8).min(nat.yields.len())),
        );
    }
    assert_eq!(
        vm.shared, nat.shared,
        "{name}: the shared data segment disagrees. A slot written at the wrong offset \
         yields the right sequence, so only this comparison sees it."
    );
    assert!(
        vm.shared.iter().any(|&b| b != 0),
        "{name}: the shared segment is entirely zero, so comparing it asserts nothing"
    );
}

/// The lexer's token stream, with the two out-of-band markers removed.
///
/// `lexer.kel` documents 63 as PENDING (no complete token this iteration) and 62
/// as end of source. What remains is the parser's input, per that file's stated
/// composition property.
fn tokens_from(yields: &[i64]) -> Vec<i64> {
    let mut out = Vec::new();
    for y in yields {
        let tok = y & 0xFF;
        if tok == 63 {
            continue;
        }
        if tok == 62 {
            break;
        }
        out.push(*y);
    }
    out
}

#[test]
fn lexer_agrees_with_the_vm_on_real_source() {
    let m = module_of("../src/selfhost/kel/lexer.kel");
    assert!(
        keleusma_native::module_refusals(&m, LowerOptions::default()).is_empty(),
        "lexer.kel must lower for this differential to mean anything"
    );

    let mut seeded = vec![0u8; shared_data_bytes_for(&m)];
    let bytes: Vec<i64> = SEED.bytes().map(i64::from).collect();
    seed(&m, &mut seeded, "len", "bytes", &bytes, 1);

    // Ten distinct codes were measured on a shorter seed; this one is strictly
    // richer, so the floor is the honest one rather than the observed maximum.
    compare("lexer.kel", &m, &seeded, 10);
}

/// The control that gives the test above its meaning.
///
/// Without it, `lexer_agrees_with_the_vm_on_real_source` could pass with the seed
/// silently not arriving, which is precisely the state the whole corpus was in.
#[test]
fn without_the_seed_the_lexer_run_is_vacuous() {
    let m = module_of("../src/selfhost/kel/lexer.kel");
    let empty = vec![0u8; shared_data_bytes_for(&m)];
    let vm = run_vm(&m, &empty);

    let mut d: Vec<i64> = vm.yields.clone();
    d.sort_unstable();
    d.dedup();
    assert_eq!(
        d,
        vec![62],
        "on an all-zero shared segment the lexer is expected to yield ONLY 62, its \
         documented end-of-source marker. It yielded {d:?}. If this changed, the \
         vacuity finding recorded in `probe_stage_vacuity` and in the handoff has \
         moved and both need restating."
    );
}

#[test]
fn parser_agrees_with_the_vm_on_the_lexers_own_output() {
    let lex = module_of("../src/selfhost/kel/lexer.kel");
    let mut lex_shared = vec![0u8; shared_data_bytes_for(&lex)];
    let bytes: Vec<i64> = SEED.bytes().map(i64::from).collect();
    seed(&lex, &mut lex_shared, "len", "bytes", &bytes, 1);
    let toks = tokens_from(&run_vm(&lex, &lex_shared).yields);
    assert!(
        toks.len() > 15,
        "the lexer produced only {} tokens from the seed; the parser would then be \
         driven on nearly nothing and this differential would assert little",
        toks.len()
    );

    let m = module_of("../src/selfhost/kel/parse.kel");
    assert!(
        keleusma_native::module_refusals(&m, LowerOptions::default()).is_empty(),
        "parse.kel must lower for this differential to mean anything"
    );
    let mut seeded = vec![0u8; shared_data_bytes_for(&m)];
    seed(&m, &mut seeded, "len", "packed", &toks, 8);

    // The unseeded parser alternates 15 and -1, so two distinct values is the
    // vacuous baseline. Three is the minimum that proves the seed arrived.
    compare("parse.kel", &m, &seeded, 3);
}

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

mod common;

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
    common::maybe_optimize(&lm);
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

// ================= `verify_yield.kel`, the fourth stage seeded through this route
//
// **THE ONE-ARRAY `seed` HELPER WAS NOT WIDENED, DELIBERATELY.** It writes one
// length slot and one array slot, and `lexer.kel` and `parse.kel` depend on it;
// changing it would put their figures back in question for no gain. This stage
// needs four PARALLEL tables against a single `op_count`, so the writers below are
// ADDITIVE and nothing existing is touched.

/// Write one scalar shared slot by name.
fn seed_scalar(m: &Module, buf: &mut [u8], slot: &str, v: i64) {
    let off = shared_offset(m, slot).unwrap_or_else(|| panic!("no shared slot `{slot}`")) as usize;
    assert!(off + 8 <= buf.len(), "slot `{slot}` does not fit the shared segment");
    buf[off..off + 8].copy_from_slice(&v.to_le_bytes());
}

/// Read one scalar shared slot by name, so a verdict can be compared before and
/// after seeding rather than asserted from the yield sequence alone.
fn read_scalar(m: &Module, buf: &[u8], slot: &str) -> i64 {
    let off = shared_offset(m, slot).unwrap_or_else(|| panic!("no shared slot `{slot}`")) as usize;
    let mut b = [0u8; 8];
    b.copy_from_slice(&buf[off..off + 8]);
    i64::from_le_bytes(b)
}

/// Write one `[Word; N]` shared table by name. No length slot: this stage bounds
/// every table with the single `op_count`.
fn seed_table(m: &Module, buf: &mut [u8], slot: &str, body: &[i64]) {
    let off = shared_offset(m, slot).unwrap_or_else(|| panic!("no shared slot `{slot}`")) as usize;
    assert!(
        off + body.len() * 8 <= buf.len(),
        "table `{slot}` does not fit the shared segment"
    );
    for (i, v) in body.iter().enumerate() {
        buf[off + i * 8..off + i * 8 + 8].copy_from_slice(&v.to_le_bytes());
    }
}

/// Seed `verify_yield.kel` with a straight-line op region.
///
/// The `class` encoding is `analyze.kel`'s, stated in that file's header: `0`
/// plain, `1` If, `2` Else, `3` EndIf, `4` Loop, `5` EndLoop, `6` Break, `7`
/// BreakIf, `8` Trap. `verify_yield.kel` documents its own `class` as "as in
/// `analyze_class`", `mark` as 1 for a Yield, and `cay` as the
/// already-always-yielding fixpoint variable. **Read out of their sources, not
/// assumed from the other stages.**
fn seed_verify_yield(m: &Module, marks: &[i64]) -> Vec<u8> {
    let n = marks.len() as i64;
    let mut buf = vec![0u8; shared_data_bytes_for(m)];
    seed_scalar(m, &mut buf, "op_count", n);
    seed_scalar(m, &mut buf, "region_start", 0);
    seed_scalar(m, &mut buf, "region_end", n);
    seed_table(m, &mut buf, "class", &vec![0i64; marks.len()]);
    seed_table(m, &mut buf, "arg", &vec![0i64; marks.len()]);
    seed_table(m, &mut buf, "mark", marks);
    seed_table(m, &mut buf, "cay", &vec![0i64; marks.len()]);
    buf
}

/// **The unseeded baseline, measured rather than assumed.**
///
/// A verdict that the empty buffer already holds cannot demonstrate anything —
/// this is the trap the three `verify_*` stages fell into, where each wrote a
/// verdict the buffer already contained and was credited as seeded while its
/// observable never moved. So the baseline is READ, and the seeded run must
/// differ from it.
#[test]
fn the_unseeded_verify_yield_verdict_is_the_baseline_a_seed_must_move() {
    let m = module_of("../src/selfhost/kel/verify_yield.kel");
    let empty = vec![0u8; shared_data_bytes_for(&m)];
    let out = run_vm(&m, &empty).shared;
    assert_eq!(
        read_scalar(&m, &out, "out_hy"),
        0,
        "an all-zero segment describes an EMPTY region, which cannot contain a Yield. \
         If this is no longer 0, the baseline moved and the seeded assertion below is \
         measuring something else."
    );
}

#[test]
fn verify_yield_agrees_with_the_vm_on_a_region_containing_a_yield() {
    let m = module_of("../src/selfhost/kel/verify_yield.kel");
    assert!(
        keleusma_native::module_refusals(&m, LowerOptions::default()).is_empty(),
        "verify_yield.kel must lower for this differential to mean anything"
    );

    // Three plain ops, the middle one a Yield.
    let seeded = seed_verify_yield(&m, &[0, 1, 0]);
    let vm = run_vm(&m, &seeded);
    let nat = run_native(&m, &seeded);

    // **THE VERDICT MOVED.** This is the guard, not the distinct-yield count:
    // this stage's `main` is `loop main(resume) { yield run() }`, so it yields the
    // SAME fall-through flag every tick BY DESIGN. A distinct-yield criterion
    // would be measuring the harness, which is why `compare` is not used here.
    let hy = read_scalar(&m, &vm.shared, "out_hy");
    assert_eq!(
        hy, 1,
        "the seeded region carries a Yield (`mark` = 1) and `out_hy` is still {hy}. \
         Either the seed did not reach the stage or it exited before its fixpoint."
    );

    // **AND THE STAGE REACHED A VERDICT rather than stopping early.** `run()`
    // returns `yf.child_fell`, initialised to 1 and published to `out_fell`; a run
    // that never folded a frame would leave the two disagreeing.
    assert_eq!(
        vm.yields.first().copied(),
        Some(read_scalar(&m, &vm.shared, "out_fell")),
        "the yielded value must be the published `out_fell`; a disagreement means the \
         fixpoint loop did not complete"
    );

    assert_eq!(
        vm.yields, nat.yields,
        "verify_yield.kel: the yield sequences diverge\n  vm={:?}\n  native={:?}",
        vm.yields, nat.yields
    );
    assert_eq!(
        vm.shared, nat.shared,
        "verify_yield.kel: the shared segment disagrees. A slot written at the wrong \
         offset still yields the right sequence, so only this comparison sees it."
    );
}

/// The control: the same region WITHOUT a Yield must leave the verdict unmoved.
///
/// **The accepting direction belongs in a control, never in the driven set** —
/// recorded for the `verify_*` trio and applying unchanged here. Without this,
/// `out_hy == 1` above could be a constant rather than a response to the input.
#[test]
fn without_a_yield_mark_the_verify_yield_verdict_does_not_move() {
    let m = module_of("../src/selfhost/kel/verify_yield.kel");
    let seeded = seed_verify_yield(&m, &[0, 0, 0]);
    let hy = read_scalar(&m, &run_vm(&m, &seeded).shared, "out_hy");
    assert_eq!(
        hy, 0,
        "a region with no Yield reports out_hy = {hy}. If this is 1, the seeded \
         assertion above proves nothing, because the verdict would not be reading \
         `mark` at all."
    );
}

// ================= `analyze.kel`
//
// `shared data wa` carries EIGHT input scalars, TWELVE parallel `[Word; 1536]`
// tables, and FIVE outputs. The `class`/`opk` encodings are stated in that file's
// own header. `out_wcet` is `cost[stream_pos] + cost[reset_pos] + region_cost`,
// where the region is the frame pushed at `[region_start, region_end)`.

/// Seed `analyze.kel` with a straight-line region between a Stream and a Reset op.
fn seed_analyze(m: &Module, costs: &[i64], local_count: i64, slot_bytes: i64) -> Vec<u8> {
    let n = costs.len();
    assert!(n >= 3, "need a Stream op, at least one body op, and a Reset op");
    let mut buf = vec![0u8; shared_data_bytes_for(m)];
    let zeros = vec![0i64; n];
    for (slot, v) in [
        ("op_count", n as i64),
        ("stream_pos", 0),
        ("reset_pos", n as i64 - 1),
        ("region_start", 1),
        ("region_end", n as i64 - 1),
        ("local_count", local_count),
        ("value_slot_bytes", slot_bytes),
        ("arena_capacity", 4096),
    ] {
        seed_scalar(m, &mut buf, slot, v);
    }
    seed_table(m, &mut buf, "cost", costs);
    for t in [
        "class",
        "arg",
        "growth",
        "shrink",
        "heap",
        "opk",
        "slot",
        "cval",
        "cint",
        "callee_slots",
        "callee_heap",
    ] {
        seed_table(m, &mut buf, t, &zeros);
    }
    buf
}

/// **THE BASELINE, MEASURED — and it disqualifies the obvious observable.**
///
/// On an all-zero segment `analyze.kel` publishes **`out_valid = 1`**, not 0: an
/// empty region is trivially bounded. **So `out_valid` is a verdict the buffer
/// already holds**, and asserting it would be the exact trap the three `verify_*`
/// stages fell into — credited as seeded while the observable never moved.
///
/// `out_wcet` is the observable that moves, and it moves because it READS `cost`.
#[test]
fn the_unseeded_analyze_verdict_shows_which_observable_can_move() {
    let m = module_of("../src/selfhost/kel/analyze.kel");
    let base = run_vm(&m, &vec![0u8; shared_data_bytes_for(&m)]).shared;
    assert_eq!(
        read_scalar(&m, &base, "out_valid"),
        1,
        "an empty region is trivially bounded, so out_valid is expected to be 1 \
         ALREADY. If this is 0, out_valid became a usable observable and the \
         reasoning below should be revisited rather than left stale."
    );
    assert_eq!(
        read_scalar(&m, &base, "out_wcet"),
        0,
        "with every cost zero and an empty region the bound must be 0; this is the \
         baseline the seeded run has to move"
    );
}

#[test]
fn analyze_agrees_with_the_vm_on_a_bounded_straight_line_region() {
    let m = module_of("../src/selfhost/kel/analyze.kel");
    assert!(
        keleusma_native::module_refusals(&m, LowerOptions::default()).is_empty(),
        "analyze.kel must lower for this differential to mean anything"
    );

    // Stream at 0, Reset at 5, body [1, 5). `out_wcet` is
    // `cost[stream_pos] + cost[reset_pos] + region_cost`.
    let costs = [2i64, 3, 5, 7, 11, 13];
    let seeded = seed_analyze(&m, &costs, 2, 8);
    let vm = run_vm(&m, &seeded);
    let nat = run_native(&m, &seeded);

    // **THE ARITHMETIC IS CHECKED, NOT JUST THE MOVEMENT.** A non-zero bound would
    // prove the stage ran; the exact sum proves it read the table it claims to.
    assert_eq!(
        read_scalar(&m, &vm.shared, "out_wcet"),
        2 + 13 + (3 + 5 + 7 + 11),
        "out_wcet must be cost[stream_pos] + cost[reset_pos] + the region cost"
    );
    // `(local_count + region peak) * value_slot_bytes` = (2 + 0) * 8.
    assert_eq!(
        read_scalar(&m, &vm.shared, "out_stack_bytes"),
        16,
        "out_stack_bytes must be (local_count + peak) * value_slot_bytes"
    );
    assert_eq!(read_scalar(&m, &vm.shared, "out_reject"), 0, "this region is bounded");

    assert_eq!(
        vm.yields, nat.yields,
        "analyze.kel: the yield sequences diverge\n  vm={:?}\n  native={:?}",
        vm.yields, nat.yields
    );
    assert_eq!(
        vm.shared, nat.shared,
        "analyze.kel: the shared segment disagrees. A slot written at the wrong \
         offset still yields the right sequence, so only this comparison sees it."
    );
}

/// The control: the same region shape with every cost zero must leave `out_wcet`
/// at 0.
///
/// Without this, the seeded assertion could be satisfied by a stage that returns a
/// constant, or by a seed landing at the wrong offset and being read back from the
/// same wrong offset.
#[test]
fn with_zero_costs_the_analyze_bound_stays_zero() {
    let m = module_of("../src/selfhost/kel/analyze.kel");
    let seeded = seed_analyze(&m, &[0, 0, 0, 0, 0, 0], 2, 8);
    let out = run_vm(&m, &seeded).shared;
    assert_eq!(
        read_scalar(&m, &out, "out_wcet"),
        0,
        "a region whose every op costs zero must bound to zero; a non-zero result \
         here would mean out_wcet is not reading `cost`"
    );
}

// ================= `codegen.kel`, seeded by CHAINING reconstruct's output
//
// **THIS STAGE IS NOT MORE OF THE SAME, and the difference is the point.**
//
// | | analyze / verify_yield | codegen |
// |---|---|---|
// | entry | `loop main(resume) { yield run() }` | `loop main(resume) { yield emit_next(resume) }` |
// | per tick | the WHOLE fixpoint, once | ONE emission, resume-driven |
// | input | flat parallel op tables | an AST |
//
// So the tick budget is **load-bearing here** where it was irrelevant twice.
//
// **AND THE AST DOES NOT NEED TO BE HAND-BUILT.** Measured 2026-08-26:
// `reconstruct.kel`'s shared block contains exactly the ten AST slots
// `codegen.kel` consumes -- `root`, `kinds`, `args`, `lhs`, `rhs`, `call_args`,
// `for_parts`, `match_parts`, `limit_parts`, `head_parts` -- at identical widths,
// plus `out_param_count`/`out_category` answering to codegen's
// `param_count`/`category`. **So the natural seed for codegen is reconstruct's
// OUTPUT**, chained exactly as `parse.kel` is driven from the lexer's own tokens.
// Hand-building a structure with invariants was the risk this avoids entirely.

/// Copy one shared slot of `words` words from one module's segment to another's,
/// resolving the name independently on each side.
fn copy_slot(
    src_m: &Module,
    src: &[u8],
    src_slot: &str,
    dst_m: &Module,
    dst: &mut [u8],
    dst_slot: &str,
    words: usize,
) {
    let s = shared_offset(src_m, src_slot).unwrap_or_else(|| panic!("src `{src_slot}`")) as usize;
    let d = shared_offset(dst_m, dst_slot).unwrap_or_else(|| panic!("dst `{dst_slot}`")) as usize;
    let n = words * 8;
    assert!(s + n <= src.len() && d + n <= dst.len(), "slot `{src_slot}` out of range");
    dst[d..d + n].copy_from_slice(&src[s..s + n]);
}

/// The ten AST tables plus the two scalars, with their widths and their names on
/// each side. Widths read from both `shared data` blocks and verified equal.
const AST_BRIDGE: &[(&str, &str, usize)] = &[
    ("root", "root", 1),
    ("kinds", "kinds", 1024),
    ("args", "args", 1024),
    ("lhs", "lhs", 1024),
    ("rhs", "rhs", 1024),
    ("call_args", "call_args", 256),
    ("for_parts", "for_parts", 256),
    ("match_parts", "match_parts", 256),
    ("limit_parts", "limit_parts", 256),
    ("head_parts", "head_parts", 256),
    ("out_param_count", "param_count", 1),
    ("out_category", "category", 1),
];

#[test]
fn codegen_agrees_with_the_vm_on_reconstructs_own_output() {
    // --- Stage 1: drive reconstruct.kel on a real parsed head ---
    let src = std::fs::read_to_string("../examples/scripts/11_signed.kel")
        .expect("read 11_signed.kel");
    let (fns, _names, _, _) = keleusma::selfhost::parse_functions(&src);
    assert_eq!(
        fns.len(),
        1,
        "11_signed.kel is expected to parse to exactly one head; it parsed to {}. \
         If the parser's view of this file changed, pick a subject by PROPERTY and \
         report it rather than adjusting this number.",
        fns.len()
    );
    let h = &fns[0];

    let rec = keleusma::selfhost::reconstruct_kel_module();
    let rec_arena = arena_for(&rec);
    let rec_vm = Vm::new(rec.clone(), &rec_arena).expect("reconstruct vm");
    let rec_seed = keleusma::selfhost::seed_reconstruct_shared(
        &rec_vm,
        h.body_records(),
        h.reconstruct_category(),
        h.param_count(),
    );
    drop(rec_vm);

    let rec_out = run_vm(&rec, &rec_seed);
    let nodes = rec_out.yields.first().copied().unwrap_or(i64::MIN);
    assert!(
        nodes > 0,
        "reconstruct.kel did not reconstruct 11_signed.kel: it yielded {nodes}. A \
         value at or below -901 is a refusal tag, `rc_fail_base() - code`; the \
         decoder lives in probe_stage_vacuity and is deliberately NOT duplicated \
         here. Two of three qualifying single-head subjects are already refused \
         with rc_range_arity; if this one joined them, this chain has no subject \
         and that is the finding."
    );

    // --- Stage 2: bridge the AST into codegen.kel ---
    let cg = module_of("../src/selfhost/kel/codegen.kel");
    assert!(
        keleusma_native::module_refusals(&cg, LowerOptions::default()).is_empty(),
        "codegen.kel must lower for this differential to mean anything"
    );
    let mut cg_seed = vec![0u8; shared_data_bytes_for(&cg)];
    for (from, to, words) in AST_BRIDGE {
        copy_slot(&rec, &rec_out.shared, from, &cg, &mut cg_seed, to, *words);
    }

    // **THE BRIDGE MUST HAVE CARRIED SOMETHING.** An all-zero AST is an empty
    // program, which codegen would traverse to no effect while agreeing perfectly.
    assert!(
        cg_seed.iter().any(|&b| b != 0),
        "the bridged AST is entirely zero, so driving codegen on it would assert \
         nothing"
    );

    // --- Stage 3: the differential ---
    let vm = run_vm(&cg, &cg_seed);
    let nat = run_native(&cg, &cg_seed);

    // **THE VACUITY GUARD, and it is the distinct-yield kind here** -- unlike the
    // two stages seeded before this one, `codegen.kel` IS a stream: it emits one
    // thing per tick, so a constant sequence means it is not emitting.
    let mut d: Vec<i64> = vm.yields.clone();
    d.sort_unstable();
    d.dedup();
    println!("\n================ codegen.kel on reconstruct's AST");
    println!("  reconstruct yielded {nodes} node(s)");
    println!("  codegen ticks       {}", vm.yields.len());
    println!("  distinct yields     {}", d.len());
    println!("================\n");
    // **THREE, NOT TWO, AND THE CONTROL BELOW IS WHY.** An all-zero AST already
    // produces TWO distinct values, so a `>= 2` guard is satisfied by the empty
    // program. That is not a hypothetical: this test was first written with `>= 2`
    // and the control failed on the first run, which is exactly what a control is
    // for. Same shape as the `parse.kel` guard above, whose unseeded baseline is
    // also two. Seeded, this run yields SIX.
    assert!(
        d.len() >= 3,
        "codegen.kel produced only {} distinct value(s) across {} ticks ({d:?}). \
         TWO is the vacuous baseline an all-zero AST reaches, so three is the \
         minimum that proves the bridged AST arrived. Either it did not reach the \
         stage or the stage exited before emitting.",
        d.len(),
        vm.yields.len()
    );

    assert_eq!(
        vm.yields, nat.yields,
        "codegen.kel: the yield sequences diverge\n  vm={:?}\n  native={:?}",
        vm.yields, nat.yields
    );
    assert_eq!(
        vm.shared, nat.shared,
        "codegen.kel: the shared segment disagrees. A slot written at the wrong \
         offset still yields the right sequence, so only this comparison sees it."
    );
}

/// The control that pins the vacuous baseline the seeded guard must clear.
///
/// **THIS FAILED ON ITS FIRST RUN AND THAT IS WHY IT EXISTS.** The seeded test was
/// written asserting `>= 2` distinct yields; this control measured the unseeded
/// stage at **exactly 2**, proving the seeded assertion was satisfiable by an
/// empty program. The seeded threshold is now 3.
///
/// **Pinned as an EQUALITY.** If codegen's unseeded behaviour changes, the seeded
/// threshold is no longer known to clear it, and this must fail and say so rather
/// than quietly still passing at some lower count.
#[test]
fn the_unseeded_codegen_baseline_is_what_the_seeded_guard_must_clear() {
    let cg = module_of("../src/selfhost/kel/codegen.kel");
    let empty = vec![0u8; shared_data_bytes_for(&cg)];
    let mut d: Vec<i64> = run_vm(&cg, &empty).yields;
    d.sort_unstable();
    d.dedup();
    assert_eq!(
        d.len(),
        2,
        "on an all-zero AST codegen.kel produces {} distinct values ({d:?}), not the \
         2 the seeded guard's threshold of 3 was derived from. Re-derive that \
         threshold rather than adjusting this number to match.",
        d.len()
    );
}


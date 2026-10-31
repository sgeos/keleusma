//! **The self-hosted stages, driven on REAL input, native against the VM.**
//!
//! # Why this file exists
//!
//! `corpus_differential` counts most of the TWELVE stage sources among its
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

/// One host turn: the shared-slot writes the host makes, then the reply it sends.
///
/// The tick drivers below are the special case with no writes and the tick as
/// the reply. A command-driven stage such as `wire.kel` needs the general form,
/// because its host sets arguments in shared slots before each command.
struct Step {
    /// Word slots written by name before the reply.
    words: Vec<(String, i64)>,
    /// Bytes written into the `bytes` array before the reply, by element index.
    bytes: Vec<(usize, u8)>,
    /// Word tables written from element zero before the reply.
    tables: Vec<(String, Vec<i64>)>,
    /// Byte arrays other than `bytes`, written from element zero.
    byte_tables: Vec<(String, Vec<u8>)>,
    reply: i64,
}

fn tick_script() -> Vec<Step> {
    (0..TICKS)
        .map(|t| Step {
            words: Vec::new(),
            bytes: Vec::new(),
            tables: Vec::new(),
            byte_tables: Vec::new(),
            reply: t,
        })
        .collect()
}

/// Apply a step's host writes. ONE function serves both sides, so the VM and the
/// JIT see identical bytes, and the comparison is of two lowerings.
fn apply_writes(m: &Module, buf: &mut [u8], step: &Step) {
    for (slot, v) in &step.words {
        seed_scalar(m, buf, slot, *v);
    }
    for (slot, body) in &step.tables {
        seed_table(m, buf, slot, body);
    }
    for (slot, body) in &step.byte_tables {
        let base =
            shared_offset(m, slot).unwrap_or_else(|| panic!("no byte array `{slot}`")) as usize;
        buf[base..base + body.len()].copy_from_slice(body);
    }
    if !step.bytes.is_empty() {
        let base = shared_offset(m, "bytes").expect("a `bytes` array slot") as usize;
        for (i, b) in &step.bytes {
            buf[base + i] = *b;
        }
    }
}

fn run_vm(m: &Module, seeded: &[u8]) -> Outcome {
    run_vm_script(m, seeded, &tick_script())
}

fn run_vm_script(m: &Module, seeded: &[u8], script: &[Step]) -> Outcome {
    let arena = arena_for(m);
    let mut vm = Vm::new(m.clone(), &arena).expect("vm loads");
    let mut shared = seeded.to_vec();
    let mut yields = Vec::new();

    for (i, step) in script.iter().enumerate() {
        apply_writes(m, &mut shared, step);
        let reply = Value::Int(step.reply);
        // The first turn calls, matching the native driver's first `f.call`.
        if i == 0 {
            let first = vm
                .call_with_shared(&mut shared, &[reply])
                .expect("vm first call");
            yields.push(scalar_of(&first));
            continue;
        }
        // **One tick is a `Reset` leg then a `Yielded` leg**, and the SAME reply
        // goes to both. Pushing only on the Yielded leg gives half as many
        // entries as the native side has calls, which presents as a length
        // mismatch rather than as a value difference.
        let mut st = vm
            .resume_with_shared(&mut shared, reply.clone())
            .unwrap_or_else(|e| panic!("vm resume at turn {i}, reply {}: {e:?}", step.reply));
        if matches!(st, VmState::Reset) {
            st = vm
                .resume_with_shared(&mut shared, reply)
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
    run_native_script(m, seeded, &tick_script())
}

fn run_native_script(m: &Module, seeded: &[u8], script: &[Step]) -> Outcome {
    let (yields, shared) = with_native(m, seeded, |turn| {
        script.iter().map(|step| turn(step, None)).collect()
    });
    Outcome { yields, shared }
}

/// A native turn: apply the step's writes, call once, and optionally copy the
/// shared segment out afterwards.
type NativeTurn<'a> = dyn FnMut(&Step, Option<&mut Vec<u8>>) -> i64 + 'a;

/// Lower `m`, then hand `body` a turn function over the native entry. The
/// buffers are sized by the published contract and checked by canaries after
/// `body` returns. Returns `body`'s result and the final shared segment.
fn with_native<R>(
    m: &Module,
    seeded: &[u8],
    body: impl FnOnce(&mut NativeTurn<'_>) -> R,
) -> (R, Vec<u8>) {
    let entry = m.entry_point.expect("entry point");
    let n_shared = shared_data_bytes_for(m);
    // **THE PRIVATE REGION IS SIZED BY THE PUBLISHED CONTRACT.**
    //
    // It was the private SLOT COUNT — one word per slot — which is narrower than
    // what the backend is entitled to write: the contract is
    // `required_persistent_capacity_for` plus the supplement, covering the slot
    // array, the persistent composite pool and the stream resume-state word.
    //
    // **The third harness in this package to carry that defect.** The corpus one
    // was caught by its canary; the two-argument one produced a SIGSEGV. This one
    // masks nothing today — measured, not assumed: every stage here lowers as a
    // DEGENERATE stream, so no resume-state word exists, and no corpus module has
    // a private composite slot. Repaired because the next stage that gains either
    // would find the buffer four times too small. For `parse.kel` the slot array
    // is 173216 B against a 692872 B contract.
    let n_priv = (required_persistent_capacity_for(m)
        + keleusma_native::region::persistent_supplement_bytes(m) as usize)
        .div_ceil(8);
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
    // **The host's obligation, exercised.** No corpus module declares a non-zero
    // initializer today, so this installs zeros — and a harness that omitted it
    // would compare a backend that never applies one against a runtime that
    // always does, the moment a stage grew one.
    common::install_private_init(m, &mut privs[..n_priv]);
    privs[n_priv] = CANARY;
    const REGION_FILL: u64 = 0x5A5A_5A5A_5A5A_5A5A;
    let mut region = vec![REGION_FILL; n_region.div_ceil(8) + 1];
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

    let result = body(&mut |step: &Step, out: Option<&mut Vec<u8>>| {
        apply_writes(m, &mut shared[..n_shared], step);
        let y = unsafe {
            f.call(
                step.reply,
                shared.as_mut_ptr(),
                privs.as_mut_ptr() as *mut u8,
                region.as_mut_ptr() as *mut u8,
            )
        };
        if let Some(out) = out {
            out.clear();
            out.extend_from_slice(&shared[..n_shared]);
        }
        y
    });

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

    let region_extent = region[..canary_at]
        .iter()
        .rposition(|&w| w != REGION_FILL)
        .map_or(0, |wi| {
            let bytes = region[wi].to_ne_bytes();
            let fill = REGION_FILL.to_ne_bytes();
            let last = bytes
                .iter()
                .zip(fill.iter())
                .rposition(|(b, f)| b != f)
                .unwrap_or(7);
            wi * 8 + last + 1
        });

    // **THE STAGES TOUCH NONE OF THEIR PLANNED ARENA REGION.**
    //
    // Measured 2026-09-17 by filling the region with a pattern and taking the
    // highest index that no longer holds it: **zero bytes**, across all five
    // native stage drives here, against plans of 520 and 600 bytes. They lower as
    // degenerate streams with no composite sites, so the entire published figure
    // is reservation -- `stream_spill_bytes` plus a locals block neither of which
    // is written.
    //
    // `region_composition.rs` reaches the same conclusion statically, from the
    // planner's terms. **This is the dynamic half**, and it is stronger: the
    // static census says the spill term is 98% of the figure for these modules,
    // while this says the whole figure goes untouched.
    //
    // **REACH PROVEN, not assumed**: writing a single byte at offset 17 of the
    // region makes this report 18. A zero that could not become non-zero would be
    // an instrument defect wearing a finding's clothes, and this package has
    // found that shape more than once.
    assert_eq!(
        region_extent, 0,
        "a stage touched {region_extent} byte(s) of its {n_region}-byte planned \
         arena region, where every stage measured touched NONE. That is a real \
         change rather than noise: say which construct the stage gained, whether \
         it still lowers as a degenerate stream, and whether the plan still covers \
         what it now writes."
    );

    shared.truncate(n_shared);
    (result, shared)
}

/// Run both sides one turn at a time. The host closure decides each step from
/// the state the virtual machine leaves, as the driver does, and every turn
/// asserts that the yields and the whole shared segments agree. Returns the
/// host's result.
fn lockstep<R>(
    m: &Module,
    seeded: &[u8],
    host: impl FnOnce(&mut dyn FnMut(&Step) -> (i64, Vec<u8>)) -> R,
) -> (R, usize) {
    let arena = arena_for(m);
    let mut vm = Vm::new(m.clone(), &arena).expect("vm loads");
    let mut vm_shared = seeded.to_vec();
    let mut turns = 0usize;
    let mut native_shared = Vec::new();
    let (result, _) = with_native(m, seeded, |nturn| {
        host(&mut |step: &Step| {
            let yv = vm_ask(m, &mut vm, &mut vm_shared, &mut (turns > 0), step);
            let yn = nturn(step, Some(&mut native_shared));
            assert_eq!(
                yv, yn,
                "turn {turns}, reply {}: the virtual machine yielded {yv} and native {yn}",
                step.reply
            );
            assert!(
                vm_shared == native_shared,
                "turn {turns}, reply {}: the shared segments disagree",
                step.reply
            );
            turns += 1;
            (yv, vm_shared.clone())
        })
    });
    (result, turns)
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
    assert!(
        off + 8 <= buf.len(),
        "slot `{slot}` does not fit the shared segment"
    );
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
    assert!(
        n >= 3,
        "need a Stream op, at least one body op, and a Reset op"
    );
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
    assert_eq!(
        read_scalar(&m, &vm.shared, "out_reject"),
        0,
        "this region is bounded"
    );

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
    assert!(
        s + n <= src.len() && d + n <= dst.len(),
        "slot `{src_slot}` out of range"
    );
    dst[d..d + n].copy_from_slice(&src[s..s + n]);
}

/// The ten AST tables plus the two scalars, by their names on each side.
///
/// **The widths are read from both `shared data` blocks, not written here.**
/// Until 2026-10-28 this table carried them as constants, 1,024 nodes and 256
/// parts, and its comment said they were verified equal. The capacity raise of
/// 2026-10-26 took both stages to 4,096 and 1,024, so the bridge copied a prefix
/// while still claiming equal widths. The subject is small enough to fit the
/// prefix, so nothing failed. [`slot_words`] now measures each side, and the
/// copy asserts the two agree.
const AST_BRIDGE: &[(&str, &str)] = &[
    ("root", "root"),
    ("kinds", "kinds"),
    ("args", "args"),
    ("lhs", "lhs"),
    ("rhs", "rhs"),
    ("call_args", "call_args"),
    ("for_parts", "for_parts"),
    ("match_parts", "match_parts"),
    ("limit_parts", "limit_parts"),
    ("head_parts", "head_parts"),
    ("out_param_count", "param_count"),
    ("out_category", "category"),
];

/// The number of word elements in the shared slot named `suffix`: one for a
/// scalar, the element count for an array, which the layout expands to one
/// slot per element.
fn slot_words(m: &Module, suffix: &str) -> usize {
    let dl = m.data_layout.as_ref().expect("a data layout");
    let scalar = format!(".{suffix}");
    let element = format!(".{suffix}[");
    dl.slots
        .iter()
        .filter(|s| s.visibility == SlotVisibility::Shared)
        .filter(|s| s.name == suffix || s.name.ends_with(&scalar) || s.name.contains(&element))
        .count()
}

#[test]
fn codegen_agrees_with_the_vm_on_reconstructs_own_output() {
    // --- Stage 1: drive reconstruct.kel on a real parsed head ---
    // **THE SUBJECT IS `11_signed.kel` WITHOUT ITS `signed` MODIFIER.** In
    // absorption 82 upstream's parser began refusing signed declarations by name,
    // so the file itself no longer parses, and no corpus file parses to a single
    // head any more; `probe_stage_vacuity.rs` pins that zero. This is the same
    // program, so its body records are the ones this chain always drove.
    let src = "fn main() -> Word {\n    21 + 21\n}\n";
    let fns = keleusma::selfhost::try_parse_functions(src)
        .expect("the subject is inside the parse.kel subset")
        .functions;
    assert_eq!(
        fns.len(),
        1,
        "the subject is expected to parse to exactly one head; it parsed to {}. \
         If the parser's view of it changed, pick a subject by PROPERTY and \
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
    // REACH: the measurement sees the whole table, the driver's own capacity.
    assert_eq!(
        slot_words(&rec, "kinds"),
        keleusma::selfhost::AST_NODE_CAP,
        "the node table's measured width is not the driver's node capacity"
    );
    for (from, to) in AST_BRIDGE {
        let words = slot_words(&rec, from);
        assert!(words > 0, "reconstruct.kel has no shared slot `{from}`");
        assert_eq!(
            words,
            slot_words(&cg, to),
            "`{from}` and `{to}` have different widths, so the bridge would truncate"
        );
        copy_slot(&rec, &rec_out.shared, from, &cg, &mut cg_seed, to, words);
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

// ================= `wire.kel`, DRIVEN BY COMMANDS ON REAL ARTIFACTS
//
// `corpus_differential` drives every stream with the tick as its reply, which
// for `wire.kel` means commands 0, 1, 2 and onward in order over an empty
// buffer. The emitters among them rewrite the buffer with an empty header, and
// command 19 then reads a name table that does not exist. The virtual machine
// faults there with `IndexOutOfBounds`, so that harness can only exempt the
// stage as fault-comparable. The fault belongs to the input order, not to
// either lowering.
//
// This drive gives `wire.kel` what its host gives it. The buffer holds a real
// auxiliary body, encoded by the reference from a compiled corpus module, and
// each command is issued with its arguments written into the shared slots
// first, as `tests/selfhost_wire.rs` documents. Index arguments are kept in
// range by counts the virtual machine reports on a first pass, so no command
// is driven outside its contract. The script then runs on both sides.

/// Corpus modules whose bodies exercise the reader families: an enum and a
/// match, struct templates, natives, and shared and private data.
const WIRE_SUBJECTS: &[&str] = &[
    "../examples/scripts/02_struct_field.kel",
    "../examples/scripts/03_enum_match.kel",
    "../examples/scripts/12_sensor_window.kel",
    "../examples/scripts/14_frame_log.kel",
    "../examples/scripts/native_witnesses/external_native_witness.kel",
];

/// The auxiliary body of `m`, as `tests/selfhost_wire.rs` builds it for its own
/// real-output comparisons. Built here from the same public types rather than
/// shared, because root `tests/` is not a library this package can import.
fn aux_body_of(m: &Module) -> Vec<u8> {
    use keleusma::wire_format::{WireAuxBody, WireChunk};
    let aux = WireAuxBody {
        chunks: m
            .chunks
            .iter()
            .map(|c| WireChunk {
                name: c.name.clone(),
                constants: c.constants.clone(),
                struct_templates: c.struct_templates.clone(),
                local_count: c.local_count,
                param_count: c.param_count,
                block_type: c.block_type,
                param_types: c.param_types.clone(),
                op_byte_offset: 0,
                op_record_count: 0,
                debug_pool_bytes: None,
            })
            .collect(),
        signatures: m.signatures.clone(),
        enum_layouts: m.enum_layouts.clone(),
        native_names: m.native_names.clone(),
        native_return_shapes: m.native_return_shapes.clone(),
        data_layout: m.data_layout.clone(),
        entry_point: m.entry_point,
        word_bits_log2: m.word_bits_log2,
        addr_bits_log2: m.addr_bits_log2,
        float_bits_log2: m.float_bits_log2,
        flags: 0,
        wcet_cycles: 0,
        wcmu_bytes: 0,
        shared_data_bytes: 0,
        private_data_bytes: 0,
        schema_hash: 0,
    };
    keleusma::wire_schema::encode_aux_body(&aux).expect("encode aux body")
}

/// A step that sets the five argument slots and the region count, then sends
/// `cmd`. Every slot is written on every step, so no argument leaks from the
/// previous command into this one.
fn wire_step(cmd: i64, nregions: i64, args: [i64; 5]) -> Step {
    let mut words = vec![("nregions".to_string(), nregions)];
    for (name, v) in ["warg", "warg2", "warg3", "warg4", "warg5"]
        .iter()
        .zip(args)
    {
        words.push((name.to_string(), v));
    }
    Step {
        words,
        bytes: Vec::new(),
        tables: Vec::new(),
        byte_tables: Vec::new(),
        reply: cmd,
    }
}

/// Ask the virtual machine one question, resuming a stage already running.
fn vm_ask(
    m: &Module,
    vm: &mut Vm<'_, '_>,
    shared: &mut [u8],
    started: &mut bool,
    step: &Step,
) -> i64 {
    apply_writes(m, shared, step);
    let reply = Value::Int(step.reply);
    let st = if *started {
        let st = vm
            .resume_with_shared(shared, reply.clone())
            .unwrap_or_else(|e| panic!("probe of command {}: {e:?}", step.reply));
        if matches!(st, VmState::Reset) {
            vm.resume_with_shared(shared, reply)
                .expect("probe after reset")
        } else {
            st
        }
    } else {
        *started = true;
        vm.call_with_shared(shared, &[reply])
            .expect("probe first call")
    };
    scalar_of(&st)
}

/// How many entries of a family to query. Small, because the point is to reach
/// each reader on real data, and the yield sequence already compares every one.
const PER_FAMILY: i64 = 4;

/// Build the command script for one artifact, using the virtual machine's own
/// counts to keep every index in range.
fn wire_script(m: &Module, art: &[u8]) -> (Vec<u8>, Vec<Step>, i64) {
    let mut seeded = vec![0u8; shared_data_bytes_for(m)];
    let body: Vec<i64> = art.iter().map(|&b| i64::from(b)).collect();
    seed(m, &mut seeded, "len", "bytes", &body, 1);

    let arena = arena_for(m);
    let mut vm = Vm::new(m.clone(), &arena).expect("vm loads");
    let mut probe = seeded.clone();
    let mut started = false;
    let mut ask = |cmd: i64, n: i64, args: [i64; 5]| {
        vm_ask(
            m,
            &mut vm,
            &mut probe,
            &mut started,
            &wire_step(cmd, n, args),
        )
    };

    let n = ask(2, 0, [0; 5]);
    assert!(
        n > 0,
        "parse_prologue reported {n} regions for a {}-byte reference artifact; the \
         readers below would all be driven against a refused header",
        art.len()
    );
    let mut script = vec![
        wire_step(0, n, [0; 5]),
        wire_step(2, n, [0; 5]),
        wire_step(3, n, [0; 5]),
    ];
    let mut kinds = Vec::new();
    for i in 0..n {
        kinds.push(ask(9, n, [i, 0, 0, 0, 0]));
        for cmd in [7, 8, 9, 11, 12] {
            script.push(wire_step(cmd, n, [i, 0, 0, 0, 0]));
        }
    }
    for k in kinds.iter().copied().chain([9999]) {
        script.push(wire_step(5, n, [k, 0, 0, 0, 0]));
    }
    script.push(wire_step(6, n, [0; 5]));

    // Each family: a count command, then per-index readers, then optional
    // second-level readers bounded by a per-index count.
    struct Family {
        count: i64,
        each: &'static [i64],
        inner_count: Option<i64>,
        inner: &'static [i64],
    }
    let families = [
        Family {
            count: 18,
            each: &[19, 20],
            inner_count: Some(20),
            inner: &[21],
        },
        Family {
            count: 22,
            each: &[23, 24, 25],
            inner_count: None,
            inner: &[],
        },
        Family {
            count: 26,
            each: &[27, 28, 29, 30, 31, 32],
            inner_count: None,
            inner: &[],
        },
        Family {
            count: 38,
            each: &[39, 40, 41, 42],
            inner_count: Some(40),
            inner: &[43],
        },
        Family {
            count: 44,
            each: &[45, 46],
            inner_count: Some(46),
            inner: &[47],
        },
        Family {
            count: 48,
            each: &[49, 50, 51],
            inner_count: Some(50),
            inner: &[52, 53, 54],
        },
        Family {
            count: 56,
            each: &[57, 58],
            inner_count: None,
            inner: &[],
        },
        Family {
            count: 59,
            each: &[60, 61, 62],
            inner_count: None,
            inner: &[],
        },
        Family {
            count: 63,
            each: &[64, 65],
            inner_count: None,
            inner: &[],
        },
        Family {
            count: 66,
            each: &[67, 68],
            inner_count: None,
            inner: &[],
        },
        Family {
            count: 69,
            each: &[71, 72, 73, 74],
            inner_count: None,
            inner: &[],
        },
        Family {
            count: 75,
            each: &[76],
            inner_count: None,
            inner: &[],
        },
        Family {
            count: 77,
            each: &[78],
            inner_count: None,
            inner: &[],
        },
    ];
    for f in &families {
        let c = ask(f.count, n, [0; 5]);
        script.push(wire_step(f.count, n, [0; 5]));
        for i in 0..c.clamp(0, PER_FAMILY) {
            for &cmd in f.each {
                script.push(wire_step(cmd, n, [i, 0, 0, 0, 0]));
            }
            if let Some(ic) = f.inner_count {
                let inner = ask(ic, n, [i, 0, 0, 0, 0]);
                for j in 0..inner.clamp(0, 2) {
                    for &cmd in f.inner {
                        script.push(wire_step(cmd, n, [i, j, 0, 0, 0]));
                    }
                }
            }
        }
    }
    for cmd in [55, 79, 82, 83, 113, 114] {
        script.push(wire_step(cmd, n, [0; 5]));
    }

    // A damaged copy of the first prologue, voted out and reported, then repaired.
    let flip = |at: usize, v: u8| Step {
        words: Vec::new(),
        bytes: vec![(at, v)],
        tables: Vec::new(),
        byte_tables: Vec::new(),
        reply: 3,
    };
    script.push(flip(5, art[5] ^ 0xFF));
    script.push(wire_step(2, n, [0; 5]));
    script.push(flip(5, art[5]));
    script.push(wire_step(3, n, [0; 5]));

    // The checksum trailer over the whole artifact, appended then verified.
    let len = art.len() as i64;
    script.push(wire_step(111, n, [len, 0, 0, 0, 0]));
    script.push(wire_step(112, n, [len + 4, 0, 0, 0, 0]));
    (seeded, script, n)
}

#[test]
fn wire_agrees_with_the_vm_on_real_artifacts_driven_by_commands() {
    let w = module_of("../src/selfhost/kel/wire.kel");
    let mut commands_driven = std::collections::BTreeSet::new();
    let mut turns = 0usize;
    for path in WIRE_SUBJECTS {
        let subject = module_of(path);
        let art = aux_body_of(&subject);
        assert!(
            art.len() + 4 <= 65536,
            "{path}: its {}-byte auxiliary body does not fit `wire.bytes`",
            art.len()
        );
        let (seeded, script, n) = wire_script(&w, &art);

        // The readers must report what the reference encoded, or the agreement
        // below could be two lowerings agreeing on a misread.
        let vm = run_vm_script(&w, &seeded, &script);
        let at = |cmd: i64| {
            script
                .iter()
                .position(|s| s.reply == cmd && s.bytes.is_empty())
                .map(|i| vm.yields[i])
                .unwrap_or_else(|| panic!("{path}: command {cmd} is not in the script"))
        };
        assert_eq!(at(69), subject.chunks.len() as i64, "{path}: chunk count");
        assert_eq!(
            at(75),
            subject.native_names.len() as i64,
            "{path}: native count"
        );
        assert_eq!(at(2), n, "{path}: region count");
        assert_eq!(at(3), 0, "{path}: an intact prologue reported disagreement");
        assert_eq!(
            at(112),
            1,
            "{path}: the sealed artifact failed its own check"
        );

        let nat = run_native_script(&w, &seeded, &script);
        if vm.yields != nat.yields {
            let i = vm
                .yields
                .iter()
                .zip(nat.yields.iter())
                .position(|(a, b)| a != b)
                .unwrap_or(vm.yields.len().min(nat.yields.len()));
            panic!(
                "{path}: wire.kel diverges at turn {i}, command {}: vm {:?}, native {:?}",
                script[i].reply,
                vm.yields.get(i),
                nat.yields.get(i)
            );
        }
        assert_eq!(
            vm.shared, nat.shared,
            "{path}: the shared segment disagrees after the script"
        );
        let mut d = vm.yields.clone();
        d.sort_unstable();
        d.dedup();
        assert!(
            d.len() >= 10,
            "{path}: only {} distinct results over {} commands; the script did not \
             reach the readers on real data",
            d.len(),
            script.len()
        );
        commands_driven.extend(script.iter().map(|s| s.reply));
        turns += script.len();
    }

    // **THE CONTROL.** One byte of the first artifact's name pool, changed on the
    // native side only, must surface. Without this a comparison that could not
    // fail would read as agreement.
    let subject = module_of(WIRE_SUBJECTS[0]);
    let art = aux_body_of(&subject);
    let (seeded, script, _) = wire_script(&w, &art);
    let vm = run_vm_script(&w, &seeded, &script);
    let mut damaged = seeded.clone();
    let base = shared_offset(&w, "bytes").expect("bytes") as usize;
    let last = base + art.len() - 1;
    damaged[last] ^= 0x01;
    let nat = run_native_script(&w, &damaged, &script);
    // The yields, not the segment: the damaged byte stays in the native segment,
    // so a segment difference would appear even if nothing read the byte. Turn 0
    // is the checksum over the whole artifact, which reads every byte.
    assert_eq!(
        script[0].reply, 0,
        "the script no longer opens with the checksum"
    );
    assert_ne!(
        vm.yields[0], nat.yields[0],
        "a one-byte difference in the artifact did not change the checksum yield"
    );
    println!(
        "wire.kel: {} distinct commands, {turns} turns over {} artifacts, native agrees with the VM",
        commands_driven.len(),
        WIRE_SUBJECTS.len()
    );
}

// ================= `verify_datalayout.kel`, DRIVEN IN ITS BATCHED PHASES
//
// This stage was recorded as blocked by design, because its verdict accumulates
// across three differently encoded phases and no single seeded buffer can
// produce one. That was a limit of a harness that seeds once and then only
// ticks. With host writes per turn, the batches are fed as
// `dl_reject_module_via_kel` in `src/selfhost/mod.rs` feeds them: the running
// state initialised once, then phase 1 over the slot visibilities, phase 2 over
// the shared layout, phase 3 over the private composite layout, each 1024
// entries per turn. A final phase 4 turn has the stage itself compare the
// shared prefix and total with the shared-layout count, where the driver does
// that comparison in Rust.

const DL_BATCH: usize = 1024;

/// A real layout with a shared block, perturbed into each constructed defect.
const DEFECT_BASE: &str = "../src/selfhost/kel/wire_debug.kel";

/// One phase's batches, each a turn writing `phase`, `count` and the arrays.
fn dl_batches(phase: i64, columns: &[(&str, Vec<i64>)], script: &mut Vec<Step>) {
    let n = columns.first().map_or(0, |(_, c)| c.len());
    for start in (0..n).step_by(DL_BATCH) {
        let end = (start + DL_BATCH).min(n);
        script.push(Step {
            words: vec![
                ("phase".to_string(), phase),
                ("count".to_string(), (end - start) as i64),
            ],
            bytes: Vec::new(),
            tables: columns
                .iter()
                .map(|(name, col)| (name.to_string(), col[start..end].to_vec()))
                .collect(),
            byte_tables: Vec::new(),
            reply: 0,
        });
    }
}

/// The columns the driver derives from a module, as `dl_reject_module_via_kel` does.
struct DlInput {
    n_slots: i64,
    buffer: i64,
    pool: i64,
    visibility: Vec<i64>,
    shared: [Vec<i64>; 3],
    private: [Vec<i64>; 2],
}

fn dl_input_of(m: &Module) -> DlInput {
    use keleusma::bytecode::SHARED_SLOT_COMPOSITE_FLAG;
    use keleusma::value_layout::ScalarKind;
    let layout = m
        .data_layout
        .as_ref()
        .expect("a subject with a data layout");
    let wb = (1usize << m.word_bits_log2) / 8;
    let fb = (1usize << m.float_bits_log2) / 8;
    let ab = (1usize << m.addr_bits_log2) / 8;
    let mut shared = [Vec::new(), Vec::new(), Vec::new()];
    for sl in &layout.shared_layout {
        let (size, bad) = if sl.kind & SHARED_SLOT_COMPOSITE_FLAG != 0 {
            (sl.len as i64, 0)
        } else {
            match ScalarKind::from_tag(sl.kind) {
                Some(k) => (k.size_in_bytes(wb, fb, ab) as i64, 0),
                None => (0, 1),
            }
        };
        shared[0].push(sl.offset as i64);
        shared[1].push(size);
        shared[2].push(bad);
    }
    DlInput {
        n_slots: layout.slots.len() as i64,
        buffer: m.shared_data_bytes as i64,
        pool: m.persistent_composite_bytes as i64,
        visibility: layout
            .slots
            .iter()
            .map(|s| i64::from(matches!(s.visibility, SlotVisibility::Shared)))
            .collect(),
        shared,
        private: [
            layout
                .private_composite_layout
                .iter()
                .map(|e| e.slot as i64)
                .collect(),
            layout
                .private_composite_layout
                .iter()
                .map(|e| e.offset as i64)
                .collect(),
        ],
    }
}

fn dl_script(d: &DlInput) -> Vec<Step> {
    let init = [
        ("io_prefix", 0),
        ("io_total", 0),
        ("io_still", 1),
        ("io_prev_slot", 0),
        ("io_prev_off", 0),
        ("io_first", 1),
        ("io_bad", 0),
        ("n_slots", d.n_slots),
        ("buffer", d.buffer),
        ("pool", d.pool),
        ("shared_count", d.shared[0].len() as i64),
    ];
    let mut script = Vec::new();
    dl_batches(1, &[("a0", d.visibility.clone())], &mut script);
    dl_batches(
        2,
        &[
            ("a0", d.shared[0].clone()),
            ("a1", d.shared[1].clone()),
            ("a2", d.shared[2].clone()),
        ],
        &mut script,
    );
    dl_batches(
        3,
        &[("a0", d.private[0].clone()), ("a1", d.private[1].clone())],
        &mut script,
    );
    script.push(Step {
        words: vec![("phase".to_string(), 4)],
        bytes: Vec::new(),
        tables: Vec::new(),
        byte_tables: Vec::new(),
        reply: 0,
    });
    let first = script.first_mut().expect("at least the final turn");
    first
        .words
        .splice(0..0, init.iter().map(|(n, v)| (n.to_string(), *v)));
    script
}

#[test]
fn verify_datalayout_agrees_with_the_vm_across_its_batched_phases() {
    let dl = module_of("../src/selfhost/kel/verify_datalayout.kel");
    // Real layouts, from one slot to the lexer's buffer of many batches.
    let mut subjects: Vec<(String, DlInput, i64)> = [
        "../examples/scripts/14_frame_log.kel",
        "../examples/scripts/native_witnesses/opcode_witness.kel",
        "../src/selfhost/kel/wire_debug.kel",
        "../src/selfhost/kel/lexer.kel",
    ]
    .iter()
    .map(|p| (p.to_string(), dl_input_of(&module_of(p)), 0))
    .collect();

    // Constructed defects, one per check, each from a real layout, so the
    // verdict is seen to move as well as to hold.
    let base = dl_input_of(&module_of(DEFECT_BASE));
    let mut out_of_bounds = dl_input_of(&module_of(DEFECT_BASE));
    out_of_bounds.buffer = 0;
    subjects.push(("shared extent past the buffer".into(), out_of_bounds, 1));
    let mut gap = dl_input_of(&module_of(DEFECT_BASE));
    gap.visibility.insert(0, 0);
    gap.n_slots += 1;
    subjects.push(("shared slots not a prefix".into(), gap, 1));
    let mut descending = dl_input_of(&module_of(DEFECT_BASE));
    descending.pool = 64;
    descending.n_slots = descending.n_slots.max(4);
    descending.private = [vec![2, 1], vec![0, 8]];
    subjects.push(("private slots descending".into(), descending, 1));
    let mut ascending = dl_input_of(&module_of(DEFECT_BASE));
    ascending.pool = 64;
    ascending.n_slots = ascending.n_slots.max(4);
    ascending.private = [vec![1, 2], vec![0, 8]];
    subjects.push(("private slots ascending".into(), ascending, 0));
    assert!(
        !base.shared[0].is_empty(),
        "the defect base declares no shared slot, so two of the defects cannot fire"
    );

    let mut multi_batch = false;
    for (name, d, want) in &subjects {
        let script = dl_script(d);
        multi_batch |= d.visibility.len() > DL_BATCH;
        let seeded = vec![0u8; shared_data_bytes_for(&dl)];
        let vm = run_vm_script(&dl, &seeded, &script);
        let verdict = *vm.yields.last().expect("a final turn");
        assert_eq!(
            verdict, *want,
            "{name}: the stage's verdict on the virtual machine is {verdict}, expected {want}"
        );
        let nat = run_native_script(&dl, &seeded, &script);
        assert_eq!(
            vm.yields, nat.yields,
            "{name}: the per-batch results disagree"
        );
        assert_eq!(vm.shared, nat.shared, "{name}: the running state disagrees");
    }
    assert!(
        multi_batch,
        "no subject spans more than one batch, so the state carried between batches \
         was never exercised"
    );
}

// ================= `wire_debug.kel` AND `wire_schema.kel`, IN LOCKSTEP
//
// Both stages answer an interactive host. After each command the host reads
// the stage's request from shared slots and supplies the next input, so no
// fixed script expresses the exchange. These are ports of the driver's own
// host loops, `wire_debug_pool_via_kel` in `src/selfhost/wire_debug.rs` and
// `decode_debug` in `src/selfhost/wire_debug_decode.rs`, run through
// `lockstep`, which compares the yield and the whole shared segment of both
// sides after every turn. The reference `DebugPool::encode` and
// `DebugPool::decode` are the oracles for what the exchange produced.

use keleusma::debug_meta::{DebugPool, DebugRecord, DebugRecordKind};

fn word_at(m: &Module, snap: &[u8], array: &str, i: usize) -> i64 {
    let off =
        shared_offset(m, array).unwrap_or_else(|| panic!("no array `{array}`")) as usize + i * 8;
    let mut b = [0u8; 8];
    b.copy_from_slice(&snap[off..off + 8]);
    i64::from_le_bytes(b)
}

fn bytes_at(m: &Module, snap: &[u8], array: &str, n: usize) -> Vec<u8> {
    let off = shared_offset(m, array).unwrap_or_else(|| panic!("no array `{array}`")) as usize;
    snap[off..off + n].to_vec()
}

fn turn_of(reply: i64) -> Step {
    Step {
        words: Vec::new(),
        bytes: Vec::new(),
        tables: Vec::new(),
        byte_tables: Vec::new(),
        reply,
    }
}

fn record_header(r: &DebugRecord) -> [i64; 3] {
    [
        i64::from(r.op_index),
        i64::from(r.kind.as_u8()),
        r.operands.len() as i64,
    ]
}

/// The encode exchange, after `wire_debug_pool_via_kel`.
fn debug_encode_in_lockstep(m: &Module, pool: &DebugPool) -> (Vec<u8>, usize) {
    let seeded = vec![0u8; shared_data_bytes_for(m)];
    lockstep(m, &seeded, |turn| {
        let mut ask = |words: Vec<(&str, i64)>, fin: Vec<i64>, bin: Vec<u8>, cmd: i64| {
            let mut s = turn_of(cmd);
            s.words = words.into_iter().map(|(n, v)| (n.to_string(), v)).collect();
            if !fin.is_empty() {
                s.tables = vec![("fin".to_string(), fin)];
            }
            if !bin.is_empty() {
                s.byte_tables = vec![("bin".to_string(), bin)];
            }
            let (y, snap) = turn(&s);
            assert!(y >= 0, "command {cmd} refused with {y}");
            (y as usize, snap)
        };

        // The stage schedules a stable merge sort over record indices.
        let records = &pool.records;
        let n = records.len();
        let mut order = [(0..n).collect::<Vec<_>>(), (0..n).collect::<Vec<_>>()];
        assert_eq!(ask(vec![("arg", n as i64)], vec![], vec![], 0).0, 0);
        let rounds = if n < 2 {
            0
        } else {
            (usize::BITS - (n - 1).leading_zeros()) as usize
        };
        let mut sorted = None;
        for _ in 0..n * rounds + 1 {
            let (action, mut snap) = ask(vec![], vec![], vec![], 1);
            let generation = word_at(m, &snap, "fin", 2) as usize;
            assert!(generation <= 1, "invalid sort generation {generation}");
            if action == 0 {
                sorted = Some(order[generation].clone());
                break;
            }
            if action == 2 {
                let a = &records[order[generation][word_at(m, &snap, "fin", 0) as usize]];
                let b = &records[order[generation][word_at(m, &snap, "fin", 1) as usize]];
                let mut fin = record_header(a).to_vec();
                fin.extend(record_header(b));
                let (needed, _) = ask(vec![], fin, vec![], 2);
                for start in (0..needed).step_by(512) {
                    let count = (needed - start).min(512);
                    let fin: Vec<i64> = (0..count)
                        .flat_map(|i| {
                            [
                                i64::from(a.operands[start + i]),
                                i64::from(b.operands[start + i]),
                            ]
                        })
                        .collect();
                    assert_eq!(ask(vec![("arg", count as i64)], fin, vec![], 3).0, 0);
                }
                let (copy, after) = ask(vec![], vec![], vec![], 4);
                assert_eq!(copy, 1, "missing comparison copy");
                snap = after;
            } else {
                assert_eq!(action, 1, "unknown sort request {action}");
            }
            let source = word_at(m, &snap, "fin", 0) as usize;
            let destination = word_at(m, &snap, "fin", 1) as usize;
            order[1 - generation][destination] = order[generation][source];
        }
        let order = sorted.expect("the sort finished within its budget");

        // The stage then schedules every output byte.
        let counts = vec![
            pool.string_pool.len() as i64,
            pool.span_pool.len() as i64,
            pool.type_pool.len() as i64,
            records.len() as i64,
        ];
        let (start, snap) = ask(vec![], counts, vec![], 5);
        assert_eq!(start, 4, "unexpected pool start");
        let mut out = bytes_at(m, &snap, "bytes", 4);
        loop {
            let (request, snap) = ask(vec![], vec![], vec![], 6);
            if request == 0 {
                break;
            }
            if request == 9 {
                out.extend(bytes_at(m, &snap, "bytes", 4));
                continue;
            }
            let index = word_at(m, &snap, "fin", 0) as usize;
            let offset = word_at(m, &snap, "fin", 1) as usize;
            let (wrote, snap) = match request {
                1 => ask(
                    vec![],
                    vec![pool.string_pool[index].len() as i64],
                    vec![],
                    7,
                ),
                3 => ask(vec![], vec![pool.type_pool[index].len() as i64], vec![], 7),
                2 => {
                    let (f, o, l) = pool.span_pool[index];
                    ask(
                        vec![],
                        vec![i64::from(f), i64::from(o), i64::from(l)],
                        vec![],
                        7,
                    )
                }
                4 => ask(
                    vec![],
                    record_header(&records[order[index]]).to_vec(),
                    vec![],
                    7,
                ),
                5 | 7 => {
                    let all: &[u8] = if request == 5 {
                        pool.string_pool[index].as_bytes()
                    } else {
                        &pool.type_pool[index]
                    };
                    let tail = &all[offset..];
                    let count = tail.len().min(4096);
                    ask(
                        vec![("arg", count as i64)],
                        vec![],
                        tail[..count].to_vec(),
                        8,
                    )
                }
                8 => {
                    let tail = &records[order[index]].operands[offset..];
                    let count = tail.len().min(1024);
                    let fin = tail[..count].iter().map(|&v| i64::from(v)).collect();
                    ask(vec![("arg", count as i64)], fin, vec![], 8)
                }
                other => panic!("unknown pool request {other}"),
            };
            out.extend(bytes_at(m, &snap, "bytes", wrote));
        }
        out
    })
}

/// The decode exchange, after `decode_debug`.
fn debug_decode_in_lockstep(m: &Module, input: &[u8]) -> (DebugPool, usize) {
    let seeded = vec![0u8; shared_data_bytes_for(m)];
    lockstep(m, &seeded, |turn| {
        let mut ask = |arg: Option<i64>, window: &[u8], cmd: i64| {
            let mut s = turn_of(cmd);
            if let Some(a) = arg {
                s.tables = vec![("arg".to_string(), vec![a])];
            }
            s.bytes = window.iter().copied().enumerate().collect();
            let (y, snap) = turn(&s);
            assert!(y >= 0, "command {cmd} refused with {y}");
            (y as usize, snap)
        };
        let fin = |snap: &[u8], i: usize| word_at(m, snap, "fin", i);
        let requested = |snap: &[u8]| {
            let start = fin(snap, 0) as usize;
            &input[start..start + fin(snap, 1) as usize]
        };

        assert_eq!(ask(Some(input.len() as i64), &[], 44).0, 0);
        let mut pool = DebugPool::default();
        let mut record: Option<DebugRecord> = None;
        let mut operand_count = 0usize;
        for _ in 0..input.len() * 2 + 16 {
            let (mut event, mut snap) = ask(None, &[], 45);
            if event == 0 {
                assert!(record.is_none(), "a record was left open");
                return pool;
            }
            if event == 1 {
                let field = requested(&snap).to_vec();
                (event, snap) = ask(Some(field.len() as i64), &field, 46);
            }
            match event {
                2 => {}
                3 => {
                    let raw = requested(&snap).to_vec();
                    for chunk in raw.chunks(1024) {
                        assert_eq!(ask(Some(chunk.len() as i64), chunk, 10).0, 0);
                    }
                    assert_eq!(ask(None, &[], 11).0, 0, "name validation did not finish");
                    pool.string_pool
                        .push(String::from_utf8(raw).expect("the stage accepted invalid text"));
                    assert_eq!(ask(None, &[], 47).0, 0);
                }
                4 => pool.span_pool.push((
                    fin(&snap, 0) as u16,
                    fin(&snap, 1) as u32,
                    fin(&snap, 2) as u32,
                )),
                5 => {
                    pool.type_pool.push(requested(&snap).to_vec());
                    assert_eq!(ask(None, &[], 47).0, 0);
                }
                6 => {
                    assert!(record.is_none(), "a record began before the last completed");
                    operand_count = fin(&snap, 2) as usize;
                    record = Some(DebugRecord {
                        op_index: fin(&snap, 0) as u32,
                        kind: DebugRecordKind::from_u8(fin(&snap, 1) as u8 - 1)
                            .expect("a known record kind"),
                        operands: Vec::new(),
                    });
                }
                7 => {
                    let r = record.take().expect("a record completed before it began");
                    assert_eq!(r.operands.len(), operand_count, "operand count");
                    pool.records.push(r);
                    assert_eq!(ask(None, &[], 47).0, 0);
                }
                8 => {
                    let r = record.as_mut().expect("operands outside a record");
                    for i in 0..fin(&snap, 0) as usize {
                        r.operands.push(fin(&snap, i + 1) as u16);
                    }
                }
                other => panic!("unknown debug request {other}"),
            }
        }
        panic!("the decode exceeded its request budget")
    })
}

/// A constructed pool reaching every record kind, multi-byte text, a type blob
/// longer than one 4096-byte window, records sharing a key so the sort's
/// stability is visible, and operand lists longer than one 15-operand batch.
fn constructed_debug_pool() -> DebugPool {
    let mut records = Vec::new();
    for i in 0..40u32 {
        records.push(DebugRecord {
            op_index: (i * 37) % 11,
            kind: DebugRecordKind::from_u8((i % 12) as u8).expect("kind"),
            operands: (0..(i % 20) as u16).map(|j| j.wrapping_mul(977)).collect(),
        });
    }
    DebugPool {
        string_pool: vec![
            "main".into(),
            "λ-term".into(),
            String::new(),
            "x".repeat(700),
        ],
        span_pool: vec![(0, 0, 0), (1, 70_000, 12), (u16::MAX, u32::MAX, 1)],
        type_pool: vec![
            vec![],
            vec![1, 2, 3],
            (0..5000u32).map(|i| i as u8).collect(),
        ],
        records,
    }
}

/// Real pools: the compiler's own debug metadata for corpus programs.
fn compiled_debug_pools() -> Vec<(String, DebugPool)> {
    use keleusma::compiler::{CompileOptions, compile_with_options};
    let mut out = Vec::new();
    for path in [
        "../examples/scripts/05_pipeline.kel",
        "../examples/scripts/08_method_dispatch.kel",
        "../src/selfhost/kel/verify_types.kel",
    ] {
        let src = std::fs::read_to_string(path).expect("read subject");
        let program = parse(&tokenize(&src).expect("lex")).expect("parse");
        let (m, _) = compile_with_options(
            &program,
            &keleusma::target::Target::default(),
            &CompileOptions { emit_debug: true },
        )
        .expect("compile with debug metadata");
        let best = m
            .chunks
            .iter()
            .filter_map(|c| c.debug_pool.clone())
            .filter(|p| !p.records.is_empty() && p.records.len() <= 300)
            .max_by_key(|p| p.records.len());
        if let Some(p) = best {
            out.push((path.to_string(), p));
        }
    }
    out
}

#[test]
fn wire_debug_and_wire_schema_agree_with_the_vm_over_a_debug_pool_round_trip() {
    let enc = module_of("../src/selfhost/kel/wire_debug.kel");
    let dec = module_of("../src/selfhost/kel/wire_schema.kel");
    let mut subjects = compiled_debug_pools();
    assert!(
        subjects.len() >= 2,
        "only {} corpus program(s) produced a usable debug pool",
        subjects.len()
    );
    subjects.push(("constructed".to_string(), constructed_debug_pool()));
    for (name, pool) in &subjects {
        let (bytes, enc_turns) = debug_encode_in_lockstep(&enc, pool);
        let mut canonical = pool.clone();
        canonical.records.sort_by(|a, b| {
            a.op_index
                .cmp(&b.op_index)
                .then(a.kind.cmp(&b.kind))
                .then(a.operands.cmp(&b.operands))
        });
        assert_eq!(
            bytes,
            canonical.encode(),
            "{name}: the encode exchange produced bytes the reference would not"
        );
        let (decoded, dec_turns) = debug_decode_in_lockstep(&dec, &bytes);
        assert_eq!(
            decoded.encode(),
            DebugPool::decode(&bytes)
                .expect("reference decode")
                .encode(),
            "{name}: the decode exchange disagrees with the reference decoder"
        );
        println!(
            "{name}: {} records, encode {enc_turns} turns, decode {dec_turns} turns, every turn agreed",
            pool.records.len()
        );
    }
}

// ================= `wire.kel`'s EMISSION JOINS, AS THE DRIVER ISSUES THEM
//
// The reader script above leaves out the commands the self-hosted compiler
// actually uses `wire.kel` for, which emit an auxiliary body. The driver runs
// each join as one call on a freshly seeded buffer: `wire_names_via_kel` issues
// 167, `wire_regions_via_kel` 168 and `wire_chunks_via_kel` 169. This seeds the
// same buffer, runs the same call on both sides, and checks the virtual
// machine's bytes against the public driver function, so a drift between this
// seeding and the driver's fails here rather than passing on a different
// input. The driver's result is then checked against the reference encoder.

/// `header_fields_of` in `src/selfhost/mod.rs`, which is private there.
fn wire_header_fields(m: &Module) -> Vec<i64> {
    vec![
        m.entry_point
            .map_or(i64::from(keleusma::wire_schema::ABSENT), |e| e as i64),
        m.word_bits_log2 as i64,
        m.addr_bits_log2 as i64,
        m.float_bits_log2 as i64,
        0,
        0,
        0,
        0,
        0,
        0,
        i64::from(keleusma::bytecode::FORMAT_FINGERPRINT),
    ]
}

/// `chunk_fields_of` in `src/selfhost/mod.rs`, which is private there.
fn wire_chunk_fields(m: &Module) -> Vec<i64> {
    use keleusma::bytecode::BlockType;
    use keleusma::wire_schema::block_tag;
    let mut out = Vec::new();
    for c in &m.chunks {
        out.extend([
            0,
            c.constants.len() as i64,
            c.struct_templates.len() as i64,
            c.param_types.len() as i64,
            i64::from(keleusma::wire_schema::ABSENT),
            0,
            0,
            0,
            c.local_count as i64,
            c.param_count as i64,
            i64::from(match c.block_type {
                BlockType::Func => block_tag::FUNC,
                BlockType::Reentrant => block_tag::REENTRANT,
                BlockType::Stream => block_tag::STREAM,
            }),
        ]);
    }
    out
}

/// Each region's payload span, located by the reader commands on the virtual
/// machine. Only which bytes to blank depends on this; the comparison is with
/// the reference artifact.
fn region_spans(w: &Module, art: &[u8], n: i64) -> std::collections::BTreeMap<u16, (usize, usize)> {
    let mut seeded = vec![0u8; shared_data_bytes_for(w)];
    let body: Vec<i64> = art.iter().map(|&b| i64::from(b)).collect();
    seed(w, &mut seeded, "len", "bytes", &body, 1);
    let arena = arena_for(w);
    let mut vm = Vm::new(w.clone(), &arena).expect("vm loads");
    let mut started = false;
    let mut out = std::collections::BTreeMap::new();
    for i in 0..n {
        let mut ask = |cmd| {
            vm_ask(
                w,
                &mut vm,
                &mut seeded,
                &mut started,
                &wire_step(cmd, n, [i, 0, 0, 0, 0]),
            )
        };
        let kind = ask(9) as u16;
        let base = ask(11) as usize;
        let len = ask(12) as usize;
        out.insert(kind, (base, len));
    }
    out
}

/// A public driver entry that runs one join on the virtual machine.
type JoinDriver = fn(&Module, &[u8], usize) -> Result<Vec<u8>, keleusma::selfhost::SelfHostError>;

/// One join in lockstep on a buffer seeded as the driver seeds it.
fn wire_join(w: &Module, subject: &Module, art: &[u8], regions: i64, cmd: i64) -> Vec<u8> {
    let (blob, _) = keleusma::selfhost::module_input(subject);
    let mut seeded = vec![0u8; shared_data_bytes_for(w)];
    let body: Vec<i64> = art.iter().map(|&b| i64::from(b)).collect();
    seed(w, &mut seeded, "len", "bytes", &body, 1);
    seed_scalar(w, &mut seeded, "nregions", regions);
    let header = wire_header_fields(subject);
    match cmd {
        167 => {}
        168 => seed_table(w, &mut seeded, "fin", &header),
        169 => {
            seed_scalar(w, &mut seeded, "warg", subject.chunks.len() as i64);
            let mut fin = wire_chunk_fields(subject);
            fin.resize(990, 0);
            fin.extend(&header);
            seed_table(w, &mut seeded, "fin", &fin);
        }
        other => panic!("not a join: {other}"),
    }
    let bin = shared_offset(w, "bin").expect("bin") as usize;
    seeded[bin..bin + blob.len()].copy_from_slice(&blob);
    let ((y, snap), turns) = lockstep(w, &seeded, |turn| turn(&turn_of(cmd)));
    assert_eq!(turns, 1);
    assert!(y >= 0, "join {cmd} refused with {y}");
    bytes_at(w, &snap, "bytes", art.len())
}

#[test]
fn wire_emission_joins_agree_with_the_vm_and_the_driver() {
    let w = module_of("../src/selfhost/kel/wire.kel");
    let mut joined = 0;
    for path in [
        "../examples/scripts/02_struct_field.kel",
        "../examples/scripts/03_enum_match.kel",
        "../examples/scripts/14_frame_log.kel",
        "../src/selfhost/kel/verify_types.kel",
        "../src/selfhost/kel/verify_datalayout.kel",
    ] {
        let subject = module_of(path);
        let art = aux_body_of(&subject);
        let (_, script, n) = wire_script(&w, &art);
        drop(script);
        let spans = region_spans(&w, &art, n);
        let drivers: [(i64, JoinDriver); 3] = [
            (167, keleusma::selfhost::wire_names_via_kel),
            (168, keleusma::selfhost::wire_regions_via_kel),
            (169, keleusma::selfhost::wire_chunks_via_kel),
        ];
        for (cmd, driver) in drivers {
            // The regions this join emits are blanked in its input, so equality
            // with the reference below is something the stage had to write.
            use keleusma::wire_schema::kind;
            let emitted: &[u16] = match cmd {
                167 => &[kind::NAMES, kind::STRING_POOL],
                168 => &[kind::NAMES, kind::STRING_POOL, kind::HEADER],
                _ => &[kind::NAMES, kind::STRING_POOL, kind::HEADER, kind::CHUNKS],
            };
            let mut blank = art.clone();
            for k in emitted {
                let (base, len) = spans[k];
                blank[base..base + len].fill(0);
            }
            assert_ne!(
                blank, art,
                "{path}: join {cmd}'s regions are all zero already"
            );
            let ours = wire_join(&w, &subject, &blank, n, cmd);
            let theirs = driver(&subject, &blank, n as usize)
                .unwrap_or_else(|e| panic!("{path}: the driver refused join {cmd}: {e:?}"));
            assert_eq!(
                ours,
                theirs[..art.len()],
                "{path}: join {cmd} seeded here disagrees with the driver's own call"
            );
            // `14_frame_log.kel` left 26 bytes of a struct name zero in NAMES and
            // STRING_POOL from absorption 124 until absorption 128, when V0.2.X's
            // pull request 685 gave the lone joins the reference's constant
            // roots, private initializers included. Every subject now matches.
            assert_eq!(
                ours, art,
                "{path}: join {cmd} rewrote the artifact into bytes the reference would not write"
            );
            joined += 1;
        }
    }
    assert_eq!(joined, 15, "expected three joins over five subjects");
}

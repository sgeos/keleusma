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

mod common;

/// Ticks to drive a `Stream` entry. Enough to leave any init branch.
const TICKS: i64 = 60;
/// Stub slots. Must be at least the corpus's distinct-native count (42).
const STUBS: usize = 48;

/// One stub's identity: name, argument count, and the RECORDED composite return
/// shape (`CompositeKind` tag and body size) if it returns one.
type NativeEntry = (String, usize, Option<(u8, u32)>);

thread_local! {
    /// Index -> (name, argc, composite return shape) for the module under test.
    static TABLE: RefCell<Vec<NativeEntry>> = const { RefCell::new(Vec::new()) };
    /// Per-native scratch for a COMPOSITE return on the native side. The stub
    /// hands back an ADDRESS, so the bytes must outlive the call; one fixed
    /// buffer per stub index is enough because a native is called at one arity
    /// and one shape (both asserted elsewhere).
    static COMPOSITE_RET: RefCell<Vec<[u8; COMPOSITE_RET_CAP]>> = const { RefCell::new(Vec::new()) };
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

/// **ONE exemption remains that a contract could not close, and it is not
/// `led.kel` any more.**
///
/// Both used to fault because the stub honoured no contract. The bytecode was
/// checked rather than assumed (`what_return_contract_does_the_bytecode_record`),
/// and the two differed:
///
/// | native | recorded return shape | derivable? |
/// |---|---|---|
/// | `host::rng_range` | `Scalar { kind: 3 }` | the TYPE, yes. the RANGE, **no** |
/// | `host::gpio_set` | `Flat { kind: 3, size: 16 }` | the SHAPE, yes — **now honoured** |
///
/// **`led.kel` IS CLOSED.** `gpio_set` records a sixteen-byte enum body, so the
/// stub returns a real body ADDRESS on both sides: an arena-resident
/// `Value::Enum(EnumBody::Flat(_))` built through `register_native_with_ctx_closure`
/// on the virtual-machine side, and a matching buffer behind the native stub.
/// The bytes come from ONE builder, so there is no second encoding to drift.
///
/// **`rogue_dungen` cannot be closed at all**, and that is unchanged.
/// `use host::rng_range(Word, Word) -> Word` records types and nothing else;
/// `[lo, hi)` exists only in the host's head. Inferring a range from the argument
/// positions would work here and is exactly the guess that must not be made.
///
/// A stub that makes a module pass by accident is worse than an exemption. The
/// discriminant chosen is 0 — the FIRST DECLARED VARIANT, a value the module can
/// genuinely match — not an arbitrary integer that would make it fault.
///
/// The value a native returns, on BOTH sides.
///
/// Deterministic, and asymmetric in the argument positions so a swapped or
/// dropped argument changes it. A random source would make the two runs
/// incomparable, which is the point of stubbing at all.
/// Largest composite return the stub table serves. Asserted against, so a
/// larger one fails loudly rather than truncating.
const COMPOSITE_RET_CAP: usize = 64;

/// **The bytes of a stubbed COMPOSITE return, identical on both sides.**
///
/// One function, called by the virtual-machine closure and by the native stub,
/// so there is no second encoding to drift — the same rule the shared-segment
/// seeding follows.
///
/// **The discriminant is 0 and the payload is zero.** For an enum body the
/// layout is `[discriminant: word_bytes][payload]`, and 0 is the FIRST DECLARED
/// VARIANT — a value the module can genuinely match, which is the whole point.
/// `Status::Ok = 0` in the rtos prelude, so `led.kel` takes its `Status::Ok`
/// arm rather than falling through to a trap.
///
/// **This is deterministic invention, and it is no more invention than
/// `stub_value` already is for a `Word`.** What it must never be is a value the
/// module cannot match: that would make the module fault and the harness would
/// be measuring the stub rather than the lowering.
fn composite_stub_bytes(size: u32, word_bytes: usize, out: &mut [u8]) {
    let n = size as usize;
    out[..n].fill(0);
    // Discriminant 0, written at the module's word width. Explicit rather than
    // relying on the fill, because a future non-zero choice must have one place
    // to change.
    let disc: u64 = 0;
    let w = word_bytes.min(8).min(n);
    out[..w].copy_from_slice(&disc.to_le_bytes()[..w]);
}

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
    let (name, argc, shape) = TABLE.with(|t| {
        t.borrow()
            .get(idx)
            .cloned()
            .unwrap_or_else(|| (format!("<unmapped #{idx}>"), 0, None))
    });
    let args = &all[..argc.min(5)];
    let parts: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    LOG.with(|l| l.borrow_mut().push(format!("{name}({})", parts.join(", "))));

    // **A COMPOSITE return is an ADDRESS natively.** Returning `stub_value`'s
    // integer here is what made `led.kel` segfault: the module dereferenced it
    // as a body pointer. The bytes come from the shared builder, so they match
    // what the virtual-machine side hands its own caller.
    if let Some((_kind, size)) = shape {
        return COMPOSITE_RET.with(|c| {
            let mut bufs = c.borrow_mut();
            if bufs.len() <= idx {
                bufs.resize(idx + 1, [0u8; COMPOSITE_RET_CAP]);
            }
            let buf = &mut bufs[idx];
            composite_stub_bytes(size, 8, buf);
            buf.as_ptr() as i64
        });
    }
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
/// Name, argument count, and the RECORDED composite return shape if any.
///
/// The shape comes from `native_return_shapes`, so a native that returns a
/// composite is stubbed as one on both sides rather than as an integer the
/// native side would dereference as a body address.
fn native_table(m: &Module) -> Vec<NativeEntry> {
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
        .enumerate()
        .zip(argc)
        .map(|((i, n), a)| {
            let shape = match m.native_return_shapes.get(i) {
                Some(WireShape::Flat { kind, size }) => Some((*kind, *size)),
                _ => None,
            };
            (n.clone(), a.unwrap_or(0), shape)
        })
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
///
/// **Seeds 4 and up sweep a small constant, and that is about COMMAND
/// SELECTORS rather than about arithmetic.** The four shapes above reach four
/// distinct values of a first parameter, which is ample for a module that
/// computes and useless for one that DISPATCHES. `wire.kel` branches twenty-odd
/// ways on its first argument, so four shapes reached four of its commands and
/// left the rest of the module unexecuted while the harness reported it as
/// running.
///
/// **Every step below is a measured mutation-sweep result, not a rationale.**
/// Round two named four real holes, all owned by `wire.kel`, and the recorded
/// repair was to drive that module with real input. Seeding the shared segment
/// was necessary and NOT sufficient:
///
/// | | `BitAnd` 54 | `Shr` 20 | `BitOr` 9 | `Shl` 48 | `CmpNe` 26 |
/// |---|---|---|---|---|---|
/// | round two | -- | -- | -- | -- | -- |
/// | seeded segment, 4 seeds | **YES** | **YES** | -- | -- | -- |
/// | 24 seeds | YES | YES | **YES** | **YES** | **YES** |
///
/// Seeding alone reached only the EMIT direction, which extracts bytes with a
/// mask and a right shift. `BitOr` and `Shl` live in the PARSE direction, which
/// reassembles a multi-byte integer, and no argument SHAPE reaches it -- only a
/// selector VALUE does. **`Shl` is why the constant runs to 19 rather than to a
/// single digit**: an intermediate setting reaching selectors 0..7 left it
/// undetected.
///
/// The intermediate row is deliberately not tabulated. It was measured before
/// `mutation_sweep.py` calibrated its timeout, and an under-sized timeout can
/// manufacture a detection but never suppress one -- so its negative results
/// stand and its positive ones are not evidence. Only the two rows above were
/// taken under the corrected instrument.
///
/// **Generic rather than tuned to `wire.kel`.** The values are not chosen by
/// reading its dispatch table -- picking `cmd == 9` because that is where the
/// undetected sites are would make this a demonstration rather than a
/// measurement, the same error the pre-registered mutation set exists to avoid.
/// Consecutive small integers reach a dense selector in any module that has one.
///
/// **Cost, since this is paid on every run including CI**: 35s at 4 seeds, 58s
/// at 24. Sublinear because a `Stream` entry and a zero-parameter entry both
/// keep a single seed, so the sweep only widens the modules it can widen.
const SEEDS: usize = 24;

fn args_for_seed(n: usize, seed: usize) -> Vec<i64> {
    (0..n)
        .map(|i| match seed {
            0 => (i as i64 + 1) * 3 + 1,
            1 => 5,
            2 => 0,
            3 => (n as i64 - i as i64) * 3 + 1,
            k => (k - 4) as i64,
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

/// Byte offset of the shared slot named `suffix`, or of its element zero.
///
/// An array slot expands to one slot per element (`wire.bytes[0]`), so a plain
/// match on `bytes` finds nothing. `shared_layout` is parallel to the SHARED
/// prefix of `slots`, so the index is counted among shared slots.
fn shared_offset(m: &Module, suffix: &str) -> Option<u32> {
    let dl = m.data_layout.as_ref()?;
    let scalar = format!(".{suffix}");
    let element0 = format!(".{suffix}[0]");
    let mut shared_ix = 0usize;
    for sl in &dl.slots {
        if sl.visibility != SlotVisibility::Shared {
            continue;
        }
        if sl.name.ends_with(&scalar) || sl.name.ends_with(&element0) {
            return dl.shared_layout.get(shared_ix).map(|l| l.offset);
        }
        shared_ix += 1;
    }
    None
}

/// A payload for any module declaring the documented `len` + `bytes`
/// convention, written identically into both sides' buffers.
///
/// **This is a convention, not a special case.** `wire.kel` and `lexer.kel` each
/// document the same host contract in their own headers: `len` at slot 0,
/// `bytes[i]` at slot `1 + i`. Keying on that is why one rule serves both.
///
/// It exists because `wire.kel` owned every undetected opcode in the mutation
/// sweep -- 131 sites of `BitAnd`, `BitOr`, `Shl` and `Shr` -- and finished after
/// 0 ticks. `cmd == 0` is a bitwise CRC-32 over `bytes[0..len]`, which is exactly
/// where those sites are, and seed 2 already drives `cmd == 0`. The only thing
/// missing was something to checksum.
const PAYLOAD: &[u8] = b"keleusma wire payload: 0123456789 ABCDEF \x01\x02\x7f\x80\xfe\xff";

/// **A REAL INPUT for a self-hosted verifier stage, built by the driver's own
/// accessor rather than reproduced here.**
///
/// `src/selfhost/mod.rs` now exposes per-item seed accessors (`fa649ec3`), which
/// this line requested precisely so the harness would not carry a second
/// encoding of the stage input formats. Using them means the bytes are the ones
/// a real driver hands the module; a constructor of our own would be free to
/// drift, which is the defect class this whole arc has been about.
///
/// **`verify_datalayout` deliberately gets nothing.** It has no accessor by joint
/// agreement: its verdict accumulates across three differently-encoded phases in
/// the retained buffer, so a single seeded buffer cannot produce a verdict at
/// all. It stays in `KNOWN_VACUOUS`, and that is correct rather than a gap.
///
/// The subject chunk is a real compiled chunk, because verifying a chunk is what
/// these stages do. Returns the WHOLE segment, sized from the stage's own layout,
/// so both sides can be given identical bytes.
/// Stages with a `stage_seed` arm. Named so the applied/not-applied line is
/// printed for exactly those, rather than for every module in the corpus.
const STAGE_SEEDED: &[&str] = &[
    "verify_depth.kel",
    "verify_typed.kel",
    "verify_structural.kel",
    // Listed so the harness PRINTS why it is blocked rather than omitting it.
    "reconstruct.kel",
];

fn stage_seed(m: &Module, name: &str) -> Result<Vec<u8>, String> {
    if !STAGE_SEEDED.contains(&name) {
        return Err("no arm for this stage".into());
    }
    // **A chunk from the SHIPPED CORPUS, not a synthetic one.** Verifying a real
    // chunk is what these stages do, and a hand-written subject only proves the
    // stage handles what the harness author thought to write. The first attempt
    // used an invented source that did not even parse, and the accessor
    // declining is how that surfaced -- which is why this reports WHY it
    // declined rather than returning `None`.
    let subject = sources()
        .into_iter()
        .filter(|p| p.file_name().unwrap_or_default().to_string_lossy() == "02_struct_field.kel")
        .find_map(|p| {
            let src = std::fs::read_to_string(&p).ok()?;
            compile(&parse(&tokenize(&src).ok()?).ok()?).ok()
        })
        .ok_or("no subject module in the corpus")?;
    let chunk = subject
        .chunks
        .iter()
        .max_by_key(|c| c.ops.len())
        .ok_or("subject has no chunk")?;

    let arena = arena_for(m);
    let vm = Vm::new(m.clone(), &arena).map_err(|e| format!("stage VM refuses to load: {e:?}"))?;
    let seed = match name {
        "verify_depth.kel" => keleusma::selfhost::seed_verify_depth_shared(&vm, chunk),
        "verify_typed.kel" => {
            // Word and float widths come from the SUBJECT module, not this stage:
            // the stage is verifying that module's chunk, so the widths it must
            // reason about are the subject's.
            let wb = (1usize << subject.word_bits_log2) / 8;
            let fb = (1usize << subject.float_bits_log2) / 8;
            let idx = subject
                .chunks
                .iter()
                .position(|c| core::ptr::eq(c, chunk))
                .unwrap_or(0);
            let sig = subject.signatures.get(idx);
            keleusma::selfhost::seed_verify_typed_shared(&vm, &subject, chunk, sig, wb, fb)
        }
        "verify_structural.kel" => {
            // The always-yielding set is the subject module's, computed by the
            // driver's own fixpoint rather than re-derived here.
            let always = keleusma::selfhost::self_hosted_always_yielding(&subject);
            keleusma::selfhost::seed_verify_structural_shared(&vm, &subject, chunk, &always)
        }
        // **`reconstruct.kel` IS drivable from here. The earlier claim that its
        // producer is private is RETRACTED** -- `parse_functions` is `pub`, and it
        // returns the parsed heads directly, so the record stream handed to the
        // stage is the driver's own rather than a second encoding written here.
        //
        // What is genuinely unreachable is `ParsedFn`'s FIELDS. Two scalars
        // therefore cannot be read off the parsed values: which heads form the
        // multiheaded group, and how many value parameters they declare. Neither
        // is part of the stage's input record format -- they are properties of the
        // SUBJECT -- and both are derived or asserted below rather than assumed,
        // because a subject that quietly stops containing the construct it is
        // named for is this harness's own recorded failure mode.
        //
        // **`seed_reconstruct_shared`, the single-head form, stays blocked.** It
        // wants `records: &[(i64, i64)]`, which cannot be built without the field
        // accessors. Those are on the `v0.2.3` line's open work and are not on this
        // tree, so exactly one of the two reconstruct paths is exercised here.
        "reconstruct.kel" => {
            let path = sources()
                .into_iter()
                .find(|p| {
                    p.file_name().unwrap_or_default().to_string_lossy() == "06_multiheaded.kel"
                })
                .ok_or("06_multiheaded.kel is not in the corpus")?;
            let src = std::fs::read_to_string(&path).map_err(|e| format!("read subject: {e}"))?;
            let (fns, _names, _, _) = keleusma::selfhost::parse_functions(&src);
            // The subject declares its three `classify` heads FIRST and then
            // `main`, so the group is the leading run. Asserted, not trusted: if
            // the file gains a function the seed would silently describe a
            // different program, and a rejected seed looks exactly like coverage.
            if fns.len() != 4 {
                return Err(format!(
                    "subject shape changed: expected 4 parsed heads in \
                     06_multiheaded.kel, got {}",
                    fns.len()
                ));
            }
            let heads: Vec<&keleusma::selfhost::ParsedFn> = fns[..3].iter().collect();
            // The parameter count is DERIVED from the compiled subject's signature
            // for the multiheaded chunk, never written as a literal, so the two
            // cannot drift apart.
            let subj = compile(
                &parse(&tokenize(&src).map_err(|e| format!("subject lex: {e:?}"))?)
                    .map_err(|e| format!("subject parse: {e:?}"))?,
            )
            .map_err(|e| format!("subject compile: {e:?}"))?;
            let idx = subj
                .chunks
                .iter()
                .position(|c| c.name == "classify")
                .ok_or("subject has no `classify` chunk")?;
            let pc = subj
                .signatures
                .get(idx)
                .map(|s| s.params.len())
                .ok_or("subject carries no signature for `classify`")?;
            if pc == 0 {
                return Err("`classify` reports zero parameters".into());
            }
            keleusma::selfhost::seed_reconstruct_multihead_shared(&vm, &heads, pc)
        }
        other => return Err(format!("no arm for {other}")),
    };
    // The stage's own layout sizes it; the harness must not assume they agree.
    let want = shared_data_bytes_for(m);
    if seed.len() != want {
        return Err(format!(
            "size mismatch: accessor {} vs harness {want}",
            seed.len()
        ));
    }
    Ok(seed)
}

fn seed_len_bytes(m: &Module, buf: &mut [u8]) -> bool {
    let (Some(len_off), Some(bytes_off)) = (shared_offset(m, "len"), shared_offset(m, "bytes"))
    else {
        return false;
    };
    let (len_off, bytes_off) = (len_off as usize, bytes_off as usize);
    if len_off + 8 > buf.len() || bytes_off + PAYLOAD.len() > buf.len() {
        return false;
    }
    buf[len_off..len_off + 8].copy_from_slice(&(PAYLOAD.len() as u64).to_le_bytes());
    buf[bytes_off..bytes_off + PAYLOAD.len()].copy_from_slice(PAYLOAD);
    true
}

struct Run {
    results: Vec<i64>,
    log: Vec<String>,
    shared: Vec<u8>,
}

fn run_vm(
    m: &Module,
    table: &[NativeEntry],
    seed: usize,
    preseed: Option<&[u8]>,
) -> Result<Run, String> {
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
    for (idx, (name, argc, shape)) in table.iter().enumerate() {
        if name.is_empty() {
            continue;
        }
        let (n, ac) = (name.clone(), *argc);

        // **A native whose RECORDED return shape is a composite is stubbed as
        // one.** `register_native_closure` cannot: it has no arena, so it can
        // only return a scalar, and the native side would then dereference that
        // scalar as a body address. `register_native_with_ctx_closure` supplies
        // the arena, which is what makes a faithful body possible at all.
        if let Some((kind, size)) = *shape {
            assert!(
                size as usize <= COMPOSITE_RET_CAP,
                "native `{n}` returns a {size}-byte composite; the stub table caps at \
                 {COMPOSITE_RET_CAP}. Raise the cap rather than truncating."
            );
            let n2 = n.clone();
            vm.register_native_with_ctx_closure(name, move |ctx, args: &[Value]| {
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
                LOG.with(|l| l.borrow_mut().push(format!("{n2}({})", parts.join(", "))));

                let wb = ctx.word_bytes;
                let fc = keleusma::flat_value::FlatComposite::build_in_arena(
                    ctx.arena,
                    size as usize,
                    |dst| {
                        composite_stub_bytes(size, wb, dst);
                        Ok(())
                    },
                )
                .map_err(|_| {
                    keleusma::vm::VmError::NativeError("arena exhausted in composite stub".into())
                })?
                .ok_or_else(|| {
                    keleusma::vm::VmError::NativeError("composite stub body not flat".into())
                })?;
                // Every composite body has a `Flat` variant, so all four kinds
                // are constructible from the same bytes. Kind is taken from the
                // RECORDED shape rather than guessed.
                Ok(match kind {
                    3 => Value::Enum(keleusma::bytecode::EnumBody::Flat(fc)),
                    2 => Value::Struct(keleusma::bytecode::StructBody::Flat(fc)),
                    1 => Value::Array(keleusma::bytecode::ArrayBody::Flat(fc)),
                    _ => Value::Tuple(keleusma::bytecode::TupleBody::Flat(fc)),
                })
            });
            continue;
        }

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
    match preseed {
        Some(bytes) => shared.copy_from_slice(bytes),
        None => {
            seed_len_bytes(m, &mut shared);
        }
    }
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

fn run_native(
    m: &Module,
    table: &[NativeEntry],
    seed: usize,
    preseed: Option<&[u8]>,
) -> Option<Run> {
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
    common::maybe_optimize(&lm);

    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");

    // Bind each declared native to its stub. The declaration is looked up FROM
    // THE MODULE, so the harness never reproduces `native_symbol`'s mangling —
    // getting that wrong is what segfaulted an earlier attempt.
    let by_symbol: std::collections::BTreeMap<String, usize> = table
        .iter()
        .enumerate()
        .filter(|(_, (n, _, _))| !n.is_empty())
        .map(|(i, (n, _, _))| {
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
    {
        // Seeded into the SAME offsets as the VM side. The segment is plain
        // bytes, so one helper serves both and there is no second encoding to
        // drift.
        let (body, _) = shared.split_at_mut(n_shared);
        match preseed {
            // **The SAME bytes, not a second construction.** A differential whose
            // two sides build their own seed compares two encodings rather than
            // two lowerings.
            Some(bytes) => body.copy_from_slice(bytes),
            None => {
                seed_len_bytes(m, body);
            }
        }
    }
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
/// **The list is down to ONE**, and every departure was forced by the
/// set-equality assertion below rather than chosen. The stages that left did so
/// because `stage_seed` hands them a real input built by the driver's own public
/// accessors, so the bytes are the ones a real driver supplies and this harness
/// carries no second encoding of any input format.
///
/// The one that remains is not waiting on an accessor. It cannot be driven from a
/// single seeded buffer at all, for the reason recorded beside it. Seeding it
/// anyway would produce a run that agrees and means nothing, which is the precise
/// defect this list exists to prevent.
const KNOWN_VACUOUS: &[&str] = &[
    // `lexer.kel` LEFT this list on 2026-08-15, and the set-equality assertion
    // is what noticed. It declares the documented `len` + `bytes` host
    // convention, so `seed_len_bytes` now gives it a real payload and it does
    // real work inside this harness rather than only in `stage_differential`.
    //
    // `verify_depth.kel`, `verify_typed.kel` and `verify_structural.kel` LEFT on
    // 2026-08-16, by the same mechanism. The
    // `v0.2.3` line landed per-item seed accessors (`fa649ec3`) that this line
    // requested, and `stage_seed` now hands it a REAL compiled chunk built by
    // `seed_verify_depth_shared` -- the driver's own encoding, not a second one
    // reproduced here.
    //
    // **`verify_datalayout.kel` will NOT leave this list by seeding, and that is
    // correct rather than a gap.** It has no accessor by joint agreement: its
    // verdict accumulates across three differently-encoded phases in the
    // retained buffer, with a whole-module contiguity comparison at the end, so
    // a single seeded buffer cannot produce a verdict at all. Do not invent a
    // batch-zero seed for it -- it would run, agree, and mean nothing.
    //
    // **`reconstruct.kel` LEFT on 2026-08-16, and the report that had kept it
    // here was WRONG.** This line told the `v0.2.3` line the stage was blocked
    // because `ParsedFn`'s producer is private. It is not: `parse_functions` is
    // `pub` and returns the parsed heads, so the multiheaded path was reachable
    // the whole time and no new accessor was needed. The `v0.2.3` line caught
    // the error. Only the single-head `seed_reconstruct_shared` was ever truly
    // blocked, and it still is, on field accessors that are not on this tree.
    "verify_datalayout.kel",
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

/// **THE `Trap` OBSERVABLE.**
///
/// `(name, expected VM fault, kind)`. A name starting `synthetic:` is compiled
/// from a source string below; anything else is a corpus file.
///
/// **THE TWO KINDS ARE NOT INTERCHANGEABLE, and conflating them cost a whole
/// implementation of this test.** The first version used only corpus files that
/// fault, and it passed. It also proved nothing about `Op::Trap`: `faulty.kel`
/// faults through the emitter's DIVISION GUARD and `rogue_dungen.kel` through its
/// BOUNDS CHECK, neither of which is the opcode. Measured afterwards with
/// `dump_opcode_module_map`: **no module in the shipped corpus that faults on the
/// virtual machine emits `Op::Trap` at all.** Mutating `Op::Trap` left the test
/// green, which is how the gap surfaced.
///
/// So a synthetic subject is not a convenience here; it is the only way to reach
/// the opcode. A multiheaded function whose guards all fail emits
/// `Trap(NoMatchingHead)`.
const TRAP_SUBJECTS: &[(&str, &str, TrapKind)] = &[
    ("synthetic:no_matching_head", "NoMatchingHead", TrapKind::Op),
    ("faulty.kel", "DivisionByZero", TrapKind::Guard),
    ("rogue_dungen.kel", "IndexOutOfBounds", TrapKind::Guard),
];

#[derive(Clone, Copy, PartialEq, Debug)]
enum TrapKind {
    /// Reaches an `Op::Trap` instruction. Asserted, not assumed.
    Op,
    /// Faults through an emitter-inserted guard (division, bounds). Valuable,
    /// and NOT evidence about `Op::Trap`.
    Guard,
}

/// A multiheaded function with no matching head, which is what emits `Op::Trap`.
const SYNTHETIC_NO_MATCHING_HEAD: &str = "\
fn pick(x: Word) -> Word when x > 100 { 1 }
fn pick(x: Word) -> Word when x < 0 { 2 }
fn main(a: Word) -> Word { pick(a) }
";

fn subject_source(name: &str) -> Option<String> {
    if let Some(key) = name.strip_prefix("synthetic:") {
        return match key {
            "no_matching_head" => Some(SYNTHETIC_NO_MATCHING_HEAD.to_string()),
            _ => None,
        };
    }
    let path = sources()
        .into_iter()
        .find(|p| p.file_name().unwrap_or_default().to_string_lossy() == name)?;
    let raw = std::fs::read_to_string(&path).ok()?;
    if path.to_string_lossy().contains("/rtos/") {
        let prelude = std::fs::read_to_string("../examples/rtos/scripts/prelude.kel").ok()?;
        Some(format!("{prelude}\n{raw}"))
    } else {
        Some(raw)
    }
}

/// **NO MODULE IS CURRENTLY EXCLUDED, and `led.kel`'s departure is the reason
/// this list is empty rather than deleted.**
///
/// `led.kel` was here because its virtual-machine run faulted with
/// `NoMatchingArm` while its native side died with **SIGSEGV, not SIGTRAP** — the
/// two sides faulting for DIFFERENT reasons, which would have been a false
/// agreement. The cause was the generic stub returning a plain integer where
/// `host::gpio_set` records a sixteen-byte enum body, which the native side then
/// dereferenced as an address.
///
/// **The stub now returns a real composite body on both sides, so `led.kel` does
/// not fault at all** and is neither a subject nor an exclusion.
///
/// **THAT COST A TRAP SUBJECT RATHER THAN GAINING ONE, and the reasoning is
/// worth keeping.** `led.kel` DOES emit `Op::Trap` (asserted in
/// `does_led_kel_reach_op_trap`), so closing the exemption looked like it would
/// hand this observable its first real-module opcode subject. It does the
/// opposite: `led.kel` matches both `Status::Ok` and `Status::Err(code)`, so a
/// FAITHFUL stub returns a valid variant, an arm matches, and the trap is never
/// reached. Reaching it needs a discriminant matching no variant — an unfaithful
/// stub, and a false agreement. **The two are mutually exclusive and
/// faithfulness wins.** The `Op::Trap` subject remains synthetic.
const TRAP_NOT_SUBJECTS: &[(&str, &str)] = &[];

/// Marker the child prints immediately before entering native code.
///
/// **The vacuity guard.** Without it, a child that died before reaching the
/// native call — a compile error, a missing file, a panic in setup — is
/// indistinguishable from one that trapped, because the parent would only see
/// "did not exit 0".
const TRAP_CHILD_MARKER: &str = "TRAP-CHILD-ENTERING-NATIVE";

/// The child half: runs ONE subject natively and is expected to die.
///
/// A no-op unless `KEL_TRAP_CHILD` names one, so the ordinary suite never pays
/// for it and never dies from it.
#[test]
fn trap_child_runs_one_module_natively() {
    let Ok(want) = std::env::var("KEL_TRAP_CHILD") else {
        return;
    };
    let src = subject_source(&want).unwrap_or_else(|| panic!("no subject named {want}"));
    let m = compile(&parse(&tokenize(&src).expect("lex")).expect("parse")).expect("compile");
    let table = native_table(&m);

    // Printed and FLUSHED before the native call, so the parent can tell "it
    // trapped" from "it never got here".
    println!("{TRAP_CHILD_MARKER}");
    use std::io::Write;
    std::io::stdout().flush().ok();

    let _ = run_native(&m, &table, 0, None);

    // Reaching this line means the native side did NOT trap where the virtual
    // machine faults. Exit non-zero WITHOUT a signal, which the parent reports
    // distinctly from a clean trap.
    println!("TRAP-CHILD-SURVIVED");
    std::process::exit(3);
}

/// **Does the native side agree that a faulting program FAULTS?**
///
/// This closes the one hole the mutation census could not. `Op::Trap` was
/// undetected across all 28 modules that emit it, and no seed could change that:
/// `every_lowering_module_executes_or_is_exempt` runs the virtual machine FIRST
/// precisely so a trapping module becomes a named exemption rather than a
/// `SIGTRAP` that kills the run — so a module that REACHES a trap is never
/// compared, and a module that IS compared reached none.
///
/// The observable therefore changes: not a returned value, but **the fact of the
/// fault**. The virtual machine reports an error; natively `llvm.trap` raises
/// `SIGTRAP`. Comparing those needs the native side in its own process.
///
/// **Cost control, stated because it is a real trade.** Under `KEL_ONLY_MODULE`
/// — how `tools/mutation_sweep.py` drives this binary — only the `Op::Trap`
/// subject runs, because that is the one the sweep needs and it is one spawn
/// rather than three. A full run drives all of them.
#[test]
fn a_trapping_programs_native_side_dies_with_sigtrap() {
    use std::os::unix::process::ExitStatusExt;

    let filtered = std::env::var("KEL_ONLY_MODULE").is_ok();
    let (mut op_checked, mut guard_checked) = (0usize, 0usize);

    for (name, want_fault, kind) in TRAP_SUBJECTS {
        if filtered && *kind != TrapKind::Op {
            continue;
        }
        let src = subject_source(name).unwrap_or_else(|| panic!("subject {name} is missing"));
        let m = compile(&parse(&tokenize(&src).expect("lex")).expect("parse")).expect("compile");

        // **THE ASSERTION THAT WOULD HAVE CAUGHT THE FIRST VERSION.** A subject
        // labelled `Op` must actually contain the opcode; a `Guard` subject must
        // not, or the two kinds are not being told apart.
        let has_trap = m
            .chunks
            .iter()
            .any(|c| c.ops.iter().any(|o| matches!(o, Op::Trap(_))));
        match kind {
            TrapKind::Op => assert!(
                has_trap,
                "{name} is labelled an Op::Trap subject and emits no Op::Trap. \
                 Mutating Op::Trap could not affect it, so it would prove nothing."
            ),
            TrapKind::Guard => assert!(
                !has_trap,
                "{name} is labelled a guard subject but DOES emit Op::Trap. Relabel it \
                 -- the distinction is what stops guard coverage being read as opcode coverage."
            ),
        }

        let table = native_table(&m);

        // **The premise, checked rather than assumed.** If it stops faulting,
        // this is comparing nothing and must say so instead of passing.
        let vm_err = match run_vm(&m, &table, 0, None) {
            Err(e) => e,
            Ok(_) => panic!(
                "{name} no longer faults on the VM side, so it is not a trap subject any more. \
                 Do not delete the row silently -- `NATIVE_EXEMPTION_AUDIT.md` predicts exactly \
                 that for the stub-artefact subjects."
            ),
        };
        assert!(
            vm_err.contains(want_fault),
            "{name}: expected the VM to fault with {want_fault}, got {vm_err}"
        );

        let out = std::process::Command::new(std::env::current_exe().expect("current exe"))
            .args([
                "--exact",
                "trap_child_runs_one_module_natively",
                "--nocapture",
            ])
            .env("KEL_TRAP_CHILD", name)
            .env_remove("KEL_ONLY_MODULE")
            .output()
            .expect("spawn the child");
        let stdout = String::from_utf8_lossy(&out.stdout);

        assert!(
            stdout.contains(TRAP_CHILD_MARKER),
            "{name}: the child never reached the native call, so its death says nothing \
             about the lowering. This is the vacuity guard firing. stdout: {stdout}\nstderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            !stdout.contains("TRAP-CHILD-SURVIVED"),
            "{name}: the native side RETURNED where the VM faults with {want_fault}. \
             A program that must abort instead produced a value."
        );
        assert_eq!(
            out.status.signal(),
            Some(SIGTRAP),
            "{name}: expected the native side to die with SIGTRAP; got code {:?}, signal {:?}",
            out.status.code(),
            out.status.signal()
        );
        match kind {
            TrapKind::Op => op_checked += 1,
            TrapKind::Guard => guard_checked += 1,
        }
    }

    // **At least one Op::Trap subject, always.** Without this the test could pass
    // on guard subjects alone, which is precisely the state the first version
    // shipped in.
    assert!(
        op_checked > 0,
        "no Op::Trap subject was checked, so this asserts nothing about the opcode"
    );
    println!(
        "  TRAP OBSERVABLE: {op_checked} Op::Trap and {guard_checked} guard subject(s) died with SIGTRAP"
    );

    // **The exclusion is ASSERTED, not commented.** If the composite return path
    // lands and the stub returns a body address, this fires and `led.kel` should
    // MOVE into TRAP_SUBJECTS -- a comment would have gone stale instead.
    if !filtered {
        for (name, why) in TRAP_NOT_SUBJECTS {
            let Some(src) = subject_source(name) else {
                continue;
            };
            let Ok(_m) = compile(&parse(&tokenize(&src).expect("lex")).expect("parse")) else {
                continue;
            };
            let out = std::process::Command::new(std::env::current_exe().expect("current exe"))
                .args([
                    "--exact",
                    "trap_child_runs_one_module_natively",
                    "--nocapture",
                ])
                .env("KEL_TRAP_CHILD", name)
                .env_remove("KEL_ONLY_MODULE")
                .output()
                .expect("spawn the child");
            assert_ne!(
                out.status.signal(),
                Some(SIGTRAP),
                "{name} now dies with SIGTRAP and is a valid trap subject. It was excluded \
                 because: {why}. MOVE it into TRAP_SUBJECTS rather than deleting this check."
            );
        }
    }
}

/// `SIGTRAP` is 5 on Linux and on macOS. Named so the one place another platform
/// would change is obvious.
const SIGTRAP: i32 = 5;

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

        // **A real stage input, from the driver's own accessor.** Computed once
        // and handed to BOTH sides, so the comparison is of two lowerings rather
        // than of two encodings.
        let stage = stage_seed(&m, &name);
        // **Say whether a seed was APPLIED, not just whether one exists.** A seed
        // the accessor declined to build and a seed the stage silently rejects
        // produce the same downstream number -- "still vacuous" -- and only the
        // first is an instrument fault. Printed for every stage that has an arm.
        if STAGE_SEEDED.contains(&name.as_str()) {
            println!(
                "  stage seed for {name}: {}",
                match &stage {
                    Ok(b) => format!(
                        "APPLIED, {} bytes, {} non-zero",
                        b.len(),
                        b.iter().filter(|x| **x != 0).count()
                    ),
                    Err(why) => format!("NOT BUILT -- {why}"),
                }
            );
        }

        let mut runs: Vec<(usize, Run, Run)> = Vec::new();
        let mut bail: Option<String> = None;
        for seed in 0..seeds {
            // **The virtual machine runs FIRST, and that ordering is load-bearing.**
            // A module that traps reports an error here; natively the same trap is
            // `llvm.trap`, which kills the process with SIGTRAP and takes the whole
            // harness with it. Asking the tolerant side first turns a fatal signal
            // into a named exemption.
            let v = match run_vm(&m, &table, seed, stage.as_deref().ok()) {
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
            let Some(n) = run_native(&m, &table, seed, stage.as_deref().ok()) else {
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

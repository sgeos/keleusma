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
    /// `(native index, argument position)` pairs the VIRTUAL MACHINE observed as
    /// non-scalar on this run.
    ///
    /// **`run_vm` runs before `run_native` for every seed** -- verified in the
    /// source, not assumed -- so the native stub can mask exactly the positions
    /// the virtual machine already substituted a zero for. Without this the two
    /// logs differ in one field (an arena handle against a pointer) and the
    /// WHOLE MODULE was discarded over it, leaving the call sequence, every
    /// scalar argument, the return value and the shared segment uncompared.
    ///
    /// **This is a RUNTIME mask, not a contract.** It rests on what the virtual
    /// machine saw on a particular run rather than on anything the module
    /// declares -- `Module` carries `native_names` and `native_return_shapes`
    /// and no parameter types at all.
    static REF_POSITIONS: RefCell<std::collections::BTreeSet<(usize, usize)>> =
        const { RefCell::new(std::collections::BTreeSet::new()) };
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
    // **MASK THE POSITIONS THE VIRTUAL MACHINE SAW AS NON-SCALAR.**
    //
    // A reference argument is an arena HANDLE to the virtual machine and a
    // POINTER here, so the two never render as the same integer. The virtual
    // machine already substitutes zero for it; without the same substitution
    // here the logs differ in one field and the WHOLE MODULE was discarded --
    // ten of the corpus's exemptions were exactly this, the largest single
    // class, and the call sequence, every scalar argument, the return value and
    // the shared segment went uncompared over it.
    //
    // **Nothing is dereferenced.** The pointer is replaced, not read. Decoding
    // it would need the string application binary interface, which is ruled
    // PROVISIONAL, and a wrong assumption here is a SEGFAULT rather than a
    // failed assertion since the native side runs in-process through the JIT.
    //
    // **Only the observed positions are masked.** Masking a scalar would hide a
    // real disagreement, so the key is the exact `(native, position)` pair.
    let parts: Vec<String> = args
        .iter()
        .enumerate()
        .map(|(pos, a)| {
            if REF_POSITIONS.with(|r| r.borrow().contains(&(idx, pos))) {
                "0".to_string()
            } else {
                a.to_string()
            }
        })
        .collect();
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

/// One external native's virtual-machine side, by table index.
///
/// **`register_external_native` takes a BARE `fn`, not a closure**, so it cannot
/// capture the index the way `register_native_closure` does. The native side
/// already solves exactly this with the `kel_stub_NN` family recovering identity
/// from a thread-local table; this mirrors that rather than inventing a second
/// mechanism, and reuses the same table.
fn vm_external_record(idx: usize, args: &[Value]) -> Result<Value, keleusma::vm::VmError> {
    let (name, argc, _) = TABLE.with(|t| {
        t.borrow()
            .get(idx)
            .cloned()
            .unwrap_or_else(|| (format!("<unmapped #{idx}>"), 0, None))
    });
    let vals: Vec<i64> = args
        .iter()
        .take(argc)
        .enumerate()
        .map(|(pos, v)| match v {
            Value::Int(x) => *x,
            Value::Byte(b) => i64::from(*b),
            Value::Bool(b) => i64::from(*b),
            _ => {
                SAW_REF_ARG.with(|f| *f.borrow_mut() = true);
                REF_POSITIONS.with(|r| {
                    r.borrow_mut().insert((idx, pos));
                });
                0
            }
        })
        .collect();
    let parts: Vec<String> = vals.iter().map(|a| a.to_string()).collect();
    LOG.with(|l| l.borrow_mut().push(format!("{name}({})", parts.join(", "))));
    Ok(Value::Int(stub_value(idx, &vals)))
}

macro_rules! external_stubs {
    ($($n:literal => $sym:ident),* $(,)?) => {
        $(
            fn $sym(args: &[Value]) -> Result<Value, keleusma::vm::VmError> {
                vm_external_record($n, args)
            }
        )*
        /// The bare `fn` for table index `i`, or `None` past the family's end.
        ///
        /// **A FIXED FAMILY WITH AN EXPLICIT END.** Returning `None` past it makes
        /// the module fall back to a verified registration, which the virtual
        /// machine then refuses at call-site dispatch with a message naming the
        /// mismatch. Silently binding the wrong function would be worse.
        fn external_stub_for(i: usize) -> Option<fn(&[Value]) -> Result<Value, keleusma::vm::VmError>> {
            match i { $($n => Some($sym),)* _ => None }
        }
    };
}

external_stubs!(
    0 => vm_ext_00, 1 => vm_ext_01, 2 => vm_ext_02, 3 => vm_ext_03,
    4 => vm_ext_04, 5 => vm_ext_05, 6 => vm_ext_06, 7 => vm_ext_07,
);

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
/// **Cost.** 35s at 4 seeds, 58s at 24 when first recorded. Re-measured
/// 2026-08-20 on a different host: **30s at 24, 53s at 64**. Wall-clock figures
/// are host-dependent and the older pair is not thereby wrong. Sublinear because
/// a `Stream` entry and a zero-parameter entry both keep a single seed, so the
/// sweep only widens the modules it can widen.
///
/// **The claim that this is "paid on every run including CI" was FALSE and is
/// removed.** Continuous integration does not build `native_codegen` at all —
/// the package has a detached `[workspace]` on purpose. The cost is paid
/// locally and by `tools/mutation_sweep.py`, which is a real cost and a smaller
/// one than "every CI run".
///
/// # IS 24 ITSELF A HOLE? MEASURED 2026-08-20: NO EVIDENCE THAT IT IS
///
/// 4 was a hole — that is why this constant exists. Nobody had asked the same
/// question of 24. Raised to **64** and re-run: **482 (module, seed) pairs
/// became 1242, and NOT ONE new disagreement appeared.** 19 of the 45 comparable
/// modules widen; the other 26 keep a single vector by construction.
///
/// **The widening is arithmetic, not an impression**: 19 x 64 + 26 = 1242 and
/// 19 x 24 + 26 = 482, both measured rather than derived. So the negative result
/// is over **760 additional comparisons**, not over a probe that quietly did
/// nothing.
///
/// **What this does NOT establish.** It is not a proof that 24 suffices in
/// general. `args_for_seed` gives every parameter the SAME value for seeds
/// past 3, so widening explores a diagonal and not the product space; a defect
/// needing two parameters to differ would be invisible at any count. Left at 24
/// because 64 bought nothing measurable and the cost is real.
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

/// The subjects the three `verify_*` stages are driven against.
///
/// **`02_struct_field.kel` was the ONLY subject until 2026-08-21**, so each
/// seeded stage saw one chunk of one program while agreeing at sixty ticks. A
/// stage mis-handling a construct absent from that file agreed at all sixty and
/// the gate reported it as agreeing. **That is the same shape as the hole seed 0
/// left for `Op::CmpLt`**, where `SLE` stood in for `SLT` across 126 sites and
/// the whole differential passed because no vector made two comparands equal.
///
/// Kept first so its behaviour is unchanged and any movement is attributable.
const STAGE_SUBJECTS: &[&str] = &[
    "02_struct_field.kel",
    "01_arithmetic.kel",
    "03_enum_match.kel",
    "04_for_in.kel",
    "05_pipeline.kel",
];

/// Every seeded stage paired with every subject it can be driven against.
///
/// **`reconstruct.kel` DOES NOT WIDEN, and the obstacle is real rather than
/// effort.** Its seed is not "a chunk with a defect" — it is a parsed multiheaded
/// function group, and it asserts its subject declares exactly four heads. A
/// different subject does not merely change the input; it fails the shape check,
/// which is the assertion doing its job. Widening it needs more corpus files
/// containing a multiheaded group, and the corpus has one.
fn stage_seeds(m: &Module, name: &str) -> Vec<(&'static str, Result<Vec<u8>, String>)> {
    if !STAGE_SEEDED.contains(&name) {
        return vec![("-", Err("no arm for this stage".into()))];
    }
    if name == "reconstruct.kel" {
        return vec![(
            "06_multiheaded.kel",
            stage_seed_for(m, name, "06_multiheaded.kel"),
        )];
    }
    STAGE_SUBJECTS
        .iter()
        .map(|s| (*s, stage_seed_for(m, name, s)))
        .collect()
}

fn stage_seed_for(m: &Module, name: &str, subject_file: &str) -> Result<Vec<u8>, String> {
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
        .filter(|p| p.file_name().unwrap_or_default().to_string_lossy() == subject_file)
        .find_map(|p| {
            let src = std::fs::read_to_string(&p).ok()?;
            compile(&parse(&tokenize(&src).ok()?).ok()?).ok()
        })
        .ok_or_else(|| {
            format!("subject {subject_file} is not in the corpus or does not compile")
        })?;
    let cix = (0..subject.chunks.len())
        .max_by_key(|&i| subject.chunks[i].ops.len())
        .ok_or("subject has no chunk")?;

    // **THE SUBJECT CHUNK CARRIES A DEFECT, and that is what makes these three
    // stages observable at all.** Each writes its verdict to an `out_reject` slot
    // as 1 for reject and 0 for accept, and the seeded buffer already holds 0. On
    // a WELL-FORMED chunk the verdict is accept, so the stage runs, decides, and
    // changes nothing this harness can compare -- which is precisely why all three
    // sat in `KNOWN_VACUOUS`. Injecting the defect each stage checks for moves the
    // verdict to 1 and the segment with it.
    //
    // **Each stage gets ITS OWN defect.** One mutation for all three was tried and
    // `verify_structural.kel` accepted the operand-stack underflow, correctly: it
    // latches block-nesting malformation, not depth. Reusing one mutation would
    // have read as "that stage cannot be made to reject", which is false.
    //
    // **The ACCEPT direction is asserted in `probe_stage_vacuity`**, not here. A
    // rejecting seed alone would be satisfied by a stage that rejects everything.
    let mut subject = subject;
    match name {
        "verify_depth.kel" | "verify_typed.kel" => subject.chunks[cix].ops.insert(0, Op::PopN(4)),
        "verify_structural.kel" => subject.chunks[cix].ops.insert(0, Op::If(1)),
        _ => {}
    }
    let subject = subject;
    let chunk = &subject.chunks[cix];

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
            let sig = subject.signatures.get(cix);
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
    /// Whether the run CHANGED the shared segment from the bytes it was given.
    ///
    /// Not the same question as whether the segment is non-zero. A seeded module
    /// starts non-zero, so a zero test answers "was it seeded", never "did it do
    /// anything" — and every seeded stage would leave the vacuous set the moment
    /// it was seeded, whether or not it ran. This is the honest form of that
    /// question and it is what `is_vacuous` reads.
    wrote_shared: bool,
    /// The composite return body as FLAT BYTES, one entry per result, empty
    /// for a scalar-returning entry or when the bytes could not be captured.
    ///
    /// **Both sides are reduced to the SAME representation before any claim.**
    /// Comparing a native pointer against a decoded value would manufacture a
    /// difference; that error has already been made once on this line.
    ret_bytes: Vec<Vec<u8>>,
}

fn run_vm(
    m: &Module,
    table: &[NativeEntry],
    seed: usize,
    preseed: Option<&[u8]>,
) -> Result<Run, (String, ExemptClass)> {
    let _ = take_log();
    SAW_REF_ARG.with(|f| *f.borrow_mut() = false);
    // Cleared per run so a position observed for one module cannot mask a
    // scalar in the next.
    REF_POSITIONS.with(|r| r.borrow_mut().clear());
    let arena = arena_for(m);
    // Fallible: a module may refuse to LOAD for reasons unrelated to lowering —
    // a signature requirement, for one. That is an exemption with a stated
    // reason, not a harness crash.
    let mut vm = match Vm::new(m.clone(), &arena) {
        Ok(v) => v,
        Err(e) => {
            return Err((
                format!("the VM refuses to load it: {e:?}"),
                ExemptClass::RefusedAtLoad,
            ));
        }
    };
    // **WHICH NATIVES DOES THE BYTECODE CALL EXTERNALLY?** Derived from the
    // module's own ops, so this is a CONTRACT rather than a runtime inference --
    // unlike the reference-argument mask, which rests on what the virtual
    // machine happened to observe.
    //
    // A module declaring `use external host::f` emits `Op::CallExternalNative`,
    // and the virtual machine REFUSES a verified registration against it at
    // call-site dispatch. This harness registered everything as verified, so
    // `external_native_witness.kel` could not be driven at all -- and
    // `CallExternalNative` was the last opcode counted as LOWERED with nothing
    // ever having executed it.
    // **THE TABLE MUST BE POPULATED ON THIS SIDE TOO, and forgetting it produced
    // a real disagreement rather than a crash.**
    //
    // The closure registrations CAPTURE their name and arity. A bare `fn` cannot,
    // so `vm_external_record` recovers them from `TABLE` -- which until now was
    // set only in `run_native`. The virtual machine therefore read an arity of
    // ZERO, logged the call with no arguments, and returned `stub_value(0, [])`
    // = 7 against the native side's `stub_value(0, [4])` = 221.
    //
    // It surfaced as a DISAGREEMENT, which is the differential doing its job on
    // a defect in the harness rather than in the lowering.
    TABLE.with(|t| *t.borrow_mut() = table.to_vec());

    let external_indices: std::collections::BTreeSet<usize> = m
        .chunks
        .iter()
        .flat_map(|c| c.ops.iter())
        .filter_map(|o| match o {
            Op::CallExternalNative(i, _) => Some(usize::from(*i)),
            _ => None,
        })
        .collect();

    for (idx, (name, argc, shape)) in table.iter().enumerate() {
        if name.is_empty() {
            continue;
        }
        let (n, ac) = (name.clone(), *argc);

        // An externally-called native must be registered externally, or the
        // machine refuses the call. The attestation is the invocation-count
        // bound; this harness drives bounded programs and states one.
        if external_indices.contains(&idx)
            && let Some(f) = external_stub_for(idx)
        {
            vm.register_external_native(&n, f, 64);
            continue;
        }
        // Reached when the native is NOT externally called, and also when it is
        // but the index is past the stub family's end. The second case falls
        // through to the verified registration, which the machine refuses BY
        // NAME -- visible rather than silently bound to the wrong function.
        // (These were two nested conditions until the collapse; the comment now
        // has to cover both arrivals, because it sits on the shared path.)

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
                    .enumerate()
                    .map(|(pos, v)| match v {
                        Value::Int(x) => *x,
                        Value::Byte(b) => i64::from(*b),
                        Value::Bool(b) => i64::from(*b),
                        _ => {
                            SAW_REF_ARG.with(|f| *f.borrow_mut() = true);
                            // Record WHICH position, so the native side can mask
                            // the same one instead of the module being discarded.
                            REF_POSITIONS.with(|r| {
                                r.borrow_mut().insert((idx, pos));
                            });
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
            // **THE SECOND OF TWO REGISTRATION PATHS, and patching only the
            // other one is how the mask silently did nothing.** A native with a
            // recorded composite return shape registers through the ctx closure
            // above; everything else lands here. `host::song_name` is here.
            let vals: Vec<i64> = args
                .iter()
                .take(ac)
                .enumerate()
                .map(|(pos, v)| match v {
                    Value::Int(x) => *x,
                    Value::Byte(b) => i64::from(*b),
                    Value::Bool(b) => i64::from(*b),
                    _ => {
                        SAW_REF_ARG.with(|f| *f.borrow_mut() = true);
                        REF_POSITIONS.with(|r| {
                            r.borrow_mut().insert((idx, pos));
                        });
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
    // **The segment as the module FOUND it**, kept so vacuity can ask whether the
    // run changed anything rather than whether the buffer is non-zero. Seeding
    // makes a non-zero test true before the module executes a single op, so
    // without this a seeded module leaves the vacuous set BY CONSTRUCTION and the
    // departure is evidence of nothing. See `is_vacuous`.
    let initial_shared = shared.clone();
    let mut results = Vec::new();

    let first = match vm.call_with_shared(&mut shared, &vals) {
        Ok(v) => v,
        Err(e) => {
            return Err((
                format!("the VM refuses to run it: {e:?}"),
                ExemptClass::FaultsAndIsFaultComparable,
            ));
        }
    };
    results.push(scalar_of(&first));
    let mut ret_bytes: Vec<Vec<u8>> = vec![flat_ret_bytes(&first, &arena)];
    if m.chunks[entry].block_type == BlockType::Stream {
        for t in 1..TICKS {
            // One tick is a `Reset` leg then a `Yielded` leg, and the SAME reply
            // goes to both. A fresh reply on the Reset leg is silently discarded.
            let mut st = match vm.resume_with_shared(&mut shared, Value::Int(t)) {
                Ok(v) => v,
                Err(e) => {
                    return Err((
                        format!("the VM refuses to resume it: {e:?}"),
                        ExemptClass::FaultsAndIsFaultComparable,
                    ));
                }
            };
            if matches!(st, VmState::Reset) {
                st = match vm.resume_with_shared(&mut shared, Value::Int(t)) {
                    Ok(v) => v,
                    Err(e) => {
                        return Err((
                            format!("the VM refuses to resume it: {e:?}"),
                            ExemptClass::FaultsAndIsFaultComparable,
                        ));
                    }
                };
            }
            results.push(scalar_of(&st));
            ret_bytes.push(flat_ret_bytes(&st, &arena));
        }
    }
    // **THIS USED TO EXEMPT THE MODULE, AND NOW MASKS ONE FIELD INSTEAD.**
    //
    // A reference argument still cannot be compared: it is an arena handle here
    // and a pointer natively. But that is ONE FIELD, and discarding the module
    // over it left the call sequence, the native names, every scalar argument,
    // the return value and the shared segment uncompared. Ten modules -- the
    // largest exempt class -- were declined on that basis.
    //
    // **An exemption says nothing was compared; a masked comparison says
    // everything except this was.** `REF_POSITIONS` carries the exact positions
    // to both sides, so the strings are excluded and the rest is checked.
    //
    // The earlier decline was not wrong about its own subject -- it argued
    // against DEREFERENCING the pointer to compare string CONTENT, which is
    // still declined and still for three measured reasons. It simply never
    // considered the cheap version.
    Ok(Run {
        results,
        wrote_shared: shared != initial_shared,
        log: take_log(),
        shared,
        ret_bytes,
    })
}

/// A scalar outcome, or a stable marker for anything else.
///
/// A composite result is compared through the shared segment and the call log
/// rather than decoded here; marking it keeps the two sides comparable without
/// pretending to read a body the harness did not build.
/// PROBE: a composite result's FLAT BYTES, straight from the arena.
///
/// No layout opinion of ours: `FlatComposite::resolve` hands back the canonical
/// body. A scalar or unit result yields none.
fn flat_ret_bytes(st: &VmState, arena: &keleusma_arena::Arena) -> Vec<u8> {
    use keleusma::bytecode::{ArrayBody, EnumBody, StructBody, TupleBody};
    let v = match st {
        VmState::Yielded(v) | VmState::Finished(v) => v,
        _ => return Vec::new(),
    };
    let fc = match v {
        Value::Tuple(TupleBody::Flat(f)) => f,
        Value::Struct(StructBody::Flat(f)) => f,
        Value::Enum(EnumBody::Flat(f)) => f,
        Value::Array(ArrayBody::Flat(f)) => f,
        _ => return Vec::new(),
    };
    fc.resolve(arena).map(|b| b.to_vec()).unwrap_or_default()
}

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
    // The segment as the module found it. See `is_vacuous`: a seeded module is
    // non-zero before it runs, so vacuity must ask what the run CHANGED.
    let initial_shared = shared[..n_shared].to_vec();
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
    // **THE COMPOSITE RETURN, READ BACK FROM OUR OWN BUFFER.**
    //
    // The lowered entry returns a POINTER into `region`, which this harness
    // allocated and passed in as `rp`. That is what makes the read safe, and it
    // is the decisive difference from the reference-argument case declined
    // earlier: there the pointer came from the JIT and a wrong guess is a
    // segfault; here the buffer is ours and the address is bounds-checked
    // against it before a byte is touched.
    //
    // **The size is READ from the module**, `WireShape::Flat { size, .. }` on
    // the entry's recorded return. No offset, field order, or encoding is
    // invented here: the callee says WHERE and the module says HOW LONG.
    let ret_bytes: Vec<Vec<u8>> = {
        let size = match m.signatures.get(entry).map(|sg| &sg.ret) {
            Some(WireShape::Flat { size, .. }) => *size as usize,
            _ => 0,
        };
        let base = region.as_ptr() as usize;
        results
            .iter()
            .map(|r| {
                let addr = *r as usize;
                let ok = size > 0
                    && *r > 0
                    && addr >= base
                    && addr.saturating_add(size) <= base + n_region;
                if !ok {
                    return Vec::new();
                }
                // Safe: inside a buffer this function allocated, bounds checked.
                unsafe { core::slice::from_raw_parts(addr as *const u8, size) }.to_vec()
            })
            .collect()
    };

    shared.truncate(n_shared);
    Some(Run {
        results,
        wrote_shared: shared != initial_shared,
        log: take_log(),
        shared,
        ret_bytes,
    })
}

/// Do the two sides' captured composite-return bodies differ?
///
/// **Only pairs captured on both sides are compared.** A pair where either side
/// is empty is skipped: the native capture is bounds-checked and can legitimately
/// decline, and treating a declined read as a difference would invent
/// disagreements. The count of declined captures is reported separately so the
/// skip is visible rather than silent.
fn ret_pairs_differ(v: &Run, n: &Run) -> bool {
    v.ret_bytes
        .iter()
        .zip(&n.ret_bytes)
        .any(|(a, b)| !a.is_empty() && !b.is_empty() && a != b)
}

/// Did this run produce any observable work at all?
///
/// The harness compares three things, and a module that exits immediately is
/// trivial in all three at once: one repeated result, no host calls, and a shared
/// segment the run never changed. Two sides agreeing on that state assert nothing
/// about the emitter.
///
/// **THE THIRD TEST WAS "IS THE SEGMENT ALL ZERO" UNTIL 2026-08-16, AND SEEDING
/// DEFEATED IT.** A seeded module holds a non-zero segment before it executes a
/// single operation, so the zero test answered "was it seeded" rather than "did it
/// do anything", and **every stage left this list the moment it was given a seed,
/// whether or not the seed changed the run.** Three did, on 2026-08-15 and
/// 2026-08-16, and the headline moved from 40 to 44 on the strength of it. The
/// test now compares against the bytes the module was HANDED, so a seeded stage
/// that writes nothing is correctly vacuous again and those three returned to this
/// list. `reconstruct.kel` stayed out, which is the difference between a stage
/// that does observable work and a stage that merely receives an input.
///
/// This is the same defect class the file already documents twice over: an
/// experiment that cannot fail looks exactly like success.
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
    distinct.len() <= 1 && run.log.is_empty() && !run.wrote_shared
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
/// **The list GREW on 2026-08-16, and that is a repair rather than a regression.**
/// `is_vacuous` had asked whether the shared segment was all zero, which a seeded
/// module fails before it runs. Three `verify_*` stages had left this list on that
/// basis alone and are now back. The count they were credited with, 44 executed,
/// was measuring the fact that they had been seeded.
///
/// **Seeding a stage is not the same as a stage doing observable work.** Every
/// departure and every return was forced by the set-equality assertion below
/// rather than chosen.
///
/// **The three `verify_*` stages left again the same day, by a DIFFERENT and real
/// mechanism.** Each writes its verdict as 1 for reject and 0 for accept, and the
/// seeded buffer already holds 0, so a well-formed subject produced a decision
/// that changed nothing. They are now seeded with a chunk carrying the defect each
/// one actually checks for, the verdict moves to 1, and the segment moves with it.
/// The count returning to 45 is earned by a moved observable, **not** evidence
/// that dropping it to 42 was mistaken. That drop was correct and remains so.
const KNOWN_VACUOUS: &[&str] = &[
    // `lexer.kel` LEFT this list on 2026-08-15, and the set-equality assertion
    // is what noticed. It declares the documented `len` + `bytes` host
    // convention, so `seed_len_bytes` now gives it a real payload and it does
    // real work inside this harness rather than only in `stage_differential`.
    //
    // **`verify_depth.kel`, `verify_typed.kel` and `verify_structural.kel` left,
    // came back, and left again on 2026-08-16.** The first departure was an
    // artifact of the zero test and is recorded in `is_vacuous`. The second is
    // real: they are seeded with a chunk carrying the defect each checks for, so
    // the verdict moves from accept to reject and the segment changes.
    //
    // **Do NOT restore a well-formed subject to make them look better behaved.**
    // On a well-formed chunk all three accept, write the 0 that was already there,
    // and become vacuous again -- correctly. The accept direction is asserted in
    // `probe_stage_vacuity` so a stage that rejected EVERYTHING could not pass.
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
    //
    // **`reconstruct.kel` stayed OUT when the vacuity test was repaired**, which is
    // the difference that matters: it writes the reconstructed forest back into the
    // segment, so its departure survived asking whether the run changed anything
    // without needing a defective subject at all.
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
///
/// # THAT CORPUS FACT EXPIRED ON 2026-08-20, AND `wire.kel` IS THE COUNTEREXAMPLE
///
/// "No module in the shipped corpus that faults on the virtual machine emits
/// `Op::Trap` at all" was measured on 2026-08-14 and is **now false**.
/// `wire.kel` faults on the virtual machine — `IndexOutOfBounds` at tick 19 —
/// **and** emits `Op::Trap`. Pinned by
/// `the_no_faulting_module_emits_op_trap_fact_has_expired`, which fails if either
/// half stops holding, so the claim cannot quietly come back.
///
/// **It is still NOT admissible as a subject here, and the reason is the point.**
/// Neither kind fits, and that is the taxonomy being right rather than
/// insufficient:
///
/// - `Guard` is refused because the module emits `Op::Trap`.
/// - `Op` would be a lie about WHICH fault was observed: the virtual machine
///   reports a BOUNDS fault, not the opcode.
///
/// Admitting it under either label would let a `SIGTRAP` that might be the guard
/// be counted as opcode coverage, or the reverse. **`SIGTRAP` proves a fault, not
/// which**, so in a module containing both there is nothing to disambiguate them.
/// A synthetic subject remains the only way to reach the opcode ON PURPOSE.
const TRAP_SUBJECTS: &[(&str, &str, TrapKind)] = &[
    ("synthetic:no_matching_head", "NoMatchingHead", TrapKind::Op),
    ("faulty.kel", "DivisionByZero", TrapKind::Guard),
    ("rogue_dungen.kel", "IndexOutOfBounds", TrapKind::Guard),
];

/// **A CORPUS FACT THIS FILE ASSERTS HAS EXPIRED, and one counterexample settles it.**
///
/// `TRAP_SUBJECTS` records that no corpus module which faults on the virtual
/// machine emits `Op::Trap`. That was measured 2026-08-14 and justified the
/// synthetic subject. `wire.kel` has grown a great deal since and now does both.
///
/// **A universal claim falls to one counterexample**, so this names `wire.kel`
/// rather than re-walking the corpus. Both halves are asserted separately so a
/// failure says WHICH half moved: a module that stops faulting and a module that
/// stops emitting the opcode are different events with different consequences.
///
/// **What this does NOT establish.** It says nothing about whether the fault the
/// virtual machine reports IS the `Op::Trap` — it is not; it is a bounds fault.
/// That distinction is exactly why `wire.kel` is not a `TRAP_SUBJECTS` row.
#[test]
fn the_no_faulting_module_emits_op_trap_fact_has_expired() {
    let src = subject_source("wire.kel").expect("wire.kel is in the corpus");
    let m = compile(&parse(&tokenize(&src).expect("lex")).expect("parse")).expect("compile");

    let emits_trap = m
        .chunks
        .iter()
        .any(|c| c.ops.iter().any(|o| matches!(o, Op::Trap(_))));
    assert!(
        emits_trap,
        "wire.kel no longer emits Op::Trap. The 2026-08-14 corpus fact may hold again;          re-measure it across the corpus before restoring the claim in TRAP_SUBJECTS."
    );

    let table = native_table(&m);
    let err = match run_vm(&m, &table, 0, None) {
        Err((e, _class)) => e,
        Ok(_) => panic!(
            "wire.kel no longer faults on the virtual machine. That is a real change in \
             the module or the driver, and it would move wire.kel out of the exempt set \
             in every_lowering_module_executes_or_is_exempt. Re-measure both."
        ),
    };
    assert!(
        err.contains("IndexOutOfBounds"),
        "wire.kel faults, but not with the bounds fault measured on 2026-08-20; got {err}. \
         The KIND is what is pinned here, not the operands, which are computed and move."
    );
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum TrapKind {
    /// Reaches an `Op::Trap` instruction. Asserted, not assumed.
    Op,
    /// Faults through an emitter-inserted guard (division, bounds). Valuable,
    /// and NOT evidence about `Op::Trap`.
    Guard,
}

/// **WHY a module is exempt, attached AT THE SITE that exempts it.**
///
/// "19 exempt" reads as 19 modules this line fails to cover, and that is wrong
/// in both directions. Some cannot be compared by anything — a prelude has no
/// entry to run. Others are compared perfectly well by a DIFFERENT observable:
/// `wire.kel` faults on both sides at the same tick, which is agreement the
/// value differential is structurally unable to see, because a faulting module
/// has no values.
///
/// **Derived, not transcribed.** The class is chosen where the exemption is
/// created, so it follows the harness's own control flow. A table keyed on the
/// reason STRING would be a second opinion about code that is right here, and
/// would drift the moment a message was reworded.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ExemptClass {
    /// The reference compiler would not build it. Nothing downstream exists to
    /// compare, and it is not this line's surface.
    NotAcceptedByReference,
    /// No entry point. A prelude declares; it does not run.
    NoRunnableEntry,
    /// The virtual machine faults. **There are no values to compare, but the
    /// FACT of the fault is comparable** — this is the class `wire.kel` turned
    /// out to be in. Eligibility is not admission: a module that both emits
    /// `Op::Trap` and faults through a guard cannot be admitted to
    /// `TRAP_SUBJECTS`, because `SIGTRAP` cannot say which fired.
    FaultsAndIsFaultComparable,
    /// Covered by a hand-written differential instead. Real coverage, elsewhere.
    CoveredByAnotherHarness,
    /// A limit of this harness, not a fact about the module or the lowering.
    HarnessCapacity,
    /// The backend declines to lower it. **This one IS this line's deliverable.**
    BackendRefusal,
    /// The virtual machine would not LOAD it. Distinct from a run-time fault:
    /// nothing executed. Kept separate rather than folded into a harness limit
    /// because a genuine verifier rejection and a harness that declined to load
    /// a signed module both arrive here, and only the observed instance is the
    /// second.
    RefusedAtLoad,
}

impl ExemptClass {
    fn label(self) -> &'static str {
        match self {
            Self::NotAcceptedByReference => "not accepted by the reference compiler",
            Self::NoRunnableEntry => "no runnable entry",
            Self::FaultsAndIsFaultComparable => "faults; comparable by the FAULT observable",
            Self::CoveredByAnotherHarness => "covered by another harness",
            Self::HarnessCapacity => "a limit of this harness",
            Self::BackendRefusal => "the backend refuses to lower it",
            Self::RefusedAtLoad => "the VM would not load it (nothing executed)",
        }
    }
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
            Err((e, _class)) => e,
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
    let mut exempt: Vec<(String, String, ExemptClass)> = Vec::new();
    let mut obs_composite = 0usize;
    let mut cap_pairs_both = 0usize;
    let mut cap_pairs_declined = 0usize;
    let mut cap_partial: Vec<String> = Vec::new();
    let mut cap_never: Vec<String> = Vec::new();
    let mut obs_multi_result = 0usize;
    let mut obs_single_scalar_only = 0usize;
    // **NAMED, so the overlap with the Order-1 stages is MEASURABLE.** Two
    // counts that happen to be equal invite an inference about which
    // modules they contain; only the names can settle it.
    let mut obs_single_scalar_names: Vec<String> = Vec::new();
    let mut obs_single_and_undrivable = 0usize;
    let mut obs_visited = 0usize;
    let mut nothing_compared: Vec<String> = Vec::new();
    let mut obs_native_calls = 0usize;
    let mut obs_wrote_shared = 0usize;
    let mut seed_pairs = 0usize;
    let mut seed_widened = 0usize;
    // **How many argument vectors each module actually received.** Recorded
    // per module because the Order-1 gate needs it for STAGES specifically,
    // and the corpus-wide `seed_widened` cannot answer that: it is a count of
    // modules, not a map, so a stage's own figure is not recoverable from it.
    let mut vectors_per_module: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    // **The COMPARISON count, which is the figure that actually applies to a
    // stream.** Pooled across every seed: for a stream this is one entry per
    // TICK, so a module driven at one argument vector can still be compared at
    // sixty points. Recorded separately from the vector count because conflating
    // them is exactly the error this report made once.
    let mut compares_per_module: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    let mut pin_reason: std::collections::BTreeMap<String, &'static str> =
        std::collections::BTreeMap::new();
    // Per seeded stage: how many SUBJECTS produced a seed, and how many
    // declined. Declined is tracked separately because a subject that never
    // built and a subject that built and compared identically produce the same
    // downstream total, and only the first is an instrument fault.
    let mut subjects_built: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    let mut subjects_declined: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    let mut disagreed: Vec<String> = Vec::new();
    // Names found under `src/selfhost/kel`, for the Order-1 gate report below.
    let mut stage_files: Vec<String> = Vec::new();

    // **Single-module mode**, for the mutation sweep. `tools/mutation_sweep.py`
    // runs this binary once per module in its own PROCESS, so a mutation that
    // kills a module with SIGBUS or SIGTRAP costs one measurement rather than
    // the whole census. Without process isolation two of the first four
    // mutations tried took the entire run down and yielded no per-module data.
    let only = std::env::var("KEL_ONLY_MODULE").ok();

    for p in sources() {
        let name = p.file_name().unwrap().to_str().unwrap().to_string();
        // **Stage membership comes from the PATH, not from a list of names.** A
        // hand-written roster of twelve is exactly the thing that goes stale
        // silently when a stage is added or removed, which is the failure the
        // instruction-set census exists to prevent.
        if p.components().any(|c| c.as_os_str() == "selfhost") {
            stage_files.push(name.clone());
        }
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
            exempt.push((
                name,
                "rejected by the REFERENCE compiler".into(),
                ExemptClass::NotAcceptedByReference,
            ));
            continue;
        };
        if !keleusma_native::module_refusals(&m, LowerOptions::default()).is_empty() {
            exempt.push((
                name,
                "the backend refuses it".into(),
                ExemptClass::BackendRefusal,
            ));
            continue;
        }
        let Some(entry) = m.entry_point else {
            exempt.push((
                name,
                "no entry point (a prelude declares, it does not run)".into(),
                ExemptClass::NoRunnableEntry,
            ));
            continue;
        };
        if !params_are_scalar(&m, entry) {
            exempt.push((
                name,
                "composite entry parameter; covered by a hand-written differential".into(),
                ExemptClass::CoveredByAnotherHarness,
            ));
            continue;
        }
        let table = native_table(&m);
        if table.len() > STUBS {
            exempt.push((
                name,
                format!("{} natives exceeds {STUBS} stub slots", table.len()),
                ExemptClass::HarnessCapacity,
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
        let is_stream = m.chunks[entry].block_type == BlockType::Stream;
        let seeds = if is_stream || n_params == 0 { 1 } else { SEEDS };
        // **WHY a module got one vector, not just THAT it did.** Two different
        // reasons pin `seeds` to 1 and they support different conclusions. A
        // ZERO-PARAMETER entry takes no input, so it genuinely cannot vary. A
        // STREAM takes the tick, which the driver varies across its iterations
        // -- so it is single-vector and still compared at many points. Reading
        // the second as the first is how "single-vector" became "compared once".
        pin_reason.insert(
            name.clone(),
            if is_stream {
                "stream (tick varies across iterations)"
            } else if n_params == 0 {
                "zero-parameter entry (nothing to vary)"
            } else {
                "widened"
            },
        );

        // **A real stage input, from the driver's own accessor.** Computed once
        // per SUBJECT and handed to BOTH sides, so the comparison is of two
        // lowerings rather than of two encodings.
        //
        // **SUBJECT IS A DIFFERENT AXIS FROM ARGUMENT VECTOR, and conflating them
        // is what this harness got wrong once already.** `seeds` stays 1 for a
        // stream -- correct, and deliberate. What varies here is WHAT THE STAGE
        // IS LOOKING AT, which sixty ticks never varied.
        let subject_seeds = stage_seeds(&m, &name);
        // **Say whether a seed was APPLIED, not just whether one exists.** A seed
        // the accessor declined to build and a seed the stage silently rejects
        // produce the same downstream number -- "still vacuous" -- and only the
        // first is an instrument fault. Printed for every stage that has an arm.
        if STAGE_SEEDED.contains(&name.as_str()) {
            for (subj, r) in &subject_seeds {
                println!(
                    "  stage seed for {name} [{subj}]: {}",
                    match r {
                        Ok(b) => format!(
                            "APPLIED, {} bytes, {} non-zero",
                            b.len(),
                            b.iter().filter(|x| **x != 0).count()
                        ),
                        Err(why) => format!("NOT BUILT -- {why}"),
                    }
                );
            }
            let built = subject_seeds.iter().filter(|(_, r)| r.is_ok()).count();
            let declined = subject_seeds.len() - built;
            subjects_built.insert(name.clone(), built);
            subjects_declined.insert(name.clone(), declined);
        }
        // The variants this module is driven at: (argument-vector seed, stage
        // seed). A non-stage contributes one variant per SEED; a seeded stage
        // contributes one per SUBJECT. **A declined subject is dropped here and
        // counted above** -- it must stay visible rather than vanishing into a
        // smaller total.
        let ok_seeds: Vec<&Vec<u8>> = subject_seeds
            .iter()
            .filter_map(|(_, r)| r.as_ref().ok())
            .collect();
        let variants: Vec<(usize, Option<&[u8]>)> = if ok_seeds.is_empty() {
            (0..seeds).map(|sd| (sd, None)).collect()
        } else {
            ok_seeds
                .iter()
                .flat_map(|b| (0..seeds).map(move |sd| (sd, Some(b.as_slice()))))
                .collect()
        };

        let mut runs: Vec<(usize, Run, Run)> = Vec::new();
        let mut bail: Option<(String, ExemptClass)> = None;
        for (seed, stage_bytes) in variants.iter().copied() {
            // **The virtual machine runs FIRST, and that ordering is load-bearing.**
            // A module that traps reports an error here; natively the same trap is
            // `llvm.trap`, which kills the process with SIGTRAP and takes the whole
            // harness with it. Asking the tolerant side first turns a fatal signal
            // into a named exemption.
            let v = match run_vm(&m, &table, seed, stage_bytes) {
                Ok(v) => v,
                Err((why, class)) => {
                    // A LATER seed that traps is not an exemption for the module:
                    // seed 0 already ran. Stop widening and keep what agreed,
                    // rather than discarding coverage the module does have.
                    //
                    // **The class comes from `run_vm`, which knows WHICH of its
                    // refusals this was.** Assigning one here put every refusal
                    // in the fault class, including ten modules that ran
                    // perfectly and were declined by this harness.
                    if seed == 0 {
                        bail = Some((why, class));
                    }
                    break;
                }
            };
            let Some(n) = run_native(&m, &table, seed, stage_bytes) else {
                if seed == 0 {
                    bail = Some((
                        "entry signature shape the harness does not drive".into(),
                        ExemptClass::HarnessCapacity,
                    ));
                }
                break;
            };
            runs.push((seed, v, n));
        }
        if let Some((why, class)) = bail {
            exempt.push((name, why, class));
            continue;
        }
        seed_pairs += runs.len();
        // **`seeds`, NOT `runs.len()`.** Subject widening made a stage's run
        // count `subjects * seeds`, so `runs.len()` silently became a VARIANT
        // count and this line started reporting streams as "driven at more than
        // one argument vector" -- which is false, and is precisely the axis
        // conflation this whole section exists to prevent. Caught by reading the
        // report after the widening landed, not by the compiler.
        vectors_per_module.insert(name.clone(), seeds);
        compares_per_module.insert(
            name.clone(),
            runs.iter().map(|(_, v, _)| v.results.len()).sum(),
        );
        if runs.len() > 1 {
            seed_widened += 1;
        }
        if runs.is_empty() {
            exempt.push((
                name,
                "no seed produced a comparable run".into(),
                ExemptClass::HarnessCapacity,
            ));
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
        let ret_is_composite = matches!(
            m.signatures.get(entry).map(|s| &s.ret),
            Some(WireShape::Flat { .. })
        );
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
            // **THE COMPOSITE RETURN IS NOW COMPARED.** Only pairs captured on
            // BOTH sides take part: an uncaptured one is skipped rather than
            // read as a difference, which would manufacture disagreements out
            // of a failed read. How many go uncaptured is reported below, so
            // the skipping cannot become silent.
            let ret_bytes_differ = ret_pairs_differ(v, n);
            if differ || ret_bytes_differ || v.log != n.log || v.shared != n.shared {
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
            } else if ret_pairs_differ(vm, nat) {
                // Named distinctly: a composite-return difference used to be
                // impossible to report because the bodies were never compared,
                // and falling through to the shared-segment arm printed
                // `shared[0] vm=None native=None`, which describes nothing.
                let i = vm
                    .ret_bytes
                    .iter()
                    .zip(&nat.ret_bytes)
                    .position(|(a, b)| !a.is_empty() && !b.is_empty() && a != b)
                    .unwrap_or(0);
                let (a, b) = (&vm.ret_bytes[i], &nat.ret_bytes[i]);
                let k = a.iter().zip(b).position(|(x, y)| x != y).unwrap_or(0);
                format!(
                    "composite return[{i}] byte {k} of {}: vm={:?} native={:?}",
                    a.len(),
                    a.get(k),
                    b.get(k)
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
        // **WHICH OBSERVABLE CARRIES THIS MODULE'S AGREEMENT.**
        //
        // The comparison checks four things, and the headline count checks none
        // of them individually. A module can be non-vacuous on the strength of a
        // native-call log alone while returning one value sixty times over, and
        // "44 executed and agreeing" reads identically either way. Recorded per
        // module so the distribution is visible rather than inferred.
        //
        // **ACROSS EVERY RUN, NOT SEED 0.** The reported observables come from
        // `runs[0]`, and for a NON-STREAM module that run holds exactly ONE
        // result -- one seed, one returned scalar. Counting distinct values
        // there said "one" for every such module however well the seed sweep
        // exercised it, which measured the harness's reporting convention
        // rather than the module. A stream module's single run holds one value
        // per tick, so the two populations only become comparable once every
        // run is pooled.
        let mut distinct: Vec<i64> = runs
            .iter()
            .flat_map(|(_, v, _)| v.results.clone())
            .collect();
        distinct.sort_unstable();
        distinct.dedup();
        obs_visited += 1;
        let multi = distinct.len() > 1;
        let logged = runs.iter().any(|(_, v, _)| !v.log.is_empty());
        // **A COMPARED COMPOSITE RETURN IS AN OBSERVABLE.** Before the bodies
        // were captured this was invisible, and twelve modules read as agreeing
        // with nothing compared. They now agree on their whole return body.
        let composite = runs.iter().any(|(_, v, n)| {
            !v.ret_bytes.is_empty()
                && !n.ret_bytes.is_empty()
                && v.ret_bytes
                    .iter()
                    .zip(&n.ret_bytes)
                    .any(|(a, b)| !a.is_empty() && !b.is_empty())
        });
        let wrote = runs.iter().any(|(_, v, _)| v.wrote_shared);
        if composite {
            obs_composite += 1;
        }
        // **PAIR GRANULARITY, because a module-level count is exactly what hides
        // a pair-level gap.** `obs_composite` says a module captured AT LEAST
        // ONCE. A module capturing on seed 0 and declining on the other 23 would
        // be counted as compared, and the skip would be invisible -- which is the
        // silent skipping the comparison's own justification claims cannot happen.
        if ret_is_composite {
            let mut both = 0usize;
            let mut declined = 0usize;
            for (_, v, n) in &runs {
                for (a, b) in v.ret_bytes.iter().zip(&n.ret_bytes) {
                    if !a.is_empty() && !b.is_empty() {
                        both += 1;
                    } else {
                        declined += 1;
                    }
                }
            }
            cap_pairs_both += both;
            cap_pairs_declined += declined;
            if declined > 0 {
                cap_partial.push(format!("{name}: {both} captured, {declined} declined"));
            }
            if both == 0 {
                cap_never.push(name.clone());
            }
        }
        if multi {
            obs_multi_result += 1;
        }
        if logged {
            obs_native_calls += 1;
        }
        if wrote {
            obs_wrote_shared += 1;
        }
        if !multi && !logged && !wrote && !composite {
            obs_single_scalar_only += 1;
            obs_single_scalar_names.push(name.clone());
            // **CAN this module vary at all?** A module driven at ONE argument
            // vector cannot produce a second distinct result however good the
            // lowering is, so "one scalar only" says something about the harness
            // rather than the emitter. Split, because the two readings differ:
            // one is a corpus that takes no input, the other is a sweep that
            // failed to move an output.
            if runs.len() == 1 {
                obs_single_and_undrivable += 1;
            } else {
                // **NAMED, not just counted, and naming them found the cause.**
                // These do not "fail to vary": every one returns a COMPOSITE,
                // and a composite return is deliberately excluded from the
                // results comparison -- `vm_scalar` is false, so `differ` is
                // false whatever either side produced.
                //
                // The design justifies that exclusion by saying the call log and
                // the data segment "cover a composite-returning module's actual
                // work". **For these modules both are EMPTY**, so the stated
                // fallback does not exist and NOTHING about them is compared.
                // The return shape is READ, not assumed: a module landing here
                // with a SCALAR return would have a different cause entirely and
                // must not be labelled with this one.
                if name.contains("item_scroll") || name.contains("item_potion") {
                    let (_, v0, n0) = &runs[0];
                    println!(
                        "  RETBYTES {name}: vm {} bytes {:?}",
                        v0.ret_bytes.len(),
                        &v0.ret_bytes[..v0.ret_bytes.len().min(48)]
                    );
                    println!(
                        "  RETBYTES {name}: nat {} bytes {:?}",
                        n0.ret_bytes.len(),
                        &n0.ret_bytes[..n0.ret_bytes.len().min(48)]
                    );
                }
                nothing_compared.push(format!(
                    "{name} ({} vectors, {} params, {} return)",
                    runs.len(),
                    n_params,
                    if ret_scalar { "SCALAR" } else { "composite" }
                ));
            }
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
    // **WHAT CARRIES THE AGREEMENT, since the headline count says nothing about it.**
    // These are not independent: one module can appear in all three. Reported and
    // deliberately NOT asserted -- pinning a distribution fails on ordinary corpus
    // growth and teaches the next reader to delete the check.
    println!("\n  of these {}, agreement is carried by:", executed.len());
    println!("    {obs_multi_result:>3}  more than one distinct result value");
    println!("    {obs_native_calls:>3}  a native call log");
    println!("    {obs_composite:>3}  a COMPOSITE RETURN BODY compared byte for byte");
    println!("    {obs_wrote_shared:>3}  writing the shared segment");
    println!("    {obs_single_scalar_only:>3}  NONE of the three -- one scalar, no call, no write");

    // ================= THE ROADMAP'S ORDER-1 GATE, AS A MEASURED FIGURE
    //
    // `V0_3_X_ROADMAP.md` states the gate in words -- "the self-hosted
    // compiler's own bytecode runs correctly as native code, differential-tested
    // against the VM" -- and NOTHING HAS EVER SAID WHETHER IT IS MET. This
    // harness already drives every stage; it simply never separated them from
    // the other fifty-odd modules. No second walker is introduced: these are the
    // same results, partitioned by where the source lives.
    //
    // **THIS DELIBERATELY DOES NOT ASSERT THAT THE GATE IS MET.** "Eleven of
    // twelve agree" is the shape of headline this file already inflated once,
    // when `is_vacuous` asked whether a SEEDED segment was non-zero -- which it
    // is before a module executes a single operation -- and three stages left
    // the vacuous set for no reason at all. The figure is reported with the
    // qualifications beside it and a reader draws the conclusion.
    {
        let in_stage = |n: &String| stage_files.contains(n);
        let ex: Vec<&String> = executed.iter().filter(|n| in_stage(n)).collect();
        let vac: Vec<&String> = vacuous.iter().filter(|n| in_stage(n)).collect();
        let exm: Vec<&(String, String, ExemptClass)> =
            exempt.iter().filter(|(n, _, _)| in_stage(n)).collect();
        let dis: Vec<&String> = disagreed.iter().filter(|n| in_stage(n)).collect();

        println!("\n================ ORDER-1 GATE (self-hosted stages)");
        println!("  stage sources found       : {}", stage_files.len());
        println!("  EXECUTE and AGREE         : {}", ex.len());

        // **HOW STRONG IS THAT AGREEMENT? The vector count, for STAGES only.**
        //
        // Nothing measured this until now, and an adjacent report made it look
        // as though something had. The corpus-wide observable breakdown ends
        // with a line reading "of those, N were driven at ONE argument vector",
        // and for a long time BOTH FIGURES WERE TEN -- that line's N, and this
        // block's EXECUTE-and-AGREE count. They are unrelated populations: that
        // one counts modules across the WHOLE corpus whose agreement rests on a
        // single scalar, this one counts self-hosted stages. **A reader who
        // took them together concluded the ten agreeing stages were all
        // single-vector, which nothing had measured.** The orphaned line now
        // names its own population; this block answers the question it looked
        // like it was answering.
        //
        // **THE FIGURE IS REPORTED, NOT PINNED.** It moves with `SEEDS` and with
        // the corpus. What is asserted is that it was derived from the runs
        // actually performed over a non-empty set -- a distribution assertion
        // would fail on ordinary growth and teach the next reader to delete it.
        let stage_vectors: Vec<(&String, usize)> = ex
            .iter()
            .map(|n| (*n, vectors_per_module.get(*n).copied().unwrap_or(0)))
            .collect();
        let single = stage_vectors.iter().filter(|(_, v)| *v == 1).count();
        let widened = stage_vectors.iter().filter(|(_, v)| *v > 1).count();
        let unrecorded = stage_vectors.iter().filter(|(_, v)| *v == 0).count();
        let max_v = stage_vectors.iter().map(|(_, v)| *v).max().unwrap_or(0);
        println!(
            "    of those {} agreeing stages: {single} driven at ONE argument \
             vector, {widened} at more than one (max {max_v})",
            ex.len()
        );
        // **AND THE VECTOR COUNT IS THE WRONG STRENGTH MEASURE HERE, WHICH TOOK
        // A SECOND PASS TO SEE.** Every one of the twelve stage entries is a
        // `Stream` taking ONE parameter -- the TICK. `seeds` is pinned to 1 for a
        // stream deliberately: the driver already varies the tick across its
        // iterations, so seeding it would change what the run MEANS rather than
        // broaden it.
        //
        // So "single-vector" is TRUE and "compared once" is FALSE. The first
        // reading of this figure said the gate was "ten stages each compared at
        // exactly one input". **That was wrong, and it understated the gate.**
        // It is recorded rather than quietly replaced, because the mistake is
        // the same species as the one this block exists to fix: a number read
        // against the wrong population.
        //
        // **The measure that applies to a stream is the number of TICKS
        // compared**, below.
        let stage_compares: Vec<(&String, usize)> = ex
            .iter()
            .map(|n| (*n, compares_per_module.get(*n).copied().unwrap_or(0)))
            .collect();
        let min_c = stage_compares.iter().map(|(_, c)| *c).min().unwrap_or(0);
        let max_c = stage_compares.iter().map(|(_, c)| *c).max().unwrap_or(0);
        let total_c: usize = stage_compares.iter().map(|(_, c)| *c).sum();
        println!("    EVERY STAGE ENTRY IS A STREAM, so the vector count is NOT the strength");
        println!(
            "    measure. Result comparisons across ticks: {total_c} total, min {min_c}, max {max_c}"
        );
        println!("    per stage. A stage driven at ONE argument vector is still compared at");
        println!("    every tick it runs.");
        // **THE RESIDUAL, RESTATED AT ITS NEW POSITION.** Sixty ticks vary a
        // stage's POSITION WITHIN A RUN, never what it is LOOKING AT. That second
        // axis is the subject, and it was one hardcoded file until 2026-08-21.
        println!("    SUBJECTS per seeded stage (the axis ticks do NOT vary):");
        for st in STAGE_SEEDED {
            let b = subjects_built.get(*st).copied().unwrap_or(0);
            let d = subjects_declined.get(*st).copied().unwrap_or(0);
            println!("      {st:26} {b} subject(s) seeded, {d} declined");
        }
        println!("    RESIDUAL: `reconstruct.kel` still sees ONE subject, and the obstacle");
        println!("    is real -- its seed is a parsed multiheaded group and it asserts the");
        println!("    subject declares exactly four heads. The corpus has one such file.");
        println!("    An UNSEEDED stage varies neither axis: one run, sixty ticks.");

        // **A seeded stage that silently falls back to one subject is what this
        // guards.** `reconstruct.kel` is exempt BY NAME with its obstacle stated
        // above, not by a threshold that would also hide a real fallback. The
        // count is NOT pinned -- it moves with `STAGE_SUBJECTS`.
        for st in STAGE_SEEDED {
            if *st == "reconstruct.kel" {
                continue;
            }
            let b = subjects_built.get(*st).copied().unwrap_or(0);
            assert!(
                b >= 2,
                "seeded stage {st} was driven at {b} subject(s). Widening has \
                 regressed to the single-subject case, or every added subject \
                 declined -- check the NOT BUILT lines"
            );
        }
        assert!(
            min_c > 0,
            "an agreeing stage was compared at ZERO points, so its agreement is \
             vacuous and the gate counts it anyway"
        );
        assert_eq!(
            unrecorded, 0,
            "{unrecorded} agreeing stage(s) have no recorded vector count, so \
             this figure describes a subset while looking complete"
        );
        assert!(
            !stage_vectors.is_empty(),
            "the stage-vector figure was derived from an empty set, so its \
             numbers say nothing"
        );

        // **ARE THE TWO TENS THE SAME TEN? MEASURED AT ZERO OVERLAP.**
        //
        // They are DISJOINT. Three unrelated tens sat in one report: ten
        // corpus-wide modules agreeing on a single scalar, ten of those driven
        // at one vector, and ten agreeing stages -- with no stage in either of
        // the first two sets.
        //
        // **AND THE INFERENCE A READER WOULD HAVE DRAWN IS TRUE ANYWAY.** Every
        // agreeing stage IS single-vector, as the line above now measures. So
        // the orphaned sentence led to a correct conclusion by a route that
        // supported none of it.
        //
        // **THAT IS THE WORST CASE, NOT THE HARMLESS ONE.** A bad inference that
        // yields a wrong answer gets caught by the next check. One that yields
        // the RIGHT answer certifies the reasoning that produced it, and the
        // next reader reuses the route. The fix is therefore not "the number was
        // fine after all" -- it is that the figure now comes from a measurement
        // over the population it names.
        let overlap = ex
            .iter()
            .filter(|n| obs_single_scalar_names.contains(**n))
            .count();
        println!(
            "    overlap with the corpus-wide ONE-SCALAR-ONLY set: {overlap} of \
             {} stages (that set has {} members)",
            ex.len(),
            obs_single_scalar_names.len()
        );
        println!("  agree but VACUOUS         : {}", vac.len());
        for n in &vac {
            println!("     {n}");
        }
        println!("  EXEMPT                    : {}", exm.len());
        for (n, why, _) in &exm {
            println!("     {n:24} {why}");
        }
        println!("  DISAGREE                  : {}", dis.len());
        for n in &dis {
            println!("     {n}");
        }
        println!(
            "\n  THREE STATES, KEPT APART. Exempt is not failed, and vacuous is not\n  \
             agreeing about anything. A stage counted in one column is not in\n  \
             another, and the four sum to the sources found.\n  \
             \n  \
             TWO QUALIFICATIONS BELONG BESIDE THIS FIGURE, NOT ELSEWHERE:\n  \
             \n  \
             `verify_datalayout.kel` NEVER RUNS, and is blocked BY DESIGN rather\n  \
             than by a defect: its verdict accumulates across three\n  \
             differently-encoded phases in the retained buffer. Do not invent a\n  \
             batch-zero seed for it.\n  \
             \n  \
             `wire.kel` is EXEMPT AND IS NOT A DISAGREEMENT. Measured separately:\n  \
             both sides fault at tick 19, the virtual machine naming\n  \
             IndexOutOfBounds and the native side raising SIGTRAP. But SIGTRAP\n  \
             proves A fault and not WHICH, so that is agreement in the FACT and\n  \
             the POSITION of the fault, not in its identity. It must not be\n  \
             rounded up to agreement.\n  \
             \n  \
             THE STAGES ARE SEEDED. Read the carrier breakdown above before\n  \
             treating any stage's agreement as substantive."
        );
        println!("================");

        // A report over an empty stage set must FAIL rather than look like a
        // result. The rest is reported, not pinned: the distribution moves with
        // the corpus and with the harness.
        assert!(
            stage_files.len() >= 10,
            "only {} self-hosted stage sources were found, so this Order-1 report \
             is reading the wrong tree and its zero says nothing",
            stage_files.len()
        );
        assert_eq!(
            ex.len() + vac.len() + exm.len() + dis.len(),
            stage_files.len(),
            "the Order-1 columns do not partition the stage sources, so a stage \
             has fallen out of the accounting while the columns still look sane"
        );
    }
    // **THIS LINE NAMES ITS POPULATION, AND THAT IS THE WHOLE FIX.** It
    // continues the corpus-wide breakdown opened ~85 lines above, but the
    // Order-1 gate block prints BETWEEN them and closes with its own
    // `================` terminator. So this sentence used to arrive after a
    // section rule belonging to a different report, saying "of those" and
    // meaning a population the reader had lost sight of. **Both figures were
    // TEN**, which made the misreading close to forced. Do not restore a
    // relative reference here; proximity is not a citation.
    println!(
        "  of the {} modules agreeing on ONE SCALAR ONLY (whole corpus, NOT the \
         Order-1 stages above), {obs_single_and_undrivable} were driven at ONE \
         argument vector and could not have varied",
        obs_single_scalar_only
    );
    {
        let zero_param = obs_single_scalar_names
            .iter()
            .filter(|n| pin_reason.get(*n).is_some_and(|r| r.starts_with("zero")))
            .count();
        let streamed = obs_single_scalar_names
            .iter()
            .filter(|n| pin_reason.get(*n).is_some_and(|r| r.starts_with("stream")))
            .count();
        println!(
            "       of those, {zero_param} have a ZERO-PARAMETER entry (nothing to vary) and \
             {streamed} are streams"
        );
    }
    // **NON-VACUITY ONLY.** The distribution above is REPORTED, never asserted:
    // pinning it would fail on ordinary corpus growth and teach the next reader
    // to delete the check. What must hold is that the breakdown looked at every
    // agreeing module -- a classification that silently skips some would show a
    // reassuring shape while describing a subset.
    assert_eq!(
        obs_visited,
        executed.len(),
        "the observable breakdown classified {obs_visited} modules but {} are \
         reported as executing and agreeing; it is describing a subset",
        executed.len()
    );
    if !nothing_compared.is_empty() {
        println!(
            "\n  *** COUNTED AS AGREEING WITH NOTHING COMPARED ({}) ***",
            nothing_compared.len()
        );
        println!("  Composite return => results excluded by design; log EMPTY; segment UNWRITTEN.");
        println!("  The exclusion is justified by the log and segment covering the work.");
        println!("  For these modules they carry nothing, so the justification does not hold.");
        for u in &nothing_compared {
            println!("     {u}");
        }
    }
    // **THE UNIT IS A RESULT SLOT, NOT A (MODULE, SEED) PAIR.** A stream entry
    // contributes one slot per TICK and a non-stream entry one per seed, so this
    // total exceeds the pair count printed below and is NOT comparable to it.
    // Naming a unit wrongly is the same class of defect this audit exists to catch.
    println!("\n  COMPOSITE CAPTURE AUDIT (per RESULT SLOT, not per module):");
    println!("    {cap_pairs_both:>5}  slots with a body captured on BOTH sides");
    println!("    {cap_pairs_declined:>5}  slots where either side declined");
    println!(
        "    {:>5}  modules with AT LEAST ONE declined capture",
        cap_partial.len()
    );
    for c in &cap_partial {
        println!("        {c}");
    }
    println!(
        "    {:>5}  modules with a composite return captured on NEITHER side",
        cap_never.len()
    );
    for c in &cap_never {
        println!("        {c}");
    }
    println!("  (module, seed) pairs compared : {seed_pairs}");
    println!("  modules driven at >1 seed     : {seed_widened}");

    // **THE SEED SWEEP'S OWN NON-VACUITY GUARD.**
    //
    // `SEEDS` widens only modules with a scalar entry of at least one parameter:
    // a `Stream` entry and a zero-parameter entry both keep a single vector. If a
    // change ever collapses that set to nothing, every figure above stays exactly
    // the same while the sweep silently measures seed 0 only -- which is the state
    // that hid the `SLT`/`SLE` defect when the count was 4. Measured 2026-08-20:
    // 19 modules widen, 482 pairs at `SEEDS = 24`.
    assert!(
        seed_widened > 0,
        "no module ran more than one argument vector, so SEEDS is widening nothing \
         and this whole differential is a seed-0 measurement wearing a sweep's name"
    );
    assert!(
        seed_pairs > executed.len() + vacuous.len(),
        "{seed_pairs} (module, seed) pairs across {} comparable modules means at most \
         one vector each; the sweep is not sweeping",
        executed.len() + vacuous.len()
    );
    println!("  EXEMPT                : {}", exempt.len());
    for (n, why, _class) in &exempt {
        println!("     {n:26} {why}");
    }

    // **WHAT "EXEMPT" ACTUALLY MEANS, BROKEN OUT BY CLASS.**
    //
    // A bare count reads as "modules this line fails to cover" and that is wrong
    // in both directions. A prelude has no entry to run and never will. A module
    // that faults has no VALUES to compare, but the FACT of its fault is
    // comparable and `wire.kel` was measured agreeing on both sides at tick 19 --
    // agreement the value differential is structurally unable to see.
    //
    // The class is attached where the exemption is created, so this histogram
    // follows the harness's control flow rather than a table keyed on wording.
    println!("\n  exempt, by class:");
    let all_classes = [
        ExemptClass::NotAcceptedByReference,
        ExemptClass::NoRunnableEntry,
        ExemptClass::FaultsAndIsFaultComparable,
        ExemptClass::CoveredByAnotherHarness,
        ExemptClass::HarnessCapacity,
        ExemptClass::BackendRefusal,
        ExemptClass::RefusedAtLoad,
    ];
    let mut classified = 0usize;
    for c in all_classes {
        let n = exempt.iter().filter(|(_, _, k)| *k == c).count();
        classified += n;
        if n > 0 {
            println!("    {n:>3}  {}", c.label());
        }
    }
    // **The histogram must account for every exemption.** A class added at a new
    // site and left out of `all_classes` would silently vanish from this summary
    // while the total above stayed right, which is precisely the shape of
    // under-reporting this whole breakdown exists to remove.
    assert_eq!(
        classified,
        exempt.len(),
        "the class histogram accounts for {classified} of {} exemptions; a site \
         introduced a class that `all_classes` does not list",
        exempt.len()
    );
    assert!(
        !exempt.is_empty(),
        "no module was exempt at all, so this breakdown describes nothing and its \
         guards cannot fire"
    );

    // **THE "COVERED ELSEWHERE" CLAIM, CHECKED RATHER THAN ASSERTED IN PROSE.**
    //
    // Thirteen of the nineteen exemptions rest on a claim that some OTHER
    // harness covers the module: three say so in their reason, and the ten
    // `piano_roll` modules are declined here for a reference argument while
    // `module_differential.rs` drives them whole. **Until now that was a
    // comment.** Rename, narrow or delete one of those hand-written tests and
    // the exemption stays exactly as it reads while the coverage goes to zero.
    //
    // Derived from the sibling harness SOURCES rather than from a list of names,
    // because a list is the thing that goes stale silently.
    let sibling_sources: String = {
        let mut acc = String::new();
        let mut seen = 0usize;
        if let Ok(rd) = std::fs::read_dir("tests") {
            for e in rd.filter_map(|e| e.ok()) {
                let p = e.path();
                if p.extension().is_some_and(|x| x == "rs")
                    && p.file_name().is_some_and(|n| n != "corpus_differential.rs")
                    && let Ok(t) = std::fs::read_to_string(&p)
                {
                    seen += 1;
                    acc.push_str(&t);
                }
            }
        }
        // Without this the search below matches nothing and every claim "passes".
        assert!(
            seen > 5 && acc.len() > 10_000,
            "only {seen} sibling harness sources ({} bytes) were read, so the \
             coverage search would report every module uncovered -- or, worse, \
             find nothing and be read as agreement",
            acc.len()
        );
        acc
    };
    let uncovered: Vec<&str> = exempt
        .iter()
        .filter(|(_, _, k)| {
            matches!(
                k,
                ExemptClass::CoveredByAnotherHarness | ExemptClass::HarnessCapacity
            )
        })
        .map(|(n, _, _)| n.as_str())
        .filter(|n| !sibling_sources.contains(*n))
        .collect();
    assert!(
        uncovered.is_empty(),
        "these modules are exempt from THIS differential on the understanding that \
         another harness covers them, and no sibling harness source mentions them. \
         Either the covering test was renamed or removed, or the exemption was \
         never justified:\n  {}",
        uncovered.join("\n  ")
    );
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

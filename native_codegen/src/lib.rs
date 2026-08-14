//! Lowering of verified Keleusma bytecode to LLVM IR (V0.3.x Workstream A).
//!
//! # Scope
//!
//! This is an early subset: 46 of the instruction set's 66 opcodes. Everything
//! else is refused rather than lowered to something plausible and wrong.
//! Widening the subset is the work of subsequent increments, tracked in
//! `docs/decisions/NATIVE_LOWERING_INVENTORY.md`.
//!
//! The integer arithmetic surface is complete as of this increment, which is
//! not the same set of opcodes it looks like. `Op::Add`, `Op::Sub`, `Op::Mul`
//! and `Op::Neg` do **not** implement `Word` arithmetic; consolidation B
//! narrowed them away from `Int` operands and the compiler emits
//! `Checked{Add,Sub,Mul,Neg}; PopN(2)` instead. Those four unchecked opcodes
//! remain unsupported here because they are reachable only for `Byte`, `Fixed`
//! and `Float`, none of whose representations are settled.
//!
//! All 66 opcodes are emitted by the reference compiler, so there is no dead
//! region of the instruction set to skip. Verified by enumeration, 2026-08-08.
//!
//! # The correctness signal
//!
//! The oracle is differential execution against the VM over the same bytecode,
//! per `docs/roadmap/V0_3_X_ROADMAP.md`. It is not a formality: the first
//! version of this lowering carried a real defect that a single test case
//! passed straight through (see `stack depth` below). Do not add an opcode
//! without adding inputs that distinguish it.
//!
//! # Two design decisions worth stating
//!
//! **The operand stack is modelled as allocas, not as hand-built phi nodes.**
//! Keleusma's control flow is structured (`If`/`Else`/`EndIf` carry targets),
//! so basic blocks fall out directly with no control-flow-graph reconstruction.
//! What does not fall out is SSA form for the operand stack across a merge.
//! Rather than construct phis by hand, each stack slot is an alloca and LLVM's
//! `mem2reg` performs the SSA construction. This trades verbose IR for a whole
//! class of absent bugs.
//!
//! **The allocas are removed by the MIDDLE END, not by an optimisation level**,
//! and the difference is worth 30x of stack frame. An earlier version of this
//! comment said mem2reg runs "at any optimisation level above none", which is
//! wrong in the way that matters: `llc -O2` is an optimisation level above none
//! and does not run it, because `mem2reg` is an `opt` pass. Measured on
//! 2026-08-09 for `thumbv7em-none-eabihf`:
//!
//! | Program | `llc -O2` alone | `opt -O1` first |
//! |---|---|---|
//! | `a + b` | 536 bytes | 0 bytes |
//! | branchy | 552 bytes | 16 bytes |
//! | handled multiply | 616 bytes | 20 bytes |
//!
//! `opt -passes=mem2reg` alone accounts for the whole difference. 512 of those
//! bytes are [`MAX_STACK`] slots that the program never uses. A pipeline that
//! runs only `llc` therefore ships a half-kilobyte frame per function, which on
//! a microcontroller is the difference between fitting and not, and any native
//! worst-case-memory bound computed on the wrong pipeline is wrong by that
//! factor.
//!
//! **Stack depth is per-block, not carried linearly.** The compile-time depth
//! must be restored from the recorded incoming edges when entering a merge
//! block, never carried across a branch. Carrying it was the original defect:
//! the then-path wrote slot 0 while the merge read slot 1, and the function
//! returned the wrong operand. Every incoming edge must agree on depth, which
//! the typed operand-stack verifier already guarantees, so a disagreement here
//! is a lowering bug and is asserted rather than tolerated.

pub mod region;

/// The PACKED width of an operand-stack value, for placing it inside a flat
/// composite body.
///
/// # Why this is carried rather than recovered
///
/// Recovering an operand's width by looking backwards from a `NewComposite`
/// needed either a change to the shared crate, a second copy of the verifier's
/// abstract interpretation, or an adjacency heuristic. None was necessary: the
/// emitter already maintains the operand stack and already pops exactly each
/// opcode's operands. It only lacked this.
///
/// # `Unknown` is the DEFAULT, and that is deliberate
///
/// A `Byte` occupies a full `i64` operand slot holding a value in `0..=255`, so
/// **a `Byte` and a `Word` are indistinguishable on this stack** — and they pack
/// into a body at ONE byte and EIGHT. Defaulting to word width would therefore
/// silently mispack any byte field, and the byte-identity oracle would only
/// catch it where the corpus happens to build one. Everything is unknown until
/// an opcode arm states otherwise, and a composite operation that consumes an
/// unknown is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Width {
    /// The operand IS the data: `bytes` of it, stored directly.
    Scalar(u32),
    /// The operand POINTS AT the data: `bytes` of body at the address it holds,
    /// copied rather than stored.
    ///
    /// **Splitting this from `Scalar` is not tidiness.** An eight-byte nested
    /// composite body and a `Word` are both eight bytes, and a single-field
    /// struct wrapping a word is exactly that shape — 80 of the corpus's
    /// constructions are `(8, 1)`. Storing one as a scalar would write the
    /// POINTER into the parent body while every downstream field offset still
    /// looked correct, which is a silent wrong answer rather than a fault.
    Body(u32),
    /// Not statically determined here. Fails a composite operation closed.
    Unknown,
}

impl Width {
    /// Byte count the value occupies in a packed body, or `None` when unknown.
    pub fn bytes(self) -> Option<u32> {
        match self {
            Width::Scalar(n) | Width::Body(n) => Some(n),
            Width::Unknown => None,
        }
    }

    /// Whether placing this operand copies from an address rather than storing
    /// the value.
    pub fn is_body(self) -> bool {
        matches!(self, Width::Body(_))
    }
}

/// A vector of `n` unknown widths.
fn alloc_vec_unknown(n: usize) -> Vec<Width> {
    vec![Width::Unknown; n]
}

/// Packed width of a declared parameter type.
///
/// `Word` is eight bytes because this backend is 64-bit throughout; a narrow-word
/// target is refused elsewhere. `Composite` is UNKNOWN rather than a guess: a
/// composite parameter's body length is not carried on the type tag, and
/// guessing it is exactly the silent mispack this type exists to prevent.
fn width_of_tag(t: TypeTag) -> Width {
    match t {
        TypeTag::Byte | TypeTag::Bool => Width::Scalar(1),
        TypeTag::Word => Width::Scalar(8),
        _ => Width::Unknown,
    }
}

use inkwell::AddressSpace;
use inkwell::IntPredicate;
use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module as LlvmModule;
use inkwell::types::IntType;
use inkwell::values::{FunctionValue, IntValue, PointerValue, ValueKind};
use keleusma::bytecode::{
    BlockType, Chunk, ConstValue, Module, Op, SharedSlotLayout, SlotVisibility, TypeTag,
};
use std::collections::BTreeMap;

/// Maximum operand-stack depth the lowering provisions slots for.
///
/// A fixed bound is appropriate because Keleusma's operand-stack depth is
/// statically known per program point; this is a provisioning ceiling, not a
/// runtime limit. Exceeding it is a lowering bug, not a program error.
pub const MAX_STACK: usize = 64;

/// Word width this backend lowers, in bits.
///
/// Enforced by [`check_word_width`]. The shift lowering masks counts by
/// `WORD_BITS - 1`, mirroring the VM, so widening word support means changing
/// both together rather than only the acceptance check.
pub const WORD_BITS: u64 = 64;

/// How an unhandled partial operation lowers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverflowPolicy {
    /// Discard the outcome flag, keeping the low word. **This is the default
    /// and it matches the VM.**
    ///
    /// Bare `a + b` compiles to `CheckedAdd; PopN(2)`, which discards the flag
    /// and the high word so the low word survives. That is wrapping addition,
    /// and it is *total*: it always produces a value. Keleusma's premise is
    /// totality, and a programmer who needs the overflow outcome has the
    /// handled `expr { ok(v) => .., overflow(h, l) => .., underflow(h, l) => .. }`
    /// form to reach for.
    #[default]
    Wrap,
    /// Branch to a native trap when the outcome flag is non-zero.
    ///
    /// This is the shape `V0_3_X_ROADMAP.md` Workstream F describes for an
    /// unhandled partial operation, and it is retained for that work. **It
    /// diverges from the VM**, measurably: with this policy `add(i64::MAX, 1)`
    /// aborts through `llvm.trap` where the VM returns the wrapped value. Do
    /// not enable it without deciding that native may lead a semantic change
    /// the VM has not made.
    Trap,
}

/// Lowering configuration.
#[derive(Clone, Copy, Debug, Default)]
pub struct LowerOptions {
    /// How an unhandled checked arithmetic operation lowers.
    pub overflow: OverflowPolicy,
}

/// Error cases the lowering refuses rather than guesses at.
#[derive(Debug)]
pub enum LowerError {
    /// Diagnostic mode collected refusals rather than stopping at the first.
    ///
    /// Always an error, never a success carrying a warning list: the module is
    /// incomplete wherever a chunk was abandoned, so handing it back as `Ok`
    /// would invite someone to run it. Produced only by [`module_refusals`],
    /// which discards the module and keeps the list.
    ///
    /// Carries the COUNT only. The list stays with the caller, because an
    /// earlier version moved it into this error and left the caller's vector
    /// empty — reporting that every module lowers cleanly.
    Diagnostic(usize),
    /// An opcode outside the currently supported subset.
    UnsupportedOp(String),
    /// The module declares a word width this backend does not lower.
    UnsupportedWordWidth(u8),
    /// A data-segment slot this backend cannot lower.
    ///
    /// Carries the slot index and the reason. Private slots and shared
    /// composite or non-integer scalar slots are refused rather than guessed
    /// at, since a wrong decode reads the host's buffer at the wrong width and
    /// yields a plausible number.
    UnsupportedDataSlot { slot: u32, why: String },
    /// The chunk's operand stack is deeper than [`MAX_STACK`] provisions for.
    ///
    /// A refusal rather than a panic. The verifier already computes the exact
    /// figure as `RuntimeFootprint::max_operand_slots`, so a caller that wants
    /// to lower such a chunk can raise the provisioning deliberately instead of
    /// discovering the ceiling through a crash.
    OperandStackTooDeep { needed: usize, provisioned: usize },
    /// The lowering produced a module LLVM's own verifier rejects.
    ///
    /// A postcondition on [`lower_module`], not a diagnostic for the caller's
    /// input. Reaching it always means a defect in this crate. It exists because
    /// `verify` was previously called only in the test harness, so malformed IR
    /// would reach a consumer while every test stayed green.
    InvalidIr(String),
}

impl core::fmt::Display for LowerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LowerError::Diagnostic(n) => {
                write!(f, "diagnostic lowering collected {n} chunk refusal(s)")
            }
            LowerError::UnsupportedOp(op) => {
                write!(f, "native lowering does not yet support opcode {op}")
            }
            LowerError::UnsupportedWordWidth(w) => {
                write!(f, "native lowering does not support word_bits_log2 = {w}")
            }
            LowerError::UnsupportedDataSlot { slot, why } => {
                write!(f, "data slot {slot} is not lowerable: {why}")
            }
            LowerError::OperandStackTooDeep {
                needed,
                provisioned,
            } => write!(
                f,
                "chunk needs {needed} operand-stack slots, more than the {provisioned} \
                 this backend provisions"
            ),
            LowerError::InvalidIr(why) => write!(
                f,
                "the lowering produced a module LLVM's own verifier rejects, \
                 which is always a defect in this crate rather than in the input: {why}"
            ),
        }
    }
}

impl std::error::Error for LowerError {}

struct Lower<'ctx> {
    b: Builder<'ctx>,
    i64t: IntType<'ctx>,
    /// The entry block, so operand slots can be allocated lazily at its top.
    ///
    /// Allocas must sit in the entry block for the memory-to-register pass to
    /// promote them, and at its TOP so that inserting one is valid however far
    /// the current block has progressed.
    entry: BasicBlock<'ctx>,
    locals: Vec<PointerValue<'ctx>>,
    slots: Vec<PointerValue<'ctx>>,
    depth: usize,
    /// The depth at which the provisioned slot array was first exceeded.
    ///
    /// Recorded rather than panicked. `MAX_STACK` is a provisioning ceiling
    /// chosen by this backend, not a limit the language imposes, so a chunk
    /// that needs more is a REFUSAL like any other unsupported construct — not
    /// a crash inside a library. Indexing a `Vec` would have panicked, and the
    /// claim that exceeding it "is a lowering bug, not a program error" was an
    /// assumption with nothing enforcing it.
    stack_overflow: Option<usize>,
    /// Packed width of each live operand, parallel to the slot at the same
    /// depth. Grown alongside `slots` and defaulted to [`Width::Unknown`], so an
    /// arm that does not state a width cannot be mistaken for one that did.
    widths: Vec<Width>,
    /// Packed width most recently stored into each local, so `GetLocal` can
    /// restore what `SetLocal` put there. A local never written in this chunk
    /// stays unknown.
    local_widths: Vec<Width>,
}

impl<'ctx> Lower<'ctx> {
    /// Index of the slot for `depth`, clamped so that an overflowing chunk
    /// keeps building valid-but-discarded IR instead of panicking. The clamp is
    /// only ever reached once `stack_overflow` is set, and the caller turns that
    /// into an error before the function is handed back.
    /// Allocate operand slots on demand, up to the refusal ceiling.
    ///
    /// **`MAX_STACK` is a ceiling, not a provisioning quantity.** Emitting all
    /// 64 slots unconditionally was measured to cost real frame bytes: the
    /// promotion pass turns them into virtual registers, a machine with roughly
    /// fourteen usable general-purpose registers cannot hold them, and the
    /// allocator spills the excess straight back. So the fixed provisioning
    /// survived optimisation as spill slots and dominated the frame, which is
    /// why the smallest frames measured sat near `64 * 8` bytes.
    ///
    /// Growing on demand asks the emitter what it actually used rather than
    /// duplicating the verifier's depth analysis in a second place, which is the
    /// drift hazard this package has already been bitten by twice.
    fn ensure_slot(&mut self, idx: usize) {
        while self.slots.len() <= idx && self.slots.len() < MAX_STACK {
            let name = format!("s{}", self.slots.len());
            let resume = self.b.get_insert_block();
            match self.entry.get_first_instruction() {
                Some(i) => self.b.position_before(&i),
                None => self.b.position_at_end(self.entry),
            }
            let p = self.b.build_alloca(self.i64t, &name).unwrap();
            if let Some(bb) = resume {
                self.b.position_at_end(bb);
            }
            self.slots.push(p);
        }
    }

    fn slot(&mut self, depth: usize) -> PointerValue<'ctx> {
        self.ensure_slot(depth);
        self.slots[depth.min(self.slots.len() - 1)]
    }

    fn push(&mut self, v: IntValue<'ctx>) {
        self.push_w(v, Width::Unknown);
    }

    /// Push, stating the value's packed width.
    ///
    /// The unstated form defaults to [`Width::Unknown`] rather than to a word,
    /// so an unlabelled arm fails a composite operation closed instead of
    /// mispacking it. See [`Width`].
    fn push_w(&mut self, v: IntValue<'ctx>, w: Width) {
        if self.depth >= MAX_STACK {
            self.stack_overflow.get_or_insert(self.depth);
        }
        let slot = self.slot(self.depth);
        self.b.build_store(slot, v).unwrap();
        if self.widths.len() <= self.depth {
            self.widths.resize(self.depth + 1, Width::Unknown);
        }
        self.widths[self.depth] = w;
        self.depth += 1;
    }

    /// Read the top operand WITHOUT consuming it.
    ///
    /// Distinct from [`Lower::pop`] because `Op::BoundsCheck` peeks: the VM
    /// reads `stack.last()` and leaves the operand for the indexing opcode that
    /// follows.
    fn peek(&mut self) -> IntValue<'ctx> {
        let slot = self.slot(self.depth - 1);
        self.b
            .build_load(self.i64t, slot, "peek")
            .unwrap()
            .into_int_value()
    }

    /// Packed width of the operand `back` entries below the top, `back == 0`
    /// being the top. Unknown when out of range, so an overflowing chunk fails a
    /// composite closed rather than indexing wildly.
    fn width_at(&self, back: usize) -> Width {
        match self.depth.checked_sub(back + 1) {
            Some(d) => self.widths.get(d).copied().unwrap_or(Width::Unknown),
            None => Width::Unknown,
        }
    }

    /// Relabel the top operand's packed width without touching its value.
    ///
    /// For a conversion that is a no-op on the bits and not on the layout:
    /// `ByteToWord` leaves the value alone and changes how many bytes it
    /// occupies inside a composite body. Silent if the stack is empty, which
    /// only happens in a chunk already refused for overflow.
    fn set_top_width(&mut self, w: Width) {
        if let Some(d) = self.depth.checked_sub(1)
            && d < self.widths.len()
        {
            self.widths[d] = w;
        }
    }

    fn pop(&mut self) -> IntValue<'ctx> {
        self.depth -= 1;
        let slot = self.slot(self.depth);
        self.b
            .build_load(self.i64t, slot, "pop")
            .unwrap()
            .into_int_value()
    }

    /// Sign-extend an operand into the 128-bit domain the checked arithmetic
    /// opcodes compute in.
    fn widen(&self, v: IntValue<'ctx>, i128t: IntType<'ctx>, name: &str) -> IntValue<'ctx> {
        self.b.build_int_s_extend(v, i128t, name).unwrap()
    }

    /// Push the `(low, high, flag)` triple every integer checked arithmetic
    /// opcode produces, given the exact result already computed in 128 bits.
    ///
    /// One helper serves add, subtract, negate and multiply because the VM
    /// routes all four through the same `checked_arith_outputs` classifier
    /// (`src/vm.rs`): only the wide expression differs. `low` is the
    /// two's-complement-wrapped 64-bit result, `high` is the bits above it, and
    /// `flag` is `0` in range, `1` above `i64::MAX`, `2` below `i64::MIN`.
    ///
    /// The shift below is arithmetic to mirror the VM's `>>` on `i128`, but
    /// **the choice is unobservable here** and no test can distinguish it. Bit
    /// `i` of the result is bit `i + 64` of the input for `i < 64` under both an
    /// arithmetic and a logical shift; they differ only in bits 64 and above,
    /// which the truncate to `i64` discards. Established by a must-fire case
    /// that did not fire (2026-08-09) and then by the argument above, in that
    /// order. Do not read a passing suite as evidence that the shift kind was
    /// checked.
    ///
    /// This mirrors the classifier at `word_bits_log2 == 6` specifically, where
    /// its `narrow` path is dead and `truncate_int_to_declared_width` is the
    /// identity. [`check_word_width`] refuses every other width, so that
    /// precondition is enforced rather than assumed.
    fn push_checked_triple(
        &mut self,
        ctx: &'ctx Context,
        func: FunctionValue<'ctx>,
        trap_bb: BasicBlock<'ctx>,
        opts: LowerOptions,
        wide: IntValue<'ctx>,
    ) {
        let triple = self.checked_triple(ctx, wide);
        self.push_triple(ctx, func, trap_bb, opts, triple);
    }

    /// Classify a 128-bit result into `(low, high, flag)` without pushing it.
    ///
    /// Separate from the push so the division family can OVERRIDE the triple
    /// before it lands: a zero divisor reifies as flag `3` with the numerator in
    /// the low slot, an outcome this classifier cannot produce because it
    /// classifies a result and a zero divisor has none.
    fn checked_triple(
        &mut self,
        ctx: &'ctx Context,
        wide: IntValue<'ctx>,
    ) -> (IntValue<'ctx>, IntValue<'ctx>, IntValue<'ctx>) {
        let i64t = self.i64t;
        let i128t = ctx.i128_type();

        let low = self.b.build_int_truncate(wide, i64t, "low").unwrap();
        let sh = self
            .b
            .build_right_shift(wide, i128t.const_int(64, false), true, "sh")
            .unwrap();
        let high = self.b.build_int_truncate(sh, i64t, "high").unwrap();

        let maxv = i128t.const_int(i64::MAX as u64, false);
        let minv = self
            .b
            .build_int_s_extend(i64t.const_int(i64::MIN as u64, true), i128t, "min")
            .unwrap();
        let ov = self
            .b
            .build_int_compare(IntPredicate::SGT, wide, maxv, "ov")
            .unwrap();
        let un = self
            .b
            .build_int_compare(IntPredicate::SLT, wide, minv, "un")
            .unwrap();
        let f2 = self
            .b
            .build_select(un, i64t.const_int(2, false), i64t.const_zero(), "f2")
            .unwrap()
            .into_int_value();
        let flag = self
            .b
            .build_select(ov, i64t.const_int(1, false), f2, "flag")
            .unwrap()
            .into_int_value();

        (low, high, flag)
    }

    /// Return a divisor that makes a 64-bit `sdiv`/`srem` defined, given that
    /// the caller has already excluded a zero divisor.
    ///
    /// The only remaining undefined case is `i64::MIN` divided by `-1`, whose
    /// true quotient is not representable. Substituting `1` for the divisor in
    /// exactly that case yields the VM's answer for both opcodes without a
    /// corrective step afterwards; see the call site for why that is exact
    /// rather than approximate.
    fn guard_min_div_neg_one(
        &mut self,
        lhs: IntValue<'ctx>,
        rhs: IntValue<'ctx>,
        i64t: IntType<'ctx>,
    ) -> IntValue<'ctx> {
        let is_min = self
            .b
            .build_int_compare(
                IntPredicate::EQ,
                lhs,
                i64t.const_int(i64::MIN as u64, true),
                "ismin",
            )
            .unwrap();
        let is_neg1 = self
            .b
            .build_int_compare(
                IntPredicate::EQ,
                rhs,
                i64t.const_int(-1i64 as u64, true),
                "isneg1",
            )
            .unwrap();
        let both = self.b.build_and(is_min, is_neg1, "minneg1").unwrap();
        self.b
            .build_select(both, i64t.const_int(1, false), rhs, "safediv")
            .unwrap()
            .into_int_value()
    }

    /// Apply the overflow policy and push a `(low, high, flag)` triple.
    fn push_triple(
        &mut self,
        ctx: &'ctx Context,
        func: FunctionValue<'ctx>,
        trap_bb: BasicBlock<'ctx>,
        opts: LowerOptions,
        (low, high, flag): (IntValue<'ctx>, IntValue<'ctx>, IntValue<'ctx>),
    ) {
        let i64t = self.i64t;

        if opts.overflow == OverflowPolicy::Trap {
            let cont = ctx.append_basic_block(func, "nooverflow");
            let bad = self
                .b
                .build_int_compare(IntPredicate::NE, flag, i64t.const_zero(), "bad")
                .unwrap();
            self.b.build_conditional_branch(bad, trap_bb, cont).unwrap();
            self.b.position_at_end(cont);
        }

        // Push order is low, high, flag, matching the VM at `src/vm.rs`. It is
        // load-bearing: `Checked*; PopN(2)` discards flag and high specifically
        // so that low survives as an uncaptured expression's value. Verified by
        // execution, not taken from the opcode's doc comment, which was wrong
        // about this until 2026-08-08.
        // `low` is the arithmetic RESULT and is a word; `Checked*; PopN(2)`
        // discards the other two, so this is the width a construction sees for
        // `a + 1`. The flag is a boolean and `high` is the overflow word, both
        // labelled so a construction consuming one is packed correctly rather
        // than refused for want of a label.
        self.push_w(low, Width::Scalar(8));
        self.push_w(high, Width::Scalar(8));
        self.push_w(flag, Width::Scalar(1));
    }
}

/// `ScalarKind::to_tag` values this backend understands. The tag space is the
/// wire format's, not this crate's, so the constants mirror
/// `src/value_layout.rs` rather than redefining it.
const SCALAR_BOOL: u8 = 1;
const SCALAR_BYTE: u8 = 2;
const SCALAR_INT: u8 = 3;
/// Bytes this backend gives a private data slot.
///
/// The layout of private storage is **this backend's own choice**, because
/// private slots are unreachable from outside a running program: the host API
/// exposes `get_shared`/`set_shared` and no private equivalent. Native code
/// therefore need not agree with the runtime's `GenericValue` representation,
/// which is just as well, since that type carries no `#[repr]` and its layout
/// is unspecified.
///
/// A flat word per slot is chosen. The only quantity crossing the boundary is
/// the SIZE of the region, which the runtime reserves as
/// `private_count * size_of::<GenericValue>()`, so a native layout fits exactly
/// when it is no wider per slot. That is asserted rather than argued, because
/// "8 is less than 32" is a fact about two current choices and not an
/// invariant.
pub const PRIVATE_SLOT_BYTES: u32 = 8;

const _: () = assert!(
    PRIVATE_SLOT_BYTES as usize <= 32,
    "a private slot wider than the runtime's reserved Value slot would overrun \
     a region sized by the runtime's arithmetic"
);

/// High bit of a shared slot's kind byte marks a composite body.
const SHARED_COMPOSITE_FLAG: u8 = 0x80;

/// Resolve a shared scalar slot to `(byte offset, load width, kind tag)`.
///
/// Refuses rather than guesses in four cases, each of which would otherwise
/// decode the host's buffer at the wrong width and return a plausible number:
/// a private slot, a slot outside the declared layout, a composite body, and a
/// scalar kind whose representation this backend has not settled.
fn resolve_shared_scalar<'ctx>(
    data: &DataCtx<'_>,
    slot: u32,
    i8t: IntType<'ctx>,
    i64t: IntType<'ctx>,
) -> Result<(u32, IntType<'ctx>, u8), LowerError> {
    if slot >= data.shared_count {
        return Err(LowerError::UnsupportedDataSlot {
            slot,
            why: String::from(
                "private slot; private storage is a later increment and its native layout is                  this backend's own choice rather than the runtime's",
            ),
        });
    }
    let e =
        data.shared_layout
            .get(slot as usize)
            .ok_or_else(|| LowerError::UnsupportedDataSlot {
                slot,
                why: String::from("shared slot index outside the declared layout table"),
            })?;
    if e.kind & SHARED_COMPOSITE_FLAG != 0 {
        return Err(LowerError::UnsupportedDataSlot {
            slot,
            why: String::from("shared composite body; Workstream C"),
        });
    }
    match e.kind {
        SCALAR_INT => Ok((e.offset, i64t, e.kind)),
        SCALAR_BYTE | SCALAR_BOOL => Ok((e.offset, i8t, e.kind)),
        other => Err(LowerError::UnsupportedDataSlot {
            slot,
            why: alloc_format_kind(other),
        }),
    }
}

fn alloc_format_kind(tag: u8) -> String {
    match tag {
        0 => String::from("Unit slot; the flat representation of Unit is unsettled"),
        4 => String::from("Fixed slot; fixed-point representation is unsettled"),
        5 => String::from("Float slot; float support is a later workstream"),
        6 => String::from("Text slot; string representation is Workstream C"),
        7 => String::from("Opaque slot; host handles are Workstream D"),
        n => format!("unknown scalar kind tag {n}"),
    }
}

/// Width in bytes of a shared scalar kind this backend lowers.
fn shared_scalar_width(kind: u8) -> Option<u32> {
    match kind {
        SCALAR_INT => Some(8),
        SCALAR_BYTE | SCALAR_BOOL => Some(1),
        _ => None,
    }
}

/// Prove that shared slots `base .. base + count` form a contiguous, uniform
/// array in the host buffer, returning `(first offset, element width, kind)`.
///
/// **Checked rather than assumed.** Measured over the corpus, all 556,496
/// adjacent shared scalar pairs are contiguous, with no exceptions. That is a
/// property of today's compiler and NOT a guarantee the wire format states, so
/// relying on it would make the lowering silently wrong if the layout ever
/// changed. Verifying it per module costs one pass over `count` table entries
/// and converts an assumption into a precondition, which is the same move the
/// `i64::MIN / -1` guard makes for a different unsound-by-default case.
fn resolve_shared_array(
    data: &DataCtx<'_>,
    base: u32,
    count: u32,
) -> Result<(u32, u32, u8), LowerError> {
    let refuse = |why: String| LowerError::UnsupportedDataSlot { slot: base, why };
    if count == 0 {
        return Err(refuse(String::from("zero-length shared array")));
    }
    let first = data
        .shared_layout
        .get(base as usize)
        .ok_or_else(|| refuse(String::from("shared array base outside the layout table")))?;
    if first.kind & SHARED_COMPOSITE_FLAG != 0 {
        return Err(refuse(String::from(
            "shared array of composite bodies; Workstream C",
        )));
    }
    let width =
        shared_scalar_width(first.kind).ok_or_else(|| refuse(alloc_format_kind(first.kind)))?;
    for i in 1..count {
        let e = data
            .shared_layout
            .get((base + i) as usize)
            .ok_or_else(|| refuse(String::from("shared array runs past the layout table")))?;
        if e.kind != first.kind {
            return Err(refuse(format!(
                "shared array is not uniform: element {i} has kind {} against {}",
                e.kind, first.kind
            )));
        }
        if e.offset != first.offset + i * width {
            return Err(refuse(format!(
                "shared array is NOT contiguous: element {i} sits at offset {} where a stride of \
                 {width} predicts {}",
                e.offset,
                first.offset + i * width
            )));
        }
    }
    Ok((first.offset, width, first.kind))
}

/// Declare the host yield hook, reusing an existing declaration.
///
/// **This is a provisional application binary interface decision**, the fourth
/// on this branch after the symbol scheme, the shared-buffer pointer and the
/// private-region layout, and like those it belongs to Workstream D.
///
/// `i64 kel_yield(i64)` takes the yielded value and returns the resume value.
/// It inverts control relative to the runtime, where `call` returns
/// `Yielded(v)` and the host calls `resume(r)`, but the OBSERVABLE SEQUENCE of
/// yielded and resumed values is identical, which is what the differential
/// oracle compares. The inversion is why this suits a reentrant `yield fn`,
/// which suspends a bounded number of times and returns, and does NOT suit a
/// divergent `loop fn`, which would spin inside native code with no way for the
/// host to stop it. `Op::Stream` and `Op::Reset` stay refused for that reason
/// rather than by omission.
fn yield_hook<'ctx>(ctx: &'ctx Context, module: &LlvmModule<'ctx>) -> FunctionValue<'ctx> {
    module.get_function("kel_yield").unwrap_or_else(|| {
        let i64t = ctx.i64_type();
        module.add_function("kel_yield", i64t.fn_type(&[i64t.into()], false), None)
    })
}

/// Emit a static string literal as a constant global and return its address.
///
/// # Layout: `{ i64 len, [n+1 x i8] bytes }`, the bytes NUL-terminated
///
/// The length is explicit and comes first because **a Keleusma string is a byte
/// string, not a C string**: nothing in the language forbids an interior NUL, so
/// a bare `char*` would silently truncate one. The trailing NUL is added anyway,
/// costing one byte, so that a host written in C can pass the pointer straight
/// to a string function for the overwhelmingly common literal that contains no
/// NUL. A host that cares reads the length; a host that does not still works.
///
/// # This is a host-visible ABI decision
///
/// On the virtual machine a string-taking native receives an owned `String`
/// through marshalling. Natively it receives this pointer. **The two embeddings
/// are therefore not source-compatible for a string-taking native**, which is
/// inherent to ahead-of-time lowering rather than a defect here, but it is new
/// surface and the operator may want a different shape.
///
/// # Why no deduplication
///
/// Identical literals get separate globals. Sharing them means naming a global
/// by a hash of its contents, and a hash collision would silently bind two
/// DIFFERENT strings to one global — a wrong-answer failure to save a handful
/// of bytes. The whole shipped corpus contains **ten** static strings.
fn static_string_global<'ctx>(
    ctx: &'ctx Context,
    module: &LlvmModule<'ctx>,
    s: &str,
) -> PointerValue<'ctx> {
    let i64t = ctx.i64_type();
    let i8t = ctx.i8_type();

    // First unused name. Linear probing, which is quadratic in the number of
    // string literals and irrelevant at ten.
    let mut n = 0usize;
    let name = loop {
        let cand = format!("kel_str_{n}");
        if module.get_global(&cand).is_none() {
            break cand;
        }
        n += 1;
    };

    let mut bytes: Vec<_> = s
        .as_bytes()
        .iter()
        .map(|b| i8t.const_int(u64::from(*b), false))
        .collect();
    bytes.push(i8t.const_zero());
    let data = i8t.const_array(&bytes);

    let ty = ctx.struct_type(&[i64t.into(), data.get_type().into()], false);
    let init = ty.const_named_struct(&[i64t.const_int(s.len() as u64, false).into(), data.into()]);

    let g = module.add_global(ty, None, &name);
    g.set_initializer(&init);
    g.set_constant(true);
    g.set_linkage(inkwell::module::Linkage::Internal);
    g.as_pointer_value()
}

/// Declare `llvm.trap`, reusing the existing declaration if the module already
/// carries one. Lowering a second chunk into the same module must not redeclare
/// it.
fn trap_declaration<'ctx>(ctx: &'ctx Context, module: &LlvmModule<'ctx>) -> FunctionValue<'ctx> {
    module.get_function("llvm.trap").unwrap_or_else(|| {
        module.add_function("llvm.trap", ctx.void_type().fn_type(&[], false), None)
    })
}

/// Lower one bytecode chunk into `module` as a function named `sym`.
///
/// The emitted signature is `param_count` × `i64` returning `i64`, which is the
/// 64-bit word width (`word_bits_log2 == 6`). Narrower word widths are a later
/// increment and are refused rather than silently lowered at the wrong width.
pub fn lower_chunk<'ctx>(
    ctx: &'ctx Context,
    module: &LlvmModule<'ctx>,
    chunk: &Chunk,
    sym: &str,
    opts: LowerOptions,
) -> Result<FunctionValue<'ctx>, LowerError> {
    let i64t = ctx.i64_type();
    let params: Vec<_> = (0..chunk.param_count).map(|_| i64t.into()).collect();
    let func = module.add_function(sym, i64t.fn_type(&params, false), None);
    // No callees are visible, so any `Op::Call` is refused. A single chunk
    // cannot resolve one: the target is an index into the module's chunk table,
    // which this entry point does not receive.
    lower_chunk_body(
        ctx,
        module,
        chunk,
        func,
        &[],
        DataCtx::default(),
        BodyCfg {
            opts,
            degenerate_yield: None,
            natives: &[],
        },
    )
}

/// Lower every chunk in a module, so that `Op::Call` resolves.
///
/// Functions are declared for all chunks BEFORE any body is lowered, which is
/// what lets a call reference a chunk that has not been lowered yet. Declaration
/// order would otherwise matter, and it should not: the type checker rejects
/// direct and mutual recursion, so the call graph is acyclic, but "acyclic" does
/// not mean "callees come first" in the chunk table.
///
/// Symbols are `kel_chunk_<index>`. **This is deliberately not the R4.2 mangling
/// scheme.** That scheme encodes purity, category, module path and type
/// arguments for EXTERNAL linkage across separately compiled artefacts, and it
/// needs metadata a `Chunk` does not carry. Nothing here is externally linked
/// yet, so an internal, obviously-provisional name is more honest than a
/// half-implemented mangling that looks authoritative.
pub fn lower_module<'ctx>(
    ctx: &'ctx Context,
    module: &LlvmModule<'ctx>,
    program: &Module,
    opts: LowerOptions,
) -> Result<Vec<FunctionValue<'ctx>>, LowerError> {
    lower_module_with(ctx, module, program, opts, None)
}

/// Every chunk-level refusal in `program`, rather than only the first.
///
/// **This exists to make the slack measurement DERIVABLE.** Every blocker ranking
/// on this line has been computed from a hand-maintained model of what the
/// lowering supports, because `lower_module` returns one verdict per module and
/// stops. Three copies of that model exist and all three went stale, in the
/// pessimistic direction, which the drift control could not detect and which
/// silently understates every other blocker class.
///
/// A module's refusal SET is the union over its chunks. That is coarser than
/// per-op and derived from the real lowering rather than restated beside it,
/// which is the property that matters.
pub fn module_refusals(program: &Module, opts: LowerOptions) -> Vec<(String, LowerError)> {
    let ctx = Context::create();
    let m = ctx.create_module("refusals");
    let mut sink = Vec::new();
    let _ = lower_module_with(&ctx, &m, program, opts, Some(&mut sink));
    sink
}

fn lower_module_with<'ctx>(
    ctx: &'ctx Context,
    module: &LlvmModule<'ctx>,
    program: &Module,
    opts: LowerOptions,
    mut refusals: Option<&mut Vec<(String, LowerError)>>,
) -> Result<Vec<FunctionValue<'ctx>>, LowerError> {
    check_word_width(program.word_bits_log2)?;
    let i64t = ctx.i64_type();
    let ptrt = ctx.ptr_type(AddressSpace::default());

    // Shared slots occupy the low indices of the unified slot space, so their
    // count is also the private boundary.
    let (shared_count, shared_layout): (u32, &[SharedSlotLayout]) = match &program.data_layout {
        Some(dl) => (
            dl.slots
                .iter()
                .filter(|s| s.visibility == SlotVisibility::Shared)
                .count() as u32,
            dl.shared_layout.as_slice(),
        ),
        None => (0, &[]),
    };
    let needs_region = program.chunks.iter().any(chunk_builds_composite);
    let data = DataCtx {
        needs_region,
        shared_count,
        shared_layout,
        has_data: program
            .data_layout
            .as_ref()
            .is_some_and(|dl| !dl.slots.is_empty()),
        slot_count: program
            .data_layout
            .as_ref()
            .map(|dl| dl.slots.len() as u32)
            .unwrap_or(0),
    };

    // **The shared buffer arrives as a trailing pointer parameter**, and only
    // when the module declares shared slots, so a module without them keeps the
    // signature it had. This mirrors the runtime, where the buffer is supplied
    // per call through `call_with_shared` rather than installed globally, and it
    // keeps the lowering reentrant: a module-level global would be shared by
    // every concurrent invocation. Like the symbol scheme, it is provisional and
    // belongs to Workstream D's application binary interface.
    // **Uniform data ABI.** A module that declares any data slot takes TWO
    // trailing pointers, the shared buffer and the private region, in that
    // order. Making the signature depend on which KINDS of slot a module
    // declares would vary the calling convention along two independent
    // dimensions, and a caller would have to reproduce that reasoning to get
    // the arity right. One rule is cheaper than two, and an unused pointer
    // costs a register.
    let has_data = program
        .data_layout
        .as_ref()
        .is_some_and(|dl| !dl.slots.is_empty());
    if shared_count > 0 {
        check_target_endianness()?;
    }
    let declared: Vec<FunctionValue<'ctx>> = program
        .chunks
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let mut params: Vec<_> = (0..c.param_count).map(|_| i64t.into()).collect();
            if has_data || needs_region {
                params.push(ptrt.into()); // shared buffer
                params.push(ptrt.into()); // private region
                // The composite body region. Host-supplied, and documented as
                // coming from the arena's BOTTOM section: the caller takes at
                // least `region::plan_chunk_region(chunk).bytes` from
                // `alloc_bottom_bytes` and releases it with `bottom_mark`.
                //
                // Naming the provenance is not decoration. A region of
                // unspecified origin would put the backend's memory outside the
                // arena's accounting, and transferring that bound is the whole
                // property this lowering exists to preserve. The backend never
                // allocates and never frees; it writes at compile-time offsets
                // into memory the host scoped.
                params.push(ptrt.into()); // composite body region
            }
            module.add_function(
                &format!("kel_chunk_{i}"),
                i64t.fn_type(&params, false),
                None,
            )
        })
        .collect();

    // A module-level precondition, checked before any body is lowered. Two
    // natives whose names differ but whose SYMBOLS coincide would each bind to
    // the same host definition, and every call site would look correct in
    // isolation. Refusing the module is the only place this is visible.
    if let Some((sym, names)) = native_symbol_collisions(&program.native_names) {
        return Err(LowerError::UnsupportedOp(format!(
            "natives {names:?} all mangle to the external symbol `{sym}`; the \
             lowering refuses rather than binding several declarations to one \
             host definition"
        )));
    }

    for (chunk, func) in program.chunks.iter().zip(declared.iter()) {
        let tail = degenerate_stream_yield(chunk, program);
        let cfg = BodyCfg {
            opts,
            degenerate_yield: tail.as_deref(),
            natives: &program.native_names,
        };
        match lower_chunk_body(ctx, module, chunk, *func, &declared, data, cfg) {
            Ok(_) => {}
            Err(e) => {
                // In diagnostic mode, record and carry on to the NEXT CHUNK.
                //
                // A chunk's own lowering still stops at its first refusal, since
                // continuing inside one would desynchronise the operand-stack
                // depth and produce nonsense. Per-chunk is nonetheless strictly
                // better than the per-module verdict every ranking has been
                // computed from: a module's refusal SET is the union over its
                // chunks, which is what makes a slack measurement derivable from
                // the real lowering instead of from a hand-maintained model. The
                // module is left unusable either way, which is why this returns
                // the refusals rather than the functions.
                if let Some(sink) = refusals.as_mut() {
                    sink.push((chunk.name.clone(), e));
                    continue;
                }
                return Err(e);
            }
        }
    }
    if let Some(sink) = refusals.as_ref() {
        // Diagnostic mode never claims a usable module: the IR is incomplete
        // wherever a chunk was abandoned, so returning it would invite someone
        // to run it.
        //
        // The error carries only a COUNT; the list stays with the caller. An
        // earlier version used `core::mem::take`, which emptied the caller's
        // vector into an error the caller discards — so `module_refusals`
        // reported that every module lowers cleanly. A wrong answer in the
        // reassuring direction, which is the one nobody questions.
        return Err(LowerError::Diagnostic(sink.len()));
    }

    // Ask LLVM to verify what we just produced. This was previously done only in
    // the test harness (`lm.verify()` in three test files, nowhere in `src/`),
    // which meant the one check that would catch malformed IR was the one never
    // run in production. A postcondition belongs at the boundary that promises
    // it, not in the tests that happen to exercise it.
    module
        .verify()
        .map_err(|e| LowerError::InvalidIr(e.to_string()))?;

    Ok(declared)
}

/// What the lowering knows about the module's data segment.
///
/// Empty for [`lower_chunk`], which receives one chunk and therefore cannot see
/// the module's layout table. Data-segment access is refused there for the same
/// reason a call is: the information required to resolve it was never supplied.
#[derive(Default, Clone, Copy)]
struct DataCtx<'a> {
    /// Number of shared slots. Shared slots occupy the low indices, so this is
    /// also the boundary above which a slot is private.
    shared_count: u32,
    /// Layout entries for the shared slots, indexed by slot.
    shared_layout: &'a [SharedSlotLayout],
    /// Whether the module declares any data slot at all, which decides whether
    /// the two trailing pointers are present.
    has_data: bool,
    /// Total declared slots, shared plus private.
    slot_count: u32,
    /// Whether ANY chunk in the module constructs a flat composite, and so
    /// whether the trailing region pointer is present.
    ///
    /// Module-wide rather than per-chunk on purpose. Deciding arity per chunk
    /// would make a caller reproduce this analysis to get the signature right,
    /// which is the same two-dimensional arity the shared/private pair already
    /// refuses. All three pointers or none.
    needs_region: bool,
}

/// How many trailing pointer parameters a function in this module carries.
///
/// **One source, used by the signature, the call-arity check and the argument
/// forwarding alike.** A hand-written `- 2` in the arity check drifted the
/// moment a third pointer was added and made every correct call read as a short
/// call; deriving it is what stops that recurring.
fn trailing_ptrs(data: &DataCtx<'_>) -> u32 {
    if data.has_data || data.needs_region {
        3
    } else {
        0
    }
}

/// Whether a chunk constructs a flat composite, and so needs somewhere to put a
/// body.
///
/// `Boxed` does not count: it carries no baked body size, the corpus contains
/// none, and the construction arm refuses it rather than placing it.
fn chunk_builds_composite(chunk: &Chunk) -> bool {
    chunk.ops.iter().any(|o| {
        matches!(
            o,
            Op::NewComposite(keleusma::bytecode::NewCompositeOperand::Flat { .. })
        )
    })
}

/// Per-chunk lowering configuration.
///
/// Exists because `lower_chunk_body` reached eight positional arguments, which
/// clippy flags and which is a real readability cost rather than a lint to
/// suppress. These two travel together: both are decided by the caller and both
/// stay constant for the whole body.
#[derive(Clone, Copy)]
struct BodyCfg<'a> {
    opts: LowerOptions,
    /// `Some(ip)` when this is a degenerate stream chunk and `ip` is the
    /// `Op::Yield` that becomes the return. Computed by the caller, which holds
    /// the bytecode module; `lower_chunk` passes `None` for the same reason it
    /// refuses `Op::Call`.
    degenerate_yield: Option<&'a [usize]>,
    /// The module's declared native names, indexed by the operand of
    /// `CallVerifiedNative`/`CallExternalNative`.
    ///
    /// Empty for `lower_chunk`, which therefore refuses a native call for the
    /// same reason it refuses `Op::Call`: the operand is an index into a
    /// module-level table a single chunk does not receive.
    ///
    /// **Names rather than indices, because that is what the runtime binds on.**
    /// `Vm::run` resolves the operand to a name through `native_name(idx)` and
    /// then searches its registry by NAME. Emitting `kel_native_<index>` would
    /// have bound the object file to declaration order, which is a property of
    /// the source text rather than of the interface.
    natives: &'a [String],
}

/// The external symbol a declared native binds to.
///
/// `host::play` becomes `kel_native_host_play`. Any character outside
/// `[A-Za-z0-9_]` becomes `_`, since a Keleusma path separator is not a legal C
/// identifier and the whole point of this backend is an object file an ordinary
/// linker resolves.
///
/// **The mapping is not injective**, which is why [`native_symbol_collisions`]
/// exists: `host::play` and `host_play` both land here. Two natives sharing a
/// symbol would silently bind both call sites to whichever the host defined,
/// and the differential oracle would only catch it if the corpus happened to
/// call both.
fn native_symbol(name: &str) -> String {
    let mut s = String::from("kel_native_");
    for c in name.chars() {
        s.push(if c.is_ascii_alphanumeric() || c == '_' {
            c
        } else {
            '_'
        });
    }
    s
}

/// Distinct native names that collide onto one external symbol.
///
/// Returns the offending symbol and the names that share it. A module-level
/// precondition rather than a per-site check, because the hazard is a PAIR of
/// declarations and no single call site can see it.
fn native_symbol_collisions(names: &[String]) -> Option<(String, Vec<String>)> {
    let mut by_symbol: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for n in names {
        by_symbol
            .entry(native_symbol(n))
            .or_default()
            .push(n.clone());
    }
    by_symbol.into_iter().find(|(_, ns)| ns.len() > 1)
}

/// Is this chunk a **degenerate stream**, lowerable as a plain function?
///
/// A degenerate stream is `Stream ; <body> ; Yield ; PopN(1) ; Reset` with the
/// `Yield` at nesting depth zero and nothing able to observe a suspension. It
/// lowers to a single entry point whose signature needs no special case, because
/// the data pointers already trail the declared parameters:
///
/// ```text
/// kel_chunk_N(resume, shared, private) -> i64
/// ```
///
/// # Why ONE entry point, not an init/step pair
///
/// `Vm::resume_after_enter` writes the resume value into local slot 0 and then
/// pushes it as the suspended `Yield`'s result. The parameter reproduces the
/// first; the `PopN(1)` discards the second, because the `yield` is the body's
/// tail expression. Iteration zero takes its value from the `call` argument and
/// iteration k from the k-th `resume`, so `f(a)`, `f(r1)`, `f(r2)` reproduces the
/// virtual machine's whole sequence with **no distinguished first call**.
///
/// # Why the module and not just the chunk
///
/// **A delegated suspension is invisible in the chunk's own ops.**
/// `resume_after_enter` writes slot 0 of `frames.first()`, the ENTRY chunk,
/// whenever that entry is a `Stream` — regardless of which frame suspended. So a
/// nested `yield fn` callee's suspension updates this chunk's resume parameter in
/// the virtual machine, while natively the `kel_yield` return reaches only the
/// callee's operand stack. The next iteration would read a stale value.
///
/// The test is exact rather than conservative: `category_can_call` enforces
/// `Fn => matches!(callee, Fn)`, so the transitive closure of a `Func` chunk
/// contains only `Func` chunks, and requiring every callee to be `Func` settles
/// it from the direct call sites alone. No call-graph walk and no
/// `compute_always_yielding`, which is behind a feature this package does not
/// enable.
///
/// Returns the indices of every `Op::Yield` that becomes a return.
fn degenerate_stream_yield(chunk: &Chunk, module: &Module) -> Option<Vec<usize>> {
    if chunk.block_type != BlockType::Stream {
        return None;
    }
    let ops = &chunk.ops;

    // The prologue must be EMPTY. `Reset` rewinds to just after `Stream`, so an
    // op before `Stream` runs once in the virtual machine and on every call
    // natively. This reads as tidiness and is the one that changes behaviour.
    if !matches!(ops.first(), Some(Op::Stream)) {
        return None;
    }
    // `Reset` last. A tail after it is unreachable in the virtual machine, which
    // rewinds rather than falling through, and reachable natively.
    if !matches!(ops.last(), Some(Op::Reset)) {
        return None;
    }
    // Slot 0 is the resume parameter; a second has no native source.
    if chunk.param_count > 1 {
        return None;
    }
    // Every callee must be `Func`, so none can suspend. An unresolvable index
    // refuses rather than skips: admitting on missing evidence is the wrong
    // default in a soundness check.
    for op in ops {
        if let Op::Call(idx, _) = op
            && module.chunks.get(*idx as usize).map(|c| c.block_type) != Some(BlockType::Func)
        {
            return None;
        }
    }

    // EVERY `Yield` must be in TAIL POSITION: nothing but block delimiters and
    // the final `PopN(1)` executes between it and `Reset` on any path.
    //
    // This replaces an earlier rule requiring exactly one `Yield` at nesting
    // depth zero. That rule was not wrong, it was **too narrow**: it described
    // the shape eight stages happen to have rather than the property that makes
    // the transformation sound. `lexer.kel` has nineteen yields nested up to
    // depth eleven, every one of them inside an `If` and none under a `Loop`, so
    // each path still yields exactly once and ends. It is a control-flow JOIN,
    // not a suspension across a back edge, and the depth rule rejected it for a
    // reason that has nothing to do with correctness.
    //
    // The walk FOLLOWS JUMPS rather than scanning linearly. A linear scan is
    // unusable here: the ops textually between a nested `Yield` and `Reset`
    // include other branches' bodies, which are on different paths. Following
    // `Else`/`EndLoop` targets asks the question that actually matters.
    let mut tail_yields: Vec<usize> = Vec::new();
    for (ip, op) in ops.iter().enumerate() {
        if !matches!(op, Op::Yield) {
            continue;
        }
        let mut j = ip + 1;
        // Net operand-stack effect of the tail, which must end at -1: the
        // segment consumes exactly the resume value the `Yield` pushed.
        let mut delta: i32 = 0;
        loop {
            match ops.get(j) {
                // A branch delimiter carries no value and ends no path.
                Some(Op::EndIf) => j += 1,
                // A forward jump past the sibling arm, or a loop back edge that
                // cannot be taken from here; follow it rather than walking into
                // code this path never runs.
                Some(Op::Else(t)) | Some(Op::EndLoop(t)) => {
                    let t = *t as usize;
                    // Refuse rather than loop forever on malformed bytecode.
                    if t <= j {
                        return None;
                    }
                    j = t;
                }
                // Any op that touches ONLY the operand stack and this frame's
                // locals, tracked by net depth rather than by name.
                //
                // This replaced a two-element allowlist on 2026-08-11. The old
                // rule admitted exactly `PopN(1)` and refused ten corpus chunks
                // whose tail is `PopN(1), Const(0), PopN(1)` — a body that
                // suspends and then evaluates a discarded trailing constant.
                // That sequence is effect-free and reaches the same depth, so
                // it is exactly as safe as the one the rule admitted. The
                // allowlist was drawn from the instructions its author had seen
                // and coincided with the property only on that sample.
                //
                // Traps are EXCLUDED deliberately, so no checked arithmetic
                // appears here: a trap is observable, and the virtual machine
                // would take it after the suspension where native code, having
                // already returned, would not.
                Some(Op::Const(c))
                    if matches!(
                        chunk.constants.get(*c as usize),
                        Some(
                            ConstValue::Int(_)
                                | ConstValue::Byte(_)
                                | ConstValue::Bool(_)
                                | ConstValue::Unit
                        )
                    ) =>
                {
                    delta += 1;
                    j += 1;
                }
                Some(Op::PushImmediate(_)) | Some(Op::GetLocal(_)) | Some(Op::Dup) => {
                    delta += 1;
                    j += 1;
                }
                Some(Op::PopN(n)) => {
                    delta -= i32::from(*n);
                    j += 1;
                }
                Some(Op::SetLocal(_)) => {
                    // Writes a local that `Reset` clears, so unobservable.
                    delta -= 1;
                    j += 1;
                }
                Some(Op::Reset) => break,
                // Anything else may consume the resumed value, write the data
                // segment, call out, or trap. None of those survives.
                _ => return None,
            }
        }
        if delta != -1 {
            return None;
        }
        tail_yields.push(ip);
    }
    if tail_yields.is_empty() {
        return None;
    }
    Some(tail_yields)
}

fn lower_chunk_body<'ctx>(
    ctx: &'ctx Context,
    module: &LlvmModule<'ctx>,
    chunk: &Chunk,
    func: FunctionValue<'ctx>,
    callees: &[FunctionValue<'ctx>],
    data: DataCtx<'_>,
    cfg: BodyCfg<'_>,
) -> Result<FunctionValue<'ctx>, LowerError> {
    let BodyCfg {
        opts,
        degenerate_yield,
        natives,
    } = cfg;
    let i64t = ctx.i64_type();
    let i128t = ctx.i128_type();
    let i8t = ctx.i8_type();

    let b = ctx.create_builder();
    let entry = ctx.append_basic_block(func, "entry");
    let trap_bb = ctx.append_basic_block(func, "trap");

    b.position_at_end(entry);
    let locals: Vec<_> = (0..chunk.local_count as usize)
        .map(|i| b.build_alloca(i64t, &format!("l{i}")).unwrap())
        .collect();
    for (i, local) in locals.iter().enumerate().take(chunk.param_count as usize) {
        b.build_store(
            *local,
            func.get_nth_param(i as u32).unwrap().into_int_value(),
        )
        .unwrap();
    }
    // Non-parameter locals start DEFINED in the VM: `Op::Call` pushes
    // `local_count - arg_count` copies of `GenericValue::Unit` into the callee's
    // frame before entering it. An uninitialised `alloca` loads as `undef`, so
    // without this a `GetLocal` of an unwritten slot is `Unit` in the VM and
    // `undef` here -- and `undef` feeding a branch or an offset is worse than a
    // wrong answer. Zero is this backend's `Unit`.
    //
    // `mem2reg` folds these stores away wherever the slot is later overwritten,
    // so the cost is nil on every chunk the compiler actually emits.
    for local in locals.iter().skip(chunk.param_count as usize) {
        b.build_store(*local, i64t.const_zero()).unwrap();
    }
    // Operand slots are allocated LAZILY by `Lower::ensure_slot`, so a chunk
    // that uses three of them pays for three. The previous unconditional
    // `MAX_STACK` provisioning was measured to dominate the native frame.
    let slots: Vec<PointerValue> = Vec::new();

    // The trailing pointer parameter, present only when the module declares
    // shared slots. Read once at entry; every access is an offset from it.
    #[allow(clippy::type_complexity)]
    let (shared_base, private_base, region_base): (
        Option<PointerValue<'ctx>>,
        Option<PointerValue<'ctx>>,
        Option<PointerValue<'ctx>>,
    ) =
        // Bound whenever the three-pointer group is present, which is what
        // decides where they sit. A module that builds a composite but declares
        // no slot still HAS these parameters; the data arms refuse on
        // `has_data` separately, so an unused pointer here is harmless.
        if data.has_data || data.needs_region {
            let n = chunk.param_count as u32;
            (
                Some(
                    func.get_nth_param(n)
                        .expect("data module declares the shared pointer")
                        .into_pointer_value(),
                ),
                Some(
                    func.get_nth_param(n + 1)
                        .expect("data module declares the private pointer")
                        .into_pointer_value(),
                ),
                Some(
                    func.get_nth_param(n + 2)
                        .expect("the trailing region pointer")
                        .into_pointer_value(),
                ),
            )
        } else {
            (None, None, None)
        };

    let trapfn = trap_declaration(ctx, module);
    b.position_at_end(trap_bb);
    b.build_call(trapfn, &[], "").unwrap();
    b.build_unreachable().unwrap();
    b.position_at_end(entry);

    // One basic block per jump target. Keleusma's structured control flow means
    // the target set is exactly the operands of If and Else.
    // NOTE `Loop`'s own operand is deliberately NOT collected. It duplicates the
    // `Break` targets whenever a break exists, and manufactures a block with no
    // incoming edge when one does not.
    let mut targets: Vec<usize> = Vec::new();
    for op in chunk.ops.iter() {
        match op {
            Op::If(t) | Op::Else(t) | Op::EndLoop(t) | Op::Break(t) | Op::BreakIf(t) => {
                targets.push(*t as usize)
            }
            _ => {}
        }
    }
    targets.sort_unstable();
    targets.dedup();
    let blocks: BTreeMap<usize, BasicBlock> = targets
        .iter()
        .map(|&t| (t, ctx.append_basic_block(func, &format!("op{t}"))))
        .collect();

    // Parameter widths come from the chunk's declared signature, which is the
    // only place they are stated. Everything else starts unknown so an
    // unlabelled write cannot be mistaken for a word.
    let mut local_widths = alloc_vec_unknown(locals.len());
    for (i, t) in chunk.param_types.iter().enumerate() {
        if i < local_widths.len() {
            local_widths[i] = width_of_tag(*t);
        }
    }

    let mut st = Lower {
        b,
        i64t,
        entry,
        locals,
        slots,
        depth: 0,
        stack_overflow: None,
        widths: Vec::new(),
        local_widths,
    };

    // Every local this chunk writes anywhere, computed up front so the decision
    // does not depend on instruction order. See the `GetLocal` arm.
    // How many times each local is WRITTEN anywhere in the chunk, counted up
    // front so the rule does not depend on instruction order.
    //
    // A local written EXACTLY ONCE has an unambiguous width: there is no second
    // write to disagree with, whatever the control flow. That admits the `let`
    // bindings the corpus is built from, which the previous rule refused
    // outright — every composite assembled from a `let` was rejected for want of
    // a width, including every array of arrays.
    //
    // Two or more writes stay unknown. Joining them would need a real dataflow
    // pass: a LINEAR walk cannot see a back edge, so a local rewritten later in
    // a loop body would be read at the width of the textually earlier write and
    // packed wrongly on every iteration after the first.
    let mut local_write_count: BTreeMap<usize, u32> = BTreeMap::new();
    for o in &chunk.ops {
        if let Op::SetLocal(n) = o {
            *local_write_count.entry(*n as usize).or_default() += 1;
        }
    }

    // Operand-stack depth at each merge point, recorded per incoming edge.
    let mut tdepth: BTreeMap<usize, usize> = BTreeMap::new();
    macro_rules! note {
        ($t:expr, $d:expr) => {{
            let (t, d) = ($t, $d);
            if let Some(&prev) = tdepth.get(&t) {
                assert_eq!(
                    prev, d,
                    "operand-stack depth disagreement entering op{t}: {prev} vs {d}. \
                     The typed verifier guarantees agreement, so this is a lowering bug."
                );
            }
            tdepth.insert(t, d);
        }};
    }

    // Whether the opcode stream has run past a terminator into code no edge
    // reaches. The compiler emits this routinely: `break;` is an unconditional
    // jump, and the arm's value push sits immediately after it, unreachable.
    let mut dead = false;

    for (i, op) in chunk.ops.iter().enumerate() {
        if let Some(&bb) = blocks.get(&i) {
            if !dead && st.b.get_insert_block().unwrap().get_terminator().is_none() {
                note!(i, st.depth);
                st.b.build_unconditional_branch(bb).unwrap();
            }
            st.b.position_at_end(bb);
            match tdepth.get(&i) {
                Some(&d) => {
                    st.depth = d;
                    dead = false;
                }
                // A block target no edge reaches. The exit of a `loop` with no
                // `break` is the real case: it is a legitimate program, not a
                // lowering bug, so it must not be an assertion failure.
                None => {
                    st.b.build_unreachable().unwrap();
                    dead = true;
                }
            }
        } else if !dead && st.b.get_insert_block().unwrap().get_terminator().is_some() {
            dead = true;
        }

        if dead {
            continue;
        }

        // **A tuple field read IS a struct field read.** `TupleField` and
        // `StructField` carry the same three variants with the same fields, and
        // a flat body does not record which composite kind it came from, so the
        // emitted address arithmetic and load are identical.
        //
        // Normalised here rather than given its own arms. Two copies of ~50
        // lines of pointer arithmetic is exactly the duplication that made one
        // predicate wrong in three places on the `v0.2.3` line and that the
        // `is_lowered` model needs a drift control to survive. One code path
        // cannot drift from itself.
        let normalised;
        let op = match op {
            Op::GetTupleField(tf) => {
                use keleusma::bytecode::{StructField as SF, TupleField as TF};
                normalised = Op::GetField(match tf {
                    TF::Flat { offset, kind } => SF::Flat {
                        offset: *offset,
                        kind: *kind,
                    },
                    TF::FlatNested {
                        offset,
                        size,
                        variant,
                    } => SF::FlatNested {
                        offset: *offset,
                        size: *size,
                        variant: *variant,
                    },
                    // The boxed form keeps its own refusal rather than being
                    // mapped onto a struct's, so the message names what the
                    // source actually contains.
                    TF::Boxed { .. } => {
                        return Err(LowerError::UnsupportedOp(format!(
                            "GetTupleField reading {tf:?} is not lowered"
                        )));
                    }
                });
                &normalised
            }
            // An enum payload read is the same shape again: the operand carries
            // an offset already measured PAST the discriminant word, so the
            // arithmetic is identical to a struct field's and normalising costs
            // nothing. Third consumer of one code path rather than a third copy.
            Op::GetEnumField(ef) => {
                use keleusma::bytecode::{EnumField as EF, StructField as SF};
                normalised = Op::GetField(match ef {
                    EF::Flat { offset, kind } => SF::Flat {
                        offset: *offset,
                        kind: *kind,
                    },
                    EF::FlatNested {
                        offset,
                        size,
                        variant,
                    } => SF::FlatNested {
                        offset: *offset,
                        size: *size,
                        variant: *variant,
                    },
                    other => {
                        return Err(LowerError::UnsupportedOp(format!(
                            "GetEnumField reading {other:?} is not lowered"
                        )));
                    }
                });
                &normalised
            }
            other => other,
        };

        match op {
            Op::GetLocal(n) => {
                let v =
                    st.b.build_load(i64t, st.locals[*n as usize], "gl")
                        .unwrap()
                        .into_int_value();
                // A local's width is trusted ONLY when the chunk never writes
                // it, so its value is the parameter the signature described.
                //
                // A linear scan of `SetLocal` cannot establish more than that:
                // it walks instructions in order and CANNOT SEE A BACK EDGE, so
                // a local rewritten later in a loop body would be read here at
                // the width of the write that appears earlier in the text and
                // packed wrongly on every iteration after the first. Anything
                // written is unknown, which costs coverage and cannot mispack.
                // Trusted when the local is never written (a parameter, seeded
                // from the signature) or written exactly once. In the
                // single-write case the width is whatever that write recorded;
                // a read that textually PRECEDES the write finds no recorded
                // width and stays unknown, which is the conservative direction.
                let idx = *n as usize;
                let w = match local_write_count.get(&idx).copied().unwrap_or(0) {
                    0 | 1 => st.local_widths.get(idx).copied().unwrap_or(Width::Unknown),
                    _ => Width::Unknown,
                };
                st.push_w(v, w);
            }
            Op::SetLocal(n) => {
                // Read the width BEFORE popping; `pop` lowers the depth and
                // `width_at` is relative to it.
                let w = st.width_at(0);
                let v = st.pop();
                st.b.build_store(st.locals[*n as usize], v).unwrap();
                let idx = *n as usize;
                if local_write_count.get(&idx).copied().unwrap_or(0) == 1 {
                    if st.local_widths.len() <= idx {
                        st.local_widths.resize(idx + 1, Width::Unknown);
                    }
                    st.local_widths[idx] = w;
                }
            }
            Op::PopN(n) => {
                st.depth -= *n as usize;
            }
            // The integer arithmetic surface. `Op::Add`, `Op::Sub`, `Op::Mul`
            // and `Op::Neg` are NOT part of it: consolidation B narrowed those
            // four away from `Int` operands, so the compiler emits
            // `Checked*; PopN(2)` for every `Word` expression and reserves the
            // unchecked opcodes for `Byte`, `Fixed` and `Float`. Verified
            // against `src/compiler.rs` and the VM's dispatch arms, which raise
            // a type error on an `Int` reaching `Op::Add`.
            Op::CheckedAdd | Op::CheckedSub => {
                let rhs = st.pop();
                let lhs = st.pop();
                let a = st.widen(lhs, i128t, "a128");
                let c = st.widen(rhs, i128t, "b128");
                let wide = match op {
                    Op::CheckedAdd => st.b.build_int_add(a, c, "s128").unwrap(),
                    _ => st.b.build_int_sub(a, c, "d128").unwrap(),
                };
                st.push_checked_triple(ctx, func, trap_bb, opts, wide);
            }
            // Both halves of the product are load-bearing for big-number
            // multiplication, so the 128-bit product is computed in full rather
            // than reduced to a 64-bit multiply with an overflow predicate.
            //
            // The `u8` operand is the Q-format fraction-bit count. The compiler
            // emits `CheckedMul(0)` for `Word * Word`, and zero fraction bits is
            // exactly integer multiply. A non-zero count is fixed-point, whose
            // VM arm shifts the product and classifies through a different
            // helper; it is refused here rather than lowered as if the operand
            // were absent.
            Op::CheckedMul(0) => {
                let rhs = st.pop();
                let lhs = st.pop();
                let a = st.widen(lhs, i128t, "a128");
                let c = st.widen(rhs, i128t, "b128");
                let wide = st.b.build_int_mul(a, c, "p128").unwrap();
                st.push_checked_triple(ctx, func, trap_bb, opts, wide);
            }
            // Negation in 128 bits rather than 64 is what makes `-i64::MIN`
            // observable: at 64 bits it wraps to itself and the overflow flag
            // would be unrecoverable.
            Op::CheckedNeg => {
                let v = st.pop();
                let a = st.widen(v, i128t, "a128");
                let wide = st.b.build_int_neg(a, "n128").unwrap();
                st.push_checked_triple(ctx, func, trap_bb, opts, wide);
            }
            // The division family. Two undefined behaviours in LLVM have to be
            // excluded before an `sdiv` or `srem` is emitted at all: a zero
            // divisor, and `i64::MIN / -1`. The VM treats them DIFFERENTLY from
            // each other and differently between the checked and unchecked
            // forms, so each is handled on the VM's terms rather than by one
            // blanket trap.
            //
            // | | zero divisor | `i64::MIN` by `-1` |
            // |---|---|---|
            // | `Div` | `VmError::DivisionByZero` | `i64::MIN`, no fault |
            // | `Mod` | `VmError::DivisionByZero` | `0`, no fault |
            // | `CheckedDiv(0)` | flag `3`, numerator in low | flag `1`, low `i64::MIN` |
            // | `CheckedMod` | flag `3`, numerator in low | flag `0`, low `0` |
            //
            // Read out of `src/vm.rs` and then CONFIRMED BY EXECUTION. The
            // inventory recorded that `i64::MIN / -1` traps like a zero divisor.
            // It does not; it wraps.
            Op::Div | Op::Mod => {
                let rhs = st.pop();
                let lhs = st.pop();

                // A zero divisor faults. This is the one case in the family that
                // reaches the trap block, and it must come first: everything
                // after it may assume a non-zero divisor.
                let cont = ctx.append_basic_block(func, "nonzerodivisor");
                let zero =
                    st.b.build_int_compare(IntPredicate::EQ, rhs, i64t.const_zero(), "divzero")
                        .unwrap();
                st.b.build_conditional_branch(zero, trap_bb, cont).unwrap();
                st.b.position_at_end(cont);

                // Substituting a divisor of 1 for the `i64::MIN / -1` case is
                // not an approximation, it is exact for BOTH opcodes:
                // `sdiv(i64::MIN, 1)` is `i64::MIN`, which is the wrapped
                // quotient the VM returns, and `srem(i64::MIN, 1)` is `0`, which
                // is the remainder it returns. No corrective select is needed
                // afterwards, and none is emitted. The substitution is inert for
                // every other input because the predicate names exactly one pair.
                let safe = st.guard_min_div_neg_one(lhs, rhs, i64t);
                let v = match op {
                    Op::Div => st.b.build_int_signed_div(lhs, safe, "sdiv").unwrap(),
                    _ => st.b.build_int_signed_rem(lhs, safe, "srem").unwrap(),
                };
                st.push(v);
            }
            // The checked forms do NOT fault on a zero divisor. They reify it as
            // flag 3 with the numerator in the low slot, so a handled
            // `zero_divisor(n)` arm can bind it; only an UNHANDLED zero divisor
            // traps, and that trap is emitted by the compiler as an ordinary
            // `Op::Trap` in the flag dispatch rather than by this opcode.
            //
            // Because there is no fault, the whole thing is branch-free: the
            // divisor is forced to a safe non-zero value and the triple is then
            // overridden by selects. Branch-free matters beyond tidiness here --
            // a new basic block would have to be reconciled with the per-block
            // operand-stack depth bookkeeping, and selects sidestep that.
            Op::CheckedDiv(0) | Op::CheckedMod => {
                let rhs = st.pop();
                let lhs = st.pop();

                let iszero =
                    st.b.build_int_compare(IntPredicate::EQ, rhs, i64t.const_zero(), "divzero")
                        .unwrap();
                let nonzero =
                    st.b.build_select(iszero, i64t.const_int(1, false), rhs, "nzdiv")
                        .unwrap()
                        .into_int_value();

                // In 128 bits `i64::MIN / -1` is 2^63, which is representable,
                // so unlike the unchecked forms no divisor substitution is
                // needed for it -- the wide division is simply not undefined.
                // That is exactly how the VM gets flag 1 with low `i64::MIN`.
                let a = st.widen(lhs, i128t, "a128");
                let c = st.widen(nonzero, i128t, "b128");
                let wide = match op {
                    Op::CheckedDiv(_) => st.b.build_int_signed_div(a, c, "q128").unwrap(),
                    _ => st.b.build_int_signed_rem(a, c, "r128").unwrap(),
                };

                let (low, high, flag) = st.checked_triple(ctx, wide);
                let low =
                    st.b.build_select(iszero, lhs, low, "zdlow")
                        .unwrap()
                        .into_int_value();
                let high =
                    st.b.build_select(iszero, i64t.const_zero(), high, "zdhigh")
                        .unwrap()
                        .into_int_value();
                let flag =
                    st.b.build_select(iszero, i64t.const_int(3, false), flag, "zdflag")
                        .unwrap()
                        .into_int_value();
                st.push_triple(ctx, func, trap_bb, opts, (low, high, flag));
            }
            // Comparisons. The VM's `compare_op` pops the right operand first,
            // then the left, and compares left against right; the order below
            // matches it. The result is 0 or 1 in an i64, which is the flat
            // representation of the VM's tagged `Bool`.
            Op::CmpEq | Op::CmpNe | Op::CmpLt | Op::CmpGt | Op::CmpLe | Op::CmpGe => {
                let pred = match op {
                    Op::CmpEq => IntPredicate::EQ,
                    Op::CmpNe => IntPredicate::NE,
                    Op::CmpLt => IntPredicate::SLT,
                    Op::CmpGt => IntPredicate::SGT,
                    Op::CmpLe => IntPredicate::SLE,
                    Op::CmpGe => IntPredicate::SGE,
                    _ => unreachable!("the outer match restricts this set"),
                };
                let rhs = st.pop();
                let lhs = st.pop();
                let c = st.b.build_int_compare(pred, lhs, rhs, "cmp").unwrap();
                let v = st.b.build_int_z_extend(c, i64t, "cmpz").unwrap();
                st.push(v);
            }
            // Logical NOT. The VM applies it only to `Bool` and raises a type
            // error otherwise, so the operand here is always 0 or 1 and the
            // comparison against zero is exact rather than merely truthy.
            Op::Not => {
                let v = st.pop();
                let c =
                    st.b.build_int_compare(IntPredicate::EQ, v, i64t.const_zero(), "not")
                        .unwrap();
                let z = st.b.build_int_z_extend(c, i64t, "notz").unwrap();
                st.push(z);
            }
            Op::BitAnd | Op::BitOr | Op::BitXor => {
                let rhs = st.pop();
                let lhs = st.pop();
                let v = match op {
                    Op::BitAnd => st.b.build_and(lhs, rhs, "band").unwrap(),
                    Op::BitOr => st.b.build_or(lhs, rhs, "bor").unwrap(),
                    Op::BitXor => st.b.build_xor(lhs, rhs, "bxor").unwrap(),
                    _ => unreachable!("the outer match restricts this set"),
                };
                st.push(v);
            }
            // Shifts. THE MASK IS NOT OPTIONAL. The VM masks the count to the
            // word width, `count & (word_bits - 1)`, so every count is defined.
            // An LLVM shift by at least the bit width yields poison, so
            // omitting the mask would be undefined behaviour on exactly the
            // inputs the VM gives a defined answer for.
            Op::Shl | Op::Shr => {
                let count = st.pop();
                let value = st.pop();
                let masked =
                    st.b.build_and(count, i64t.const_int(WORD_BITS - 1, false), "shmask")
                        .unwrap();
                let v = match op {
                    Op::Shl => st.b.build_left_shift(value, masked, "shl").unwrap(),
                    // Documented as an ARITHMETIC, sign-preserving right
                    // shift, so `ashr` rather than `lshr`.
                    Op::Shr => st.b.build_right_shift(value, masked, true, "shr").unwrap(),
                    _ => unreachable!("the outer match restricts this set"),
                };
                st.push(v);
            }
            Op::Dup => {
                let v = st.pop();
                st.push(v);
                st.push(v);
            }
            // Scalar constants only. The compiler routes most literals here
            // rather than through `PushImmediate`, including the bounds and
            // stride of a range `for`, so this is required for iteration.
            // Composite and string constants are Workstream C and are refused.
            Op::Const(idx) => {
                let cv = chunk.constants.get(*idx as usize).ok_or_else(|| {
                    LowerError::UnsupportedOp(format!("Const({idx}) out of range"))
                })?;
                // The constant's own variant states its packed width exactly,
                // which makes this the least ambiguous producer in the set.
                // `Unit` stays unknown: it is a placeholder that nothing reads,
                // and giving it a width would let it be packed into a body.
                // A static string is a REFERENCE, not a packable scalar, so it
                // cannot be produced as an `i64` literal like the others.
                if let ConstValue::StaticStr(s) = cv {
                    let g = static_string_global(ctx, module, s);
                    let addr = st.b.build_ptr_to_int(g, i64t, "kstr").unwrap();
                    // Unknown width, deliberately. The operand is an address,
                    // and packing an address into a composite body as though it
                    // were a scalar is exactly the mistake `Width::Unknown`
                    // exists to make impossible.
                    st.push_w(addr, Width::Unknown);
                } else {
                    let (v, w): (i64, Width) = match cv {
                        ConstValue::Int(i) => (*i, Width::Scalar(8)),
                        ConstValue::Byte(x) => (*x as i64, Width::Scalar(1)),
                        ConstValue::Bool(b) => (*b as i64, Width::Scalar(1)),
                        // Unit is pushed and popped without being read. Same
                        // placeholder, same caveat, as `PushImmediate(0)`.
                        ConstValue::Unit => (0, Width::Unknown),
                        other => {
                            return Err(LowerError::UnsupportedOp(format!(
                                "Const holding {other:?}"
                            )));
                        }
                    };
                    let c = i64t.const_int(v as u64, true);
                    st.push_w(c, w);
                }
            }
            // Encoding per `Op::PushImmediate`: 0 = Unit, 1 = true, 2 = false,
            // 3 = None, 4..=19 = Int(operand - 4).
            Op::PushImmediate(imm) => {
                let v: i64 = match *imm {
                    // Unit carries no value the lowering can read. It is pushed
                    // as a block's result and popped again, and every loop the
                    // compiler emits contains one. Zero is a placeholder, sound
                    // only because nothing consumes it; if Unit ever reaches a
                    // comparison, this is wrong.
                    0 => 0,
                    1 => 1,
                    2 => 0,
                    // `None` needs an Option representation this backend has not
                    // settled. Refusing beats inventing one silently.
                    3 => {
                        return Err(LowerError::UnsupportedOp("PushImmediate(None)".into()));
                    }
                    n @ 4..=19 => (n as i64) - 4,
                    other => {
                        return Err(LowerError::UnsupportedOp(format!(
                            "PushImmediate({other}) is reserved"
                        )));
                    }
                };
                let c = i64t.const_int(v as u64, true);
                st.push(c);
            }
            // `Loop` is a runtime no-op. Its operand is the exit index, carried
            // for `Break` and `BreakIf`, and the block for it is created from
            // those rather than from here.
            Op::Loop(_) => {}
            // The back edge, and the whole of the backward-jump problem. The
            // header's depth was established when it was first entered by
            // fall-through, so the back edge only has to AGREE with it, which is
            // exactly what `note!` asserts.
            Op::EndLoop(t) | Op::Break(t) => {
                note!(*t as usize, st.depth);
                st.b.build_unconditional_branch(blocks[&(*t as usize)])
                    .unwrap();
            }
            Op::BreakIf(t) => {
                let c = st.pop();
                let nz =
                    st.b.build_int_compare(IntPredicate::NE, c, i64t.const_zero(), "brknz")
                        .unwrap();
                let cont = ctx.append_basic_block(func, &format!("nobrk{i}"));
                note!(*t as usize, st.depth);
                st.b.build_conditional_branch(nz, blocks[&(*t as usize)], cont)
                    .unwrap();
                st.b.position_at_end(cont);
            }
            Op::If(t) => {
                let c = st.pop();
                let nz =
                    st.b.build_int_compare(IntPredicate::NE, c, i64t.const_zero(), "nz")
                        .unwrap();
                let then_bb = ctx.append_basic_block(func, &format!("then{i}"));
                note!(*t as usize, st.depth);
                st.b.build_conditional_branch(nz, then_bb, blocks[&(*t as usize)])
                    .unwrap();
                st.b.position_at_end(then_bb);
            }
            Op::Else(t) => {
                note!(*t as usize, st.depth);
                st.b.build_unconditional_branch(blocks[&(*t as usize)])
                    .unwrap();
            }
            Op::EndIf => {}
            Op::Trap(_) => {
                st.b.build_unconditional_branch(trap_bb).unwrap();
            }
            // A direct call to another chunk. Only reachable through
            // `lower_module`, which declares every chunk up front; `lower_chunk`
            // passes no callees and therefore refuses.
            //
            // The VM sets the callee's frame base to `stack.len() - arg_count`,
            // so the arguments sit in DECLARATION order with the last argument
            // on top. Popping yields them in reverse, hence the reversal below.
            // Getting this backwards is invisible for a one-argument callee and
            // for any callee whose arguments happen to be equal, which is most
            // of the obvious test cases.
            Op::Call(idx, arg_count) => {
                let callee = callees.get(*idx as usize).copied().ok_or_else(|| {
                    LowerError::UnsupportedOp(format!(
                        "Call({idx}, {arg_count}) needs the whole module; lower_module resolves \
                         it, lower_chunk cannot"
                    ))
                })?;

                // The VM tolerates an argument count below the callee's local
                // count by filling the remainder with Unit. That is a distinct
                // calling convention from a plain native call, so it is refused
                // rather than approximated: an arity mismatch here would produce
                // a call LLVM accepts and the VM does not agree with.
                let declared = callee.count_params() - trailing_ptrs(&data);
                if u32::from(*arg_count) != declared {
                    return Err(LowerError::UnsupportedOp(format!(
                        "Call({idx}, {arg_count}) passes {arg_count} arguments to a chunk \
                         declaring {declared} parameters; the VM's Unit-fill convention for a \
                         short call is not lowered"
                    )));
                }

                let mut args: Vec<_> = (0..*arg_count).map(|_| st.pop()).collect();
                args.reverse();
                let mut args: Vec<inkwell::values::BasicMetadataValueEnum> =
                    args.into_iter().map(|a| a.into()).collect();
                // Forward the shared buffer. Every function in a module with
                // shared slots takes the trailing pointer, so a call that
                // omitted it would pass garbage where the callee expects the
                // host's buffer.
                if let (Some(sb), Some(pb), Some(rb)) = (shared_base, private_base, region_base) {
                    args.push(sb.into());
                    args.push(pb.into());
                    args.push(rb.into());
                }

                let ret = match st
                    .b
                    .build_call(callee, &args, "call")
                    .unwrap()
                    .try_as_basic_value()
                {
                    ValueKind::Basic(v) => v.into_int_value(),
                    ValueKind::Instruction(_) => {
                        unreachable!("every lowered chunk returns an i64, never void")
                    }
                };
                st.push(ret);
            }
            // A call out to a host-registered native, as a direct call to an
            // external symbol.
            //
            // # The argument-count byte is NOT a count
            //
            // Its high bit is the B35 P7 error-reify flag, and a site carrying it
            // pushes TWO slots `(code, flag)` rather than one. Reading the byte
            // as a count would both call with 128 too many arguments and leave
            // the operand stack one slot short at every such site. Refused
            // rather than approximated; measured at 0 of 999 sites in the
            // `piano_roll` family, so refusing costs nothing today.
            //
            // # Why the result is pushed with an UNKNOWN width
            //
            // `Module::native_return_shapes` records `WireShape::Top` for a
            // native declared without a resolving `use ... -> R` signature, and
            // **zero of the 999 call sites in the corpus have anything else**. A
            // design that refused an unknown return shape would lower none of
            // them. `push` defaults to `Width::Unknown`, which fails closed at a
            // USE of the width rather than at the call, and that is sound here
            // for a reason the corpus shows directly: there are 1643 `PopN`
            // against those 999 calls, so the results are overwhelmingly
            // discarded, and a width that is never used is never needed.
            //
            // # Argument order
            //
            // `Vm::run` does `stack.drain(len - n..)`, so the native receives its
            // arguments in DECLARATION order with the last on top of the stack.
            // Popping yields them reversed, hence the reversal — the same trap
            // the `Op::Call` arm above documents, and just as invisible for a
            // one-argument native or one whose arguments happen to be equal.
            // Sixty-eight of the corpus sites take one argument.
            Op::CallVerifiedNative(idx, n) | Op::CallExternalNative(idx, n) => {
                if n & 0x80 != 0 {
                    return Err(LowerError::UnsupportedOp(format!(
                        "native call #{idx} sets the B35 P7 error-reify flag \
                         (argument byte {n:#04x}), which reifies a soft host \
                         failure as a two-slot (code, flag) result; the two-slot \
                         form is not lowered"
                    )));
                }
                let argc = usize::from(n & 0x7F);
                let name = natives.get(usize::from(*idx)).ok_or_else(|| {
                    LowerError::UnsupportedOp(format!(
                        "native call #{idx} indexes a native table of {} entries; \
                         lower_module resolves it, lower_chunk cannot",
                        natives.len()
                    ))
                })?;
                let sym = native_symbol(name);

                // Reuse an existing declaration, as `yield_hook` and
                // `trap_declaration` do, since a second chunk calling the same
                // native must not redeclare it. An arity disagreement between
                // two sites is refused: LLVM would accept the call and the host
                // would read a garbage argument.
                let f = match module.get_function(&sym) {
                    Some(f) => {
                        let declared = f.count_params() as usize;
                        if declared != argc {
                            return Err(LowerError::UnsupportedOp(format!(
                                "native `{name}` is called with {argc} arguments \
                                 here and {declared} at an earlier site; the \
                                 lowering refuses rather than emitting a call \
                                 whose arity disagrees with its declaration"
                            )));
                        }
                        f
                    }
                    None => {
                        let params: Vec<inkwell::types::BasicMetadataTypeEnum> =
                            (0..argc).map(|_| i64t.into()).collect();
                        module.add_function(&sym, i64t.fn_type(&params, false), None)
                    }
                };

                let mut args: Vec<_> = (0..argc).map(|_| st.pop()).collect();
                args.reverse();
                let args: Vec<inkwell::values::BasicMetadataValueEnum> =
                    args.into_iter().map(|a| a.into()).collect();

                let ret = match st
                    .b
                    .build_call(f, &args, "native")
                    .unwrap()
                    .try_as_basic_value()
                {
                    ValueKind::Basic(v) => v.into_int_value(),
                    ValueKind::Instruction(_) => {
                        unreachable!("every native is declared returning i64, never void")
                    }
                };
                st.push(ret);
            }
            // `Byte` occupies a full `i64` slot holding a value in `0..=255`.
            // **That invariant is what makes `ByteToWord` free**, and it is the
            // representation the reference implementation already implies: the
            // `v0.2.3` session measured that `Byte as Word` zero-extends, so
            // `0xFF` reads as `255` rather than `-1`.
            //
            // Every producer of a `Byte` must maintain the invariant. Today the
            // only one is `WordToByte` below, which masks. The unchecked
            // arithmetic opcodes would be the others, and they remain
            // unsupported for an unrelated reason recorded in the inventory:
            // `Op::Add` cannot be lowered without knowing whether its operands
            // are `Byte` or `Fixed`, and the opcode does not say.
            Op::WordToByte => {
                let v = st.pop();
                let m =
                    st.b.build_and(v, i64t.const_int(0xFF, false), "tobyte")
                        .unwrap();
                st.push_w(m, Width::Scalar(1));
            }
            // A no-op, and deliberately written as one rather than as a
            // redundant mask. If it ever needs to do work, the representation
            // invariant above has been broken somewhere else and masking here
            // would hide that rather than fix it.
            //
            // **It is a no-op in VALUE and not in WIDTH.** The bits are already
            // correct, but the operand now packs into eight bytes rather than
            // one. Leaving the old label would pack a `Word` into a single byte
            // and silently truncate it, with every later field offset still
            // looking right — so the relabel is the whole content of this arm.
            Op::ByteToWord => st.set_top_width(Width::Scalar(8)),
            // Peek-and-trap. **`BoundsCheck` does NOT pop**; the VM reads
            // `stack.last()` and leaves the operand in place for the indexing
            // opcode that follows. A lowering that consumed it would corrupt
            // every subsequent slot, and the differential oracle would show it
            // as a wrong value rather than as a stack error.
            //
            // One UNSIGNED compare covers both failure directions. The VM
            // rejects `value < 0 || value >= bound`; reinterpreting a negative
            // `i64` as unsigned makes it enormous, so `uge bound` catches the
            // negative case in the same instruction. `bound` is a `u16`, so it
            // can never itself be large enough to make that reinterpretation
            // ambiguous.
            Op::BoundsCheck(bound) => {
                let v = st.peek();
                let cont = ctx.append_basic_block(func, "inbounds");
                let bad =
                    st.b.build_int_compare(
                        IntPredicate::UGE,
                        v,
                        i64t.const_int(u64::from(*bound), false),
                        "oob",
                    )
                    .unwrap();
                st.b.build_conditional_branch(bad, trap_bb, cont).unwrap();
                st.b.position_at_end(cont);
            }
            // Data-segment access, shared and private.
            //
            // SHARED slots decode a host-owned byte buffer whose encoding is
            // part of the wire format: a layout table gives each slot an offset
            // and a kind tag, and scalars are stored little-endian there.
            //
            // PRIVATE slots use a flat word array of this backend's own
            // choosing, at `PRIVATE_SLOT_BYTES` per slot indexed from the
            // private boundary. That is legitimate because private storage is
            // unreachable from outside a running program, so no external
            // consumer can observe the layout, and the only quantity crossing
            // the boundary is the region size.
            //
            // A private ARRAY is not a composite here. The compiler expands
            // `h.a: [Word; 4]` into four consecutive scalar slots named
            // `h.a[0]` through `h.a[3]`, so an indexed access is `base + index`
            // in slot space and needs no stride table.
            Op::GetData(_)
            | Op::SetData(_)
            | Op::GetDataIndexed(_, _)
            | Op::SetDataIndexed(_, _) => {
                let (slot, indexed, bound) = match op {
                    Op::GetData(s) | Op::SetData(s) => (*s, false, 0u32),
                    Op::GetDataIndexed(b, n) | Op::SetDataIndexed(b, n) => (*b, true, *n),
                    _ => unreachable!("the outer match restricts this set"),
                };
                let is_read = matches!(op, Op::GetData(_) | Op::GetDataIndexed(_, _));
                if !data.has_data {
                    return Err(LowerError::UnsupportedDataSlot {
                        slot,
                        why: String::from(
                            "no data layout is visible; lower_chunk receives one chunk and cannot \
                             see the module's layout table",
                        ),
                    });
                }

                // An indexed access pops its index first, and for a write the
                // VM pops the index BEFORE the value, so the value is beneath
                // it on the stack.
                let index = if indexed { Some(st.pop()) } else { None };

                if let Some(ix) = index {
                    // Bounds check against the declared element count, with one
                    // unsigned compare covering the negative case, exactly as
                    // Op::BoundsCheck does.
                    let cont = ctx.append_basic_block(func, "inbounds_data");
                    let bad =
                        st.b.build_int_compare(
                            IntPredicate::UGE,
                            ix,
                            i64t.const_int(u64::from(bound), false),
                            "dataoob",
                        )
                        .unwrap();
                    st.b.build_conditional_branch(bad, trap_bb, cont).unwrap();
                    st.b.position_at_end(cont);
                }

                if slot < data.shared_count {
                    // Shared. An indexed shared access would need the layout
                    // entries for the whole range proven contiguous, which the
                    // table does not state, so it is refused rather than
                    // assumed.
                    let base = shared_base.expect("has_data implies the shared pointer");
                    // A direct access resolves one slot; an indexed access
                    // proves the whole range contiguous and uniform first, then
                    // computes `first + index * width`.
                    let (addr, width, kind) = if let Some(ix) = index {
                        let (first_off, w, k) = resolve_shared_array(&data, slot, bound)?;
                        let byte =
                            st.b.build_int_add(
                                i64t.const_int(u64::from(first_off), false),
                                st.b.build_int_mul(
                                    ix,
                                    i64t.const_int(u64::from(w), false),
                                    "sstride",
                                )
                                .unwrap(),
                                "soff",
                            )
                            .unwrap();
                        let a = unsafe {
                            st.b.build_in_bounds_gep(i8t, base, &[byte], "sdataptr")
                                .unwrap()
                        };
                        (a, if w == 8 { i64t } else { i8t }, k)
                    } else {
                        let (byte_off, w, k) = resolve_shared_scalar(&data, slot, i8t, i64t)?;
                        let a = unsafe {
                            st.b.build_in_bounds_gep(
                                i8t,
                                base,
                                &[i64t.const_int(u64::from(byte_off), false)],
                                "sdataptr",
                            )
                            .unwrap()
                        };
                        (a, w, k)
                    };
                    if is_read {
                        let raw =
                            st.b.build_load(width, addr, "sdataload")
                                .unwrap()
                                .into_int_value();
                        let v = if width == i64t {
                            raw
                        } else {
                            st.b.build_int_z_extend(raw, i64t, "sdatazext").unwrap()
                        };
                        let v = if kind == SCALAR_BOOL {
                            let nz =
                                st.b.build_int_compare(
                                    IntPredicate::NE,
                                    v,
                                    i64t.const_zero(),
                                    "sboolnz",
                                )
                                .unwrap();
                            st.b.build_int_z_extend(nz, i64t, "sboolz").unwrap()
                        } else {
                            v
                        };
                        st.push(v);
                    } else {
                        let v = st.pop();
                        let narrowed = if width == i64t {
                            v
                        } else {
                            st.b.build_int_truncate(v, width, "sdatatrunc").unwrap()
                        };
                        st.b.build_store(addr, narrowed).unwrap();
                    }
                } else {
                    // Private. Flat word array indexed from the boundary.
                    if slot >= data.slot_count {
                        return Err(LowerError::UnsupportedDataSlot {
                            slot,
                            why: String::from("slot index outside the declared slot table"),
                        });
                    }
                    let base = private_base.expect("has_data implies the private pointer");
                    let rel = slot - data.shared_count;
                    let byte_index =
                        st.b.build_int_mul(
                            match index {
                                Some(ix) => {
                                    st.b.build_int_add(
                                        ix,
                                        i64t.const_int(u64::from(rel), false),
                                        "pidx",
                                    )
                                    .unwrap()
                                }
                                None => i64t.const_int(u64::from(rel), false),
                            },
                            i64t.const_int(u64::from(PRIVATE_SLOT_BYTES), false),
                            "pbyte",
                        )
                        .unwrap();
                    let addr = unsafe {
                        st.b.build_in_bounds_gep(i8t, base, &[byte_index], "pdataptr")
                            .unwrap()
                    };
                    // **Alignment is part of this ABI, not an accident.** A
                    // slot is a whole word, so the region must be aligned to
                    // `PRIVATE_SLOT_BYTES` and every access is naturally
                    // aligned within it. The alignment is set explicitly
                    // because the default inkwell emits is narrower than the
                    // access, and a store whose declared alignment exceeds the
                    // pointer's true alignment is undefined behaviour that
                    // presents as a bus fault rather than a wrong answer. A
                    // caller passing a `Vec<u8>` base, whose alignment contract
                    // is one byte, violates this; the harness allocates a word
                    // slice for exactly that reason.
                    if is_read {
                        let load = st.b.build_load(i64t, addr, "pdataload").unwrap();
                        load.into_int_value()
                            .as_instruction()
                            .expect("a load is an instruction")
                            .set_alignment(PRIVATE_SLOT_BYTES)
                            .expect("PRIVATE_SLOT_BYTES is a power of two");
                        st.push(load.into_int_value());
                    } else {
                        let v = st.pop();
                        let store = st.b.build_store(addr, v).unwrap();
                        store
                            .set_alignment(PRIVATE_SLOT_BYTES)
                            .expect("PRIVATE_SLOT_BYTES is a power of two");
                    }
                }
            }
            // Flat composite construction. The body goes at a COMPILE-TIME
            // offset in the host-supplied region, so there is no allocator, no
            // cursor and nothing that can fail at run time. See `region.rs`.
            Op::NewComposite(keleusma::bytecode::NewCompositeOperand::Flat {
                count,
                byte_size,
                ..
            }) => {
                let region = region_base.ok_or_else(|| {
                    LowerError::UnsupportedOp(
                        "NewComposite needs the region pointer, which lower_chunk does not \
                         receive"
                            .into(),
                    )
                })?;
                let plan = crate::region::plan_chunk_region(chunk);
                let site = plan.sites.iter().find(|s| s.op_index == i).ok_or_else(|| {
                    LowerError::UnsupportedOp(format!(
                        "no region placement for the NewComposite at op {i}"
                    ))
                })?;

                // Widths, in OPERAND order. `width_at(0)` is the top of the
                // stack, which is the LAST operand, so the run is reversed.
                let n = *count as usize;
                let mut widths: Vec<Width> = Vec::with_capacity(n);
                for back in (0..n).rev() {
                    // An unknown width cannot be placed. Refusing is the whole
                    // point of the default: a guess here mispacks silently, and
                    // a Byte and a Word are indistinguishable on this stack.
                    let w = st.width_at(back);
                    if w.bytes().is_none() {
                        return Err(LowerError::UnsupportedOp(format!(
                            "NewComposite at op {i} has an operand of unknown packed width"
                        )));
                    }
                    widths.push(w);
                }

                // The reference packs fields cumulatively with NO padding, so
                // the widths must account for the baked body exactly. A
                // mismatch means this backend's model of the layout has drifted
                // from the compiler's, and emitting anyway would write a body
                // the VM reads differently.
                let total: u32 = widths.iter().filter_map(|w| w.bytes()).sum();
                if total != u32::from(*byte_size) {
                    return Err(LowerError::UnsupportedOp(format!(
                        "NewComposite at op {i} packs {total} bytes but the instruction bakes \
                         {byte_size}; the layout model has drifted"
                    )));
                }

                let vals: Vec<IntValue<'ctx>> = {
                    let mut v: Vec<_> = (0..n).map(|_| st.pop()).collect();
                    v.reverse();
                    v
                };

                let mut off = site.offset;
                for (v, w) in vals.iter().zip(widths.iter()) {
                    let addr = unsafe {
                        st.b.build_in_bounds_gep(
                            i8t,
                            region,
                            &[i64t.const_int(u64::from(off), false)],
                            "cfieldptr",
                        )
                        .unwrap()
                    };
                    // Truncate to the field's width, then store UNALIGNED. The
                    // layout is chosen for density: a Word can sit at offset 1.
                    // Declaring an alignment the pointer does not have is
                    // undefined behaviour that presents as a bus fault.
                    let n_bytes = w.bytes().expect("checked above");
                    if w.is_body() {
                        // A NESTED BODY: the operand holds the ADDRESS of
                        // `n_bytes` of body, so placing it is a COPY. Storing it
                        // as a scalar would write the pointer into the parent
                        // while every downstream offset still looked correct.
                        //
                        // Both sides are alignment 1. The layout packs
                        // cumulatively, so neither the source body nor this
                        // field is guaranteed anything better, and declaring an
                        // alignment the pointer lacks is undefined behaviour.
                        //
                        // **This was written, deleted as unverifiable, and
                        // restored.** The claim was wrong: it was unobservable
                        // only through the VM differential. The test owns the
                        // region buffer, so it can read the copied bytes back
                        // directly, and a nested field's neighbour is readable
                        // through the already-supported flat path. "The
                        // differential cannot see X" was a fact about that
                        // differential, not about X.
                        let src = st
                            .b
                            .build_int_to_ptr(*v, ctx.ptr_type(AddressSpace::default()), "cnestsrc")
                            .unwrap();
                        st.b.build_memcpy(
                            addr,
                            1,
                            src,
                            1,
                            i64t.const_int(u64::from(n_bytes), false),
                        )
                        .map_err(|e| {
                            LowerError::UnsupportedOp(format!(
                                "NewComposite at op {i} could not copy a {n_bytes}-byte nested \
                                 body: {e}"
                            ))
                        })?;
                        off += n_bytes;
                        continue;
                    }
                    let store = match n_bytes {
                        8 => st.b.build_store(addr, *v).unwrap(),
                        1 => {
                            let t = st.b.build_int_truncate(*v, i8t, "cfbyte").unwrap();
                            st.b.build_store(addr, t).unwrap()
                        }
                        other => {
                            return Err(LowerError::UnsupportedOp(format!(
                                "NewComposite at op {i} has a {other}-byte scalar field; \
                                 only 1 and 8 are lowered"
                            )));
                        }
                    };
                    store.set_alignment(1).expect("1 is a power of two");
                    off += n_bytes;
                }

                // The composite operand IS its address. Its packed width is the
                // body length, so a nested construction places it correctly.
                let base = unsafe {
                    st.b.build_in_bounds_gep(
                        i8t,
                        region,
                        &[i64t.const_int(u64::from(site.offset), false)],
                        "cbody",
                    )
                    .unwrap()
                };
                let as_int = st.b.build_ptr_to_int(base, i64t, "cbodyint").unwrap();
                // A construction pushes the body's ADDRESS, so it is a `Body`:
                // placing it inside another body copies from that address.
                st.push_w(as_int, Width::Body(u32::from(*byte_size)));
            }
            // Flat array element read. The same address-plus-typed-load as a
            // field, with a RUNTIME index and a compile-time element size.
            //
            // The bound is not checked here: the compiler emits `Op::BoundsCheck`
            // before the index, which this backend already lowers and which peeks
            // rather than pops for exactly this reason.
            Op::GetIndex(keleusma::bytecode::ArrayElem::Flat { kind }) => {
                use keleusma::value_layout::ScalarKind as SK;
                let elem: u64 = match kind {
                    SK::Int => 8,
                    SK::Byte | SK::Bool => 1,
                    other => {
                        return Err(LowerError::UnsupportedOp(format!(
                            "GetIndex reading {other:?} is not lowered"
                        )));
                    }
                };
                // The index is popped BEFORE the array, matching the virtual
                // machine's order; reversing them reads the array handle as an
                // index and indexes by a pointer.
                let index = st.pop();
                let addr_int = st.pop();
                let base = st
                    .b
                    .build_int_to_ptr(addr_int, ctx.ptr_type(AddressSpace::default()), "cibase")
                    .unwrap();
                let byte =
                    st.b.build_int_mul(index, i64t.const_int(elem, false), "cistride")
                        .unwrap();
                let addr = unsafe {
                    st.b.build_in_bounds_gep(i8t, base, &[byte], "ciaddr")
                        .unwrap()
                };
                let (v, w) = if elem == 8 {
                    let iv =
                        st.b.build_load(i64t, addr, "ciint")
                            .unwrap()
                            .into_int_value();
                    iv.as_instruction()
                        .expect("a load is an instruction")
                        .set_alignment(1)
                        .expect("1 is a power of two");
                    (iv, 8u32)
                } else {
                    let bv =
                        st.b.build_load(i8t, addr, "cibyte")
                            .unwrap()
                            .into_int_value();
                    bv.as_instruction()
                        .expect("a load is an instruction")
                        .set_alignment(1)
                        .expect("1 is a power of two");
                    // Zero, not sign: a `Byte` holds `0..=255` in a full slot.
                    (st.b.build_int_z_extend(bv, i64t, "cizext").unwrap(), 1u32)
                };
                st.push_w(v, Width::Scalar(w));
            }
            // Enum discriminant test. **PEEKS, like `BoundsCheck`**: the virtual
            // machine reads `stack.last()` and leaves the enum in place for the
            // field read that follows, so popping here would consume a value the
            // next opcode still needs.
            //
            // A flat enum's discriminant is the first packed value, so it is the
            // word at offset ZERO of the body. Only the flat representation is
            // lowered: the boxed form compares TYPE AND VARIANT NAMES, which
            // needs string data this backend has no representation for. The
            // `v0.2.3` line established that the boxed construction path is
            // unreachable at a 64-bit word, so refusing it costs nothing real —
            // but it is refused rather than assumed away.
            Op::IsEnum(_enum_const, _var_const, disc_const) => {
                let expected = match chunk.constants.get(*disc_const as usize) {
                    Some(ConstValue::Int(v)) => *v,
                    other => {
                        return Err(LowerError::UnsupportedOp(format!(
                            "IsEnum discriminant constant is {other:?}, not an Int"
                        )));
                    }
                };
                let addr_int = st.peek();
                let base =
                    st.b.build_int_to_ptr(addr_int, ctx.ptr_type(AddressSpace::default()), "cenum")
                        .unwrap();
                // Unaligned, like every other body access: the layout packs
                // cumulatively and guarantees nothing about the body's address.
                let disc =
                    st.b.build_load(i64t, base, "cdisc")
                        .unwrap()
                        .into_int_value();
                disc.as_instruction()
                    .expect("a load is an instruction")
                    .set_alignment(1)
                    .expect("1 is a power of two");
                let eq =
                    st.b.build_int_compare(
                        IntPredicate::EQ,
                        disc,
                        i64t.const_int(expected as u64, true),
                        "cisenum",
                    )
                    .unwrap();
                let z = st.b.build_int_z_extend(eq, i64t, "cisenumz").unwrap();
                // A boolean packs into one byte, like every other `Bool`.
                st.push_w(z, Width::Scalar(1));
            }
            // Nested composite read: a sub-range of the parent, so it is
            // POINTER ARITHMETIC and nothing else.
            //
            // A composite operand is an address and a nested body is contiguous
            // inside its parent, so re-wrapping is just `parent + offset` with
            // the child's length. No copy, no load, and `variant` does not reach
            // the machine: it says how to interpret bytes, which only the reader
            // of a scalar field needs.
            Op::GetField(keleusma::bytecode::StructField::FlatNested { offset, size, .. }) => {
                let parent = st.pop();
                let addr =
                    st.b.build_int_add(
                        parent,
                        i64t.const_int(u64::from(*offset), false),
                        "cnestoff",
                    )
                    .unwrap();
                st.push_w(addr, Width::Body(u32::from(*size)));
            }
            // The same, indexed: the element offset is `index * size`.
            Op::GetIndex(keleusma::bytecode::ArrayElem::FlatNested { size, .. }) => {
                let index = st.pop();
                let parent = st.pop();
                let byte =
                    st.b.build_int_mul(index, i64t.const_int(u64::from(*size), false), "cnstride")
                        .unwrap();
                let addr = st.b.build_int_add(parent, byte, "cnestidx").unwrap();
                st.push_w(addr, Width::Body(u32::from(*size)));
            }
            // Flat field read: a constant offset from the body address and one
            // unaligned typed load, which is what `GetData` already does.
            Op::GetField(keleusma::bytecode::StructField::Flat { offset, kind }) => {
                use keleusma::value_layout::ScalarKind as SK;
                let addr_int = st.pop();
                let base = st
                    .b
                    .build_int_to_ptr(addr_int, ctx.ptr_type(AddressSpace::default()), "cfbase")
                    .unwrap();
                let addr = unsafe {
                    st.b.build_in_bounds_gep(
                        i8t,
                        base,
                        &[i64t.const_int(u64::from(*offset), false)],
                        "cfaddr",
                    )
                    .unwrap()
                };
                let (v, w) = match kind {
                    SK::Int => {
                        let iv =
                            st.b.build_load(i64t, addr, "cfint")
                                .unwrap()
                                .into_int_value();
                        iv.as_instruction()
                            .expect("a load is an instruction")
                            .set_alignment(1)
                            .expect("1 is a power of two");
                        (iv, 8u32)
                    }
                    // A `Byte` occupies a full `i64` slot holding `0..=255`, so
                    // the extension is ZERO and not sign. Sign-extending would
                    // read `0xFF` as `-1` and break the invariant that makes
                    // `ByteToWord` free.
                    SK::Byte | SK::Bool => {
                        let bv =
                            st.b.build_load(i8t, addr, "cfbyte")
                                .unwrap()
                                .into_int_value();
                        bv.as_instruction()
                            .expect("a load is an instruction")
                            .set_alignment(1)
                            .expect("1 is a power of two");
                        let z = st.b.build_int_z_extend(bv, i64t, "cfzext").unwrap();
                        (z, 1u32)
                    }
                    other => {
                        return Err(LowerError::UnsupportedOp(format!(
                            "GetField reading {other:?} is not lowered"
                        )));
                    }
                };
                st.push_w(v, Width::Scalar(w));
            }
            // Suspension. **`Yield` is pop-one, push-one**: it pops the value
            // to yield and pushes the value the host resumes with. Treating it
            // as pop-only underflows the very next instruction, which is how the
            // opcode's shape was originally established.
            // In a DEGENERATE stream chunk this `Yield` is the return, not a
            // suspension. The value it would have pushed is the resume value,
            // which the following `PopN(1)` discards, and the following `Reset`
            // clears state that a fresh native frame does not carry. Both become
            // unreachable and the loop's own `dead` tracking skips them.
            Op::Yield if degenerate_yield.is_some_and(|ys| ys.contains(&i)) => {
                let v = st.pop();
                st.b.build_return(Some(&v)).unwrap();
            }
            // The envelope of a degenerate stream. `Stream` marks the point
            // `Reset` rewinds to, and with an empty prologue that is the entry
            // block, so it lowers to nothing. `Reset` is unreachable after the
            // return above and is accepted rather than refused so that the
            // `dead` path does not have to special-case it.
            Op::Stream | Op::Reset if degenerate_yield.is_some() => {}
            Op::Yield => {
                let v = st.pop();
                let hook = yield_hook(ctx, module);
                let resumed = match st
                    .b
                    .build_call(hook, &[v.into()], "yield")
                    .unwrap()
                    .try_as_basic_value()
                {
                    ValueKind::Basic(b) => b.into_int_value(),
                    ValueKind::Instruction(_) => {
                        unreachable!("kel_yield returns an i64, never void")
                    }
                };
                st.push(resumed);
            }
            Op::Return => {
                let v = st.pop();
                st.b.build_return(Some(&v)).unwrap();
            }
            other => return Err(LowerError::UnsupportedOp(format!("{other:?}"))),
        }
    }

    // A chunk whose ops end without `Op::Return`. `verify()` ADMITS this: both
    // `verify_stack_depth` and `check_chunk_seeded` compute the region's
    // terminal depth and then discard it with `.map(|_| ())`. The VM defines the
    // case as returning `Unit` (`src/vm.rs:4801`), and without a terminator here
    // the function is malformed IR rather than merely wrong -- which nothing
    // caught, because `lower_module` never verified its own output either (see
    // the `verify` call added below).
    //
    // Zero is this backend's `Unit`. `st.depth > 0` is guarded because `pop`
    // decrements a `usize` unconditionally and would underflow on an empty
    // stack, matching the VM's `stack.pop().unwrap_or(Unit)`.
    //
    // DELIBERATELY NOT bug-compatible: the VM additionally leaves the callee's
    // whole frame on the shared operand stack in this path, unlike `Op::Return`
    // which truncates to `old_frame.base`. That leak is a worst-case-memory
    // under-count reported to the runtime owner. Reproducing a defect for
    // fidelity would be the wrong call; revisit if the VM side is fixed.
    if !dead && st.b.get_insert_block().unwrap().get_terminator().is_none() {
        let v = if st.depth > 0 {
            st.pop()
        } else {
            i64t.const_zero()
        };
        st.b.build_return(Some(&v)).unwrap();
    }

    // Checked once at the end rather than at each push, because the slot array
    // is a provisioning decision and the useful diagnostic is the depth the
    // chunk actually needed.
    if let Some(needed) = st.stack_overflow {
        return Err(LowerError::OperandStackTooDeep {
            needed: needed + 1,
            provisioned: MAX_STACK,
        });
    }

    Ok(func)
}

/// Check that the build host is little-endian, which shared-slot access
/// requires.
///
/// The runtime decodes a shared slot with an explicit little-endian reader. An
/// LLVM `load` uses TARGET endianness. The two agree only on a little-endian
/// target, and every target on the committed list is one, so this refuses
/// rather than silently byte-swapping the host's buffer.
///
/// **The check is on the build host, not on the emitted target**, because the
/// lowering is not given a target triple. That is sufficient for the just-in-time
/// path and for ahead-of-time emission to the host, and it is NOT sufficient for
/// cross-compilation to a big-endian target. Making it sufficient means moving
/// the check onto the `TargetMachine`, which is a change to the entry points
/// rather than to this function.
pub fn check_target_endianness() -> Result<(), LowerError> {
    if cfg!(target_endian = "little") {
        Ok(())
    } else {
        Err(LowerError::UnsupportedDataSlot {
            slot: 0,
            why: String::from(
                "shared slots are stored little-endian and this host is big-endian; an LLVM load \
                 would byte-swap them",
            ),
        })
    }
}

/// Check that a module's declared word width is one this backend lowers.
///
/// Only 64-bit words (`word_bits_log2 == 6`) are supported. A narrower width
/// would need every arithmetic result masked to the declared width, which is a
/// later increment; refusing is correct until then.
pub fn check_word_width(word_bits_log2: u8) -> Result<(), LowerError> {
    if word_bits_log2 == 6 {
        Ok(())
    } else {
        Err(LowerError::UnsupportedWordWidth(word_bits_log2))
    }
}

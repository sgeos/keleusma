//! Lowering of verified Keleusma bytecode to LLVM IR (V0.3.x Workstream A).
//!
//! # Scope
//!
//! This is a subset of the instruction set: **60 of 66 opcodes lower**, 2 are
//! named refusals and 3 are unproven (re-derived 2026-08-27; the subset was
//! **46** when this note was written). Everything outside it is refused rather
//! than lowered to something plausible and wrong.
//!
//! **Re-derive rather than trusting the figure**, since it moves as the subset
//! widens: `cd native_codegen && cargo test --test isa_lowering_census -- --nocapture`.
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

/// Whether an operand is an integer or a floating-point value.
///
/// **Separate from [`Width`], and deliberately not folded into it.** A `Float`
/// and a `Word` are both eight bytes, so a width alone cannot tell them apart —
/// `width_of_declared_shape` collapses `WireShape::Scalar { kind }` to a size and
/// discards exactly this. Every float operation therefore needs a channel width
/// cannot provide, and overloading `Width::Scalar` would reinstate the collapse
/// being repaired.
///
/// **`Unknown` is a real answer, never a default.** A bitcast is only correct
/// when the tag is right; an operation on an operand of unknown kind refuses
/// rather than guessing, which is the discipline the width model already
/// applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperandKind {
    /// An integer, which is every operand this backend produced before floats.
    Int,
    /// A floating-point value, held on the operand stack **bitcast to `i64`** so
    /// the stack stays homogeneous. The alternative — a stack of value enums —
    /// touches 46 pop sites for no gain the optimiser does not already give.
    Float,
    /// Not determined here. Fails a float-sensitive operation closed.
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
        // **`Fixed` IS EIGHT BYTES, AND THAT IS MEASURED RATHER THAN ASSUMED.**
        // The reference packs `struct { a: Fixed<16>, b: Fixed<16> }` at
        // `byte_size: 16`, identical to a pair of `Word`s. A Q-format value is
        // an `i64` of fixed-point bits, so it occupies a full slot.
        //
        // It was `Unknown` until 2026-08-21, and NOT for a stated reason: it sat
        // inside an assertion whose doc comment justified only `Composite`. This
        // line declined to widen it on the ground that doing so "would newly
        // admit composites carrying `Fixed` fields, a packing change with its
        // own risk" -- a risk that was asserted and never measured. The risk of
        // widening is the risk of GUESSING a width, and there is nothing here to
        // guess.
        TypeTag::Fixed => Width::Scalar(8),
        TypeTag::Word => Width::Scalar(8),
        // `Composite` stays unknown because a body length is genuinely not
        // carried on the tag. `Float` stays unknown because this backend has no
        // float representation at all -- redundant with the module-level guard
        // that refuses a float by every route, and kept as a second line.
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
    /// Admit a stream whose suspension is DELEGATED to a tail-position callee.
    ///
    /// **Off by default, and that default is the decision, not an oversight.**
    /// The transform is verified by execution on a synthetic module of the
    /// qualifying shape (`delegated_suspension.rs`). The one module in the
    /// shipped corpus that qualifies is `codegen.kel`, and it **cannot be
    /// execution-differentiated by this subproject**: its input is an
    /// abstract-syntax-tree block whose 78 slot constants and two seeding
    /// helpers are private to `src/selfhost/mod.rs`, a file this line may read
    /// but must not edit. Admitting it by default would rest on `lower_module`
    /// returning `Ok`, which is a fact about the compiler and not about the
    /// program.
    ///
    /// Turning this on is therefore an explicit statement that unexecuted
    /// lowering is acceptable for the module in hand. See
    /// `docs/decisions/NATIVE_DELEGATED_SUSPENSION.md`.
    pub delegated_suspension: bool,
}

/// Packed width a producing instruction fixes by ITSELF, independent of its
/// operands, or `None`.
///
/// **Only operand-independent sources qualify, and that is the whole safety
/// argument.** A `Const` carries its kind, and the four arithmetic instructions
/// that push a `(low, high, flag)` triple push `low` at a literal word width
/// whatever they were given. An instruction whose result width depends on what
/// it consumed cannot be classified here, because doing so would need the very
/// analysis this pre-pass runs ahead of.
///
/// `which` selects among a multi-push instruction's results. **A triple's `low`
/// is the arithmetic result; its `flag` is a boolean.** Classifying on the
/// instruction alone would label a flag as a word.
fn instruction_fixed_width(chunk: &Chunk, op_index: usize, which: u32) -> Option<Width> {
    match chunk.ops.get(op_index)? {
        Op::Const(c) if which == 0 => match chunk.constants.get(*c as usize)? {
            keleusma::bytecode::ConstValue::Int(_) => Some(Width::Scalar(8)),
            keleusma::bytecode::ConstValue::Byte(_) => Some(Width::Scalar(1)),
            keleusma::bytecode::ConstValue::Bool(_) => Some(Width::Scalar(1)),
            _ => None,
        },
        // `push_triple` pushes `low` at `Width::Scalar(8)` as a literal.
        Op::CheckedAdd | Op::CheckedSub | Op::CheckedMul(_) | Op::CheckedNeg if which == 0 => {
            Some(Width::Scalar(8))
        }
        _ => None,
    }
}

/// Packed widths that can be trusted for locals the chunk writes MORE THAN ONCE.
///
/// # Why a multi-write local is normally untrusted, and why that is right
///
/// The width pass is a linear walk and **cannot see a back edge**, so a local
/// rewritten inside a loop would be read at the width of whichever write appears
/// earlier in the text and packed wrongly on every iteration after the first.
/// Declining to trust it costs coverage and cannot mispack, which is the correct
/// direction for a decision that is otherwise silent.
///
/// # What narrows it
///
/// **"Cannot see a back edge" only matters when the writes DISAGREE.** If every
/// write to a local stores a value of the same width, that is the width whichever
/// write reached the read, back edge or not. This certifies exactly that case,
/// and only from sources whose width is fixed by the instruction rather than by
/// its operands — so no circularity arises and no fixpoint is needed.
///
/// **A `for` loop's induction variable is the motivating case**: it is written
/// twice, once from a constant and once from an arithmetic result, and both are
/// words. Measured on `12_sensor_window.kel` and `14_frame_log.kel`.
///
/// # One unclassifiable write sinks the local
///
/// A write this cannot classify yields no certification at all, rather than
/// being ignored as though the remaining writes agreed.
fn certified_local_widths(chunk: &Chunk) -> BTreeMap<usize, Width> {
    // `(op index, which push)` for every live operand-stack slot.
    //
    // **Driven by `op_depth_effect`, whose contract is true pop and push counts.**
    // `Op::stack_growth`/`stack_shrink` are the PEAK model — a transient reach
    // and a net — and their own documentation says they are not pop counts.
    // A shadow stack driven by them desynchronises at every pop-and-push
    // instruction, which is a mistake this repository has recorded once already.
    let mut stack: Vec<(usize, u32)> = Vec::new();
    let mut agreed: BTreeMap<usize, Option<Width>> = BTreeMap::new();
    for (i, op) in chunk.ops.iter().enumerate() {
        if let Op::SetLocal(n) = op {
            let found = stack
                .last()
                .and_then(|&(pi, k)| instruction_fixed_width(chunk, pi, k));
            let slot = agreed.entry(usize::from(*n)).or_insert(found);
            if *slot != found {
                *slot = None;
            }
        }
        let (required, delta) = keleusma::verify::op_depth_effect(op, chunk);
        for _ in 0..required.max(0) {
            stack.pop();
        }
        for k in 0..(required + delta).max(0) {
            stack.push((i, k as u32));
        }
    }
    agreed
        .into_iter()
        .filter_map(|(idx, w)| w.map(|w| (idx, w)))
        .collect()
}

/// Packed width implied by a DECLARED boundary shape, or [`Width::Unknown`].
///
/// One definition serving both call paths. A native's declared return shape and
/// a chunk's declared return shape answer the same question — how many bytes
/// this value occupies inside a composite body — and a second copy of the
/// mapping would be a second opinion about packing, which is exactly the class
/// of defect the operand-width machinery exists to prevent.
///
/// # Fails closed, and that is the whole contract
///
/// **`Top`, an absent entry, and a scalar kind this backend cannot size all
/// yield `Unknown`, never a default.** A default here would convert a missing
/// table entry into a silent mispack: a `Byte` and a `Word` are
/// indistinguishable on the operand stack, so a guessed width writes the wrong
/// number of bytes and the body reads back as a plausible wrong value rather
/// than as an error. `Unknown` instead fails at the USE, which is the existing
/// refusal.
fn width_of_declared_shape(shape: Option<&keleusma::bytecode::WireShape>) -> Width {
    match shape {
        Some(keleusma::bytecode::WireShape::Scalar { kind }) => {
            match keleusma::value_layout::ScalarKind::from_tag(*kind) {
                Some(k) => Width::Scalar(u32::try_from(k.size_in_bytes(8, 8)).unwrap_or(0)),
                None => Width::Unknown,
            }
        }
        Some(keleusma::bytecode::WireShape::Flat { size, .. }) => Width::Body(*size),
        _ => Width::Unknown,
    }
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
    /// An opcode for which this backend has no lowering, or none for the shape
    /// encountered.
    ///
    /// `op` carries the opcode's identity as **data**. It was previously
    /// recoverable only as the leading word of the message, and
    /// `isa_lowering_census` read it that way — so `Const(60000) out of range`,
    /// a malformed constant INDEX, was credited to the `Const` OPCODE, which
    /// this backend lowers in nearly every module of the corpus. Whether a
    /// refusal concerns an opcode is now a property of the type rather than of
    /// English word order.
    UnsupportedOp {
        /// The opcode this refusal is about. An ISA opcode name.
        op: String,
        /// What specifically was not lowerable. May be empty when the backend
        /// has no arm for the opcode at all.
        detail: String,
    },
    /// A type or feature this backend lacks, not attributable to one opcode.
    ///
    /// Floats are the standing case: a float in a chunk signature is a property
    /// of the signature, and calling it an unsupported opcode produced the
    /// sentence "does not yet support opcode chunk 0 has a Float in its
    /// signature".
    UnsupportedShape(String),
    /// The input's own integrity failed — an out-of-range index, an arity
    /// mismatch, a reserved encoding.
    ///
    /// **Not a statement that any opcode is unlowerable.** The opcode named in
    /// the message is the one whose operand was malformed, and the backend
    /// lowers it fine for well-formed input.
    MalformedInput(String),
    /// A defect in this crate rather than in the input, like
    /// [`LowerError::InvalidIr`], but detected before IR was produced.
    ///
    /// Kept distinct so a consumer can tell "your program uses a feature I
    /// lack" from "I am broken". Reporting the second as the first invites a
    /// user to rewrite a program that was never the problem.
    Internal(String),
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
    /// A loop-body composite the host can still be holding when the next
    /// iteration overwrites it.
    ///
    /// [`region::plan_chunk_region`] gives each site ONE offset for the life of
    /// the chunk, so a site inside a loop rewrites the same bytes every
    /// iteration. A composite is an arena handle rather than a copy, and an
    /// overwrite in place advances no epoch, so a host holding iteration `n`'s
    /// value calls `resolve`, SUCCEEDS, and reads iteration `n+1`'s bytes.
    ///
    /// **The failure is a silently wrong value, not a `Stale` error**, which is
    /// why it is refused here instead of left to a runtime guard: there is no
    /// runtime guard it trips. `docs/proofs/COMPOSITE_REGION_REUSE.md` §4.1.1.
    ///
    /// Not reusing the slot instead would mean one region per iteration, which
    /// is unbounded in the iteration count and gives up the bounded-memory
    /// property this backend exists to provide. Refusal is the disposition that
    /// keeps the bound and loses the defect.
    YieldEscapingLoopComposite {
        /// Index into the chunk's `ops` of the construction site.
        site_op: usize,
        /// Index into the chunk's `ops` of the `Yield` that can carry it out.
        yield_op: usize,
    },
    /// The lowering produced a module LLVM's own verifier rejects.
    ///
    /// A postcondition on [`lower_module`], not a diagnostic for the caller's
    /// input. Reaching it always means a defect in this crate. It exists because
    /// `verify` was previously called only in the test harness, so malformed IR
    /// would reach a consumer while every test stayed green.
    InvalidIr(String),
}

impl LowerError {
    /// A refusal that genuinely concerns `op`.
    ///
    /// Takes the opcode name separately from the prose so the two cannot drift:
    /// the census reads `op`, and the sentence is for a human.
    fn unsupported_op(op: &str, detail: String) -> Self {
        LowerError::UnsupportedOp {
            op: op.to_string(),
            detail,
        }
    }
}

/// The opcode's variant name, derived from the opcode itself.
///
/// This is deriving a name from an `Op` value, not parsing English out of a
/// message — `Debug` for these variants opens with the variant name.
fn op_variant_name(op: &Op) -> String {
    format!("{op:?}")
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect()
}

impl core::fmt::Display for LowerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LowerError::Diagnostic(n) => {
                write!(f, "diagnostic lowering collected {n} chunk refusal(s)")
            }
            LowerError::UnsupportedOp { op, detail } if detail.is_empty() => {
                write!(f, "native lowering does not yet support opcode {op}")
            }
            LowerError::UnsupportedOp { op, detail } => {
                write!(
                    f,
                    "native lowering does not yet support opcode {op}: {detail}"
                )
            }
            LowerError::UnsupportedShape(what) => {
                // A neutral lead-in, because these messages are CLAUSES rather
                // than noun phrases. "does not support {what}" rendered
                // "does not support chunk 0 carries a Float CONSTANT ...".
                write!(f, "native lowering refused this module: {what}")
            }
            LowerError::MalformedInput(what) => {
                write!(f, "the module is malformed and was not lowered: {what}")
            }
            LowerError::Internal(what) => {
                write!(f, "a defect in this crate rather than in the input: {what}")
            }
            LowerError::UnsupportedWordWidth(w) => {
                write!(f, "native lowering does not support word_bits_log2 = {w}")
            }
            LowerError::UnsupportedDataSlot { slot, why } => {
                write!(f, "data slot {slot} is not lowerable: {why}")
            }
            LowerError::YieldEscapingLoopComposite { site_op, yield_op } => write!(
                f,
                "the composite built at op {site_op} is yielded at op {yield_op} from inside \
                 a loop, so reusing its fixed offset would hand the host the next \
                 iteration's bytes with no Stale error; refused rather than \
                 miscompiled"
            ),
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
    /// Kind of each operand-stack entry, parallel to `widths`. Seeded by the
    /// PRODUCING opcode — `IntToFloat` and a float `Const` push `Float` — so
    /// this is local dataflow rather than signature threading.
    kinds: Vec<OperandKind>,
    /// Packed width most recently stored into each local, so `GetLocal` can
    /// restore what `SetLocal` put there. A local never written in this chunk
    /// stays unknown.
    local_widths: Vec<Width>,
    /// Kind most recently stored into each local, so `GetLocal` restores what
    /// `SetLocal` put there — the same round-trip the widths already make.
    local_kinds: Vec<OperandKind>,
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
    /// Push with an explicit kind. `push_w` is this with `Int`, which is what
    /// every pre-float producer means.
    fn push_k(&mut self, v: IntValue<'ctx>, w: Width, k: OperandKind) {
        self.push_w(v, w);
        if self.kinds.len() < self.widths.len() {
            self.kinds.resize(self.widths.len(), OperandKind::Unknown);
        }
        if let Some(d) = self.depth.checked_sub(1) {
            if self.kinds.len() <= d {
                self.kinds.resize(d + 1, OperandKind::Unknown);
            }
            self.kinds[d] = k;
        }
    }

    /// The kind of the operand `back` entries down, `Unknown` if untracked.
    fn kind_at(&self, back: usize) -> OperandKind {
        match self.depth.checked_sub(back + 1) {
            Some(d) => self.kinds.get(d).copied().unwrap_or(OperandKind::Unknown),
            None => OperandKind::Unknown,
        }
    }

    fn push_w(&mut self, v: IntValue<'ctx>, w: Width) {
        if self.depth >= MAX_STACK {
            self.stack_overflow.get_or_insert(self.depth);
        }
        let slot = self.slot(self.depth);
        self.b.build_store(slot, v).unwrap();
        if self.widths.len() <= self.depth {
            self.widths.resize(self.depth + 1, Width::Unknown);
        }
        // **Mark the slot Int here, not only in `push_k`.** The stack reuses
        // slots, so a `Float` tag left by an earlier operand would otherwise
        // survive into an integer pushed at the same depth and make a later
        // bitcast reinterpret it.
        if self.kinds.len() <= self.depth {
            self.kinds.resize(self.depth + 1, OperandKind::Unknown);
        }
        self.kinds[self.depth] = OperandKind::Int;
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

/// Why a data slot of this kind is refused.
///
/// **A DATA SLOT IS NOT A COMPOSITE BODY FIELD, and the distinction is why
/// `Fixed` is lowered in one and refused here.** A body field is INTERNAL: the
/// compiler packs it, the same program reads it, and no one outside sees the
/// layout, so the backend agreeing with the reference is a fact rather than a
/// choice. A shared data slot is **HOST-VISIBLE**, and its layout is an
/// application binary interface -- the same class of question as the string ABI
/// (ruled provisional) and the float ABI (undecided, blocking two opcodes).
///
/// So the `Fixed` case here stays refused deliberately, not by omission, even
/// though the identical-looking exclusion on `GetField` and `GetIndex` was
/// lifted. Settling a host-visible layout by writing whichever version compiles
/// is the trade this line refuses.
fn alloc_format_kind(tag: u8) -> String {
    match tag {
        0 => String::from("Unit slot; the flat representation of Unit is unsettled"),
        // **The REPRESENTATION is settled; the host-visible SCALE is not.**
        // `ScalarKind::Fixed` is a signed two's-complement Q-format integer of
        // the runtime's word width, and a backend lowering it at the stated
        // offset would agree with the reference byte for byte. What is absent is
        // `N`: it is carried by the opcodes and the compile-time type, and
        // `SharedSlotLayout` has no field holding it, so `Fixed<16>` and
        // `Fixed<8>` — a factor of 256 apart — are indistinguishable to a host.
        //
        // The previous wording, "fixed-point representation is unsettled", sent
        // a reader looking for a decision made long ago. Corrected per the ACTION
        // recorded in `docs/decisions/FIXED_SHARED_SLOT_ABI.md` for whichever
        // line owns this message; this one does.
        4 => String::from(
            "Fixed slot; the host-visible fraction-bit scale is unspecified, so two \
             programs whose values differ by a factor of 2^N share one layout",
        ),
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
            delegated_call: None,
            natives: &[],
            native_shapes: &[],
            chunk_signatures: &[],
            float_bytes: 8,
            // `lower_chunk` sees no module, so it resolves no call and needs no
            // per-site offsets; it refuses `Op::Call` for the same reason.
            call_regions: &[],
            // Nobody is asking this single-chunk path what it lowered.
            visited: None,
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
/// # Precondition: the module must be ADMISSIBLE, and this function does not check it
///
/// **Verified is not enough.** A module can pass [`keleusma::verify::verify`] and
/// still be refused by `Vm::new`, which additionally requires a statically
/// extractable resource bound. This function accepts such a module and lowers it,
/// and the code it produces is **not memory-safe**.
///
/// Measured rather than supposed: mutating one `CheckedAdd` to `CheckedSub` in a
/// corpus module yields bytecode that is `verify()`-clean, refused by
/// `auto_arena_capacity_for`, `module_wcmu` and `Vm::new` alike, accepted here
/// without complaint, and whose lowered form died with SIGBUS.
///
/// **The caller is responsible for admissibility.** The guarantee this project
/// sells is a resource bound; an execution path that runs what the bound analysis
/// refuses is a hole in it. Enforcing the check here was considered and not done —
/// it would couple a pure lowering function to the resource analysis and pay that
/// cost on every call. `no_lowerable_corpus_module_is_unbounded` in
/// `tests/corpus_differential.rs` pins that no shipped corpus module violates it.
pub fn lower_module<'ctx>(
    ctx: &'ctx Context,
    module: &LlvmModule<'ctx>,
    program: &Module,
    opts: LowerOptions,
) -> Result<Vec<FunctionValue<'ctx>>, LowerError> {
    lower_module_with(ctx, module, program, opts, None, None)
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
/// The name reported for a refusal that rejects the WHOLE module rather than
/// one chunk. Not a chunk name, and deliberately bracketed so it cannot collide
/// with one.
pub const MODULE_LEVEL_REFUSAL: &str = "<module>";

pub fn module_refusals(program: &Module, opts: LowerOptions) -> Vec<(String, LowerError)> {
    let ctx = Context::create();
    let m = ctx.create_module("refusals");
    let mut sink = Vec::new();
    let outcome = lower_module_with(&ctx, &m, program, opts, Some(&mut sink), None);
    // **A MODULE-LEVEL REFUSAL USED TO BE INVISIBLE HERE, and callers read the
    // emptiness of this vector as "the backend accepts it".**
    //
    // `lower_module_with` reports per-CHUNK refusals through the sink and
    // rejects the whole module by RETURNING. The return value was discarded, so
    // a module the backend cannot lower at all produced ZERO refusals — the
    // same answer as a module it lowers perfectly. Measured: a float-signature
    // module gave `0 entries` from here while `lower_module` gave `Err`.
    //
    // Two guards already had this shape before the float one: the native-symbol
    // collision check and the word-width check. Both were equally unreportable.
    //
    // **`Diagnostic` IS A MODE SENTINEL, NOT A REFUSAL.** In diagnostic mode
    // `lower_module_with` ALWAYS returns `Err(Diagnostic(sink.len()))`, even
    // when the sink is empty and the module lowers cleanly, because the IR it
    // built is incomplete wherever a chunk was abandoned. Pushing every `Err`
    // therefore marked EVERY module refused -- measured: the support census went
    // to 0 lowering, 15 refused, and its own control fired.
    if let Err(e) = outcome
        && !matches!(e, LowerError::Diagnostic(_))
    {
        sink.push((MODULE_LEVEL_REFUSAL.to_string(), e));
    }
    sink
}

/// One chunk's refusal: the chunk's name and why it was refused.
///
/// `MODULE_LEVEL_REFUSAL` in the name position means the whole module was
/// rejected rather than any one chunk.
pub type ChunkRefusal = (String, LowerError);

/// Which op indices the lowering visited, per chunk, parallel to
/// `Module::chunks`.
///
/// `Some(indices)` for a chunk that lowered to completion; `None` for one that
/// refused, whose partial record is deliberately withheld because its last entry
/// is the op that failed. See [`module_lowered_op_indices`].
pub type LoweredOpIndices = Vec<Option<Vec<usize>>>;

/// Per-chunk record of which op indices the lowering ACTUALLY VISITED, beside
/// the same refusals [`module_refusals`] reports.
///
/// Returns a vector parallel to `program.chunks`: `Some(indices)` for a chunk
/// that lowered to completion, `None` for one that refused. A module-level
/// refusal yields `None` for every chunk, since nothing in it was lowered.
///
/// # Why chunk-level success is not enough, and what this exists to prevent
///
/// **The op loop skips code no edge reaches.** `break;` lowers to an
/// unconditional branch and whatever follows it in the opcode stream is
/// unreachable, so `lower_chunk_body` marks itself `dead` and steps over those
/// ops without ever entering their match arms. An opcode occurring ONLY in such
/// a region of an otherwise clean chunk was therefore never lowered, and
/// concluding "this chunk lowered, so the backend supports every opcode in it"
/// would credit a lowering that does not exist — which is the same shape as
/// every coverage error this line has recorded: a signal answering a narrower
/// question than the one asked.
///
/// **Indices, not names.** The refusal list carries chunk NAMES, and a module
/// holding two chunks of one name makes a name lookup ambiguous exactly when it
/// matters. This is positional, so it cannot be confused.
///
/// # What a visited index does NOT mean
///
/// **Not that the emitted code is correct.** It means the backend produced
/// something for that op. Correctness belongs to the differential oracle.
pub fn module_lowered_op_indices(
    program: &Module,
    opts: LowerOptions,
) -> (Vec<ChunkRefusal>, LoweredOpIndices) {
    let ctx = Context::create();
    let m = ctx.create_module("lowered_ops");
    let mut sink = Vec::new();
    let mut visits = Vec::new();
    let outcome = lower_module_with(&ctx, &m, program, opts, Some(&mut sink), Some(&mut visits));
    // A module-level refusal returns BEFORE the chunk loop, so `visits` is short
    // or empty rather than all-`None`. Normalising here keeps the postcondition
    // ("parallel to `program.chunks`") true for every caller, instead of leaving
    // each one to rediscover the case. `Diagnostic` is the diagnostic-mode
    // sentinel and is not a refusal — see `module_refusals`.
    if let Err(e) = outcome
        && !matches!(e, LowerError::Diagnostic(_))
    {
        sink.push((MODULE_LEVEL_REFUSAL.to_string(), e));
        return (sink, vec![None; program.chunks.len()]);
    }
    debug_assert_eq!(visits.len(), program.chunks.len());
    visits.resize(program.chunks.len(), None);
    (sink, visits)
}

fn lower_module_with<'ctx>(
    ctx: &'ctx Context,
    module: &LlvmModule<'ctx>,
    program: &Module,
    opts: LowerOptions,
    mut refusals: Option<&mut Vec<(String, LowerError)>>,
    mut visits: Option<&mut Vec<Option<Vec<usize>>>>,
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
        return Err(LowerError::MalformedInput(format!(
            "natives {names:?} all mangle to the external symbol `{sym}`; the \
             lowering refuses rather than binding several declarations to one \
             host definition"
        )));
    }

    // **A FLOAT IN ANY SIGNATURE REFUSES THE MODULE, so the absence of float
    // miscompiles stops being incidental.**
    //
    // Measured before this existed: `fn p(a: Float) -> Float { a }` LOWERED with
    // no refusal. Only float CONSTANTS were guarded, by `Op::Const`. Nothing
    // stopped a float VALUE reaching the operand stack — what stopped a wrong
    // answer was that no float OPERATION was supported, so the value was never
    // operated on. That is a property of what is unimplemented, not a guard.
    //
    // **It is also a live ABI defect rather than only a hazard.** The lowered
    // entry takes `i64`; a float-typed Keleusma function should receive a
    // double. A host calling it under the real C ABI would read an FP register
    // this code never wrote. The pass-through case happens to round-trip a bit
    // pattern, which is correct by accident and only inside this harness.
    //
    // **The tag is compared numerically on purpose.** `ScalarKind::Float` is
    // behind the `floats` feature, so naming the variant would make this guard
    // vanish in a build without it — exactly when it is still needed, because
    // the wire tag is stable regardless of which features the READER was built
    // with.
    const SCALAR_FLOAT_TAG: u8 = 5;
    if let Some((idx, _)) = program.signatures.iter().enumerate().find(|(_, sg)| {
        matches!(sg.ret, keleusma::bytecode::WireShape::Scalar { kind } if kind == SCALAR_FLOAT_TAG)
            || sg.params.iter().any(|p| {
                matches!(p, keleusma::bytecode::WireShape::Scalar { kind } if *kind == SCALAR_FLOAT_TAG)
            })
    }) {
        return Err(LowerError::UnsupportedShape(format!(
            "chunk {idx} has a Float in its signature and this backend has no float \
             representation: no `f64_type`, no float opcode lowered, and an entry ABI \
             of `i64` where a double belongs. Refusing the module is the guard; the \
             absence of float arithmetic is not one"
        )));
    }

    // **A SIGNATURE IS NOT THE ONLY ROUTE A FLOAT TAKES, and the other routes
    // used to be closed only by accident.**
    //
    // Measured: `fn p(w: Word) -> Word { let f = 1.5; ... }` has NO float in any
    // signature and DOES carry `ConstValue::Float`. Before this, that module was
    // refused only because `Op::Add` was unsupported -- a property of what is
    // unimplemented, not a guard. The moment `Op::Add` lowers as an integer add
    // (which is correct for `Byte` and `Fixed`), that program would SILENTLY
    // MISCOMPILE.
    //
    // So the routes are enumerated and each is closed:
    //   1. chunk signatures        -- above
    //   2. chunk constants         -- here
    //   3. native return shapes    -- here
    //   4. data-segment slots      -- here
    //
    // **The list is a claim, and `float_guard_routes.rs` tests each route with a
    // named test per route.** A guard that closes three of four while reading as
    // total is the shape this line keeps finding.
    //
    // **AND THIS COMMENT WAS ITSELF AN INSTANCE OF IT UNTIL 2026-08-24.** It
    // cited a test called `the_float_guard_closes_every_route_it_names` **that
    // was never written**, and route 3 -- the native return shape -- had no test
    // at all. Proved by disabling this guard: every other test in that file
    // still passed. A citation to a test that does not exist cannot fail, so it
    // read as coverage every time anyone checked, while being coverage for three
    // routes. `comment_citations.rs` now scans this package for exactly
    // that shape.
    for (i, c) in program.chunks.iter().enumerate() {
        if let Some(k) = c
            .constants
            .iter()
            .position(|k| matches!(k, ConstValue::Float(_)))
        {
            return Err(LowerError::UnsupportedShape(format!(
                "chunk {i} carries a Float CONSTANT at index {k}. A float reaches this \
                 module without appearing in any signature, and the integer \
                 arithmetic lowering would silently miscompile it"
            )));
        }
    }
    if let Some(i) = program
        .native_return_shapes
        .iter()
        .position(|sh| matches!(sh, keleusma::bytecode::WireShape::Scalar { kind } if *kind == SCALAR_FLOAT_TAG))
    {
        return Err(LowerError::UnsupportedShape(format!(
            "native {i} declares a Float RETURN SHAPE; its result would reach the \
             operand stack as a float this backend cannot represent"
        )));
    }
    // Route 4, the data segment, is closed AT THE ACCESS rather than at the
    // declaration, and deliberately not re-checked here. `resolve_shared_scalar` admits
    // only `SCALAR_INT`, `SCALAR_BYTE` and `SCALAR_BOOL`, refusing anything else
    // as `UnsupportedDataSlot`.
    //
    // **Measured, because the obvious statement is wrong**: a module that
    // DECLARES a float slot and never reads it LOWERS. That is safe by
    // construction rather than by refusal -- an unread slot puts no float on the
    // operand stack, so there is nothing for the integer arithmetic below to
    // miscompile. Every ACCESS refuses, which is the point where a float would
    // actually arrive.
    //
    // A second check here would be a parallel model of a guard that already
    // exists and the two could drift. `float_guard_routes.rs` tests this route
    // through the EXISTING refusal.

    // A delegated suspension affects exactly TWO chunks: the entry, whose tail
    // call becomes the return, and the callee, whose yields become returns.
    let delegated = if opts.delegated_suspension {
        delegated_suspension_plan(program)
    } else {
        None
    };

    for (i, (chunk, func)) in program.chunks.iter().zip(declared.iter()).enumerate() {
        let mut tail = degenerate_stream_yield(chunk, program);
        let mut delegated_call = None;
        if let Some((entry, callee, call_ix)) = delegated {
            if i == entry {
                // An EMPTY yield list, deliberately: it makes `Stream` and
                // `Reset` lower to nothing, exactly as in a degenerate stream,
                // while marking no `Yield` for return -- the entry has none.
                tail = Some(Vec::new());
                delegated_call = Some(call_ix);
            } else if i == callee {
                tail = Some(
                    chunk
                        .ops
                        .iter()
                        .enumerate()
                        .filter(|(_, o)| matches!(o, Op::Yield))
                        .map(|(ix, _)| ix)
                        .collect(),
                );
            }
        }
        let call_regions = region::plan_call_site_regions(program, i);
        let seen = core::cell::RefCell::new(Vec::new());
        let cfg = BodyCfg {
            opts,
            degenerate_yield: tail.as_deref(),
            delegated_call,
            natives: &program.native_names,
            native_shapes: &program.native_return_shapes,
            chunk_signatures: &program.signatures,
            float_bytes: 1u32 << program.float_bits_log2 >> 3,
            call_regions: &call_regions,
            visited: visits.as_ref().map(|_| &seen),
        };
        match lower_chunk_body(ctx, module, chunk, *func, &declared, data, cfg) {
            Ok(_) => {
                // A CLEAN chunk, so every recorded index lowered. See
                // `BodyCfg::visited` for why a refused chunk's list is dropped
                // instead: its last entry is the op that failed.
                if let Some(v) = visits.as_mut() {
                    v.push(Some(seen.into_inner()));
                }
            }
            Err(e) => {
                if let Some(v) = visits.as_mut() {
                    v.push(None);
                }
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
    /// Byte width of the runtime's `Float`, from the module header's
    /// `float_bits_log2`.
    ///
    /// **Carried rather than assumed.** `Float` is `f32` under `narrow-float-32`
    /// and `f64` otherwise, so a hard-coded `double` would be the wrong type in
    /// a build that has no `f64`. Only 8 is lowered today; any other width is
    /// REFUSED rather than approximated, because a float of the wrong width is a
    /// silently wrong number and not a fault.
    float_bytes: u32,
    /// `Some(ip)` when this is a degenerate stream chunk and `ip` is the
    /// `Op::Yield` that becomes the return. Computed by the caller, which holds
    /// the bytecode module; `lower_chunk` passes `None` for the same reason it
    /// refuses `Op::Call`.
    degenerate_yield: Option<&'a [usize]>,
    /// Op index of a tail-position `Call` whose result is RETURNED rather than
    /// pushed, because the callee suspends on the caller's behalf. See
    /// [`delegated_suspension_plan`].
    delegated_call: Option<usize>,
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
    /// Return-value shape of each declared native, parallel to `natives`.
    ///
    /// **Consulted rather than ignored.** Before 2026-08-14 every native result
    /// was pushed at `Width::Unknown`, which is correct when the shape is `Top`
    /// and needlessly lossy when it is not: a composite built from a signatured
    /// native's result was refused for a width the module actually declares.
    /// `rogue_dungen` is the corpus case.
    native_shapes: &'a [keleusma::bytecode::WireShape],
    /// Declared signature of each chunk, parallel to `Module::chunks`.
    ///
    /// **The chunk-call analogue of `native_shapes`.** Without it an `Op::Call`
    /// result carries no width and any composite packing one is refused. Empty
    /// for the single-chunk entry point, which resolves no call at all.
    chunk_signatures: &'a [keleusma::bytecode::ChunkSignature],
    /// Region offset for each `Op::Call` in this chunk, by op index.
    ///
    /// **The caller-allocated return slot, expressed through the pointer the
    /// caller already passes.** A callee writes its flat sites at offsets it
    /// plans from zero, so handing every call site the same region base makes
    /// two calls to one callee overwrite each other — the `10_multbyte.kel`
    /// defect, where `p[0]` read `r[0]`'s value. Giving each site a disjoint
    /// block fixes it without changing any signature: the callee still receives
    /// one region pointer and never names an arena.
    call_regions: &'a [(usize, u32)],
    /// Where to record the op indices this body actually LOWERED, if anywhere.
    ///
    /// **Exists because "the chunk lowered" does not imply "every op in it was
    /// lowered".** The op loop skips anything in code no edge reaches (`if dead
    /// { continue; }`), and the compiler emits such code routinely — a `break`
    /// is an unconditional branch and whatever follows it in the opcode stream
    /// is unreachable. So an opcode witnessed ONLY in a dead region of an
    /// otherwise clean chunk was never visited, and a census that inferred
    /// support from chunk-level success would credit the backend with a lowering
    /// it does not have.
    ///
    /// Recorded at the TOP of the iteration, before the arm runs. That is sound
    /// for the only consumer, [`module_lowered_op_indices`], because a chunk's
    /// lowering stops at its first refusal and the consumer keeps this list only
    /// for chunks that lowered CLEANLY — in which case every recorded index
    /// succeeded. It is NOT sound to read for a refused chunk, whose last
    /// recorded index is the one that failed.
    ///
    /// A `RefCell` rather than `&mut` so `BodyCfg` stays `Copy`.
    visited: Option<&'a core::cell::RefCell<Vec<usize>>>,
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
/// Result width for the generic arithmetic surface, or `None` to refuse.
///
/// **Refusing is the default and the point.** A matched `Byte` pair yields a
/// `Byte`; a matched eight-byte pair yields the same width, which after the
/// module-level float guard can only be `Fixed`. Everything else -- an unknown
/// width, a mismatched pair, a body operand -- is refused, because a guess here
/// is a silent wrong answer rather than a fault.
///
/// `byte_only` refuses the eight-byte case, for `Op::Mul`, whose `Fixed` form
/// the compiler emits as `Op::FixedMul` instead.
fn arith_result_width(l: Width, r: Width, byte_only: bool) -> Option<Width> {
    match (l, r) {
        (Width::Scalar(1), Width::Scalar(1)) => Some(Width::Scalar(1)),
        (Width::Scalar(8), Width::Scalar(8)) if !byte_only => Some(Width::Scalar(8)),
        _ => None,
    }
}

/// Truncate to eight bits when the result is a `Byte`, and leave it alone
/// otherwise.
///
/// **The asymmetry is the whole content.** A `Byte` must be masked to hold the
/// representation invariant that makes `ByteToWord` a no-op. A `Fixed` must NOT
/// be, and masking one would truncate it to eight bits while every later field
/// offset still looked correct -- a silent wrong answer of exactly the kind the
/// `Width::Body` split exists to prevent.
fn mask_if_byte<'ctx>(
    b: &inkwell::builder::Builder<'ctx>,
    i64t: inkwell::types::IntType<'ctx>,
    v: inkwell::values::IntValue<'ctx>,
    out: Width,
) -> inkwell::values::IntValue<'ctx> {
    if out == Width::Scalar(1) {
        b.build_and(v, i64t.const_int(0xFF, false), "bmask")
            .unwrap()
    } else {
        v
    }
}

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
/// Can this module's suspension be delegated across ONE call edge?
///
/// Returns `(entry, callee, call_op_index)` when it can.
///
/// # Why this is not a widening of [`degenerate_stream_yield`]
///
/// That predicate asks whether a chunk suspends and returns in tail position.
/// This one asks whether an entry hands its ENTIRE body to a callee that does.
/// The two share a soundness argument but not a shape, and folding them together
/// is what would turn a refusal into a silent miscompile.
///
/// # The soundness argument, in one line
///
/// On resume the virtual machine writes the input into slot 0 of the ENTRY frame
/// and pushes it as the suspended frame's yield value. When every `Yield` in the
/// callee is immediately followed by `Return`, that pushed value is returned
/// straight out and discarded by the caller's `PopN(1)`, so it is DEAD. The only
/// live path is the entry's slot 0, which the next native call supplies as its
/// argument. Executed, not reasoned: see
/// `probe_delegated_suspension.rs::the_resume_value_reaches_the_entrys_slot_zero_and_is_dead_in_the_callee`.
///
/// # Every clause refuses a case the transform cannot model
fn delegated_suspension_plan(module: &Module) -> Option<(usize, usize, usize)> {
    let entry = module.entry_point?;
    let chunk = module.chunks.get(entry)?;
    if chunk.block_type != BlockType::Stream {
        return None;
    }
    // Clause 1: the entry is a prologue-free `Stream ... Call ; PopN(1) ; Reset`.
    // An op before `Stream` runs once in the VM and on every native call.
    let ops = &chunk.ops;
    if !matches!(ops.first(), Some(Op::Stream)) || !matches!(ops.last(), Some(Op::Reset)) {
        return None;
    }
    if ops.len() < 4 {
        return None;
    }
    // The call must be in TAIL position: `Call ; PopN(1) ; Reset` and nothing else.
    let call_ix = ops.len() - 3;
    if !matches!(ops.get(ops.len() - 2), Some(Op::PopN(1))) {
        return None;
    }
    let Some(Op::Call(target, _)) = ops.get(call_ix) else {
        return None;
    };
    let callee_ix = *target as usize;
    // The entry must contain no `Yield` of its own; one would be the ordinary
    // degenerate case and the two transforms must not both fire.
    if ops.iter().any(|o| matches!(o, Op::Yield)) {
        return None;
    }
    // Nor any other call: a second callee would run after the delegated return.
    if ops
        .iter()
        .enumerate()
        .any(|(i, o)| i != call_ix && matches!(o, Op::Call(_, _)))
    {
        return None;
    }

    let callee = module.chunks.get(callee_ix)?;
    // Clause 2: EVERY `Yield` in the callee is immediately followed by `Return`.
    // One that is not makes the resumed value live in the callee, and the
    // transform would lose it silently. This is where the general case is
    // refused, and refusing it is the point.
    let mut yields = 0usize;
    for (i, op) in callee.ops.iter().enumerate() {
        if matches!(op, Op::Yield) {
            yields += 1;
            if !matches!(callee.ops.get(i + 1), Some(Op::Return)) {
                return None;
            }
        }
    }
    if yields == 0 {
        return None;
    }
    // Clause 3: the callee calls only `Func` chunks, so suspension is exactly
    // one frame deep. An unresolvable index refuses rather than skips.
    for op in &callee.ops {
        if let Op::Call(t, _) = op
            && module.chunks.get(*t as usize).map(|c| c.block_type) != Some(BlockType::Func)
        {
            return None;
        }
    }
    // Clause 5: no OTHER call site of the callee anywhere in the module. A
    // second caller would reach a `Yield` lowered as a return and take it for an
    // ordinary result.
    for (ci, c) in module.chunks.iter().enumerate() {
        for (oi, op) in c.ops.iter().enumerate() {
            if let Op::Call(t, _) = op
                && *t as usize == callee_ix
                && !(ci == entry && oi == call_ix)
            {
                return None;
            }
        }
    }
    Some((entry, callee_ix, call_ix))
}

/// Does this module delegate its suspension across one call edge, and to what?
///
/// A thin public view of `delegated_suspension_plan`, which is private, so the
/// name is given as text rather than as a link. It exists so that a
/// CENSUS can ask the question over a corpus without restating the predicate,
/// and it delegates rather than reimplements for exactly that reason: a second
/// copy of this test would be a second opinion about which modules the lowering
/// will transform, and the only opinion that matters is the lowering's.
///
/// **This is a query, not a switch.** Calling it neither enables the delegated
/// transform nor changes what `lower_module` does; the mechanism stays behind
/// `LowerOptions::delegated_suspension`, which remains off by default.
///
/// Returns `(entry chunk, callee chunk, call op index)` for a qualifying module
/// and `None` otherwise. `None` covers both "not a stream entry at all" and
/// "a stream entry whose shape this transform refuses", which are different
/// facts; a caller that needs to tell them apart must read the ops itself.
pub fn delegated_suspension_subject(module: &Module) -> Option<(usize, usize, usize)> {
    delegated_suspension_plan(module)
}

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
                // Building a composite that is then discarded.
                //
                // **The same mistake the allowlist above already made once**,
                // one construct further along. `rogue_ai_boss`, `_hunter` and
                // `_tracker` all end `Yield ; PopN(1) ; Const(0) x3 ;
                // NewComposite(Tuple, 3) ; PopN(1) ; Reset` -- a trailing tuple
                // built and thrown away. Their net delta is ALREADY exactly
                // `-1`, the value this rule demands; only the allowed-op set
                // rejected them.
                //
                // Sound for the same reason as the constants: it writes only
                // the scratch region, nothing reads it because the value is
                // discarded by the following `PopN` and `Reset` rewinds, and it
                // can neither trap nor call out nor touch the data segment.
                //
                // `Flat` only. A `Boxed` body allocates outside the region and
                // the emitter refuses it anyway, so admitting it here would
                // widen the predicate ahead of the lowering.
                Some(Op::NewComposite(keleusma::bytecode::NewCompositeOperand::Flat {
                    count,
                    ..
                })) => {
                    delta += 1 - i32::from(*count);
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
        delegated_call,
        natives,
        native_shapes,
        chunk_signatures,
        call_regions,
        visited,
        float_bytes,
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
        kinds: Vec::new(),
        // **Parameter kinds start Unknown, not Int.** A float-typed parameter
        // is exactly the case the entry ABI does not yet handle, and defaulting
        // to `Int` would let it be read as an integer instead of refused.
        local_kinds: vec![OperandKind::Unknown; local_widths.len()],
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
    // Which of those multiply-written locals nonetheless have a knowable width,
    // because every write agrees. Narrows the rule above without weakening it.
    let certified_widths = certified_local_widths(chunk);

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

        // The op is about to be lowered. See `BodyCfg::visited` for why this is
        // recorded here rather than after the arm, and why only a CLEAN chunk's
        // list may be read.
        if let Some(v) = visited {
            v.borrow_mut().push(i);
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
                        return Err(LowerError::unsupported_op(
                            "GetTupleField",
                            format!("GetTupleField reading {tf:?} is not lowered"),
                        ));
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
                        return Err(LowerError::unsupported_op(
                            "GetEnumField",
                            format!("GetEnumField reading {other:?} is not lowered"),
                        ));
                    }
                });
                &normalised
            }
            other => other,
        };

        // **A FLOAT MAY ONLY REACH AN OPCODE THAT KNOWS IT IS ONE — a WHITELIST,
        // not a per-opcode patch.**
        //
        // Before floats were lowered, a module with a float local was refused
        // only because no float OPERATION was supported. `float_guard_routes.rs`
        // names that plainly: *"a property of what is unimplemented, not a
        // guard"*. Implementing the operations REMOVED that accidental block, so
        // a module whose float arises from `as Float` with no float constant or
        // signature now reaches lowering — and the module-level guard does not
        // cover it, because it scans signatures, constants, native shapes and
        // data slots, none of which such a module has.
        //
        // A blacklist would have to name every arm that must refuse, and missing
        // one is a silently wrong number: `Op::Div` on a double's bit pattern is
        // an integer division that produces a plausible value. **So the default
        // is refusal**, and only the arms that were written to understand a
        // float operand are exempt. Moves are exempt because they copy bits
        // without interpreting them.
        {
            // **Only the operands the opcode actually POPS.** A first attempt
            // checked the top two stack entries regardless, which refused
            // `Op::Const` for a float sitting *below* it — `Const` consumes
            // nothing. The count comes from `op_depth_effect`, which returns
            // (required, delta), rather than from a guess about each opcode.
            let (required, _) = keleusma::verify::op_depth_effect(op, chunk);
            let consumes_float =
                (0..required as usize).any(|i| st.kind_at(i) == OperandKind::Float);
            let float_aware = matches!(
                op,
                Op::Add
                    | Op::Sub
                    | Op::Mul
                    | Op::FloatToInt
                    | Op::SetLocal(_)
                    | Op::GetLocal(_)
                    | Op::Return
                    | Op::PopN(_)
                    | Op::Dup
            );
            if consumes_float && !float_aware {
                return Err(LowerError::unsupported_op(
                    &op_variant_name(op),
                    format!(
                        "{op:?} would consume a float operand, and this arm was not \
                         written for one. Interpreting a double's bit pattern as an \
                         integer is a plausible wrong number rather than a fault, so \
                         the operand kind fails closed here"
                    ),
                ));
            }
        }

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
                    // **Trusted when every write agrees**, which is the only case
                    // the back-edge objection does not reach. See
                    // [`certified_local_widths`].
                    _ => certified_widths
                        .get(&idx)
                        .copied()
                        .unwrap_or(Width::Unknown),
                };
                // **Restore the kind on the same rule as the width.** Only a
                // singly-written local carries a trusted tag; anything else is
                // `Unknown` and refuses at the use rather than being read as an
                // integer, which is what a float would silently become.
                let k = match local_write_count.get(&idx).copied().unwrap_or(0) {
                    0 | 1 => st
                        .local_kinds
                        .get(idx)
                        .copied()
                        .unwrap_or(OperandKind::Unknown),
                    _ => OperandKind::Unknown,
                };
                st.push_k(v, w, k);
            }
            Op::SetLocal(n) => {
                // Read the width BEFORE popping; `pop` lowers the depth and
                // `width_at` is relative to it.
                let w = st.width_at(0);
                let k = st.kind_at(0);
                let v = st.pop();
                st.b.build_store(st.locals[*n as usize], v).unwrap();
                let idx = *n as usize;
                if local_write_count.get(&idx).copied().unwrap_or(0) == 1 {
                    if st.local_widths.len() <= idx {
                        st.local_widths.resize(idx + 1, Width::Unknown);
                    }
                    st.local_widths[idx] = w;
                    // **The KIND round-trips on the same rule as the width.** A
                    // local written more than once could hold a float on one
                    // path and an integer on another, and a tag that survived
                    // only one of them would let a bitcast reinterpret the
                    // other. Multiply-written locals therefore stay `Unknown`,
                    // which refuses at the use.
                    if st.local_kinds.len() <= idx {
                        st.local_kinds.resize(idx + 1, OperandKind::Unknown);
                    }
                    st.local_kinds[idx] = k;
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
            // **THE FIXED-POINT CONVERSIONS, REPRODUCED FROM THE VM HANDLER**
            // rather than from a definition of Q-format arithmetic. Rounding,
            // saturation and the failure mode are decisions already made in
            // `vm.rs`, and guessing any of them yields code that lowers, passes
            // the support census, and computes the wrong number.
            //
            // The two opcodes fail DIFFERENTLY on an out-of-range fraction
            // count, and the VM says why: `WordToFixed` converts an in-range
            // integer whose result merely overflows the Fixed range, so it
            // saturates; an out-of-range COUNT is corrupt input, for which
            // `FixedToWord` fails closed. **The count is a static operand here**,
            // so both guards become lowering-time refusals rather than emitted
            // runtime checks — the structural verifier already bounds it, so
            // neither refusal is reachable from a verified module.
            // **`FixedMul` LOWERS; `FixedDiv` DOES NOT, and the difference is a
            // RUNTIME FAULT rather than a difficulty.**
            //
            // Both are widen-shift-clamp in the VM and both SATURATE, unlike the
            // checked families which wrap — `fixed_checked_outputs` says so
            // directly. `FixedMul` has no runtime failure: an out-of-range
            // fraction count is static and refused below, and every product
            // saturates rather than faulting.
            //
            // **`FixedDiv` NOW LOWERS, and the reason it did not was STALE.**
            // The note here read: reproducing the VM's unconditional
            // `DivisionByZero` "needs the runtime-fault lowering, which
            // `RUNTIME_FAULTS.md` defers to V0.4.0 — and the existing trap branch
            // here is gated on `OverflowPolicy::Trap`, so it is a policy, not the
            // unconditional fault the VM raises."
            //
            // **`Op::Div | Op::Mod` above already emits exactly that fault**,
            // ungated: it compares the divisor to zero and branches to `trap_bb`.
            // That path was built for the integer forms and this refusal was
            // never revisited. **A stale blocker costs more than a stale figure:
            // a wrong number misleads a reader, a wrong blocker stops work.**
            Op::FixedMul(frac_bits) => {
                if u32::from(*frac_bits) >= WORD_BITS as u32 {
                    return Err(LowerError::unsupported_op(
                        "FixedMul",
                        format!(
                            "FixedMul({frac_bits}) has a fraction count at or beyond the \
                         {WORD_BITS}-bit word width; the VM fails closed here and the \
                         count is static, so the lowering refuses"
                        ),
                    ));
                }
                let rhs = st.pop();
                let lhs = st.pop();
                let a = st.widen(lhs, i128t, "fxm.a");
                let c = st.widen(rhs, i128t, "fxm.b");
                let product = st.b.build_int_mul(a, c, "fxm.p").unwrap();
                // Arithmetic shift: the VM uses `product >> frac_bits` on a
                // signed wide value, so a negative product keeps its sign.
                let sh = i128t.const_int(u64::from(*frac_bits), false);
                let shifted = st.b.build_right_shift(product, sh, true, "fxm.sh").unwrap();
                let max = st.widen(i64t.const_int(i64::MAX as u64, false), i128t, "fxm.max");
                let min = st.widen(i64t.const_int(i64::MIN as u64, true), i128t, "fxm.min");
                let over =
                    st.b.build_int_compare(inkwell::IntPredicate::SGT, shifted, max, "fxm.over")
                        .unwrap();
                let under =
                    st.b.build_int_compare(inkwell::IntPredicate::SLT, shifted, min, "fxm.under")
                        .unwrap();
                let hi =
                    st.b.build_select(over, max, shifted, "fxm.selhi")
                        .unwrap()
                        .into_int_value();
                let clamped =
                    st.b.build_select(under, min, hi, "fxm.sello")
                        .unwrap()
                        .into_int_value();
                st.push(st.b.build_int_truncate(clamped, i64t, "fxm").unwrap());
            }
            // Reproduces `src/vm.rs` `Op::FixedDiv` in all three of its
            // behaviours. **Read from that arm, not pattern-matched from
            // `FixedMul` above** -- the two shift in OPPOSITE directions, and
            // copying the neighbour would produce a plausible wrong answer that
            // still agrees on zero.
            Op::FixedDiv(frac_bits) => {
                // (1) The VM fails closed on an out-of-range fraction count. The
                // count is a static operand, so this is a lowering-time refusal,
                // exactly as `FixedMul` and `FixedToWord` already do.
                if u32::from(*frac_bits) >= WORD_BITS as u32 {
                    return Err(LowerError::unsupported_op(
                        "FixedDiv",
                        format!(
                            "FixedDiv({frac_bits}) has a fraction count at or beyond the \
                         {WORD_BITS}-bit word width; the VM fails closed here and the \
                         count is static, so the lowering refuses"
                        ),
                    ));
                }
                let rhs = st.pop();
                let lhs = st.pop();

                // (2) A zero divisor faults UNCONDITIONALLY -- the VM returns
                // `VmError::DivisionByZero` with no construct to catch it. Same
                // shape as `Op::Div` above, and it must come first so the divide
                // below may assume a non-zero divisor.
                let cont = ctx.append_basic_block(func, "fxd.nonzerodivisor");
                let zero =
                    st.b.build_int_compare(IntPredicate::EQ, rhs, i64t.const_zero(), "fxd.zero")
                        .unwrap();
                st.b.build_conditional_branch(zero, trap_bb, cont).unwrap();
                st.b.position_at_end(cont);

                // (3) Widen, shift the DIVIDEND LEFT by the fraction count, then
                // divide and SATURATE. `FixedMul` shifts its PRODUCT RIGHT; this
                // is the asymmetry the VM specifies and the one place a copied
                // idiom would be silently wrong.
                let a = st.widen(lhs, i128t, "fxd.a");
                let c = st.widen(rhs, i128t, "fxd.b");
                let sh = i128t.const_int(u64::from(*frac_bits), false);
                let dividend = st.b.build_left_shift(a, sh, "fxd.sh").unwrap();
                // **NO `guard_min_div_neg_one` HERE, and its absence is
                // deliberate.** That guard exists for the WRAPPING integer forms,
                // where `i64::MIN / -1` overflows the result type. Here both
                // operands are widened to `i128` first, so the quotient cannot
                // overflow the wide type, and the clamp below carries the value
                // back into range. Copying the guard in would be inert at best.
                let quotient = st.b.build_int_signed_div(dividend, c, "fxd.q").unwrap();
                let max = st.widen(i64t.const_int(i64::MAX as u64, false), i128t, "fxd.max");
                let min = st.widen(i64t.const_int(i64::MIN as u64, true), i128t, "fxd.min");
                let over =
                    st.b.build_int_compare(IntPredicate::SGT, quotient, max, "fxd.over")
                        .unwrap();
                let under =
                    st.b.build_int_compare(IntPredicate::SLT, quotient, min, "fxd.under")
                        .unwrap();
                let hi =
                    st.b.build_select(over, max, quotient, "fxd.selhi")
                        .unwrap()
                        .into_int_value();
                let clamped =
                    st.b.build_select(under, min, hi, "fxd.sello")
                        .unwrap()
                        .into_int_value();
                st.push(st.b.build_int_truncate(clamped, i64t, "fxd").unwrap());
            }
            Op::FixedToWord(frac_bits) => {
                if u32::from(*frac_bits) >= WORD_BITS as u32 {
                    return Err(LowerError::unsupported_op(
                        "FixedToWord",
                        format!(
                            "FixedToWord({frac_bits}) has a fraction count at or beyond the \
                         {WORD_BITS}-bit word width; the VM fails closed here and the \
                         count is static, so the lowering refuses rather than emitting \
                         a trap"
                        ),
                    ));
                }
                let v = st.pop();
                // Arithmetic, sign-preserving: the VM uses `bits >> frac_bits`
                // on a signed value, so negatives keep their sign.
                let sh = i64t.const_int(u64::from(*frac_bits), false);
                st.push(st.b.build_right_shift(v, sh, true, "fx2w").unwrap());
            }
            Op::WordToFixed(frac_bits) => {
                // The VM's corrupt-input arm saturates by sign when the count
                // reaches the WIDE width. That arm is unreachable from a
                // verified module and reproducing it would require emitting a
                // branch for a case that cannot occur, so it is refused.
                if u32::from(*frac_bits) >= 2 * WORD_BITS as u32 {
                    return Err(LowerError::unsupported_op(
                        "WordToFixed",
                        format!(
                            "WordToFixed({frac_bits}) has a fraction count at or beyond the \
                         wide width; the VM treats this as corrupt input"
                        ),
                    ));
                }
                let v = st.pop();
                // Widen, shift, CLAMP, narrow. The VM saturates at the word's
                // MAX/MIN rather than wrapping, so the clamp is the semantics
                // and not a safety afterthought.
                let wide = st.widen(v, i128t, "w2fx.wide");
                let sh = i128t.const_int(u64::from(*frac_bits), false);
                let shifted = st.b.build_left_shift(wide, sh, "w2fx.shl").unwrap();
                // MAX/MIN sign-extended from their 64-bit forms, so the
                // constants cannot be got wrong by hand-writing 128-bit
                // literals.
                let max = st.widen(i64t.const_int(i64::MAX as u64, false), i128t, "w2fx.max");
                let min = st.widen(i64t.const_int(i64::MIN as u64, true), i128t, "w2fx.min");
                let over =
                    st.b.build_int_compare(inkwell::IntPredicate::SGT, shifted, max, "w2fx.over")
                        .unwrap();
                let under =
                    st.b.build_int_compare(inkwell::IntPredicate::SLT, shifted, min, "w2fx.under")
                        .unwrap();
                let clamped_hi =
                    st.b.build_select(over, max, shifted, "w2fx.selhi")
                        .unwrap()
                        .into_int_value();
                let clamped =
                    st.b.build_select(under, min, clamped_hi, "w2fx.sello")
                        .unwrap()
                        .into_int_value();
                st.push(st.b.build_int_truncate(clamped, i64t, "w2fx").unwrap());
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
                    LowerError::MalformedInput(format!("Const({idx}) out of range"))
                })?;
                // The constant's own variant states its packed width exactly,
                // which makes this the least ambiguous producer in the set.
                // `Unit` stays unknown: it is a placeholder that nothing reads,
                // and giving it a width would let it be packed into a body.
                // A static string is a REFERENCE, not a packable scalar, so it
                // cannot be produced as an `i64` literal like the others.
                // **A float constant is pushed as its BIT PATTERN**, tagged
                // `Float` so a later operation knows to bitcast rather than to
                // treat the bits as an integer. The stack stays homogeneous
                // `i64`; the tag is what carries the difference a width cannot.
                if let ConstValue::Float(f) = cv {
                    if float_bytes != 8 {
                        return Err(LowerError::UnsupportedShape(format!(
                            "a float constant at a float width of {float_bytes} bytes; \
                             only 8 is lowered, and another width is refused rather \
                             than approximated because a float of the wrong width is a \
                             silently wrong number, not a fault"
                        )));
                    }
                    let bits = i64::from_ne_bytes(f.to_ne_bytes());
                    let v = i64t.const_int(bits as u64, false);
                    st.push_k(v, Width::Scalar(8), OperandKind::Float);
                } else if let ConstValue::StaticStr(s) = cv {
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
                            return Err(LowerError::unsupported_op(
                                "Const",
                                format!("Const holding {other:?}"),
                            ));
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
                        return Err(LowerError::MalformedInput("PushImmediate(None)".into()));
                    }
                    n @ 4..=19 => (n as i64) - 4,
                    other => {
                        return Err(LowerError::MalformedInput(format!(
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
                    LowerError::Internal(format!(
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
                    return Err(LowerError::MalformedInput(format!(
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
                    // **A disjoint region block per CALL SITE**, not the caller's
                    // own base. Two calls to one callee otherwise write to the
                    // same offsets and the first result is destroyed by the
                    // second; `composite_return_aliasing.rs` pins that case.
                    let off = call_regions
                        .iter()
                        .find(|(ip, _)| *ip == i)
                        .map(|(_, o)| *o)
                        .unwrap_or(0);
                    if off == 0 {
                        args.push(rb.into());
                    } else {
                        let base = unsafe {
                            st.b.build_in_bounds_gep(
                                i8t,
                                rb,
                                &[i64t.const_int(u64::from(off), false)],
                                "callee_region",
                            )
                            .unwrap()
                        };
                        args.push(base.into());
                    }
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
                // A DELEGATED suspension. The callee returned at its `Yield`, so
                // this value is the yielded one and the host must see it now.
                // The `PopN(1)` and `Reset` that follow are the virtual
                // machine's bookkeeping for a frame this side has already left,
                // and the loop's `dead` tracking skips them.
                if delegated_call == Some(i) {
                    st.b.build_return(Some(&ret)).unwrap();
                } else {
                    // **SEEDED FROM THE CALLEE'S DECLARED RETURN**, which is
                    // exactly what the native arm does with `native_shapes`.
                    //
                    // Without this an `Op::Call` result was pushed at
                    // [`Width::Unknown`] unconditionally, so a composite packing
                    // a call result was refused for a width the module already
                    // declares. `Module::signatures` is the same table the typed
                    // verifier uses to seed a call's result.
                    //
                    // # This was written, reverted, and re-landed, on purpose
                    //
                    // It changes no corpus chunk — no shipped module packs a
                    // call result into a composite — so for one increment
                    // nothing in the tree could execute it, and it was reverted
                    // rather than shipped unverified. It returns with
                    // `module_source_differential.rs`, which runs a
                    // multi-function program written inline against the
                    // reference. **That test is refused for an unknown packed
                    // width without this line**, so it fails if this is removed.
                    //
                    // An undeclared or unsizable return still yields `Unknown`
                    // and still fails closed at the use.
                    let w = width_of_declared_shape(
                        chunk_signatures.get(usize::from(*idx)).map(|sg| &sg.ret),
                    );
                    st.push_w(ret, w);
                }
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
                    return Err(LowerError::UnsupportedShape(format!(
                        "native call #{idx} sets the B35 P7 error-reify flag \
                         (argument byte {n:#04x}), which reifies a soft host \
                         failure as a two-slot (code, flag) result; the two-slot \
                         form is not lowered"
                    )));
                }
                let argc = usize::from(n & 0x7F);
                let name = natives.get(usize::from(*idx)).ok_or_else(|| {
                    LowerError::MalformedInput(format!(
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
                            return Err(LowerError::MalformedInput(format!(
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
                // Take the declared return shape when the module carries one.
                // `Top`, an absent entry, and a shape this backend does not
                // model all fall back to `Width::Unknown`, which fails closed at
                // a USE rather than here — the behaviour every unsignatured
                // native still gets, and that is all of them in the shipped
                // corpus today.
                let w = width_of_declared_shape(native_shapes.get(usize::from(*idx)));
                st.push_w(ret, w);
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
            // **THE GENERIC ARITHMETIC SURFACE: `Byte` and `Fixed` only.**
            //
            // These were recorded for a long time as blocked on the operator's
            // float representation. **They were not.** `Op::Add` is emitted for
            // `Byte`, `Fixed` AND `Float` -- three types, where two separate
            // records each named two and disagreed. `Fixed` arithmetic is a
            // plain wrapping `i64` operation because the format is
            // scale-independent; `Byte` is the same operation plus a mask to
            // eight bits. **Only `Float` needs a representation this backend
            // lacks**, and the module-level guard now excludes it by every route
            // it can take, tested per route in `float_guard_routes.rs`.
            //
            // Semantics taken from the virtual machine, not assumed:
            // `binary_arith` computes a `Byte` in `i64` and masks with `& 0xFF`;
            // `Fixed` is `wrapping_add`/`wrapping_sub` on the underlying bits.
            // LLVM's `add`/`sub`/`mul` wrap by default (no `nsw`), which is the
            // matching behaviour.
            //
            // **WIDTH IS THE DISCRIMINATOR AND IT REFUSES WHEN UNSURE.** A
            // `GetLocal` carries a signature-derived width only where the chunk
            // never writes that local; anything written is `Unknown`. An unknown
            // or mismatched pair is REFUSED rather than guessed, which costs
            // coverage and cannot mispack -- the same trade the composite
            // packing already makes.
            // **The two conversions, which are what makes a float REACHABLE.**
            //
            // The operand stack is homogeneous `i64`, so a float lives on it as
            // its bit pattern and the `OperandKind` tag says so. `IntToFloat`
            // converts a real integer to a real double and stores the bits;
            // `FloatToInt` reads the bits back as a double and truncates toward
            // zero, which is what `as Word` means on the reference side.
            Op::IntToFloat | Op::FloatToInt => {
                if float_bytes != 8 {
                    return Err(LowerError::UnsupportedShape(format!(
                        "{op:?} at a float width of {float_bytes} bytes; only 8 is \
                         lowered, and another width is refused rather than \
                         approximated"
                    )));
                }
                let f64t = ctx.f64_type();
                // **Read the kind BEFORE popping.** `pop` lowers the depth and
                // `kind_at` is relative to it, so checking after the pop asks
                // about the wrong operand — or about an empty stack. The same
                // rule is written at `SetLocal`, and this arm broke it.
                let operand_kind = st.kind_at(0);
                let v = st.pop();
                if matches!(op, Op::IntToFloat) {
                    let f = st.b.build_signed_int_to_float(v, f64t, "sitofp").unwrap();
                    let bits =
                        st.b.build_bit_cast(f, i64t, "fbits")
                            .unwrap()
                            .into_int_value();
                    st.push_k(bits, Width::Scalar(8), OperandKind::Float);
                } else {
                    // **REFUSED unless the operand is KNOWN to be a float.** A
                    // bitcast of an integer's bits to a double is a silently
                    // wrong number, so an unknown or integer kind fails closed
                    // here rather than reinterpreting.
                    if operand_kind != OperandKind::Float {
                        return Err(LowerError::unsupported_op(
                            "FloatToInt",
                            format!(
                                "operand kind is {operand_kind:?}, not Float. \
                                 Reading an integer's bits as a double would be a \
                                 silently wrong number rather than a fault"
                            ),
                        ));
                    }
                    let f =
                        st.b.build_bit_cast(v, f64t, "asf")
                            .unwrap()
                            .into_float_value();
                    let i = st.b.build_float_to_signed_int(f, i64t, "fptosi").unwrap();
                    st.push_w(i, Width::Scalar(8));
                }
            }
            Op::Add | Op::Sub | Op::Mul => {
                // **FLOAT DISPATCH COMES FIRST, and it is decided by KIND.**
                // `Op::Add` is emitted for `Byte`, `Fixed` AND `Float`, and a
                // width cannot separate the third: a float and a word are both
                // eight bytes. If EITHER operand is a float, BOTH must be, or
                // the pair is refused — a mixed pair would mean bitcasting an
                // integer's bits to a double, which is a silently wrong number.
                let (kl, kr) = (st.kind_at(1), st.kind_at(0));
                if kl == OperandKind::Float || kr == OperandKind::Float {
                    if float_bytes != 8 {
                        return Err(LowerError::UnsupportedShape(format!(
                            "float arithmetic at a float width of {float_bytes} \
                             bytes; only 8 is lowered"
                        )));
                    }
                    if kl != OperandKind::Float || kr != OperandKind::Float {
                        return Err(LowerError::unsupported_op(
                            "Add",
                            format!(
                                "{op:?} with operand kinds {kl:?} and {kr:?}: one \
                                 side is a float and the other is not, so no \
                                 lowering is correct for both. Refused rather \
                                 than reinterpreted"
                            ),
                        ));
                    }
                    let f64t = ctx.f64_type();
                    let rhs_bits = st.pop();
                    let lhs_bits = st.pop();
                    let l =
                        st.b.build_bit_cast(lhs_bits, f64t, "lf")
                            .unwrap()
                            .into_float_value();
                    let r =
                        st.b.build_bit_cast(rhs_bits, f64t, "rf")
                            .unwrap()
                            .into_float_value();
                    let res = match op {
                        Op::Add => st.b.build_float_add(l, r, "fadd").unwrap(),
                        Op::Sub => st.b.build_float_sub(l, r, "fsub").unwrap(),
                        _ => st.b.build_float_mul(l, r, "fmul").unwrap(),
                    };
                    let bits =
                        st.b.build_bit_cast(res, i64t, "resbits")
                            .unwrap()
                            .into_int_value();
                    st.push_k(bits, Width::Scalar(8), OperandKind::Float);
                } else {
                    let (wl, wr) = (st.width_at(1), st.width_at(0));
                    // `Op::Mul` on `Fixed` does not exist: the compiler emits
                    // `Op::FixedMul(n)` for it. Admitting an eight-byte operand here
                    // would add a lowering arm for a case that cannot occur, which
                    // is untested code that looks tested.
                    let byte_only = matches!(op, Op::Mul);
                    let out = arith_result_width(wl, wr, byte_only).ok_or_else(|| {
                        LowerError::unsupported_op(
                            &op_variant_name(op),
                            format!(
                                "{op:?} with operand widths {wl:?} and {wr:?}: this backend \
                         lowers the generic arithmetic surface only for a matched \
                         Byte pair (1 byte each) or, for Add and Sub, a matched \
                         Fixed pair (8 bytes each). Refusing rather than guessing, \
                         since a wrong choice here is a silent wrong answer"
                            ),
                        )
                    })?;
                    let rhs = st.pop();
                    let lhs = st.pop();
                    let raw = match op {
                        Op::Add => st.b.build_int_add(lhs, rhs, "gadd").unwrap(),
                        Op::Sub => st.b.build_int_sub(lhs, rhs, "gsub").unwrap(),
                        _ => st.b.build_int_mul(lhs, rhs, "gmul").unwrap(),
                    };
                    st.push_w(mask_if_byte(&st.b, i64t, raw, out), out);
                }
            }
            // The unary half of the same surface. The virtual machine negates a
            // `Byte` with `u8::wrapping_neg`, which is `(-a) & 0xFF`, and a
            // `Fixed` with `i64::wrapping_neg`.
            Op::Neg => {
                let w = st.width_at(0);
                let out = arith_result_width(w, w, false).ok_or_else(|| {
                    LowerError::unsupported_op(
                        "Neg",
                        format!(
                            "Neg with operand width {w:?}: lowered only for a Byte (1 \
                         byte) or a Fixed (8 bytes) operand, and refused rather \
                         than guessed otherwise"
                        ),
                    )
                })?;
                let v = st.pop();
                let raw = st.b.build_int_neg(v, "gneg").unwrap();
                st.push_w(mask_if_byte(&st.b, i64t, raw, out), out);
            }
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
                    LowerError::Internal(
                        "NewComposite needs the region pointer, which lower_chunk does not \
                         receive"
                            .into(),
                    )
                })?;
                let plan = crate::region::plan_chunk_region(chunk);
                let site = plan.sites.iter().find(|s| s.op_index == i).ok_or_else(|| {
                    LowerError::Internal(format!(
                        "no region placement for the NewComposite at op {i}"
                    ))
                })?;

                // **THE ONE PLACEMENT THIS PASS MAKES THAT FAILS SILENTLY.**
                // The offset just chosen is fixed for the life of the chunk, so
                // a site inside a loop rewrites it every iteration. That is
                // sound for a value confined to its iteration and UNSOUND for
                // one the host receives by `yield`, because the host holds a
                // handle and an overwrite advances no epoch.
                //
                // Refused HERE, at the placement, rather than in a preflight, so
                // the next reader of this arm meets the constraint where the
                // decision is actually made.
                //
                // The refusal consumes an over-approximation and only ever
                // REFUSES on it. That is what keeps the recorded objection to
                // verdict-consuming placement intact: a wrong verdict here
                // rejects a sound program loudly, and never places a value
                // where the defect can occur.
                if let Some(h) = crate::region::yield_escape_hazards(chunk)
                    .into_iter()
                    .find(|h| h.site_op == i)
                {
                    return Err(LowerError::YieldEscapingLoopComposite {
                        site_op: h.site_op,
                        yield_op: h.yield_op,
                    });
                }

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
                        return Err(LowerError::unsupported_op(
                            "NewComposite",
                            format!(
                                "NewComposite at op {i} has an operand of unknown packed \
                             width: operand {} of {n} counting from the first, \
                             which is {back} back from the top of the stack",
                                n - back
                            ),
                        ));
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
                    return Err(LowerError::unsupported_op(
                        "NewComposite",
                        format!(
                            "NewComposite at op {i} packs {total} bytes but the instruction bakes \
                         {byte_size}; the layout model has drifted"
                        ),
                    ));
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
                            LowerError::unsupported_op(
                                "NewComposite",
                                format!(
                                    "NewComposite at op {i} could not copy a {n_bytes}-byte nested \
                                 body: {e}"
                                ),
                            )
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
                            return Err(LowerError::unsupported_op(
                                "NewComposite",
                                format!(
                                    "NewComposite at op {i} has a {other}-byte scalar field; \
                                 only 1 and 8 are lowered"
                                ),
                            ));
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
                    // **`Fixed` IS HANDLED EXACTLY LIKE `Int`, and that is
                    // measured rather than assumed.** A Q-format value is an
                    // `i64` of fixed-point bits occupying a full slot, and the
                    // reference packs `struct { a: Fixed<16>, b: Fixed<16> }` at
                    // sixteen bytes -- identical to a pair of `Word`s. THE BITS
                    // ARE THE VALUE: no zero-extension, no mask, no rescale.
                    // Scaling lives in the opcodes that consume them.
                    SK::Int | SK::Fixed => 8,
                    SK::Byte | SK::Bool => 1,
                    other => {
                        return Err(LowerError::unsupported_op(
                            "GetIndex",
                            format!("GetIndex reading {other:?} is not lowered"),
                        ));
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
                        return Err(LowerError::MalformedInput(format!(
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
                    // **`Fixed` READS EXACTLY LIKE `Int`.** Eight raw bytes of
                    // Q-format bits, unaligned like every other body access. The
                    // bits ARE the value -- do not zero-extend, mask, or rescale
                    // them; the consuming opcode knows the scale.
                    //
                    // It fell to the catch-all until 2026-08-21, one increment
                    // after the operand WIDTH was widened. Packing a field and
                    // reading it back are separate arms, and only packing
                    // followed from the width.
                    SK::Int | SK::Fixed => {
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
                        return Err(LowerError::unsupported_op(
                            "GetField",
                            format!("GetField reading {other:?} is not lowered"),
                        ));
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
            other => {
                return Err(LowerError::unsupported_op(
                    &op_variant_name(other),
                    String::new(),
                ));
            }
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

#[cfg(test)]
mod certification_tests {
    use super::*;
    use keleusma::bytecode::{NewCompositeOperand, Op};
    use keleusma::value_layout::CompositeKind;

    /// A chunk carrying only what the pre-pass reads, built from a real compile
    /// so its other fields stay whatever the compiler considers valid rather
    /// than being invented here and drifting from it.
    fn chunk_with(ops: Vec<Op>) -> Chunk {
        let src = "fn main(v: Word) -> Word { v + 1 }";
        let m = keleusma::compiler::compile(
            &keleusma::parser::parse(&keleusma::lexer::tokenize(src).expect("lex")).expect("parse"),
        )
        .expect("compile");
        let mut c = m.chunks.into_iter().next().expect("one chunk");
        c.ops = ops;
        c
    }

    /// The accepting case: two writes, both from sources whose width is fixed by
    /// the instruction, and both the same.
    ///
    /// **`Const(0)` is an `Int` in the chunk this borrows**, which is what makes
    /// it a word; the assertion below would be meaningless if it were not.
    #[test]
    fn two_agreeing_writes_certify_the_local() {
        let c = chunk_with(vec![
            Op::Const(0),
            Op::SetLocal(0),
            Op::GetLocal(0),
            Op::Const(0),
            Op::CheckedAdd,
            Op::PopN(2),
            Op::SetLocal(0),
            Op::Return,
        ]);
        assert!(
            matches!(
                c.constants.first(),
                Some(keleusma::bytecode::ConstValue::Int(_))
            ),
            "this subject needs an Int constant to be about word widths at all"
        );
        let certified = certified_local_widths(&c);
        assert_eq!(certified.get(&0), Some(&Width::Scalar(8)));
    }

    /// **THE REFUSING CASE.** One write comes from a `GetLocal`, whose width is a
    /// property of the operand rather than of the instruction, so the local is
    /// not certified at all — rather than the unclassifiable write being ignored
    /// as though the others agreed.
    ///
    /// Exercised here rather than from source because **no program this line can
    /// currently write reaches it**: locals written more than once are, in the
    /// shipped corpus and in every source form tried, loop counters, and those
    /// are always a constant plus an arithmetic result. That is a fact about the
    /// compiler's output, not a reason to leave the path untested.
    #[test]
    fn one_unclassifiable_write_sinks_the_local() {
        let c = chunk_with(vec![
            Op::Const(0),
            Op::SetLocal(0),
            Op::GetLocal(1),
            Op::SetLocal(0),
            Op::Return,
        ]);
        assert_eq!(
            certified_local_widths(&c).get(&0),
            None,
            "a local with an unclassifiable write must not be certified"
        );
    }

    /// A multi-push instruction is distinguished by WHICH push is taken. The
    /// flag slot of a checked triple is not the arithmetic result, and
    /// certifying on the instruction alone would label it a word.
    #[test]
    fn the_flag_slot_of_a_triple_is_not_certified_as_a_word() {
        assert_eq!(
            instruction_fixed_width(&chunk_with(vec![Op::CheckedAdd]), 0, 0),
            Some(Width::Scalar(8)),
            "push 0 of a checked triple is the arithmetic result"
        );
        assert_eq!(
            instruction_fixed_width(&chunk_with(vec![Op::CheckedAdd]), 0, 2),
            None,
            "push 2 of a checked triple is the overflow flag, not a word"
        );
    }

    /// A composite construction is not a fixed-width source: nothing about the
    /// instruction alone says how a local holding one should be packed.
    #[test]
    fn a_composite_construction_is_not_a_fixed_width_source() {
        let c = chunk_with(vec![Op::NewComposite(NewCompositeOperand::Flat {
            kind: CompositeKind::Struct,
            count: 0,
            byte_size: 8,
        })]);
        assert_eq!(instruction_fixed_width(&c, 0, 0), None);
    }
}

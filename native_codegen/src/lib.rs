//! Lowering of verified Keleusma bytecode to LLVM IR (V0.3.x Workstream A).
//!
//! # Scope
//!
//! This is an early subset: 35 of the instruction set's 66 opcodes. Everything
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

use inkwell::IntPredicate;
use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module as LlvmModule;
use inkwell::types::IntType;
use inkwell::values::{FunctionValue, IntValue, PointerValue};
use keleusma::bytecode::{Chunk, ConstValue, Op};
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
    /// An opcode outside the currently supported subset.
    UnsupportedOp(String),
    /// The module declares a word width this backend does not lower.
    UnsupportedWordWidth(u8),
    /// The chunk's operand stack is deeper than [`MAX_STACK`] provisions for.
    ///
    /// A refusal rather than a panic. The verifier already computes the exact
    /// figure as `RuntimeFootprint::max_operand_slots`, so a caller that wants
    /// to lower such a chunk can raise the provisioning deliberately instead of
    /// discovering the ceiling through a crash.
    OperandStackTooDeep { needed: usize, provisioned: usize },
}

impl core::fmt::Display for LowerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LowerError::UnsupportedOp(op) => {
                write!(f, "native lowering does not yet support opcode {op}")
            }
            LowerError::UnsupportedWordWidth(w) => {
                write!(f, "native lowering does not support word_bits_log2 = {w}")
            }
            LowerError::OperandStackTooDeep {
                needed,
                provisioned,
            } => write!(
                f,
                "chunk needs {needed} operand-stack slots, more than the {provisioned} \
                 this backend provisions"
            ),
        }
    }
}

impl std::error::Error for LowerError {}

struct Lower<'ctx> {
    b: Builder<'ctx>,
    i64t: IntType<'ctx>,
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
}

impl<'ctx> Lower<'ctx> {
    /// Index of the slot for `depth`, clamped so that an overflowing chunk
    /// keeps building valid-but-discarded IR instead of panicking. The clamp is
    /// only ever reached once `stack_overflow` is set, and the caller turns that
    /// into an error before the function is handed back.
    fn slot(&self, depth: usize) -> PointerValue<'ctx> {
        self.slots[depth.min(self.slots.len() - 1)]
    }

    fn push(&mut self, v: IntValue<'ctx>) {
        if self.depth >= self.slots.len() {
            self.stack_overflow.get_or_insert(self.depth);
        }
        let slot = self.slot(self.depth);
        self.b.build_store(slot, v).unwrap();
        self.depth += 1;
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
        self.push(low);
        self.push(high);
        self.push(flag);
    }
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
    let i128t = ctx.i128_type();

    let params: Vec<_> = (0..chunk.param_count).map(|_| i64t.into()).collect();
    let func = module.add_function(sym, i64t.fn_type(&params, false), None);

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
    let slots: Vec<_> = (0..MAX_STACK)
        .map(|i| b.build_alloca(i64t, &format!("s{i}")).unwrap())
        .collect();

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

    let mut st = Lower {
        b,
        i64t,
        locals,
        slots,
        depth: 0,
        stack_overflow: None,
    };

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

        match op {
            Op::GetLocal(n) => {
                let v =
                    st.b.build_load(i64t, st.locals[*n as usize], "gl")
                        .unwrap()
                        .into_int_value();
                st.push(v);
            }
            Op::SetLocal(n) => {
                let v = st.pop();
                st.b.build_store(st.locals[*n as usize], v).unwrap();
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
                let v: i64 = match cv {
                    ConstValue::Int(i) => *i,
                    ConstValue::Byte(x) => *x as i64,
                    ConstValue::Bool(b) => *b as i64,
                    // Unit is pushed and popped without being read. Same
                    // placeholder, same caveat, as `PushImmediate(0)`.
                    ConstValue::Unit => 0,
                    other => {
                        return Err(LowerError::UnsupportedOp(format!(
                            "Const holding {other:?}"
                        )));
                    }
                };
                let c = i64t.const_int(v as u64, true);
                st.push(c);
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
            Op::Return => {
                let v = st.pop();
                st.b.build_return(Some(&v)).unwrap();
            }
            other => return Err(LowerError::UnsupportedOp(format!("{other:?}"))),
        }
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

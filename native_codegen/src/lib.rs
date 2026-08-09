//! Lowering of verified Keleusma bytecode to LLVM IR (V0.3.x Workstream A).
//!
//! # Scope
//!
//! This is an early subset: 22 of the instruction set's 66 opcodes. Everything
//! else is refused rather than lowered to something plausible and wrong.
//! Widening the subset is the work of subsequent increments, tracked in
//! `docs/decisions/NATIVE_LOWERING_INVENTORY.md`.
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
//! `mem2reg` performs the SSA construction at any optimisation level above
//! none. This trades verbose IR at `-O0` for a whole class of absent bugs.
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
use keleusma::bytecode::{Chunk, Op};
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
}

impl<'ctx> Lower<'ctx> {
    fn push(&mut self, v: IntValue<'ctx>) {
        self.b.build_store(self.slots[self.depth], v).unwrap();
        self.depth += 1;
    }

    fn pop(&mut self) -> IntValue<'ctx> {
        self.depth -= 1;
        self.b
            .build_load(self.i64t, self.slots[self.depth], "pop")
            .unwrap()
            .into_int_value()
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
    let mut targets: Vec<usize> = Vec::new();
    for op in chunk.ops.iter() {
        if let Op::If(t) | Op::Else(t) = op {
            targets.push(*t as usize);
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

    for (i, op) in chunk.ops.iter().enumerate() {
        if let Some(&bb) = blocks.get(&i) {
            if st.b.get_insert_block().unwrap().get_terminator().is_none() {
                note!(i, st.depth);
                st.b.build_unconditional_branch(bb).unwrap();
            }
            st.b.position_at_end(bb);
            st.depth = *tdepth
                .get(&i)
                .expect("merge block with no recorded incoming edge");
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
            Op::CheckedAdd => {
                let rhs = st.pop();
                let lhs = st.pop();
                let a = st.b.build_int_s_extend(lhs, i128t, "a128").unwrap();
                let c = st.b.build_int_s_extend(rhs, i128t, "b128").unwrap();
                let sum = st.b.build_int_add(a, c, "s128").unwrap();

                let low = st.b.build_int_truncate(sum, i64t, "low").unwrap();
                let sh =
                    st.b.build_right_shift(sum, i128t.const_int(64, false), true, "sh")
                        .unwrap();
                let high = st.b.build_int_truncate(sh, i64t, "high").unwrap();

                let maxv = i128t.const_int(i64::MAX as u64, false);
                let minv =
                    st.b.build_int_s_extend(i64t.const_int(i64::MIN as u64, true), i128t, "min")
                        .unwrap();
                let ov =
                    st.b.build_int_compare(IntPredicate::SGT, sum, maxv, "ov")
                        .unwrap();
                let un =
                    st.b.build_int_compare(IntPredicate::SLT, sum, minv, "un")
                        .unwrap();
                let f2 =
                    st.b.build_select(un, i64t.const_int(2, false), i64t.const_zero(), "f2")
                        .unwrap()
                        .into_int_value();
                let flag =
                    st.b.build_select(ov, i64t.const_int(1, false), f2, "flag")
                        .unwrap()
                        .into_int_value();

                if opts.overflow == OverflowPolicy::Trap {
                    let cont = ctx.append_basic_block(func, "nooverflow");
                    let bad =
                        st.b.build_int_compare(IntPredicate::NE, flag, i64t.const_zero(), "bad")
                            .unwrap();
                    st.b.build_conditional_branch(bad, trap_bb, cont).unwrap();
                    st.b.position_at_end(cont);
                }

                // Push order is low, high, flag, matching the VM at
                // `src/vm.rs`. It is load-bearing: `CheckedAdd; PopN(2)`
                // discards flag and high specifically so that low survives as
                // an uncaptured expression's value. Verified by execution, not
                // taken from the opcode's doc comment, which was wrong about
                // this until 2026-08-08.
                st.push(low);
                st.push(high);
                st.push(flag);
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

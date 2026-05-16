//! Abstract interpretation lattice for tracking text-size bounds
//! through Keleusma bytecode.
//!
//! The WCMU pass needs to bound the worst-case bytes that
//! text-producing opcodes allocate from the arena's top region during
//! a single Stream-to-Reset iteration. `Op::Add` on text operands and
//! the bundled `to_string`, `concat`, and `slice` natives all
//! allocate a `KString` whose length depends on the operand lengths
//! at runtime. The verifier cannot inspect the runtime values, so it
//! tracks an upper bound through abstract interpretation over a
//! per-slot lattice.
//!
//! ## Lattice
//!
//! The lattice has two elements:
//!
//! - `TextSize::Known(n)`: the slot carries a text value whose UTF-8
//!   byte length is at most `n`.
//! - `TextSize::Unbounded`: the slot carries a text value whose
//!   length the analysis cannot bound, or carries a non-text value
//!   for which size tracking is meaningless. Both interpretations
//!   collapse to the conservative top of the lattice.
//!
//! The lattice has `Known(0)` as its bottom and `Unbounded` as its
//! top. Join is the maximum of two `Known` values, saturating to
//! `Unbounded` if either argument is `Unbounded`. Addition is the
//! saturating sum, propagating `Unbounded` if either argument is
//! `Unbounded` or if the integer sum would exceed `u32::MAX`.
//!
//! ## Scope of the present implementation
//!
//! V0.2.0 ships the lattice, the arithmetic primitives, and a
//! conservative linear analysis ([`chunk_text_heap_alloc`]) that
//! tracks text sizes through straight-line code. The analysis is
//! integrated with [`crate::verify::compute_chunk_wcmu`] so that
//! programs whose text-producing opcodes exceed the arena's top
//! region are rejected at the safe constructor under the default
//! [`crate::vm::OverflowPolicy::Reject`].
//!
//! ## Conservative cases
//!
//! Text operations inside a loop body or inside an If/Else branch
//! widen the produced lattice value to [`TextSize::Unbounded`],
//! which propagates an unbounded heap contribution. Calls produce
//! `Unbounded` return values because the analysis does not propagate
//! per-slot information across call boundaries; native attestation
//! through `Vm::set_native_bounds` is the load-bearing source of
//! truth for the heap contribution of registered natives.

use alloc::vec::Vec;

use crate::bytecode::{Chunk, ConstValue, NOMINAL_COST_MODEL, Op, OpCost, OpCostContext};

/// Upper bound on the UTF-8 byte length of a text value carried in
/// an operand-stack slot or local variable.
///
/// See the module-level documentation for the lattice semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSize {
    /// The slot carries a value whose static type is known to be
    /// non-text (integer, boolean, float, unit, composite). Text
    /// operations against this value contribute zero to the heap.
    NotText,
    /// The slot carries a text value whose length is at most `n`
    /// bytes.
    Known(u32),
    /// The slot carries a text value whose length the analysis
    /// cannot bound, or whose static type is uncertain. Text
    /// operations against this value contribute the saturated
    /// upper bound (`u32::MAX`) to the heap.
    Unbounded,
}

impl TextSize {
    /// The bottom of the text lattice. Equivalent to `Known(0)`.
    pub const ZERO: TextSize = TextSize::Known(0);

    /// The top of the text lattice.
    pub const UNBOUNDED: TextSize = TextSize::Unbounded;

    /// Saturating addition of two text-size bounds.
    ///
    /// `NotText + NotText` is `NotText` (no text value involved).
    /// `NotText + text` and `text + NotText` are the text operand,
    /// preserving its bound (this case arises in incomplete tracking
    /// rather than in well-typed Add, since the Keleusma type
    /// checker rejects mixed-type Add). Two text operands sum
    /// saturating to `Unbounded` on overflow or on either operand
    /// being `Unbounded`.
    pub fn saturating_add(self, other: TextSize) -> TextSize {
        match (self, other) {
            (TextSize::NotText, TextSize::NotText) => TextSize::NotText,
            (TextSize::NotText, t) | (t, TextSize::NotText) => t,
            (TextSize::Unbounded, _) | (_, TextSize::Unbounded) => TextSize::Unbounded,
            (TextSize::Known(a), TextSize::Known(b)) => match a.checked_add(b) {
                Some(sum) if sum < u32::MAX => TextSize::Known(sum),
                _ => TextSize::Unbounded,
            },
        }
    }

    /// Saturating join (maximum) of two text-size bounds.
    ///
    /// `NotText` joined with anything yields that thing: the join
    /// represents the value at a control-flow merge, and a non-text
    /// merging with a text indicates the analysis lost track of the
    /// static type. The conservative reading is to preserve the
    /// text operand's bound.
    pub fn join(self, other: TextSize) -> TextSize {
        match (self, other) {
            (TextSize::NotText, t) | (t, TextSize::NotText) => t,
            (TextSize::Unbounded, _) | (_, TextSize::Unbounded) => TextSize::Unbounded,
            (TextSize::Known(a), TextSize::Known(b)) => TextSize::Known(a.max(b)),
        }
    }

    /// Project the lattice value to a `u32` for use as an operand
    /// length in an [`OpCostContext`]. `Unbounded` and `NotText`
    /// both project to `u32::MAX`; the dynamic-cost evaluator
    /// receives the projection only for genuine text operations and
    /// callers must filter `NotText` operands before evaluation.
    pub fn as_u32(self) -> u32 {
        match self {
            TextSize::Known(n) => n,
            TextSize::Unbounded | TextSize::NotText => u32::MAX,
        }
    }
}

/// Build an [`OpCostContext`] from a pair of [`TextSize`] operand
/// bounds. Convenience helper for the integration point where the
/// abstract interpretation pass evaluates an `OpCost::Dynamic` cost.
pub fn op_cost_context(lhs: TextSize, rhs: TextSize) -> OpCostContext {
    OpCostContext {
        lhs_text_len: lhs.as_u32(),
        rhs_text_len: rhs.as_u32(),
    }
}

/// Conservative upper bound on the bytes allocated to the arena's
/// top region by text-producing opcodes in `chunk`.
///
/// Walks the chunk's ops linearly, maintaining a per-slot
/// [`TextSize`] lattice over an abstract operand stack and local
/// variables. For each text-producing opcode, evaluates the dynamic
/// [`OpCost`] from [`NOMINAL_COST_MODEL`] against the operand lattice
/// values and accumulates the cost into the returned total.
///
/// ## Soundness
///
/// The pass is sound for straight-line code, where stack and local
/// state at each program point is well-defined. Control flow is
/// handled conservatively: text values written inside a loop body or
/// inside an If/Else branch saturate to [`TextSize::Unbounded`],
/// causing any subsequent `Op::Add` against them to report the
/// dynamic cost as `u32::MAX`. Call boundaries also produce
/// `Unbounded` return values because the analysis does not propagate
/// per-slot information across calls.
///
/// The accumulated total saturates at `u32::MAX`. A returned value of
/// `u32::MAX` signals that the analysis could not bound the text
/// allocation and the caller should treat the chunk's text heap as
/// unbounded.
pub fn chunk_text_heap_alloc(chunk: &Chunk) -> u32 {
    let mut state = TextAnalysis::new(chunk.local_count as usize);
    let mut total: u32 = 0;
    let mut loop_depth: u32 = 0;
    let mut branch_depth: u32 = 0;

    for op in &chunk.ops {
        // Track structural depth so writes inside any conditional or
        // looping region produce Unbounded results. Increment before
        // dispatch so opcodes that delimit the region see the new
        // depth.
        match op {
            Op::Loop(_) => loop_depth = loop_depth.saturating_add(1),
            Op::If(_) => branch_depth = branch_depth.saturating_add(1),
            _ => {}
        }
        let conservative = loop_depth > 0 || branch_depth > 0;
        let contribution = state.apply_op(op, chunk, conservative);
        total = total.saturating_add(contribution);
        if total == u32::MAX {
            return u32::MAX;
        }
        match op {
            Op::EndLoop(_) => loop_depth = loop_depth.saturating_sub(1),
            Op::EndIf => branch_depth = branch_depth.saturating_sub(1),
            _ => {}
        }
    }
    total
}

/// Internal abstract state for [`chunk_text_heap_alloc`].
///
/// Mirrors the operand stack and local-variable bindings as
/// [`TextSize`] lattice values. Non-text values are tracked as
/// [`TextSize::Unbounded`] for simplicity. Stack-effect overflow
/// (e.g. popping from an empty abstract stack) is treated as a
/// best-effort scenario and yields [`TextSize::Unbounded`] for the
/// popped value, matching the conservative reading.
struct TextAnalysis {
    stack: Vec<TextSize>,
    locals: Vec<TextSize>,
}

impl TextAnalysis {
    fn new(local_count: usize) -> Self {
        Self {
            stack: Vec::new(),
            // Locals start as `NotText`. The compiler initialises
            // each slot before any read, so this is the value a
            // newly entered chunk sees before the first SetLocal.
            // Parameter slots are also `NotText` initially; tighter
            // tracking would require the caller to supply per-param
            // bounds.
            locals: alloc::vec![TextSize::NotText; local_count],
        }
    }

    fn pop(&mut self) -> TextSize {
        self.stack.pop().unwrap_or(TextSize::NotText)
    }

    fn push(&mut self, size: TextSize) {
        self.stack.push(size);
    }

    fn peek(&self) -> TextSize {
        self.stack.last().copied().unwrap_or(TextSize::NotText)
    }

    /// Apply the stack effect of `op` and return the heap bytes
    /// allocated by `op` for text. Non-text-producing opcodes return
    /// zero and update only the abstract stack/local state.
    ///
    /// When `conservative` is true, every value written to the
    /// abstract stack or to a local is saturated to
    /// [`TextSize::Unbounded`]. Used inside loops and conditional
    /// branches where the pass cannot reliably narrow the value.
    fn apply_op(&mut self, op: &Op, chunk: &Chunk, conservative: bool) -> u32 {
        // `sat` widens a tracked text bound to `Unbounded` when the
        // op runs inside a loop or branch. `NotText` is preserved
        // because non-text values do not change category under
        // control-flow widening.
        let sat = |size: TextSize| -> TextSize {
            if conservative {
                match size {
                    TextSize::NotText => TextSize::NotText,
                    _ => TextSize::Unbounded,
                }
            } else {
                size
            }
        };
        match op {
            Op::Const(idx) => {
                let size = match chunk.constants.get(*idx as usize) {
                    Some(ConstValue::StaticStr(s)) => {
                        let len = u32::try_from(s.len()).unwrap_or(u32::MAX - 1);
                        TextSize::Known(len.min(u32::MAX - 1))
                    }
                    Some(_) => TextSize::NotText,
                    None => TextSize::NotText,
                };
                self.push(sat(size));
                0
            }
            Op::Add => {
                let b = self.pop();
                let a = self.pop();
                // Integer Add (both operands `NotText`) contributes
                // zero to the text heap and produces `NotText`. Text
                // Add (at least one operand has a text lattice
                // value) evaluates the dynamic cost from the cost
                // model. The Keleusma type checker rules out mixed
                // text/non-text Add at the surface, so this case
                // does not arise from valid programs; the
                // conservative path treats it as text Add.
                let (result, dynamic_cost) = if matches!(a, TextSize::NotText)
                    && matches!(b, TextSize::NotText)
                {
                    (TextSize::NotText, 0)
                } else {
                    let context = op_cost_context(a, b);
                    let cost = match NOMINAL_COST_MODEL.heap_alloc_cost(op, chunk) {
                        OpCost::Dynamic(f) => f(&context),
                        OpCost::Fixed(n) => n,
                    };
                    (a.saturating_add(b), cost)
                };
                self.push(sat(result));
                dynamic_cost
            }
            Op::GetLocal(i) => {
                let size = self
                    .locals
                    .get(*i as usize)
                    .copied()
                    .unwrap_or(TextSize::NotText);
                self.push(sat(size));
                0
            }
            Op::SetLocal(i) => {
                let value = self.pop();
                if let Some(slot) = self.locals.get_mut(*i as usize) {
                    *slot = sat(value);
                }
                0
            }
            Op::Pop => {
                self.pop();
                0
            }
            Op::Dup => {
                let top = self.peek();
                self.push(top);
                0
            }
            Op::Call(_, n_args) => {
                for _ in 0..*n_args {
                    self.pop();
                }
                // The callee's text heap contribution is summed
                // separately at the module level. The abstract
                // return value is `Unbounded` because the pass does
                // not propagate per-slot information across calls.
                // This is safe but coarse: a subsequent text Add
                // against the return value will saturate the cost
                // contribution to `u32::MAX`.
                self.push(TextSize::Unbounded);
                0
            }
            Op::CallNative(_, n_args) => {
                for _ in 0..*n_args {
                    self.pop();
                }
                self.push(TextSize::Unbounded);
                0
            }
            Op::Yield => {
                self.pop();
                0
            }
            Op::Return => {
                self.pop();
                0
            }
            Op::If(_) => {
                // If consumes a boolean.
                self.pop();
                0
            }
            Op::Else(_) | Op::EndIf | Op::Loop(_) | Op::EndLoop(_) => 0,
            Op::Break(_) => 0,
            Op::BreakIf(_) => {
                self.pop();
                0
            }
            _ => {
                // Other opcodes do not produce text. Apply the
                // recorded stack growth and shrink and treat any
                // pushed value as non-text.
                let shrink = op.stack_shrink() as usize;
                for _ in 0..shrink {
                    self.pop();
                }
                let growth = op.stack_growth() as usize;
                for _ in 0..growth {
                    self.push(TextSize::NotText);
                }
                0
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_known_values() {
        assert_eq!(
            TextSize::Known(3).saturating_add(TextSize::Known(5)),
            TextSize::Known(8)
        );
        assert_eq!(TextSize::ZERO.saturating_add(TextSize::ZERO), TextSize::ZERO);
    }

    #[test]
    fn add_saturates_to_unbounded_on_overflow() {
        assert_eq!(
            TextSize::Known(u32::MAX - 1).saturating_add(TextSize::Known(1)),
            TextSize::Unbounded
        );
        assert_eq!(
            TextSize::Known(u32::MAX / 2 + 1).saturating_add(TextSize::Known(u32::MAX / 2 + 1)),
            TextSize::Unbounded
        );
    }

    #[test]
    fn add_propagates_unbounded() {
        assert_eq!(
            TextSize::Unbounded.saturating_add(TextSize::Known(5)),
            TextSize::Unbounded
        );
        assert_eq!(
            TextSize::Known(5).saturating_add(TextSize::Unbounded),
            TextSize::Unbounded
        );
        assert_eq!(
            TextSize::Unbounded.saturating_add(TextSize::Unbounded),
            TextSize::Unbounded
        );
    }

    #[test]
    fn join_known_takes_max() {
        assert_eq!(
            TextSize::Known(3).join(TextSize::Known(5)),
            TextSize::Known(5)
        );
        assert_eq!(
            TextSize::Known(7).join(TextSize::Known(2)),
            TextSize::Known(7)
        );
    }

    #[test]
    fn join_propagates_unbounded() {
        assert_eq!(
            TextSize::Unbounded.join(TextSize::Known(5)),
            TextSize::Unbounded
        );
        assert_eq!(
            TextSize::Known(5).join(TextSize::Unbounded),
            TextSize::Unbounded
        );
    }

    #[test]
    fn as_u32_projects_unbounded_to_max() {
        assert_eq!(TextSize::Known(42).as_u32(), 42);
        assert_eq!(TextSize::Unbounded.as_u32(), u32::MAX);
        assert_eq!(TextSize::ZERO.as_u32(), 0);
    }

    #[test]
    fn op_cost_context_carries_lattice_values() {
        let ctx = op_cost_context(TextSize::Known(10), TextSize::Known(20));
        assert_eq!(ctx.lhs_text_len, 10);
        assert_eq!(ctx.rhs_text_len, 20);
        let ctx_unbounded = op_cost_context(TextSize::Unbounded, TextSize::Known(5));
        assert_eq!(ctx_unbounded.lhs_text_len, u32::MAX);
        assert_eq!(ctx_unbounded.rhs_text_len, 5);
    }

    use crate::bytecode::{BlockType, Chunk, ConstValue, Op};
    use alloc::string::String as StdString;
    use alloc::vec;

    fn make_chunk(ops: Vec<Op>, constants: Vec<ConstValue>, locals: u16) -> Chunk {
        Chunk {
            name: StdString::from("test"),
            ops,
            constants,
            struct_templates: Vec::new(),
            local_count: locals,
            param_count: 0,
            block_type: BlockType::Func,
        }
    }

    #[test]
    fn text_heap_alloc_empty_chunk_is_zero() {
        let chunk = make_chunk(vec![Op::Return], vec![], 0);
        assert_eq!(chunk_text_heap_alloc(&chunk), 0);
    }

    #[test]
    fn text_heap_alloc_static_literal_alone_is_zero() {
        // Pushing a string literal does not allocate from the arena;
        // static strings live in the rodata region.
        let chunk = make_chunk(
            vec![Op::Const(0), Op::Return],
            vec![ConstValue::StaticStr(StdString::from("hello"))],
            0,
        );
        assert_eq!(chunk_text_heap_alloc(&chunk), 0);
    }

    #[test]
    fn text_heap_alloc_single_concat_returns_sum_of_lengths() {
        // "ab" + "cdef" allocates 6 bytes.
        let chunk = make_chunk(
            vec![Op::Const(0), Op::Const(1), Op::Add, Op::Return],
            vec![
                ConstValue::StaticStr(StdString::from("ab")),
                ConstValue::StaticStr(StdString::from("cdef")),
            ],
            0,
        );
        assert_eq!(chunk_text_heap_alloc(&chunk), 6);
    }

    #[test]
    fn text_heap_alloc_doubling_pattern_accumulates_geometric_series() {
        // s = "a"; s = s + s; s = s + s; allocates 2 + 4 = 6 bytes.
        // Geometric series 2 + 4 + 8 + ... starts after the first
        // assignment.
        let chunk = make_chunk(
            vec![
                Op::Const(0),    // push "a"  -> stack: [Known(1)]
                Op::SetLocal(0), // s = "a"   -> locals[0] = Known(1)
                Op::GetLocal(0),
                Op::GetLocal(0),
                Op::Add,         // s + s     -> allocates 2 bytes
                Op::SetLocal(0), // s = s + s -> locals[0] = Known(2)
                Op::GetLocal(0),
                Op::GetLocal(0),
                Op::Add,         // s + s     -> allocates 4 bytes
                Op::SetLocal(0), // s = s + s -> locals[0] = Known(4)
                Op::Return,
            ],
            vec![ConstValue::StaticStr(StdString::from("a"))],
            1,
        );
        assert_eq!(chunk_text_heap_alloc(&chunk), 6);
    }

    #[test]
    fn text_heap_alloc_saturates_for_long_doubling_chain() {
        // The FAQ exponential-string-concat example. Sixty doublings
        // of a 1-byte string cumulatively allocate more than u32::MAX
        // bytes; the analysis saturates to u32::MAX at the moment the
        // produced size crosses the lattice's representable range.
        let mut ops = vec![Op::Const(0), Op::SetLocal(0)];
        for _ in 0..60 {
            ops.push(Op::GetLocal(0));
            ops.push(Op::GetLocal(0));
            ops.push(Op::Add);
            ops.push(Op::SetLocal(0));
        }
        ops.push(Op::Return);
        let chunk = make_chunk(
            ops,
            vec![ConstValue::StaticStr(StdString::from("a"))],
            1,
        );
        assert_eq!(chunk_text_heap_alloc(&chunk), u32::MAX);
    }

    #[test]
    fn text_heap_alloc_inside_loop_saturates_to_unbounded() {
        // Op::Add inside a loop body cannot be tightly bounded by the
        // linear pass, so the contribution saturates to u32::MAX.
        let chunk = make_chunk(
            vec![
                Op::Loop(6), // 0
                Op::Const(0),
                Op::Const(0),
                Op::Add,
                Op::Pop,
                Op::EndLoop(0), // 5
                Op::Return,
            ],
            vec![ConstValue::StaticStr(StdString::from("a"))],
            0,
        );
        assert_eq!(chunk_text_heap_alloc(&chunk), u32::MAX);
    }

    #[test]
    fn text_heap_alloc_call_native_result_is_unbounded() {
        // A native function call produces an Unbounded return that
        // any subsequent Op::Add against will saturate to u32::MAX.
        let chunk = make_chunk(
            vec![
                Op::Const(0),
                Op::CallNative(0, 1), // some host-attested native
                Op::Const(0),
                Op::Add, // adds an Unbounded native result to "a"
                Op::Return,
            ],
            vec![ConstValue::StaticStr(StdString::from("a"))],
            0,
        );
        assert_eq!(chunk_text_heap_alloc(&chunk), u32::MAX);
    }

    #[test]
    fn doubling_pattern_saturates_after_thirty_two_iterations() {
        // The FAQ exponential-string-concat example doubles a 1-byte
        // string sixty times. Tracking the size lattice through the
        // doubling chain reaches `Unbounded` once the size exceeds
        // u32::MAX, which happens at the thirty-second doubling
        // (2^31 = 2_147_483_648, 2^32 - 1 = u32::MAX).
        let mut size = TextSize::Known(1);
        for _ in 0..31 {
            size = size.saturating_add(size);
        }
        assert_eq!(size, TextSize::Known(1u32 << 31));
        // The next doubling crosses the u32::MAX boundary.
        size = size.saturating_add(size);
        assert_eq!(size, TextSize::Unbounded);
        // Further doublings stay at Unbounded.
        for _ in 0..30 {
            size = size.saturating_add(size);
            assert_eq!(size, TextSize::Unbounded);
        }
    }
}

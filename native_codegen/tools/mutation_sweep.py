#!/usr/bin/env python3
"""Per-opcode mutation sweep of the native emitter.

WHAT THIS ANSWERS
-----------------
For each opcode the shipped corpus emits, would a defect in its lowering be
DETECTED by the corpus differential?  `NATIVE_MUTATION_CENSUS.md` answered that
for four hand-picked mutations and found one opcode -- `CmpLt` -- whose boundary
defect nothing caught.  Four samples is not a census.

METHOD, AND THE TWO THINGS THAT MAKE IT HONEST
----------------------------------------------
1.  **The mutation set is PRE-REGISTERED**, in `MUTATIONS` below, and was
    written and committed before any of it was run.  Choosing which opcodes to
    perturb after seeing which ones look safe turns a sweep into a
    demonstration.

2.  **Every module runs in its own PROCESS.**  Two of the first four mutations
    tried killed the whole test binary with SIGBUS and SIGTRAP, yielding no
    per-module data at all.  A signal IS detection, and it is recorded
    distinctly from a reported disagreement because the two mean different
    things: a disagreement is the harness noticing a wrong value, a signal is
    the module executing something invalid.

Each mutation runs only against the modules that actually EMIT the mutated
opcode, read from `dump_opcode_module_map`.  A module with no site for an opcode
cannot detect a defect in it, and counting it as "did not detect" would
understate the corpus.

OUTCOMES PER MODULE
-------------------
  AGREE      the differential passed -- this module did NOT detect the defect
  DISAGREE   the differential reported a mismatch -- detected
  SIGNAL     the process died on a signal -- detected, fatally
  HANG       the run did not terminate inside its CALIBRATED budget -- detected.
             A total-functional language whose whole value proposition is a
             definitive WCET bound does not get to loop forever, so
             non-termination is a real observation and not merely a timeout.
             The first attempt at this sweep had NO timeout and stalled twelve
             minutes on one module, because turning `CheckedAdd` into a
             subtraction stops a loop counter ever reaching its bound.

             **THE BUDGET IS MEASURED, AND A FIXED ONE PRODUCED FALSE
             POSITIVES.** With a flat 20s and a differential widened to 24
             seeds, `wire.kel` took 30.7s unmutated, exceeded the budget under
             every mutation, and was scored as detecting all of them. Four
             opcodes looked closed that were not. See `calibrate()`.
  NOLOWER    the module stopped lowering -- the mutation broke the emitter
             rather than changing its meaning, so it is not a semantic
             perturbation and the row is not evidence either way

USAGE
-----
    python3 tools/mutation_sweep.py            # the whole sweep
    python3 tools/mutation_sweep.py CmpLt Not  # named opcodes only
"""

import os
import shutil
import subprocess
import sys
import time
import tempfile

# A mutated module may simply not terminate; see HANG above.
#
# **This is a FLOOR, not the budget.** The budget is measured per module by
# `calibrate()`; see the note there for the false positive a fixed value caused.
PER_MODULE_TIMEOUT = 20
# How much slower than its healthy self a module may run before the sweep calls
# it non-terminating.
HANG_MULTIPLIER = 6
# A module that cannot finish unmutated inside this is already pathological.
CALIBRATION_CEILING = 300
BUILD_TIMEOUT = 900

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
LIB = os.path.join(ROOT, "src", "lib.rs")

# ---------------------------------------------------------------------------
# THE PRE-REGISTERED MUTATION SET.
#
# One entry per opcode with sites in the corpus.  Each is a semantic
# perturbation: it changes what the lowered code MEANS, not whether it compiles.
# `old` must occur exactly once in `src/lib.rs`; the driver asserts that and
# refuses to run a mutation it cannot place, so a silent no-op is impossible.
# ---------------------------------------------------------------------------
MUTATIONS = {
    # --- comparisons: predicate swaps, boundary and inversion -------------
    "CmpEq": ("Op::CmpEq => IntPredicate::EQ,", "Op::CmpEq => IntPredicate::NE,"),
    "CmpNe": ("Op::CmpNe => IntPredicate::NE,", "Op::CmpNe => IntPredicate::EQ,"),
    "CmpLt": ("Op::CmpLt => IntPredicate::SLT,", "Op::CmpLt => IntPredicate::SLE,"),
    "CmpGt": ("Op::CmpGt => IntPredicate::SGT,", "Op::CmpGt => IntPredicate::SGE,"),
    "CmpLe": ("Op::CmpLe => IntPredicate::SLE,", "Op::CmpLe => IntPredicate::SLT,"),
    "CmpGe": ("Op::CmpGe => IntPredicate::SGE,", "Op::CmpGe => IntPredicate::SGT,"),
    # --- arithmetic --------------------------------------------------------
    "CheckedAdd": (
        'Op::CheckedAdd => st.b.build_int_add(a, c, "s128").unwrap(),',
        'Op::CheckedAdd => st.b.build_int_sub(a, c, "s128").unwrap(),',
    ),
    "CheckedSub": (
        '_ => st.b.build_int_sub(a, c, "d128").unwrap(),',
        '_ => st.b.build_int_add(a, c, "d128").unwrap(),',
    ),
    "CheckedMul": (
        'let wide = st.b.build_int_mul(a, c, "p128").unwrap();',
        'let wide = st.b.build_int_add(a, c, "p128").unwrap();',
    ),
    "CheckedNeg": (
        'let wide = st.b.build_int_neg(a, "n128").unwrap();',
        'let wide = st.widen(v, i128t, "n128");',
    ),
    # --- bitwise and shifts ------------------------------------------------
    "BitAnd": (
        'Op::BitAnd => st.b.build_and(lhs, rhs, "band").unwrap(),',
        'Op::BitAnd => st.b.build_or(lhs, rhs, "band").unwrap(),',
    ),
    "BitOr": (
        'Op::BitOr => st.b.build_or(lhs, rhs, "bor").unwrap(),',
        'Op::BitOr => st.b.build_and(lhs, rhs, "bor").unwrap(),',
    ),
    "BitXor": (
        'Op::BitXor => st.b.build_xor(lhs, rhs, "bxor").unwrap(),',
        'Op::BitXor => st.b.build_and(lhs, rhs, "bxor").unwrap(),',
    ),
    "Shl": (
        'Op::Shl => st.b.build_left_shift(value, masked, "shl").unwrap(),',
        'Op::Shl => st.b.build_right_shift(value, masked, true, "shl").unwrap(),',
    ),
    "Shr": (
        'Op::Shr => st.b.build_right_shift(value, masked, true, "shr").unwrap(),',
        'Op::Shr => st.b.build_right_shift(value, masked, false, "shr").unwrap(),',
    ),
    "Not": (
        'st.b.build_int_compare(IntPredicate::EQ, v, i64t.const_zero(), "not")',
        'st.b.build_int_compare(IntPredicate::NE, v, i64t.const_zero(), "not")',
    ),
    # --- operand and local traffic ----------------------------------------
    # Both re-anchored 2026-09-08, when the direct `Vec` index became a bounds-
    # checked `get`: an out-of-range local index is now a REFUSAL rather than a
    # panic. The mutation still says the same thing -- read or write the WRONG
    # local -- and now says it on the index the lookup actually uses.
    "GetLocal": (
        'let Some(slot) = st.locals.get(*n as usize).copied() else {\n'
        '                    return Err(LowerError::MalformedInput(format!(\n'
        '                        "GetLocal names slot {n} in a chunk with {} locals",',
        'let Some(slot) = st.locals.get((*n as usize).saturating_sub(1)).copied() else {\n'
        '                    return Err(LowerError::MalformedInput(format!(\n'
        '                        "GetLocal names slot {n} in a chunk with {} locals",',
    ),
    "SetLocal": (
        'let Some(slot) = st.locals.get(*n as usize).copied() else {\n'
        '                    return Err(LowerError::MalformedInput(format!(\n'
        '                        "SetLocal names slot {n} in a chunk with {} locals",',
        'let Some(slot) = st.locals.get((*n as usize).saturating_sub(1)).copied() else {\n'
        '                    return Err(LowerError::MalformedInput(format!(\n'
        '                        "SetLocal names slot {n} in a chunk with {} locals",',
    ),
    "Dup": (
        "Op::Dup => {\n                let v = st.pop();\n                st.push(v);\n                st.push(v);",
        "Op::Dup => {\n                let v = st.pop();\n                st.push(v);\n                st.push(i64t.const_zero());",
    ),
    "Const": (
        "ConstValue::Int(i) => (*i, Width::Scalar(8)),",
        "ConstValue::Int(i) => (i.wrapping_add(1), Width::Scalar(8)),",
    ),
    "PushImmediate": ("                    1 => 1,", "                    1 => 2,"),
    # --- control flow with a real predicate -------------------------------
    "If": (
        'st.b.build_int_compare(IntPredicate::NE, c, i64t.const_zero(), "nz")',
        'st.b.build_int_compare(IntPredicate::EQ, c, i64t.const_zero(), "nz")',
    ),
    "BreakIf": (
        'st.b.build_int_compare(IntPredicate::NE, c, i64t.const_zero(), "brknz")',
        'st.b.build_int_compare(IntPredicate::EQ, c, i64t.const_zero(), "brknz")',
    ),
    "Return": (
        # **RE-REGISTERED 2026-09-06 AFTER GOING STALE, WHICH IS THE POINT.**
        #
        # The previous text expected `st.b.build_return(Some(&v))`. The emitter
        # was refactored to `build_typed_return(&st.b, func, v)` at some point
        # after 2026-08-16, so this mutation stopped PLACING and `Return` -- 52
        # sites -- contributed no coverage evidence at all until the 2026-09-06
        # re-run reported `UNPLACEABLE (0 matches)`.
        #
        # **The refusal is what caught it.** A mutation that does not place is a
        # silent no-op and looks exactly like "nothing detected it", so without
        # the placement check this would have read as a HOLE. A pre-registered
        # set drifts out of date with the code it mutates, silently.
        #
        # The replacement returns a constant zero instead of the popped value,
        # which is the same DISCRIMINATING shape the original had.
        "Op::Return => {\n                let v = st.pop();\n                build_typed_return(&st.b, func, v);",
        "Op::Return => {\n                let _v = st.pop();\n                build_typed_return(&st.b, func, i64t.const_zero().into());",
    ),
    # --- conversions -------------------------------------------------------
    "ByteToWord": (
        "Op::ByteToWord => st.set_top_width(Width::Scalar(8)),",
        "Op::ByteToWord => st.set_top_width(Width::Scalar(1)),",
    ),
}

# ---------------------------------------------------------------------------
# ROUND TWO: the DISCRIMINATING set, added after round one reported eight
# opcodes undetected.
#
# An "undetected" result has two very different causes and round one cannot tell
# them apart:
#
#   * a GENUINE HOLE -- the corpus never observes this opcode's contribution;
#   * an EQUIVALENT MUTANT -- the perturbation does not change behaviour, so
#     there was nothing to detect and the corpus is not at fault.
#
# `PushImmediate 1 => 2` is the clearest case of the second: booleans are
# consumed by `If`/`BreakIf`, which test `!= 0`, and 2 is exactly as truthy as 1.
#
# Each mutation below replaces the opcode's RESULT with a constant, which is the
# most observable change available. If a maximally destructive mutation is still
# undetected, the hole is real.
# ---------------------------------------------------------------------------
MUTATIONS_STRONG = {
    "BitAnd": (
        'Op::BitAnd => st.b.build_and(lhs, rhs, "band").unwrap(),',
        'Op::BitAnd => { let _ = (lhs, rhs); i64t.const_zero() },',
    ),
    "BitOr": (
        'Op::BitOr => st.b.build_or(lhs, rhs, "bor").unwrap(),',
        'Op::BitOr => { let _ = (lhs, rhs); i64t.const_zero() },',
    ),
    "Shl": (
        'Op::Shl => st.b.build_left_shift(value, masked, "shl").unwrap(),',
        'Op::Shl => { let _ = (value, masked); i64t.const_zero() },',
    ),
    "Shr": (
        'Op::Shr => st.b.build_right_shift(value, masked, true, "shr").unwrap(),',
        'Op::Shr => { let _ = (value, masked); i64t.const_zero() },',
    ),
    "CmpNe": (
        "let c = st.b.build_int_compare(pred, lhs, rhs, \"cmp\").unwrap();",
        "let c = st.b.build_int_compare(if matches!(op, Op::CmpNe) { IntPredicate::EQ } else { pred }, lhs, rhs, \"cmp\").unwrap();",
    ),
    "Dup": (
        "Op::Dup => {\n                let v = st.pop();\n                st.push(v);\n                st.push(v);",
        "Op::Dup => {\n                let v = st.pop();\n                let _ = v;\n                st.push(i64t.const_zero());\n                st.push(i64t.const_zero());",
    ),
    "PushImmediate": ("                    1 => 1,", "                    1 => 0,"),
}

# ---------------------------------------------------------------------------
# ROUND THREE: the 25 opcodes sweep one skipped -- the MEMORY AND COMPOSITE
# surface, where the only genuine codegen defect this line has found lived.
#
# Most were skipped because opcodes share an emitter arm and one swap could not
# be attributed. The fix is to GUARD the mutation on the opcode, the way round
# two guarded `CmpNe` with `matches!(op, Op::CmpNe)`. `GetData` and `SetData`
# share a path, so each is guarded on `is_read`.
#
# EVERY VARIANT HERE WAS CONFIRMED REACHABLE FIRST by
# `variant_distribution_of_the_skipped_opcodes`. `GetField(FlatNested)` has ZERO
# sites and is deliberately absent: mutating it would repeat the `PushImmediate`
# error, where the largest apparent hole was a mutation of an operand the corpus
# never emits.
# ---------------------------------------------------------------------------
MUTATIONS_ROUND3 = {
    # --- the shared data arm, split by direction and by indexing -----------
    # **RE-REGISTERED 2026-09-06.** The original anchored on a call to
    # `resolve_shared_scalar` before it gained `float_bytes` and the call site
    # reformatted across lines, so it had silently stopped placing. The
    # discriminating property is kept -- touch the WRONG BYTES of the shared
    # region -- by shifting the resolved offset, which leaves the module
    # lowerable so the mutation stays SEMANTIC rather than aborting lowering.
    "GetData": (
        "                        let (byte_off, w, k) =\n                            resolve_shared_scalar(&data, slot, i8t, i64t, float_bytes)?;",
        "                        let (byte_off, w, k) =\n                            resolve_shared_scalar(&data, slot, i8t, i64t, float_bytes)?;\n                        let byte_off = byte_off.wrapping_add(1);",
    ),
    # **RE-REGISTERED 2026-09-06.** The original anchored on a call to
    # `resolve_shared_scalar` before it gained `float_bytes` and the call site
    # reformatted across lines, so it had silently stopped placing. The
    # discriminating property is kept -- touch the WRONG BYTES of the shared
    # region -- by shifting the resolved offset, which leaves the module
    # lowerable so the mutation stays SEMANTIC rather than aborting lowering.
    "SetData": (
        "                        let (byte_off, w, k) =\n                            resolve_shared_scalar(&data, slot, i8t, i64t, float_bytes)?;",
        "                        let (byte_off, w, k) =\n                            resolve_shared_scalar(&data, slot, i8t, i64t, float_bytes)?;\n                        let byte_off = byte_off.wrapping_add(2);",
    ),
    # **RE-REGISTERED 2026-09-06.** The original anchored on a call to
    # `resolve_shared_array` before it gained `float_bytes` and the call site
    # reformatted across lines, so it had silently stopped placing. The
    # discriminating property is kept -- touch the WRONG BYTES of the shared
    # region -- by shifting the resolved offset, which leaves the module
    # lowerable so the mutation stays SEMANTIC rather than aborting lowering.
    "GetDataIndexed": (
        "                        let (first_off, w, k) =\n                            resolve_shared_array(&data, slot, bound, float_bytes)?;",
        "                        let (first_off, w, k) =\n                            resolve_shared_array(&data, slot, bound, float_bytes)?;\n                        let first_off = first_off.wrapping_add(1);",
    ),
    # **RE-REGISTERED 2026-09-06.** The original anchored on a call to
    # `resolve_shared_array` before it gained `float_bytes` and the call site
    # reformatted across lines, so it had silently stopped placing. The
    # discriminating property is kept -- touch the WRONG BYTES of the shared
    # region -- by shifting the resolved offset, which leaves the module
    # lowerable so the mutation stays SEMANTIC rather than aborting lowering.
    "SetDataIndexed": (
        "                        let (first_off, w, k) =\n                            resolve_shared_array(&data, slot, bound, float_bytes)?;",
        "                        let (first_off, w, k) =\n                            resolve_shared_array(&data, slot, bound, float_bytes)?;\n                        let first_off = first_off.wrapping_add(2);",
    ),
    # --- the division family, guarded so each attributes -------------------
    # **RE-REGISTERED 2026-09-06. THE SHAPE CHANGED, AND THAT IS RECORDED.**
    #
    # The original anchored on the arm's operand pops and swapped them. That text
    # no longer exists: the arm now pops once and routes through
    # `guard_min_div_neg_one`, so there is nothing to swap at that anchor. The
    # replacement keeps the property that MATTERS -- the opcode computes the wrong
    # arithmetic -- by swapping the OPERATION instead of the operands: `Div` emits
    # a remainder.
    #
    # **Not chosen because it looked likely to be caught.** It is the minimal
    # unique edit to the arm that still exists, and it is registered before being
    # run, as the pre-registration discipline requires.
    "Div": (
        'Op::Div => st.b.build_int_signed_div(lhs, safe, "sdiv").unwrap(),',
        'Op::Div => st.b.build_int_signed_rem(lhs, safe, "sdiv").unwrap(),',
    ),
    # Re-registered 2026-09-06 for the same reason as `Div` above, with the same
    # shape change: `Mod` emits a quotient.
    "Mod": (
        '_ => st.b.build_int_signed_rem(lhs, safe, "srem").unwrap(),',
        '_ => st.b.build_int_signed_div(lhs, safe, "srem").unwrap(),',
    ),
    # --- composites --------------------------------------------------------
    "NewComposite": (
                "                let mut off = site.offset;",
                "                let mut off = site.offset + 8;",
    ),
    "GetField": (
        '                        &[i64t.const_int(u64::from(*offset), false)],\n                        "cfaddr",',
        '                        &[i64t.const_int(u64::from(*offset) + 1, false)],\n                        "cfaddr",',
    ),
    "GetTupleField": (
        "                    TF::Flat { offset, kind } => SF::Flat {\n                        offset: *offset,",
        "                    TF::Flat { offset, kind } => SF::Flat {\n                        offset: *offset + 1,",
    ),
    "GetEnumField": (
        "                    EF::Flat { offset, kind } => SF::Flat {\n                        offset: *offset,",
        "                    EF::Flat { offset, kind } => SF::Flat {\n                        offset: *offset + 1,",
    ),
    # **RE-REGISTERED 2026-09-06.** The arm read `SK::Int => 8,` when this was
    # written; `Fixed` was folded into the same arm, so the anchor stopped
    # matching and `GetIndex` had silently lost its coverage. Shape unchanged: a
    # wrong element stride, which keeps the module lowerable so the mutation
    # stays SEMANTIC.
    "GetIndex": (
        # **A ONE-LINE ANCHOR, AND THE TWO-LINE ONE FAILED FOR A READING ERROR.**
        # The arm's two lines are separated by a comment block in the source. A
        # first attempt built the anchor from a `grep -v` view with comments
        # stripped, so the text looked contiguous and did not place. A display
        # filter is not the file.
        "                    SK::Int | SK::Fixed => 8,",
        "                    SK::Int | SK::Fixed => 4,",
    ),
    "IsEnum": (
        "                    Some(ConstValue::Int(v)) => *v,",
        "                    Some(ConstValue::Int(v)) => *v + 1,",
    ),
    # --- calls and conversions --------------------------------------------
    # `args.reverse()` appears twice, once per call arm; the surrounding line
    # disambiguates so each attributes to its own opcode.
    "Call": (
        "                let mut args: Vec<_> = (0..*arg_count).map(|_| st.pop()).collect();\n                args.reverse();",
        "                let mut args: Vec<_> = (0..*arg_count).map(|_| st.pop()).collect();",
    ),
    "CallVerifiedNative": (
        "                let mut args: Vec<_> = (0..argc).map(|_| st.pop()).collect();\n                args.reverse();",
        "                let mut args: Vec<_> = (0..argc).map(|_| st.pop()).collect();",
    ),
    "WordToByte": (
        '                    st.b.build_and(v, i64t.const_int(0xFF, false), "tobyte")',
        '                    st.b.build_and(v, i64t.const_int(0x7F, false), "tobyte")',
    ),
    "Trap": (
        "            Op::Trap(_) => {\n                st.b.build_unconditional_branch(trap_bb).unwrap();",
        "            Op::Trap(_) => {\n                st.b.build_return(Some(&i64t.const_zero())).unwrap();",
    ),
    # **RE-REGISTERED 2026-09-06.** The degenerate-yield arm moved from
    # `st.b.build_return(Some(&v))` to `build_typed_return(...)` -- the same
    # refactor that retired `Return`'s mutation -- so this stopped placing. The
    # bare call is NOT unique (the `Op::Return` arm carries it too), so the
    # anchor keeps the guard line. Shape unchanged: return a constant instead of
    # the yielded value.
    "Yield": (
        "            Op::Yield if degenerate_yield.is_some_and(|ys| ys.contains(&i)) => {\n                let v = st.pop();\n                build_typed_return(&st.b, func, v);",
        "            Op::Yield if degenerate_yield.is_some_and(|ys| ys.contains(&i)) => {\n                let _v = st.pop();\n                build_typed_return(&st.b, func, i64t.const_zero().into());",
    ),
}

# ---------------------------------------------------------------------------
# ROUND THREE, DISCRIMINATING.  Added AFTER round three reported `Trap` and
# `WordToByte` undetected, exactly as `MUTATIONS_STRONG` was added after round
# one, and kept in its own table so the pre-registered set above stays the set
# that was committed before running.
#
# `WordToByte` only.  `Trap` is deliberately absent, and its absence is the
# finding rather than a gap -- see `TRAP_IS_UNDETECTABLE_BY_CONSTRUCTION`.
#
# A FALSIFIED PREDICTION IS RECORDED HERE BECAUSE IT NARROWED THE CAUSE.  The
# round-three mutation was `0xFF -> 0x7F`, which differs only in bit 7, and the
# seeded payload at the time held no byte above `0x7F`, so the mutant looked
# equivalent by construction of the payload.  Predicted: adding `\x80\xfe\xff`
# would make it detected.  It did NOT.  Masking is therefore not the mechanism,
# and replacing the result outright is what separates "the site is reached and
# its value is unobserved" from "the site is never reached at all".
# ---------------------------------------------------------------------------
MUTATIONS_ROUND3_STRONG = {
    "WordToByte": (
        '                    st.b.build_and(v, i64t.const_int(0xFF, false), "tobyte")',
        '                    st.b.build_and(v, i64t.const_zero(), "tobyte")',
    ),
}

# ---------------------------------------------------------------------------
# REACHABILITY, not semantics.  Zeroing `WordToByte`'s result was still
# undetected, which leaves two very different readings: the site is EXECUTED and
# its value never reaches an observable, or the site is NEVER EXECUTED.
#
# The mutations here do not change a value at all.  They branch to the trap
# block, so an executed site kills the process and the sweep records SIGNAL.
# AGREE therefore means the instruction never ran, which no value perturbation
# can establish.  This is the instrument-rather-than-grep rule applied to
# "does anything ever do X".
# ---------------------------------------------------------------------------
# **A SIGN PROBE, pre-registered with its expected answer BEFORE it was run.**
#
# Round one's `Shr` mutation flips `build_right_shift`'s sign-extend flag from
# true to false -- an ARITHMETIC right shift becomes a LOGICAL one.  Those two
# produce identical bits for every NON-NEGATIVE operand and differ only when the
# shifted value is negative.  So an undetected round-one result has a specific
# possible cause that no other round tests: the sites run, but never on a
# negative value.
#
# Round two already detects `Shr` 1/1 by replacing the result with a constant, so
# the sites are known to execute and be observable.  That rules out "never runs"
# and leaves exactly this question.
#
# The probe traps when the shifted value is negative, so the outcomes read:
#
#   AGREE     no negative operand ever reaches an `Shr` site -- round one's
#             mutation is EQUIVALENT on this corpus, and its earlier DETECTED
#             result must have come from a tree where a negative one did occur
#   SIGNAL    a negative operand does reach a site, so the mutation is NOT
#             equivalent and the change of status has some other cause
#
# **PREDICTION, recorded before running: AGREE.** `wire.kel` is the only module
# with `Shr` sites and it computes CRC-32 and byte assembly, which stay inside
# 32 bits and therefore never set an `i64` sign bit.  Written down so that a
# result matching it cannot be a story told afterwards, and so that a result
# contradicting it is reportable rather than quietly dropped.
MUTATIONS_SIGN_PROBE = {
    "Shr": (
        '                    Op::Shr => st.b.build_right_shift(value, masked, true, "shr").unwrap(),',
        '                    Op::Shr => {\n'
        '                        let neg = st.b.build_int_compare(IntPredicate::SLT, value, i64t.const_zero(), "shrneg").unwrap();\n'
        '                        let cont = ctx.append_basic_block(func, "shrnonneg");\n'
        '                        st.b.build_conditional_branch(neg, trap_bb, cont).unwrap();\n'
        '                        st.b.position_at_end(cont);\n'
        '                        st.b.build_right_shift(value, masked, true, "shr").unwrap()\n'
        '                    }',
    ),
    # **THE CONTROL, carried into this table so the round has one of its own.**
    # Identical to round one's: it edits an arm with ZERO emitted sites, so it
    # cannot be detected by anything. A round in which this reports DETECTED is
    # measuring the harness, which is exactly how a flat timeout budget once
    # turned four undetected opcodes into false closures.
    "PushImmediate": ("                    1 => 1,", "                    1 => 2,"),
}

MUTATIONS_REACHABILITY = {
    "WordToByte": (
        "            Op::WordToByte => {\n                let v = st.pop();",
        "            Op::WordToByte => {\n                st.b.build_unconditional_branch(trap_bb).unwrap();\n                st.b.position_at_end(ctx.append_basic_block(func, \"wtbreach\"));\n                let v = st.pop();",
    ),
}

# **THE FINDING ROUND THREE ACTUALLY PRODUCED, and it is not a value defect.**
#
# `Trap` was undetected across all 28 modules that emit it, under a maximally
# destructive mutation: branch-to-trap replaced by return-zero, so a program that
# must abort instead returns a value.  Nothing noticed.
#
# It is a GENUINE hole and no seed can close it, because the harness excludes the
# evidence by construction.  `corpus_differential` runs the virtual machine FIRST
# precisely so that a trapping module is turned into a named exemption instead of
# a SIGTRAP that kills the whole run -- the comment at that site says so.  So a
# module that REACHES a trap is exempted and never compared, and a module that is
# compared is one whose virtual-machine run did not fault, which means it reached
# no trap either.  Every compared run has an unexecuted trap block.
#
# Closing it needs a different OBSERVABLE, not better inputs: for a module whose
# virtual-machine run faults, run the native side in a subprocess and require it
# to die with SIGTRAP.  That is agreement on the FACT of the fault rather than on
# a returned value, and it is the named next increment.
TRAP_IS_UNDETECTABLE_BY_CONSTRUCTION = True

# Opcodes with sites that were NOT perturbed in rounds one and two, each with the
# reason.  Recorded so the sweep's coverage is explicit rather than implied by
# omission.
#
# **This dict is a HISTORICAL record, not a current one.** Round three perturbed
# seventeen of these, so an entry here does not mean the opcode is still
# unperturbed.  The summary computes the real residue by subtracting every
# mutation table, which is why it cannot go stale the way the printed count did.
NOT_PERTURBED = {
    "EndIf": "lowers to nothing; a structural marker with no emitted code",
    "Loop": "lowers to nothing; its operand is consumed by Break/EndLoop",
    "Stream": "lowers to nothing in the degenerate-stream transform",
    "Reset": "lowers to nothing in the degenerate-stream transform",
    "Else": "an unconditional branch to a computed block; perturbing the target "
    "produces invalid IR, not a different meaning",
    "Break": "same as Else",
    "EndLoop": "same as Else",
    "Trap": "an unconditional branch to the trap block; removing it leaves a "
    "block with no terminator, which is invalid IR",
    "WordToByte": "reserved for a follow-up: the arm masks and re-widens, and a "
    "single-site swap changes two things at once",
    "Div": "shares its arm with Mod through a multi-branch guard sequence; a "
    "single-site swap changes both and cannot attribute",
    "Mod": "see Div",
    "GetData": "shares one arm with SetData/GetDataIndexed/SetDataIndexed; the "
    "indexed base offset was mutated separately in the earlier census",
    "SetData": "see GetData",
    "GetDataIndexed": "see GetData",
    "SetDataIndexed": "see GetData",
    "NewComposite": "reserved for a follow-up: the arm writes a body field by "
    "field and a single-site swap is not a clean semantic change",
    "GetField": "reserved for a follow-up (offset perturbation was covered by "
    "the earlier census through the indexed-shared-array base)",
    "GetTupleField": "normalised into GetField before lowering; see GetField",
    "GetEnumField": "normalised into GetField before lowering; see GetField",
    "GetIndex": "see GetField",
    "IsEnum": "reserved for a follow-up",
    "Call": "argument-order reversal was covered by the earlier census and by "
    "aot_linkage.rs; a fresh single-site swap here would duplicate it",
    "CallVerifiedNative": "argument-order reversal is already a standing test",
    "Yield": "only reachable in the degenerate-stream transform, where it is a "
    "return; the delegated-suspension mutation covers that path",
    "PopN": "perturbing the depth desynchronises the emitter's operand stack and "
    "aborts lowering rather than changing meaning",
}


def run(cmd, **kw):
    return subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, **kw)


def calibrate(modules):
    """Unmutated wall time per module, so the HANG budget is MEASURED.

    **This exists because a fixed budget silently turned into a false
    positive.**  `PER_MODULE_TIMEOUT` was 20 seconds against a corpus whose
    slowest module took about 4.  Raising the differential's seed count to 24
    took `wire.kel` to **30.7 seconds unmutated**, so it exceeded the budget on
    every mutation and was scored as detecting all of them -- including
    `PushImmediate`, whose mutation edits an arm with zero sites and therefore
    cannot be detected at all.  Four opcodes looked closed that were not.

    A timeout is only evidence of non-termination while a healthy run fits
    comfortably inside it, and "comfortably" is a fact about the corpus that
    changes whenever the harness does.  Measuring it removes the standing
    obligation to remember to re-tune the constant.

    The floor keeps a fast module from getting an unusably tight budget; the
    multiplier is what still catches a real infinite loop, which does not
    terminate at any budget.
    """
    budget = {}
    # **WHICH MODULES ACTUALLY EXECUTE, MEASURED HERE BECAUSE THE RUN IS
    # ALREADY HAPPENING.** An EXEMPT module never executes, so no mutation of
    # an opcode it carries can EVER be detected. Without this the case reports
    # as `NOT SEMANTIC (lowering aborted)`, blaming the mutation for something
    # true before the mutation existed, and sending the next reader to redesign
    # a mutation when the gap is in the CORPUS.
    executes = set()
    for mod in sorted(modules):
        env = dict(os.environ, KEL_ONLY_MODULE=mod)
        t0 = time.monotonic()
        try:
            # **THE CALIBRATION MUST RUN EXACTLY WHAT THE SWEEP RUNS.**
            #
            # The budget is a HANG threshold, and this docstring's own argument
            # is that a timeout is evidence of non-termination only while a
            # healthy run fits comfortably inside it. If calibration measured the
            # whole nine-test binary (385s) while the sweep ran one filtered test
            # (<1s), every budget would be hundreds of times too generous and a
            # real infinite loop would sit inside it undetected -- the same class
            # of false verdict this function was written to remove, inverted.
            #
            # So the command below is kept IDENTICAL to the one in `main`. If one
            # changes, the other must.
            r0 = run(
                [
                    "cargo",
                    "test",
                    "--test",
                    "corpus_differential",
                    "--",
                    "--nocapture",
                    "--skip",
                    "how_deep_does_the_undetected_set_go",
                    "--skip",
                    "which_subjects_would_notice_a_wrong_backend",
                ],
                env=env,
                timeout=CALIBRATION_CEILING,
            )
            base = time.monotonic() - t0
            if "EXECUTED AND AGREEING : 1" in (r0.stdout + r0.stderr):
                executes.add(mod)
        except subprocess.TimeoutExpired:
            # Already pathological unmutated. Give it the ceiling and say so,
            # rather than silently handing it a budget derived from a run that
            # never finished.
            base = CALIBRATION_CEILING
            print(f"  !! {mod} did not finish unmutated inside {CALIBRATION_CEILING}s")
        budget[mod] = max(PER_MODULE_TIMEOUT, base * HANG_MULTIPLIER)
    if not budget:
        # **An empty driven set is a NAMING error, not a result.** Asking for an
        # opcode that the SELECTED table does not carry -- `--reachability Shr`,
        # say, when `Shr` lives only in the round-one table -- used to reach
        # `max()` on an empty dict and die in a traceback, which reads like a
        # broken tool rather than a mistyped request.
        print("  nothing to calibrate: no named opcode has sites in the selected table")
        return budget, executes
    slow = max(budget.items(), key=lambda kv: kv[1])
    print(f"  calibrated {len(budget)} modules; slowest budget {slow[0]} {slow[1]:.0f}s")
    dead = sorted(set(budget) - executes)
    if dead:
        print(f"  {len(dead)} of {len(budget)} do NOT execute unmutated: {dead}")
    print()
    return budget, executes


def opcode_module_map():
    r = run(
        [
            "cargo",
            "test",
            "--test",
            "probe_agreement_depth",
            "dump_opcode",
            "--",
            "--nocapture",
        ]
    )
    out = {}
    for line in r.stdout.splitlines():
        if line.startswith("OPCODEMAP "):
            parts = line.split()
            out[parts[1]] = parts[2:]
    if not out:
        sys.exit("could not read the opcode map; is the test building?")
    return out


def check_placement():
    """Does every registered mutation still MATCH the emitter, exactly once?

    **THIS EXISTS BECAUSE NINE MUTATIONS DECAYED SILENTLY.** Measured 2026-09-06:
    `Return`, `Div`, `Mod`, `GetData`, `SetData`, `GetDataIndexed`,
    `SetDataIndexed`, `Yield` and `GetIndex` had all stopped placing. Every one
    was a REFORMATTING or a SIGNATURE change -- `resolve_shared_*` gained
    `float_bytes`, `build_return` became `build_typed_return`, `SK::Int` folded
    into `SK::Int | SK::Fixed` -- and no opcode had stopped being lowered.

    **Nothing announced it.** The sweep was too expensive to run, so the coverage
    for those opcodes was gone for weeks with no signal. A mutation that does not
    place is a silent no-op that looks exactly like "nothing detected it".

    This check is TEXTUAL and needs no sweep, so it can run in the ordinary test
    suite. It is not a substitute for running the sweep: placing is necessary for
    a verdict, never sufficient.
    """
    original = open(LIB).read()
    tables = {
        "round one": MUTATIONS,
        "round two (strong)": MUTATIONS_STRONG,
        "round three": MUTATIONS_ROUND3,
        "round three (strong)": MUTATIONS_ROUND3_STRONG,
        "sign probe": MUTATIONS_SIGN_PROBE,
        "reachability": MUTATIONS_REACHABILITY,
    }
    bad = []
    total = 0
    print("\n================ MUTATION PLACEMENT")
    for label, table in tables.items():
        misses = []
        for opcode, (old, _new) in sorted(table.items()):
            total += 1
            n = original.count(old)
            if n != 1:
                misses.append((opcode, n))
                bad.append((label, opcode, n))
        print(f"  {label:22} {len(table):3} entries, {len(misses)} not placing"
              + (f"  {[f'{o} x{n}' for o, n in misses]}" if misses else ""))
    print(f"  ------------------------------------------------")
    print(f"  {total} registered mutations, {len(bad)} do not place exactly once")
    print("================\n")
    return bad


def main():
    wanted = list(sys.argv[1:])
    if "--check-placement" in wanted:
        bad = check_placement()
        if bad:
            print("MUTATIONS THAT NO LONGER PLACE:")
            for label, opcode, n in bad:
                print(f"  {label}: {opcode} matches the emitter {n} times, expected 1")
            sys.exit(1)
        sys.exit(0)
    table = MUTATIONS
    if "--strong" in wanted:
        wanted.remove("--strong")
        table = MUTATIONS_STRONG
        print("ROUND TWO: discriminating (result replaced by a constant)\n")
    if "--round3" in wanted:
        wanted.remove("--round3")
        table = MUTATIONS_ROUND3
        print("ROUND THREE: the memory and composite surface\n")
    if "--round3-strong" in wanted:
        wanted.remove("--round3-strong")
        table = MUTATIONS_ROUND3_STRONG
        print("ROUND THREE, discriminating (result replaced by a constant)\n")
    if "--reachability" in wanted:
        wanted.remove("--reachability")
        table = MUTATIONS_REACHABILITY
        print("REACHABILITY: an executed site traps, so AGREE means never run\n")
    if "--sign-probe" in wanted:
        wanted.remove("--sign-probe")
        table = MUTATIONS_SIGN_PROBE
        print(
            "SIGN PROBE: a NEGATIVE shifted operand traps, so AGREE means the\n"
            "round-one arithmetic-to-logical flip is EQUIVALENT on this corpus.\n"
            "Prediction recorded before running: AGREE.\n"
        )
    backup = tempfile.NamedTemporaryFile(delete=False, suffix=".rs").name
    shutil.copy(LIB, backup)
    original = open(LIB).read()

    mapping = opcode_module_map()
    # Calibrate over exactly the modules this invocation will drive, so a
    # single-opcode run does not pay for the whole corpus.
    driven = {m for op, mods in mapping.items()
              if (not wanted or op in wanted) and op in table
              for m in mods}
    budgets, executes = calibrate(driven)
    results = {}
    try:
        for opcode, (old, new) in sorted(table.items()):
            if wanted and opcode not in wanted:
                continue
            mods = mapping.get(opcode, [])
            if not mods:
                results[opcode] = ("NO SITES", [])
                continue

            n = original.count(old)
            if n != 1:
                # Refuse rather than run: a mutation that does not place is a
                # silent no-op, which looks exactly like "nothing detected it".
                results[opcode] = (f"UNPLACEABLE ({n} matches)", [])
                continue
            open(LIB, "w").write(original.replace(old, new))
            assert new in open(LIB).read(), "mutation did not land"

            # Build ONLY the target the sweep runs. `--tests` relinks every
            # test binary against LLVM and dominated the first attempt's wall
            # clock.
            build = run(
                ["cargo", "build", "--test", "corpus_differential"],
                timeout=BUILD_TIMEOUT,
            )
            if build.returncode != 0:
                results[opcode] = ("BUILD FAILED", [])
                open(LIB, "w").write(original)
                continue

            per = []
            for mod in mods:
                env = dict(os.environ, KEL_ONLY_MODULE=mod)
                try:
                    # **RUN ONLY THE TEST THIS SWEEP CLASSIFIES ON.**
                    #
                    # Without the filter, `cargo test` runs all nine tests in the
                    # binary. Measured 2026-09-06: a single-module invocation cost
                    # **385 seconds**, and a NONEXISTENT module name cost 387 --
                    # so essentially the whole per-module cost was tests that
                    # cannot detect a mutation. Filtered to the classifying test
                    # it is **under one second**.
                    #
                    # For `CmpEq`, carried by 48 modules, that is 5.1 hours to 52
                    # seconds; over 25 mutations it is what makes round one a
                    # coffee break instead of sixty hours.
                    #
                    # **THE ORACLE IS UNTOUCHED.** This does not narrow what is
                    # compared, which would be the dangerous change -- it stops
                    # running unrelated analyses. Calibrated: with `CmpEq` mutated
                    # EQ->NE, 44 of the 48 carrying modules DISAGREE.
                    r = run(
[
                            "cargo",
                            "test",
                            "--test",
                            "corpus_differential",
                            "--",
                            "--nocapture",
                            # **EXCLUDE THE EXPENSIVE ANALYSES, DO NOT INCLUDE ONE
                            # TEST. THE DIFFERENCE COST A REAL DETECTION.**
                            #
                            # The first version of this speed-up filtered to
                            # `every_lowering_module_executes_or_is_exempt` alone,
                            # on the reasoning that it is the test the classifier
                            # reads. **That silently removed
                            # `a_trapping_programs_native_side_dies_with_sigtrap`,
                            # which is what detects an `Op::Trap` mutation.**
                            # `Trap` went from DETECTED 30/30 (2026-08-15) to
                            # UNDETECTED across 34, and would have been recorded
                            # as a reopened hole that did not exist.
                            #
                            # The calibration that was run -- `CmpEq`, 44/48 --
                            # could not have caught it, because `CmpEq` is caught
                            # by the differential test itself. **A calibration only
                            # covers the detection PATHS it happens to exercise.**
                            #
                            # So the filter now removes the two tests that are
                            # expensive and cannot detect anything, and keeps every
                            # other detection path. One module: 385s unfiltered,
                            # 5s here.
                            "--skip",
                            "how_deep_does_the_undetected_set_go",
                            "--skip",
                            "which_subjects_would_notice_a_wrong_backend",
                        ],
                        env=env,
                        timeout=budgets.get(mod, PER_MODULE_TIMEOUT),
                    )
                except subprocess.TimeoutExpired:
                    per.append((mod, "HANG"))
                    continue
                txt = r.stdout + r.stderr
                # **Classify on the EXIT STATUS first.** An earlier version
                # tested `"EXEMPT" in txt`, which is true of every run because
                # the summary always prints an EXEMPT line, so every DISAGREE
                # was misfiled as NOLOWER and `CmpLt` came back "undetected" --
                # contradicting a result already verified by hand. The
                # contradiction is what exposed it.
                if "signal:" in txt:
                    per.append((mod, "SIGNAL"))
                elif r.returncode != 0:
                    per.append((mod, "DISAGREE"))
                elif "EXECUTED AND AGREEING : 1" in txt:
                    per.append((mod, "AGREE"))
                else:
                    # Exit 0 with nothing executed: the module was exempted,
                    # most often because the mutation stopped it lowering.
                    per.append((mod, "NOLOWER"))
            detected = [m for m, o in per if o in ("DISAGREE", "SIGNAL", "HANG")]
            nolower = [m for m, o in per if o == "NOLOWER"]
            # **SAY HOW IT WAS DETECTED, not merely that it was.** The three
            # outcomes are not interchangeable evidence. A `HANG` is a TIMEOUT,
            # and a timeout is only evidence of a defect while the unmutated run
            # comfortably fits the budget -- raising the seed count moved several
            # modules toward it, and an inflated `DETECTED` reads exactly like a
            # closed hole. `PushImmediate` is the calibration case: its mutation
            # edits an arm with ZERO sites, so it CANNOT be detected, and any
            # verdict other than undetected is an instrument artefact.
            kinds = ", ".join(
                f"{k} {sum(1 for _, o in per if o == k)}"
                for k in ("DISAGREE", "SIGNAL", "HANG")
                if any(o == k for _, o in per)
            )
            if len(nolower) == len(per):
                # **TWO VERY DIFFERENT CAUSES REACH HERE, AND CONFLATING THEM
                # SENDS EFFORT AT THE WRONG THING.** If NO carrying module
                # executes even UNMUTATED, the mutation is irrelevant: the opcode
                # has no executing witness and NOTHING could detect a change to
                # it. Measured 2026-09-06: `BitAnd`, `BitOr`, `BitXor` and `Shr`
                # are each carried by exactly one module, `wire.kel`, which is
                # EXEMPT. Two rounds reported them `NOT SEMANTIC`, which reads as
                # "the mutation was badly chosen" when the truth is "this opcode
                # has no witness that runs".
                if not any(m in executes for m, _ in per):
                    results[opcode] = (
                        "NO EXECUTING WITNESS (corpus gap, not a mutation defect)",
                        per,
                    )
                else:
                    results[opcode] = ("NOT SEMANTIC (lowering aborted)", per)
            elif detected:
                results[opcode] = (f"DETECTED by {len(detected)}/{len(per)} [{kinds}]", per)
            else:
                results[opcode] = (f"**UNDETECTED** across {len(per)}", per)
            open(LIB, "w").write(original)
            print(f"  {opcode:<18} {results[opcode][0]}", flush=True)
    finally:
        open(LIB, "w").write(original)
        same = open(LIB).read() == original
        print(f"\nemitter restored byte-identical: {same}")

    print("\n================ MUTATION SWEEP")
    undetected = []
    for opcode, (verdict, per) in sorted(results.items()):
        print(f"  {opcode:<18} {verdict}")
        if verdict.startswith("**UNDETECTED**"):
            undetected.append(opcode)
    print("\n  UNDETECTED OPCODES:", undetected or "none")
    # **COMPUTED, not a stored count.** This line read
    # `len(NOT_PERTURBED)` and printed "not perturbed: 25" underneath a round
    # three that had just perturbed seventeen of those twenty-five. A tally
    # maintained by hand beside a table that grows is a claim that goes stale
    # silently, which is the failure this whole census exists to catch.
    perturbed = (
        set(MUTATIONS)
        | set(MUTATIONS_STRONG)
        | set(MUTATIONS_ROUND3)
        | set(MUTATIONS_ROUND3_STRONG)
        | set(MUTATIONS_REACHABILITY)
    )
    residue = sorted(set(NOT_PERTURBED) - perturbed)
    print(f"  never perturbed in ANY round: {len(residue)} {residue}")
    print("================")


if __name__ == "__main__":
    main()

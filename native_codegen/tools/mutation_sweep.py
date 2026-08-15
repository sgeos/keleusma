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
  HANG       the run did not terminate inside PER_MODULE_TIMEOUT -- detected.
             A total-functional language whose whole value proposition is a
             definitive WCET bound does not get to loop forever, so
             non-termination is a real observation and not merely a timeout.
             The first attempt at this sweep had NO timeout and stalled twelve
             minutes on one module, because turning `CheckedAdd` into a
             subtraction stops a loop counter ever reaching its bound.
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
import tempfile

# A mutated module may simply not terminate; see HANG above.
PER_MODULE_TIMEOUT = 20
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
    "GetLocal": (
        'st.b.build_load(i64t, st.locals[*n as usize], "gl")',
        'st.b.build_load(i64t, st.locals[(*n as usize).saturating_sub(1)], "gl")',
    ),
    "SetLocal": (
        "st.b.build_store(st.locals[*n as usize], v).unwrap();",
        "st.b.build_store(st.locals[(*n as usize).saturating_sub(1)], v).unwrap();",
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
        "Op::Return => {\n                let v = st.pop();\n                st.b.build_return(Some(&v)).unwrap();",
        "Op::Return => {\n                let _v = st.pop();\n                st.b.build_return(Some(&i64t.const_zero())).unwrap();",
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
    "GetData": (
        "                        let (byte_off, w, k) = resolve_shared_scalar(&data, slot, i8t, i64t)?;",
        "                        let (byte_off, w, k) = resolve_shared_scalar(&data, slot, i8t, i64t)?;\n                        let byte_off = if is_read { byte_off + 1 } else { byte_off };",
    ),
    "SetData": (
        "                        let (byte_off, w, k) = resolve_shared_scalar(&data, slot, i8t, i64t)?;",
        "                        let (byte_off, w, k) = resolve_shared_scalar(&data, slot, i8t, i64t)?;\n                        let byte_off = if is_read { byte_off } else { byte_off + 1 };",
    ),
    "GetDataIndexed": (
        "                        let (first_off, w, k) = resolve_shared_array(&data, slot, bound)?;",
        "                        let (first_off, w, k) = resolve_shared_array(&data, slot, bound)?;\n                        let first_off = if is_read { first_off + 1 } else { first_off };",
    ),
    "SetDataIndexed": (
        "                        let (first_off, w, k) = resolve_shared_array(&data, slot, bound)?;",
        "                        let (first_off, w, k) = resolve_shared_array(&data, slot, bound)?;\n                        let first_off = if is_read { first_off } else { first_off + 1 };",
    ),
    # --- the division family, guarded so each attributes -------------------
    "Div": (
        "            Op::Div | Op::Mod => {\n                let rhs = st.pop();\n                let lhs = st.pop();",
        "            Op::Div | Op::Mod => {\n                let rhs = st.pop();\n                let lhs = if matches!(op, Op::Div) { rhs } else { st.pop() };\n                let _unused = st.depth;",
    ),
    "Mod": (
        "            Op::Div | Op::Mod => {\n                let rhs = st.pop();\n                let lhs = st.pop();",
        "            Op::Div | Op::Mod => {\n                let rhs = st.pop();\n                let lhs = st.pop();\n                let (lhs, rhs) = if matches!(op, Op::Mod) { (rhs, lhs) } else { (lhs, rhs) };",
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
    "GetIndex": (
        "                let elem: u64 = match kind {\n                    SK::Int => 8,",
        "                let elem: u64 = match kind {\n                    SK::Int => 4,",
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
    "Yield": (
        "            Op::Yield if degenerate_yield.is_some_and(|ys| ys.contains(&i)) => {\n                let v = st.pop();\n                st.b.build_return(Some(&v)).unwrap();",
        "            Op::Yield if degenerate_yield.is_some_and(|ys| ys.contains(&i)) => {\n                let v = st.pop();\n                let _ = v;\n                st.b.build_return(Some(&i64t.const_zero())).unwrap();",
    ),
}

# Opcodes with sites that are NOT perturbed, each with the reason.  Recorded so
# the sweep's coverage is explicit rather than implied by omission.
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


def main():
    wanted = list(sys.argv[1:])
    table = MUTATIONS
    if "--strong" in wanted:
        wanted.remove("--strong")
        table = MUTATIONS_STRONG
        print("ROUND TWO: discriminating (result replaced by a constant)\n")
    if "--round3" in wanted:
        wanted.remove("--round3")
        table = MUTATIONS_ROUND3
        print("ROUND THREE: the memory and composite surface\n")
    backup = tempfile.NamedTemporaryFile(delete=False, suffix=".rs").name
    shutil.copy(LIB, backup)
    original = open(LIB).read()

    mapping = opcode_module_map()
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
                    r = run(
                        ["cargo", "test", "--test", "corpus_differential", "--", "--nocapture"],
                        env=env,
                        timeout=PER_MODULE_TIMEOUT,
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
            if len(nolower) == len(per):
                results[opcode] = ("NOT SEMANTIC (lowering aborted)", per)
            elif detected:
                results[opcode] = (f"DETECTED by {len(detected)}/{len(per)}", per)
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
    print(f"  not perturbed: {len(NOT_PERTURBED)}, each with a stated reason")
    print("================")


if __name__ == "__main__":
    main()

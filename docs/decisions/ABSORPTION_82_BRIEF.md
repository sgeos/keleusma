# Absorption 82

Computed against native `13c9ed11` and upstream `cc48dc61`, with 78 unabsorbed commits. This is a receipt for a completed rehearsal and merge, not a prediction filed before the measurement.

`git merge-tree --write-tree HEAD origin/v0.2.3` exited one and produced tree `cce35a5f3a83fed3d3871094025d20df35aea633` with one conflicting file, `docs/process/REVERSE_PROMPT.md`. Upstream had rewritten its bounded state to 175 lines, and this line had only appended 570 lines of native reports after the former line 3088. The resolution keeps upstream's file and appends those lines unchanged. The merge is `bd62e6fd`. Absorption 81 had completed at upstream `89fc6aac`, which the 2026-10-22 handoff misreported as absorption 80.

## What the absorbed runtime changed for this backend

Upstream made seven operations total. `Div` and `Mod` push the checked triple. `BoundsCheck`, `GetDataIndexed`, `SetDataIndexed` and flat Word or Byte `GetIndex` push the index, four bound limbs and a flag, and the compiler dispatches a set flag to the new `Trap(IndexOutOfBounds)`, which pops five. Word and Byte division now compile to `CheckedDiv(0)` and `CheckedMod`. No opcode was added and the bytecode version is unchanged.

The backend still lowered the former single-result contracts. On the absorbed tree it built cleanly and failed 162 of 776 non-corpus tests, 114 of them as one `PopN` underflow panic. The lowering now follows the new contracts in both the direct and the coroutine paths. Checked division is computed in 64 bits, which is exact and removed the corpus's only compiler-runtime symbol. A matched Fixed `Div` or `Mod` is lowered, because upstream answered report 4 by giving those opcodes a Fixed arm. A Unit fallback on an indexed read is lowered only where the bare bounds trap follows it.

## What the absorption exposed in this line's own instruments

Every straight-line walk driven by `op_depth_effect` charged a terminator's pops to its fallthrough edge. That was harmless while each `Trap` popped zero. One such walk is in the emitter, in `certified_local_widths`, where it could certify a width a local never holds. Four test walks shared the defect, and three of them were passing. All five now skip a `Trap`.

Upstream's self-hosted parser now refuses signed declarations, method calls and qualified names by name. The corpus therefore no longer supplies a single-head subject, and `11_signed.kel` stopped parsing. The stage tests use that file's program without the modifier, and the zero corpus count is pinned.

## Verification limits

The receipt for this absorption is in [the native brief](../process/handoffs/v0.3.0-BRIEF.md) and the archive. The complete native gates were run on the final tree and their records name it. Corpus-derived figures were re-derived, and each moved pin records its measured cause. The split of the bounds-transfer inversion change between upstream's re-costing and this line's lowering was not measured.

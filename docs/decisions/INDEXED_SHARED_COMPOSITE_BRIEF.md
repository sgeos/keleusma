# BRIEF — the indexed shared composite slot

**Line**: V0.3.X native code generation. **Drafted**: 2026-09-11.

## The last shape

Every data-slot form lowers except one: `io.items[i]` where `items` is an array of composites in a
**shared** block. It is refused because the layout entries for the range were not proven contiguous
and uniform.

## Why it is now a small change rather than a new mechanism

Two pieces already exist and neither was written for this:

- **`resolve_shared_array` already proves a range contiguous and uniform** for scalars — same kind,
  offsets at a fixed stride — and refuses a composite base with a placeholder reason.
- **The layout states each element's length.** Measured: a three-element field gives entries at
  offsets 0, 16, 32, each with `len` 16 and the composite flag set.

So the increment is: let that resolver take the composite case, with the stated `len` as the stride
instead of a scalar width, and let the shared composite path use it for an indexed access.

## The wrong turns

1. **Do not derive the stride from the offsets.** The length is stated per entry. Use it, and require
   the offsets to AGREE with it — that is a check, not a derivation, and it is the opposite direction
   from the private pool, where the size is inferred and the offsets are the evidence.
2. **Do not accept a range that mixes kinds.** An element whose composite kind differs from the base's
   would be re-wrapped differently by the reference even if its bytes copied correctly.
3. **Do not drop the existing scalar behaviour.** The same function serves both; a composite branch
   that changes what a scalar range returns would break arms with no relation to this work.
4. **Do not forget that the bound is the instruction's.** The unsigned check against the declared
   element count is already emitted before the shared/private split and must remain the guard.
5. **Do not let the refusal's placeholder reason survive as a comment.** It names a workstream, and a
   stale pointer to an abandoned label is worse than no pointer.

## What done looks like

An indexed shared composite reads and writes agree with the reference at more than one index, the
host's bytes carry the element bodies at the offsets the layout states, a non-uniform or mixed-kind
range is refused with the property named, and no scalar shared array behaviour changes.

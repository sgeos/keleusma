# BRIEF — the uninitialised composite data slot

**Line**: V0.3.X native code generation. **Drafted**: 2026-09-11, after the measurement.

## The defect, measured before it was described

```
struct F { a: Word, b: Word }
private data log { latest: F, count: Word }
fn main(t: Word) -> Word {
    if t > 0 { log.latest = F { a: 11, b: 22 }; }
    log.latest.a
}
```

| path | reference | this backend |
|---|---|---|
| `t = 0`, the slot never written | **faults**, `TypeError("cannot access field on Unit")` | **returns 0** |
| `t = 5`, the slot written | `Int(11)` | `11` |

**A silently wrong value where the reference faults** — the same shape as the unguarded array index
that returned the caller's buffer filler.

## Why it exists, and where the responsibility sits

A composite private slot's `private_init` is `Unit`, deliberately: the compiler preserves a
write-before-read contract for composite and `Text` slots rather than baking a zero body. The
reference **rejects an unconditional read-before-write**, so the contract is checked — but the check
is **flow-insensitive**, and a write on one path satisfies it.

**That looks like a gap in the reference's check and it is the other line's call, not mine.** What is
mine is that my backend must not answer where the reference faults. The pool is bytes; zeros are
indistinguishable from a written body of zeros.

## The choice, and why refusing is the wrong one here

- **Refusing** needs a definite-assignment analysis. `14_frame_log.kel` writes its slot inside
  `for i in 0..3` and reads after the loop; nothing available to this backend proves that range
  non-empty, so a sound analysis refuses it. **That would undo an increment completed today** and
  refuse a program the reference accepts and runs correctly.
- **Trapping** reproduces what the reference does: it faults at run time on the same input. A per-slot
  initialisation word in the persistent region, set on write and tested on read, is fixed-size,
  fixed-offset, statically bounded, and adds no opcode.

## The wrong turns

1. **Do not use a sentinel body.** Comparing the pool bytes against a magic value makes a legitimate
   body that happens to equal the sentinel fault. The flag must be separate from the data.
2. **Do not put the flag in the ephemeral region.** It must survive `Op::Reset` exactly as the slot
   does, so it belongs in the persistent region beside the stream resume-state word.
3. **Do not forget the published host figure.** `persistent_supplement_bytes` is what a host adds to
   the runtime's sizing. A flag that the function does not account for is a buffer overrun in every
   embedder that believed it.
4. **Do not test only the unwritten path.** A trap on every read would also pass that test. The
   written path must still agree, and `14_frame_log.kel` must still run.
5. **Do not describe the reference's flow-insensitive check as a defect in the reverse prompt.** State
   what was measured — unconditional rejected, conditional accepted, and the runtime faults — and let
   the other line rule on whether that is intended.

## What done looks like

The unwritten path faults on both sides, the written path agrees on both, the corpus subject still
runs, the flag is accounted for in the figure a host is told to allocate, and the measurement is
reported to the other line as a question rather than a verdict.

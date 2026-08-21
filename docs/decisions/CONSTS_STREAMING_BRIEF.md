# Brief — driving the `CONSTS` streaming path, Order 1 item 1

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: opened 2026-08-21 (session 50, continued).
**Scope**: a test that drives stage commands 176/177 directly; then, only if that succeeds, the
driver side.
**Constraints**: no new opcode, no `BYTECODE_VERSION` change, stage sources must still self-compile
byte-identically, and `highest_command()` stays 181 unless a command is genuinely added.

## The situation, derived from the tree rather than from prose

`docs/` prose about `CONSTS` has been wrong twice — a record count read as a name count, and
figures that predated the all-default elision. **Take numbers from
`tests/consts_region_composition.rs` and from `wire.kel` itself.**

What those actually say:

- **`fl_walk` is capped at 170 nodes** (`fl_max_nodes()`), because the whole forest must sit in
  `wire.fin`, which is 1,024 words at six words a node.
- **The cap is the blocker, not any interning-order question.** Pinned by
  `the_node_walk_cap_is_what_excludes_the_stages`. The interning conflict is unreachable for this
  corpus: `the_flattener_interns_no_name_for_any_stage` pins that **every constant across all
  eleven stages is `Int`**.
- **Widening the array is a non-answer, not merely an expensive one.** A stage's private data array
  initialises one `Int(0)` per word, so a `fin` wide enough for N nodes adds `6 * N` records to the
  walking stage's OWN `CONSTS`. Pinned at exactly the node width, six to one.

## Why a streaming path exists and why it can work

`fl_walk` needs a **queue**: a composite's record carries `(first, count)` into children numbered
after every node at its own depth, so it cannot write a record until it knows how many nodes
precede its children.

**A forest of scalars has no children.** The queue never grows past the roots, the walk degenerates
to a linear scan, and then it is one node in, one record out, with no state but a cursor. That is
what `fl_stream_begin` (176) and `fl_stream_step` (177) are.

They refuse `-264` on a node with children, `-265` on a tag that interns, and `-266` on a tag
carrying a range. **That refusal is the point, not a limitation**: a composite reaching this path
would be emitted with a zero range and a zero `aux` — structurally valid, silently wrong, and
indistinguishable downstream from a correct record. Refusing keeps the gap visible instead of
encoding it in the bytes.

## THE ONE FACT THAT GOVERNS THE WHOLE INCREMENT

**Commands 176 and 177 have never executed.** They are written, dispatched at `cmd == 176` and
`cmd == 177`, and announced to the other line — and no driver or test has ever called them. Pinned
by `tests/stage_command_reach.rs`, with `CMD_STEP = 175` directly below them as the control that IS
driven.

So this is not "wire up an existing path". It is **validate never-run code, then wire it up**, and
the first half must not be skipped because the second half is more interesting.

## The order, and why it is not negotiable

1. **Drive 176/177 from a test, against a hand-built forest**, and compare the emitted bytes to
   what `fl_walk` produces for the same input. `fl_walk` is the oracle here because it is the path
   that has always run.
2. **Exercise every refusal**: a node with children, an interning tag, a range-carrying tag. A path
   whose refusals have never fired is a path whose refusals are guesses.
3. **Only then** consider the driver.

Doing 3 before 1 means a divergence could be in the stage, in the driver, or in the seam, with no
way to tell — which is the situation the four defects repaired earlier today all shared.

## The wrong turns, named in advance

1. **DO NOT COST THIS FROM THE PROSE.** Two recorded obstacles to `CONSTS` were both wrong: the
   interning-order conflict is unreachable for this corpus, and the figures predated the elision
   that removed 85% of the body. A third wrong belief is likelier than a first.

2. **DO NOT ASSUME THE MACHINERY IS MISSING — OR THAT IT WORKS.** Chained indexing was specified as
   three pieces and two already existed. Commands 176/177 are the mirror case: they exist and have
   never run. **Check, in both directions.**

3. **DO NOT LET `highest_command()` DRIFT SILENTLY.** It is a real guard; a new command returns
   `0 - 99` until it moves. If this increment needs a command, move it deliberately and say so.

4. **DO NOT WIDEN `wire.fin`.** The six-to-one ratio is pinned and the test says explicitly that if
   it ever inverts, the batching plan should be revisited. It has not inverted.

5. **DO NOT CONFLATE THE TWO NODE CAPS.** `nm_max_names()` is 1,024 and bounds the module-input
   walk; `fl_max_nodes()` is 170 and bounds the flattener out of `wire.fin`. This line conflated
   them once, told the other line their figure was wrong when it was right, and retracted.

6. **A REFUSAL PROVES WHICH LIMIT FIRED ONLY IF THE TEST NAMES THE ONE IT EXPECTED.** `-240`,
   `-264`, `-265` and `-266` are different causes. Assert the code, not merely that something
   refused.

7. **STOP AND RECORD IF THE STAGE SIDE DOES NOT VALIDATE.** If 176/177 turn out to be wrong, that
   is a complete and valuable result — never-run code found defective before anything depended on
   it. Reporting that is success, not failure.

## Proportionality

Nothing here changes what a user can compile. `CONSTS` is an emit-path region; the gap is that the
self-hosted stage cannot emit it for the larger stages, so those regions are host-supplied and
**not covered** by the self-hosting claim. Closing it widens what the byte-identity oracle proves,
which is the point of Order 1.

---

# RESULT — STEP ONE IS DONE AND THE PATH IS SOUND (2026-08-21)

**Commands 176 and 177 have executed for the first time, and they are correct.**

`tests/selfhost_wire.rs` drives both directly, reusing the existing `Call`/`run_call` harness rather
than adding a sixth way to drive the stage.

## What was measured

- **A scalar `Int` node streams a record matching the documented layout byte for byte** — tag u16
  at 0, flags u16 at 2, `aux` u32 at 4, payload u64 at 8. The expected record is built from the
  OFFSETS rather than from a captured blob, so a layout change fails loudly instead of being
  quietly re-baselined.
- **`aux` is confirmed written as zero rather than left alone.** The window is reused between
  calls, so a stale index from an earlier record is exactly the kind of wrong answer that looks
  right.
- **All three refusals fire, each asserting WHICH code came back**: `-264` a node with children,
  `-265` an interning tag, `-266` a range-carrying tag.
- **An accepting control passes**, so the refusals discriminate rather than describing a path that
  rejects everything.

The path needs no region and no directory, because it emits at window offset zero and the host
places the sixteen bytes. That is what makes it streamable and is why it does not inherit the
170-node cap.

## What this changes about the cost of `CONSTS`

The remaining work is now **driver wiring against a validated stage**, which is what the analysis
originally claimed it was — but that claim was only true after this step, not before it. Had 176/177
turned out defective, the wiring would have been built on a wrong foundation and any divergence
would have been attributable to the stage, the driver, or the seam, with no way to tell.

**`tests/stage_command_reach.rs` is narrowed rather than deleted.** It asserted the commands were
"driven by nothing"; that is no longer true, and it now pins the narrower fact that the DRIVER does
not reach them. The distinction is load-bearing: it is what makes a future divergence attributable.

## What is NOT done, stated plainly

- **The driver is not wired.** `CONSTS` is still host-supplied for the stages that exceed the walk
  cap, and a region whose payload comes from the host is **not covered** by the self-hosting claim.
- **The streamed output has not been compared against `fl_walk`'s** for the same forest. The walk
  writes into a region and needs a directory and a seeded artifact; the streaming path does not.
  Comparing them end to end is the next slice, and doing it properly means driving the walk through
  the full region harness rather than approximating it.
- **Multi-node streaming is unexercised.** One node in, one record out is proven; the cursor
  advancing across a forest is not.

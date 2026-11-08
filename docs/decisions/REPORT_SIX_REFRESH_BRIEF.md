# BRIEF — refresh report six with the dynamic corpus figures, and close this line

**Filed 2026-09-23, against `d3584f09`, backlog 0, suite 654.**

## Why this closes the arena-touch line rather than continuing it

`streaming_arena_touch.rs` now drives **13 of 28** streaming corpus modules. The
remaining 15 need machinery that exists in `corpus_differential.rs` — external
stub families, `register_native_with_ctx_closure` for composite returns,
argument-count inference, reference-argument masking, a composite return cap.

**Extracting that is what the brief for this work pre-committed NOT to do**: *"if
the existing driver cannot reach these modules without changes to
`corpus_differential.rs` itself, stop and report the shape rather than rewriting a
tuned instrument late in a session."* Eleven more piano-roll modules would also be
confirmatory — the static census already establishes the flat reservation
dominates, and every dynamic figure so far agrees.

So the useful remaining act is the one the completion condition names: **report six
quotes only static corpus figures to the other line, and dynamic ones now exist.**

## What changes, and what must not

Report six's dynamic bullet cites three hand-written streams and the twelve stages.
It should also cite **the first corpus module outside the stages ever measured
dynamically**: `14_frame_log.kel`, 48 bytes of a 600-byte plan.

**And it must say what is still unmeasured, with the reasons**, because a
disclosure that grows a dynamic half without bounding it invites the reader to
over-read it. Eleven modules need host natives registered, three need a non-`Int`
first argument, one the backend refuses.

## Wrong turns, named

1. **Do not let the new figure go unwatched.** `outstanding_reports.rs` exists
   because *"a disclosure whose figures have drifted is worse than none"*. A newly
   quoted number must be pinned where the guard can re-derive it, or it will rot
   exactly as the ones it watches would have.

2. **Do not weaken the report's own limits paragraph.** It says the two halves
   bound nothing about an arbitrary program. Adding a corpus datapoint does not
   change that, and the paragraph must survive intact.

3. **Do not present 13 of 28 as coverage of the corpus.** It is 13 of 28, and the
   count belongs in the report beside the figure.

4. **Do not describe the reservation as a defect.** It is fixed, static,
   documented, safe, and its rationale stands. The finding is the SIZE of the
   looseness, and the decision is the operator's.

5. **`src/` and `tests/` at the repository root are read-only to this line**, and
   `REVERSE_PROMPT.md` is shared — append and edit this line's own section only.

## Pre-commitment

- **If pinning the new figure makes the guard fail for an unrelated reason, fix the
  guard rather than dropping the pin.** An unwatched published figure is the thing
  this guard exists to prevent.
- **If the 48-byte figure proves unstable across runs, do not quote it at all.** A
  number that moves between runs is not a measurement, and publishing it to another
  line would be worse than publishing nothing.

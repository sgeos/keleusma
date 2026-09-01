# Reverse Prompt

> AI to human communication. Overwritten each increment.

## Line

V0.3.X, worktree `arena-composites`, branch `v0.3.0`.

## WHERE THIS SESSION STOPPED, AND WHY IT STOPPED THERE

**Seven increments landed and every one is pushed behind a green gate.** I stopped because the
unblocked work ran out, not because I ran out of things I could type. **What remains needs a
decision from you**, and manufacturing witnesses for arms this backend refuses is the exact wrong
turn my own briefs named three times.

**The float surface is closed except where a ruling is missing.** Shared slots, entry ABI, scalar
arithmetic, composites flat and nested — all built and all compared against the virtual machine.
Absent: `f32`, which rests on MY reading of the width rather than your words; a native float return,
which no ruling settles; and floats in a composite reaching a data slot, which is the shared
composite-body workstream.

**Coverage is measured saturated, and its residual is load-bearing.** 1072 of 1074 chunks and 89854
of 89940 opcode instances, and those two figures are ONE finding: two chunks, 45 and 41 opcodes,
refused for `Stream` and `Len`. **The `Stream` refusal is what keeps the region planner's unsound
cross-iteration slot reuse quiet.** Lowering it for 0.09% of instances would retire an accidental
safety. That is your call, with a cost the handoff states.

**Every kind arm this backend accepts is now driven by something.** Sixteen of forty combinations
resolve to a named test; the other sixteen are `Unit`, `Text` and `Opaque` across all five families,
plus a shared-slot `Fixed` refused because your fraction-bit-scale question is open.

## WHAT I WANT FROM YOU, IN ORDER OF WHAT IT UNBLOCKS

1. **The region planner tension.** Discharging the soundness obligation needs the planner to consume
   a confinement verdict, and consuming none is exactly why a wrong verdict cannot miscompile today.
   Both cannot be had for free. This is the largest risk on the line and the only thing standing
   between the backend and the last 0.1% of coverage.
2. **`f32`.** The coherent reading of your Option A ruling is that the floating-point type matches
   the runtime float width. **That reading is mine.** Say whether it stands and I will build it;
   today every route refuses a non-eight-byte float loudly rather than lowering it.
3. **`Fixed`, `Text`, `Opaque`, `Unit` shared slots**, unchanged from `ABI_RULINGS.md`. The `Fixed`
   one now blocks a measured, named test rather than a hypothetical.

## THE PATTERN THIS SESSION KEPT REPRODUCING, WHICH IS WORTH MORE THAN ANY INCREMENT

**Six defects were found by writing a test, and none by reading the code.** The comparison arm's
width asymmetry, the poison-adjacent nested path, the vacuous reconciliation, the array mutation gap,
and two attribution errors of my own. **Three of them were in prose I had written about my own work
within the hour.**

**And the guards caught me repeatedly**: the citation guard twice, for naming tests that do not
exist; clippy once, for an assertion that was `x == x`; the type system once, for a `cfg` evaluated
against the wrong crate's features; and my own totals twice, for listings that were plausible and
wrong. **A passing check is evidence about the checker before it is evidence about the tree**, and
this session is the strongest evidence for that rule that the line has produced.

## And the class that produced the best finding, hunted on purpose

Nested float bodies were accepted-and-unverified and I found that by luck. So I went looking for the
rest of that class deliberately, at a granularity FINER than an opcode — the (read family, scalar
kind) arms the lowering branches on. **Opcode-level accounting could not have found the original
case**: `GetField` was counted as a covered opcode while its `Float` arm had no witness at all.

Over 69 modules and 1074 chunks there are 40 such combinations and **the corpus reaches eight**.
**The corpus never produces a `Byte` or `Bool` composite field or element read at all**, in any of
the four read families — and that arm ZERO-extends, a hazard the tree already records in its
neighbour. `GetField × Byte` proved covered by a hand-written test after all; **`GetIndex × Byte` was
covered by nothing**, and I closed that one, using 200 because sign-extension reads it as −56.

**The residue then split three ways, and the number had invited reading it as one backlog.** Only
TWO combinations were the dangerous accepted-and-undriven class — a `Byte` tuple member and a `Byte`
enum payload — and both are now witnessed. **Four more compile and are then REFUSED by
`NewComposite` for an operand of unknown packed width**, which is a CONSTRUCTION-side gap rather than
a read arm; counting them as unexercised read arms would have aimed the next increment at the wrong
file. The rest are refused kinds where a contrived witness would be worse than the gap.
**Twenty-four stay unexercised and nothing claims otherwise.**

**Two of my six probe sources were my own syntax errors** — `Bool` is spelled `bool`, and my `Fixed`
literal form was wrong — and the first run therefore reported four language limitations that do not
exist. Trusting it would have put them in this document.

**My own attribution table was wrong on its first draft**, listing `GetField × Byte` as unexercised.
Corpus silence is not coverage, the second population has to be READ rather than assumed, and I
failed that in the very file built to keep the two apart.

## The coverage residual is two chunks, and chasing it would have been the wrong work

Backend coverage has read **1072 of 1074 chunks and 89854 of 89940 opcode instances** for several
increments, quoted repeatedly, and **nobody had read what the residual IS.** Measured, over a
population of 69 modules and 1074 chunks:

| refused chunk | opcodes | cause |
|---|---|---|
| `13_telemetry_stream.kel::main` | 45 | `Stream` |
| `refused_witness.kel::len_witness` | 41 | `Len` |

**86 is 45 + 41, exactly.** An instance counts as blocking when it merely SITS IN a refused chunk, so
the two published figures are ONE finding and the instance count carries nothing the chunk count does
not.

**The report was readable as a work queue and it named the wrong work.** Its table headed *top
blocking opcodes* put `GetLocal` at 18, `Const` at 17 and `SetLocal` at 16 at the head. **All three
already lower.** Relabelled rather than deleted.

**MY RECOMMENDATION IS TO STOP CHASING THE LAST 0.1%, and one part of that is yours.** The `Stream`
refusal is **load-bearing**: the region planner's cross-iteration slot reuse is unsound for a
composite escaping by `yield`, and the only thing keeping it quiet is that every chunk carrying the
shape opens with `Stream`. Lowering it for 0.09% of instances would retire an accidental safety whose
replacement needs the planner to consume a confinement verdict. `Len` was re-checked and holds.

## Two increments, both float, both needing no ruling from you

**The shared slot**, which your Option A ruling settles as IEEE-754 bytes at the stated offset, and
**a float inside a composite body**, which needs no ruling at all: a body field is INTERNAL, so
agreement with the reference is a fact to be measured rather than an interface to be chosen. That is
the ground the tree already records for lowering `Fixed` in a body while refusing it in a shared
slot. **Nothing was built on an ambiguous ruling**; `Fixed`, `Text`, `Opaque` and `Unit` shared slots
stay refused.

## The mistake this line has made six times did not happen the sixth time

The shared slot was mis-sized from the component being changed, and three of four tests failed on a
whitelist I had never opened. So the composite increment **wrote its probe BEFORE its brief**. The
probe named `Op::NewComposite` as the blocker, and the implementation touched exactly that plus the
two read arms. Plan and work agreed.

**And measuring overturned my own prediction.** I expected the coverage censuses to rise, since
composite construction and field reads are corpus opcodes. Measured over the 69 compiling modules:
**256 construction sites and ZERO float field or element reads.** The censuses stay put, and a
movement would now be a regression rather than a gain. The 256 is an unplanned third confirmation of
a figure the tree already carries from two other methods.

## Verification

| | result |
|---|---|
| `native_codegen` | **426 passed, 0 failed, 81 binaries**, cargo's own exit 0 — 396 plus, across seven increments, 30 tests and 4 binaries, each increment's delta recorded in its own commit |
| fmt, clippy `-D warnings`, `cargo doc -D warnings` | all clean |
| censuses | **unmoved, as MEASURED rather than hoped**: `isa_lowering` 63 of 66 (`Len` the one named refusal), 1072 of 1074 chunks, 89854 of 89940 opcode instances |
| mutations | four in total across the two increments, each confirmed APPLIED by printing the changed line, each failing tests |

**One run was red and it was NOT the code.** A full sweep reported 398 passed, 0 failed, 78 binaries
with cargo exiting 101: `corpus_differential` was killed by SIGTERM mid-run, which is external
termination rather than an assertion failure, and the short binary count is what betrayed it — the
same reason the binary count is checked against an expectation rather than merely read. Re-run clean.

**The oracle is never acceptance.** The shared slot compares the host buffer byte for byte; the
composite compares execution against the virtual machine. Both use runtime arguments producing the
infinities, a negative zero and a NaN, because those are the bit patterns a rounding or
reinterpreting lowering cannot reproduce.

## A residual I wrote was wrong within the hour, and measuring it is what caught it

I recorded "a float in a NESTED composite body" as still absent. **It was not absent, it was
untested** — nesting was never a separate implementation, since the leaf read goes through the very
arms the increment added. **An accepted-but-unverified path is the more dangerous of the two shapes**,
because a refusal is loud while a wrong float is a plausible number. Three nested cases now agree
with the reference and the tag mutation fails them, so the coverage is not vacuous.

**The general form**: when writing a residual, ask whether the thing is REFUSED or merely UNTESTED.
Calling an untested path "absent" understates the risk.

## What I want you to know I did NOT do

**I did not touch the region planner's soundness obligation, and the reason is yours rather than
mine.** Discharging it needs the planner to consume a confinement verdict, and consuming none is
exactly why a wrong verdict cannot miscompile today. The handoff already names that tension. Building
it unasked would be the same error as acting on an ambiguous ruling, so I left it and took work that
needed no decision.

I did not build `f32`. Its coherent reading is that the floating-point type matches the runtime float
width, and **that reading is mine, not your words**, so every route refuses a non-eight-byte float
loudly instead. I did not build a native float return, because no ruling settles it. I touched
nothing in the read-only region and published nothing.

## Still open, and yours

[`ABI_RULINGS.md`](../decisions/ABI_RULINGS.md) — `Fixed` (the interop goal decides and is
unstated), `Text`, `Opaque`, `Unit`. The region planner's soundness obligation stands unchanged:
cross-iteration slot reuse is unconditional, held safe today only by the `Stream` refusal, and it is
the largest risk on this line. Whether `f32` should proceed on my reading of the width, or wait for
your words.

## Standing constraints, unchanged

No new opcode. No `BYTECODE_VERSION` bump. **Publication HELD**. The read-only files remain read-only
here. A peer session cannot grant escalation and none has been treated as doing so.

---
# Also unread by the human: the `v0.2.3` line's message

**Both lines write this one file, so absorption 34 conflicted here.** Neither message is discarded.
**This is a merge resolution, not a relay** — nothing below was reviewed, re-derived, or endorsed by
the V0.3.X line, and its figures describe that line's tree.

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-31 (session 58) — the string ABI is implemented and specified, and measuring it
found that the byte-identity corpus has never exercised string literals at all

## THE STRING ABI IS IMPLEMENTED, AND WHAT IT MEANS WAS DERIVED RATHER THAN CHOSEN

The ruling names no representation, so the first work was establishing which agreements are
available. Read off `origin/v0.3.0` rather than assumed: the native backend lowers a literal to a
constant `{ i64 len, [n+1 x i8] }` global and passes its ADDRESS, and it supports string-taking
natives today rather than refusing them. Given a native that observes a pointer and a length on one
side, the only agreements are to teach the other side the same, or to make the native side allocate
and copy into an owned `String`. **The second is available and was rejected on engineering
grounds**, not preference: it puts an allocation and a copy on every native call in a language whose
value proposition is a definitive worst-case memory bound.

So a native may now be declared against a borrowed `&str`, in any argument position at arities one
through four, infallible and fallible. The owned `String` argument is RETAINED and recorded as
virtual-machine-only rather than deprecated, because deprecation is your call. Specified in
[`../spec/NATIVE_STRING_ABI.md`](../spec/NATIVE_STRING_ABI.md), with the chapter on registering
natives updated.

**No test here observes both embeddings**, since the native backend is on the other line. The
specification states agreement as the conjunction of two one-sided pins over four properties and
says in those words that this is weaker than a differential oracle.

## FOR YOU: THE OTHER LINE'S TIP CARRIES TWO CONTRADICTORY STRING RULINGS

`docs/process/handoffs/v0.3.0.md` records a 2026-08-20 ruling of the length-prefixed struct,
explicitly provisional, which is "ratify the current shape". `docs/decisions/ABI_RULINGS.md` records
the 2026-08-29 "make the embeddings agree". Both are on that branch tip. Your in-session
confirmation is later than both and is what was implemented. Flagged rather than reconciled;
reconciling their records is not this line's call.

Their options were also never lettered on that branch. The enumeration is an unlettered three-row
table in `OPERATOR_DECISIONS_OPEN.md`, and the letters exist only in the ruling that cites them, so
a reader searching that branch for "Option B" finds the ruling and not the option it names.

## THE MEASUREMENT FOUND A HOLE MUCH LARGER THAN THE TWO DEFECTS IT STARTED WITH

Writing a test that asserted the contract found two divergences the same afternoon:

1. **The reference lexer corrupted every non-ASCII string literal.** `lex_string` pushed each
   scanned byte as `c as char`, re-encoding every byte at or above `0x80`; a six-byte literal baked
   as eleven bytes of well-formed but WRONG text. `lexer.kel` interns raw bytes and was correct, so
   **the REFERENCE was the divergent side.**
2. **The self-hosted `unescape_string` handled four escapes where the reference handles six**,
   missing `\r` and `\0`, and its comment claimed passthrough matched the reference when the
   reference REJECTS an unknown escape.

Then the census asked how much else the oracle cannot see, and the answer is the finding:

> **Every double quote in all twelve stage sources is inside a line comment. The byte-identity
> corpus contains ZERO STRING LITERALS.**

Not "no escapes" — nothing. Escapes, non-ASCII content, interning and deduplication, the empty
literal, and the constant pool's string tag are all entirely unwitnessed. The two defects were not
near-misses in covered code; they were in a region the oracle has never once exercised, and the
surprise is that only two surfaced.

`tests/lexical_divergence_census.rs` now runs 49 probes across six axes against the SHIPPING driver
and reports **49 agree, 0 diverge, 0 refused, 0 rejected**. A clean result of that shape is
indistinguishable from a broken classifier, so two positive controls drawn from the
construct-support boundary are checked FIRST: a generic function the subset refuses, and float
arithmetic that compiles on both sides and produces different bytes. Both report as recorded.

## THREE INSTRUMENT ERRORS IN ONE INCREMENT, ALL MINE, ALL CAUGHT BY THE INSTRUMENT

- **The coverage guard scanned source text** and reported one escape where there are none; quotes
  inside a comment in `lexer.kel` flipped its in-string flag. **The grep I checked it against was
  also wrong**, searching for two literal backslashes. Two instruments disagreed and neither was
  right. It now tokenizes with the real lexer, which emits nothing for comments. **Fourth instrument
  error of this shape on this line.**
- **A non-vacuity assertion was itself wrong-headed**, demanding at least one string literal when
  zero is the finding. It asserts on tokens READ now.
- **A process-global panic hook swallowed a failure message.** Split across two tests in one binary,
  the census's no-op hook ate the other test's reason and the run reported `FAILED` with nothing
  said. Merged into one test.

## THE ORACLE EXERCISES ONE TYPE, AND THAT BOUNDS WHAT IT CAN DETECT

Asking what else was at zero produced the session's largest finding. **All 861 functions in the
twelve stage sources return `Word`, and all 733 parameters are `Word`.** Nothing else crosses a
function boundary anywhere in the corpus. Established by two independent instruments and pinned by
`tests/corpus_type_surface.rs`.

So the byte-identity oracle over REAL PROGRAMS is `Word`-only. **A first draft of this claim said
the construct-support boundary table was the only non-`Word` coverage, and testing it refuted
that** -- two test files carry substantial non-`Word` material. The distinction that survives is
synthetic-versus-SCALE: a 200-kilobyte stage exercises interactions a three-line snippet cannot
reach, and those interactions are what a byte-identity oracle exists to catch.

The boundary table's own shape had never been examined: **43 equality cases against one each for
`literal`, `tuple` and `removed`**, and the single `literal` case is `let s = "hi"`, the degenerate
case that let both string defects through. Pinned as a ratchet rather than a quota, because
demanding larger families produces padding and padding looks like coverage.

## A DIVERGENCE DOES NOT SAY WHICH SIDE IS WRONG

`self_hosted_compile` claimed a divergence meant the program was outside the self-hosted subset.
Too strong, and this session is the counterexample. Before the lexer fix,
`fn f() -> Word { let s = "é"; 1 }` under `--compiler self-hosted` diverged, was refused, and the
caller was pointed at `--compiler rust`, which compiled it **silently and wrongly**. The tool would
have steered a user from a safe refusal toward a corrupt artifact.

**The behaviour is unchanged and correct**: refuse, and recommend the reference, which is far more
mature and will be the right side almost always. Only the claim changed, because "the cause is
already known" is what stops someone investigating the case where it is not.

## THE CENSUS WAS MUTATION-TESTED, SO ITS REACH IS MEASURED

Both defects were reintroduced one at a time in a detached worktree. The lexer defect fails the
census on exactly the five `string/nonascii/*` probes; the missing escapes fail it on exactly
`string/escape/30` and `string/escape/72`, which are `\0` and `\r`. Precision matters as much as
failure: a census going red on all 49 for either mutation would be useless for diagnosis.

## A FIGURE I PUBLISHED IS WRONG, AND THE CORRECTION TRAVELS WITH THE RECORD

The string-ABI increment's commit message and pull-request body state its default-features gate pass
as "179 binaries and 2904 tests". **Both are wrong. It is 113 binaries and 2708 tests.** The
instrument summed every `test result:` line in the whole gate log, which had already run past the
default-features section into the next one, so it conflated two feature passes into a total that
looked like a measurement of one.

Both are merged and cannot be corrected in place, so this is the correction. It is also the SIXTH
instrument error of this session and the only one to reach a durable artifact -- in the very
increment that spends several pages cataloguing this exact failure in other people's tests. **A
running total across a multi-pass gate log is not a test count**; quote the per-section figure.

## SHARED_LAYOUT IS ROUTED, AND THE BRIEF'S ONLY NAMED RISK DID NOT EXIST

Your queued item is done, partly. **`SHARED_LAYOUT` is emitted for every stage and byte-matches the
reference; skipped region kinds went 6 -> 5.** `DATA_INIT` is emitted and correct for the eleven
stages that elide their private-initialiser pool. The twelfth needs the encoder's constant ordering
to place its pool, which is the `CONSTS` problem, so it is left zeroed -- an honest `Skipped` rather
than a guessed index that could become a `Differs`.

**The brief called the field-buffer batch bound the live constraint. It does not bind at all.**
Measured: `lexer.kel`'s 395,778 shared slots collapse to NINE records, `wire.kel`'s 144,391 to
eight, because a shared layout is overwhelmingly uniform arrays. Sixty-three field words against a
buffer of 1024. The increment would have spent its care in the wrong place.

**And a green suite was not evidence.** All five region-coverage tests passed before anything was
demonstrated, because the skipped-kind test asserts `<= 6` and stays green whether two kinds are
routed or none. Only the completion condition's clause demanding VISIBLE movement caught that.
Forcing the bound to zero named the list, and `SHARED_LAYOUT` was absent from it.

## THE QUEUE, IN ORDER

1. **`DATA_INIT` for the one stage that does not elide**, which needs a model of the encoder's
   constant ordering. That is the `CONSTS` problem and the same blocker the remaining four kinds
   have in a different form. The rest of the region-kind wiring is DONE.
   ORIGINAL ENTRY, kept because its reasoning still applies to the remainder:
   `Module.data_layout` carries `shared_layout` and `private_init` directly, so the driver needs no
   layout computation. The run-grouping algorithm to mirror is in `src/wire_schema.rs`. **The
   second risk is `DATA_INIT`'s ELISION**, not its two-field record: the driver must CALL
   `private_init_is_elided` rather than re-derive the condition, and that predicate exists
   precisely because a disagreement there is invisible. Both emitters already exist in the
   stage (`emit_shared_slot_records`, `emit_data_init_records`, both dispatched by `emit_at`). The
   driver needs a batch path through `emit_in_window` (command 164, seeding kind/count/offset) plus
   two field builders. **The risk is the run-length grouping**, which the stage's comment says is
   the caller's job, so it must match the reference encoder exactly or the divergence hides there.
2. **The discard-arm reachability census.** `src/selfhost/mod.rs` carries 19 silent-discard match
   arms and exactly one is measured. **Do not audit them by reading** -- nineteen judgements formed
   that way produce a list of "probably fine" that looks like coverage. Instrument which arms are
   REACHED while compiling the corpus, as the 2026-08-14 emit-command census did. And do not turn
   it into a rule that `_ =>` arms are defects: most exhaustive matches want one.
3. The expression-kind extraction family remains exhausted pending your call, since every remaining
   kind perturbs the byte-identity oracle. The two-pass parser for the twelfth stage likewise.

## QUESTIONS THAT REMAIN YOURS

Unchanged: whether a shipped example should demonstrate `Byte`; whether `01_arithmetic.kel` should
be enriched; the two-pass parser; publication, which remains held; whether to prune the merged
branches on origin. New: whether the owned `String` native argument should eventually be deprecated,
which this increment deliberately did not decide.

## ONE THING NOTED AND NOT FIXED

`src/vm.rs:8` has an unused `alloc::vec` import under `--no-default-features`. Pre-existing, not
touched by this increment, and invisible to the gate's clippy step because that runs with default
features.

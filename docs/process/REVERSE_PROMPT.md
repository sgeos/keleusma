# Reverse Prompt

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

## THE QUEUE, IN ORDER

1. **The region-kind wiring**, next and scouted to the mechanism. Both emitters already exist in the
   stage (`emit_shared_slot_records`, `emit_data_init_records`, both dispatched by `emit_at`). The
   driver needs a batch path through `emit_in_window` (command 164, seeding kind/count/offset) plus
   two field builders. **The risk is the run-length grouping**, which the stage's comment says is
   the caller's job, so it must match the reference encoder exactly or the divergence hides there.
2. **A coverage census of the rest of the oracle**, which the lexical one implies. The string path
   was at zero; nothing has audited what else is. This is measurable the same way and is the
   natural successor.
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

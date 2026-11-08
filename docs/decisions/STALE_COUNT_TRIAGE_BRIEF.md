# BRIEF — the unre-derivable-count class, and the last `catch_unwind`

**Written**: 2026-08-27, sixth loop iteration. **For this line's own use.**

## Why this, and why it can no longer be declined

This class has been recorded open and **deliberately declined twice**. What changed is the evidence:
**2026-08-26 produced four confirmed instances of it in this line's own artifacts.**

| instance | shape |
|---|---|
| "32 commits ahead of `v0.3.0`" | a count reproduced by **none** of nine measures, inserted *by the provenance-repair commit itself* |
| `analyze.kel` has "nine parallel op tables" | **twelve**; a truncated read reported as a complete list |
| the `0..16384` step cap is a per-tick budget | a correctly-measured number attached to **the wrong axis** |
| `assert!(nodes > 0)` standing for "is a real node count" | green on a **stale stack index** for weeks |

**Four in one day is not a run of bad luck; it is a base rate.** Declining a third time after that
would be indefensible.

**AND THE SIZING PASS FOR THIS BRIEF COMMITTED THE SAME ERROR LIVE.** A first measurement reported
8621 of 8621 comment lines as "carrying a re-derivation marker" — because `grep -n` prefixes every
line with `file.rs:<digits>`, so the filter matched **grep's own output** rather than the comment.
A vacuous filter, and exactly the self-inclusion shape already recorded twice on this line.

## The population, measured

| | count |
|---|---|
| comment lines in `native_codegen/{src,tests}` | 9058 |
| carrying a multi-digit number | 382 |
| naming a command, a `file:line`, or "measured" | 38 |
| unmarked | 344 |
| unmarked and undated | 281 |
| **unmarked, undated, and counting MODULES / CHUNKS / OPCODES / SITES / TESTS / STAGES** | **29** |

**The 29 is the target. The 281 is not**, and the difference matters: the wider set is heavily
diluted with definitional constants (`0..=255`, `i64`) and with figures sourced to a *named function*
rather than a path — `ty_max_steps()` is 1801 is perfectly re-derivable and my filter wrongly counted
it as unmarked. **A count that is definitional is not stale and never can be.**

## Goal 1 — triage all 29

For each: **re-derive it, or cite where it comes from, or mark it definitional.** The prior is that
several are stale, because the corpus has grown repeatedly (three scripts at absorption 6, a fourth
at 7) and claims like *"126 sites across 25 modules"* were written before that.

**A stale count is not necessarily a wrong conclusion.** Say which it is. Several of these sit in
prose whose ARGUMENT survives a moved denominator; changing the number there is bookkeeping, and
saying so prevents the next reader treating a corrected figure as a discovered defect.

## Goal 2 — the last `catch_unwind`

One site remains, in `probe_stage_vacuity.rs`, guarding `parse_functions`. The handoff records
`try_parse_functions -> Result` as existing and says the replacement "goes on absorption" — that has
now happened many times.

**Two real hazards it removes**: `catch_unwind` does not work under `panic = "abort"`, and the site
installs a **silencing panic hook** for the duration, which swallows any unrelated panic message.

## Prior failures to avoid here

1. **A vacuous filter** — committed during this brief's own sizing. **Prove the instrument
   discriminates before believing a count it produces.**
2. **An audit whose population includes its own output** — twice recorded. The triage's own prose
   will contain numbers; do not let the instrument count them as findings.
3. **Correcting a number in isolation** — recorded: a figure updated alone makes its neighbours look
   current. When one moves, re-derive the whole report it sits in.
4. **Reporting a stale figure as a defect.** Distinguish *the number moved* from *the claim is now
   false*.
5. **Subject-shopping / relaxing an assertion** to reach green.
6. **Running the two suites in parallel** — invalidates the perf canary.

## Specific wrong turns

- **Do not sweep the 281.** The 29 is the defensible population; widening it is how a tractable
  triage becomes an unfinished one.
- **Do not add a guard that pins all 29.** Most are prose about a moving corpus; pinning them would
  manufacture failures on ordinary growth and teach the next reader to delete the guard.
- **Do not edit `src/selfhost/`** or the other read-only files.
- **Do not "fix" a definitional constant** by attaching a command to it.

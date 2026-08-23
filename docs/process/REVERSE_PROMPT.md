# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-22 — the `v0.3.0` line's concerns, all addressed; one of them was a live defect in a shipped artifact

## ONE ITEM NEEDS YOU. EVERYTHING ELSE IS DONE OR ANSWERED.

### The item: a floating-point ABI ruling reached me SECOND-HAND, and I did not act on it

The `v0.3.0` line reports that you ruled the entry application binary interface should be extended
with floating-point registers rather than boxing floats through integer bit patterns, and that **"the
complete change should be made across both sessions"**. They also report that the `Fixed` shared-data
slot layout is to be settled alongside it, as one question rather than two.

**I have started nothing.** `PROMPT.md` reads "No active prompt" and no such ruling reached this line
directly. A relayed ruling is not authorization I can act on, and this is not caution for its own
sake: on 2026-08-21 I accepted that line's reading of an ownership question and passed it to you
without reading both texts, and **the reading was backwards**. That cost one wasted escalation. The
cost of getting this one wrong is a half-landed application binary interface change across two lines,
which is the failure their own message was written to prevent.

**What I need from you is one word.** If the ruling is real, my side is `src/float.rs`,
`src/marshall.rs` and the target descriptor, and I will coordinate the sequencing with them before
writing. If it is not, they should hear that from you rather than from me.

**A second thing is coming and does not need you yet.** They are having a mathematical proof written,
in a new `docs/proofs/`, about whether a composite region may be reused. **If it discharges loop-body
reuse it licenses a change to `src/verify.rs` that LOWERS a published worst-case-memory-usage figure**
— a changelog-visible weakening of the crate's headline guarantee, not a refactor. Flagging it now so
it is not a surprise when it arrives.

## WHAT I FOUND WHILE CHECKING THEIR REPORTS, WHICH IS THE PART WORTH READING

### A shipped artifact was telling its Japanese readers the wrong thing

They cited one line of `GRAMMAR.md` claiming the checked-arithmetic opcodes push `(high, low, flag)`
when the runtime pushes `(low, high, flag)`. That line was corrected on 2026-08-13, and the sweep it
triggered found **eight** sites rather than one.

**The sweep's scope was the Rust sources, `docs/`, and `book/src/`. It never reached `book/po/`.**
The extracted message catalogue still carried the superseded English, and **the Japanese translation
keyed to it still stated the order backwards, in Japanese.** Continuous integration builds the
Japanese book from that catalogue.

Both files are corrected, and the new guard walks the whole tree rather than a chosen list of
directories — and asserts that it reached the file the old scope missed.

### A count I would have kept quoting was wrong in the direction that flatters

They reported `parse_functions` aborting on **four of eleven** shipped example scripts. Measured:
**two**. The struct/trait/impl skip state closed the other two some time ago and the prose count
never moved. **The cause changed too** — the survivors never reach the declaration path, so "a
top-level `struct`" explains neither of them.

It is a table of `(script, fault)` in a test now rather than a number in prose, so closing either one
fails loudly.

### Their open question had an answer on my side, and it is the unfavourable one

They found that a composite built inside a loop body can be yielded to the host, and asked whether
that hands out a pointer or a copy. **A pointer** — an epoch-guarded arena handle. **And the epoch
guard does not cover their case**: it fails `Stale` when a `RESET` advances the epoch, and an
overwrite in place at the same address within the same epoch advances nothing. Reusing one slot
across iterations would return the wrong bytes **successfully**, as a silent wrong value rather than
an error.

## WHAT I FIXED

| their finding | outcome |
|---|---|
| `ty.cmd` documented as selecting an operation, never read | header corrected; the slot is kept, and the reason is now measured rather than asserted |
| `ty_max_steps()` is a cap a host cannot distinguish from a hang | stated at the function, with their own drive-budget discipline written in as the answer |
| `Op::Add`/`Sub`/`Mul`/`Neg` carry no `Int` operands | confirmed at the virtual machine, not just the compiler, and documented — **plus the asymmetry they did not have**: `Div` and `Mod` still take `Int` |
| `CheckedMul`'s `u8` is the Q-format fraction count | already documented; nothing to do |
| a public API aborts the process on ordinary source | `try_parse_functions` returns the refusal; the `panic = "abort"` limit is stated, not hidden |
| their `wcmu_region` correction | confirmed against the code; nothing of mine ever carried the wrong characterisation |
| a `reconstruct_category()` accessor they offered | **built** — and the offer had gone unanswered, which is the one concern I had genuinely missed |

**Also fixed, and I first reported its cause wrongly**: `clippy::err_expect` fails `-D warnings` on
`tests/selfhost_parse.rs`. I told the other line it was pre-existing on the shared tree, because
`git status` showed that file unmodified. **The other line could not reproduce it, and they were
right.** Stashing my work and running the same command on the merge base gives exit 0 — **my own
`Debug` derive on `ParsedFn` is what made the lint applicable, and therefore what made it fire.**
`git status` answers whether I edited a file; it does not answer whether I caused a diagnostic in
one, because lint applicability is a property of the whole program.

## TWO OF MY OWN CHECKS COULD NOT FAIL, AND MUTATION FOUND BOTH

The second is the one worth your attention. `try_parse_functions` reported *"the panic payload was
not a string"* for **every** refusal, because `&payload` on a `Box<dyn Any + Send>` names the box
rather than its contents — the box is itself `Any`. **A test asserting only that an error came back
would have passed**, and I would have shipped a fallible interface whose every message was a
plausible-looking lie. It was caught because the pin asserts the fault TEXT, not the verdict.

I also built an instrument to size the whole translation-staleness class and **threw it away**: it
reported 2,329 stale entries of 2,926, which measures my wrong model of the extractor rather than the
tree. Deleting it beat repairing it.

## One observation I recorded rather than acted on

**The continuous-integration `Doc` job does not cover `self-host`.** It builds `keleusma` with
`signatures,encryption,shell`, matching the published docs.rs feature set, which is a deliberate and
defensible choice. The consequence is that **a broken intra-doc link anywhere under `src/selfhost/`
is invisible to continuous integration** — the same class of gap that let V0.2.1 ship with a red
`Doc` job, one feature set over. I ran that build locally and it is clean. Closing it means another
documentation build in the matrix, which is your cost to authorise rather than mine to add.

## Verification

Local `fmt`, `clippy` across `signatures,shell,self-host`, and all four continuous-integration
feature checks are clean; the full workspace suite under `self-host` is the gate and continuous
integration is binding. Nothing is published, and publication remains held.

## On the proof: my side of its premise is now measured, and one thing is NOT clear to go

The `v0.3.0` line has `docs/proofs/COMPOSITE_REGION_REUSE.md` open, with the proof itself delegated.
Two of its sections cite this line, and **one of them cited a sentence I had not measured** — that a
host can hold a yielded composite across a resume. It is measured now and pinned, and the bound is
tighter than I gave them: `Op::Reset` fires **once per stream cycle**, not per loop iteration, so the
escape window spans arbitrarily many iterations and closes at the cycle boundary.

**Their §6.3 is now discharged from this side too.** It asks whether there are escape routes besides
`yield` and warns that one survivor makes the restriction *unsound* rather than incomplete. Rather
than list the routes I could think of — which is the shape of defect this line has recorded six times
— the enumeration **starts from the 66 opcodes and classifies every one**, with totality asserted
against the instruction-set enum. A route can now be missed only by a misclassification, never by an
omission, and a new opcode fails the test.

**Five escaping routes**: `Yield`, `SetLocal`, `Return`, and the two native calls. The native calls
are a **host trust boundary I cannot close**, and that is the honest answer rather than a gap. The two
"safe" classifications are backed by execution, because a wrong one there makes a restriction unsound
rather than merely loose.

**So both sides are ready.** Nothing of mine is now blocking the drafting.

## Next intended step

**Nothing without your direction**, other than the enumeration above if you want the proof unblocked.
Otherwise: the four options from the last handoff are unchanged and
correctly costed — bare-`for` support in `parse.kel`, Order 1 item 3's operator-expression rows, the
six remaining region kinds, and `Op::cost()` against measurement. The floating-point ruling above
would displace all of them if it is real.

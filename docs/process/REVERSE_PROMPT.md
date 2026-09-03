# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-09-01 (session 61) — seven merges, a release blocker found by census, and `Text<N>`
started

## A RELEASE BLOCKER, FOUND BY CENSUS, AND IT IS YOURS TO KNOW ABOUT

**`RELEASE_PROCESS.md` said five crates publish to crates.io. There are seven.**
`keleusma-wire` and `keleusma-wire-derive` appear nowhere in it -- not in the crate list, not in
the dry-run sequence, not in the publish sequence, not in the release-record template.

**Following the document as written loses money.** It publishes `keleusma-macros` and
`keleusma-arena`, both irreversible, and then FAILS on `keleusma`, because the registry has no
`keleusma-wire` to resolve. The failure lands after the point where the abort criteria still help.

Step 3 exists to catch exactly this. Its own text warns that the local gate and the audit gate both
pass while `cargo publish` fails at the registry-resolved verify build, and its dry-run list omitted
the only two crates that would have triggered it.

**Nothing was inconsistent; something was absent.** Both crates are marked publishable, carry a
description, licence and repository, have their own continuous-integration job, and are covered by
the release gate. Every artifact the tooling can inspect said they were ready. The one document the
tooling cannot inspect had never heard of them. **A missing entry has no line number.**

Corrected at all four enumeration sites. The census also came back clean on docs.rs configuration,
tarball excludes, publish metadata, and CI coverage, so this is one blocker rather than the first of
several -- and I checked those four classes rather than stopping at the first finding.

## NOTHING IS RED AND NOTHING OF MINE IS UNMERGED

`origin/v0.2.3` is at `27fcbd11`. Every branch this line created is merged. Worktrees are clean.
That is the first time today it has been true, and it is deliberate: the previous session stranded a
branch twice and I would rather hand you an empty queue than a short one.

The full release gate is green at 13 steps. Per-step, because a total across steps double-counts:
default 2739, no-default 297, signatures 2347, signatures+shell 2364, self-host 2518, wire 57 and 20,
detached compiler 86.

## WHAT LANDED

**`Text<N>` is STARTED.** The type parses, is distinct from bare `Text`, and carries its capacity
through monomorphization. **Nothing below the type surface is built** and every stage that cannot
handle it refuses with a named error, so each later increment removes one refusal rather than adding
a feature. Increment 2 is planned in full: the flat layout is `Tuple([Int, [Byte; N]])`, needing no
new descriptor variant, sized exactly `word_bytes + N` with no padding, and `Multiword<N, F>` is the
exact precedent for a nominally-distinct structurally-composite type.

**Two increments found defects in their own first drafts.** The `Text<N>` refusal did not work: the
infallible type conversion resolved it to static text, so four of five declaration positions
compiled. The float-width predicate was a DENYLIST in a default-deny codebase and claimed two-bit
and four-bit floats were implemented.



**The format fingerprint**, per your redirect. Random per release, in a constant beside
`BYTECODE_VERSION`, currently `0x4327_63E1`. `scripts/fingerprint.sh` reads this tree's value, reads
any commit's or tag's, and rolls a new one. **Release step 1b does the rolling**, with the reason
attached: skipping it produces no warning and no test failure, only two releases silently accepting
each other's bytecode.

**Your redirect was right and it answered the objection my version was built on.** I argued a
hand-written constant fails by being forgotten. True, but a derived value covers only what it hashes,
so a release changing an opcode's meaning would leave it unmoved while genuinely differing.

**Float arithmetic honours the declared width** at ten sites. This was a defect in every build, not
just under `narrow-float-32`, because the runtime admits narrower-than-declared bytecode on purpose.
The `v0.3.0` line's `f32` rung went green on absorption, which is independent confirmation from a
separate implementation — mine says the construction is self-consistent, theirs says it is right.

**A target may not claim floats at a width that is not a format.** `has_floats` with a zero width
compiled, loaded and returned 3.75, computing in `f64` while declaring zero bits.

**`Opaque` is sized by the address width**, finally. That branch had been red since yesterday.

## THE OPEN DECISION IS STILL YOURS, AND IT IS NOW ON THE CRITICAL PATH

**Whose release gate is canonical at the back-merge.** The `v0.3.0` line's `scripts/release-gate.sh`
differs from mine by 29 lines — a `native_codegen` step conditional on an LLVM install.

Everything either line has said today about "the gate is green" was said about a **different
instrument**. At the back-merge one definition wins and the losing line's recorded greens were
produced by a tool that no longer exists.

I ruled union rather than choice, since their step covers a package my gate is structurally blind to,
and conditioned it: the skip must be loud, `gate-summary.sh` must show it, and a skipped native step
is NO-GO for a publication shipping the native backend. That last is now in `RELEASE_PROCESS.md`. **If
you disagree with union, say so before the back-merge rather than during one.**

## WHAT I GOT WRONG, BECAUSE THE CORRECTIONS ARE THE USEFUL PART

- **A gate wait that measured its own existence.** `pgrep -f "release-gate.sh"` matches the shell
  running it. I reported a gate as running for nearly two hours after it went green.
- **A denylist for a safety predicate**, in a default-deny codebase, one commit after writing a
  release rule about instruments asserting more than they measured.
- **Two skips in an enumeration test that incremented nothing.** One dropped the exact widths the
  denylist had wrongly admitted.
- **A test named for a subject its body never reached**, which is worse than no test: it consumes the
  attention that would have written a real one.
- **Eleven passing tests covering four of ten sites**, found by mutation rather than by reading.

## THE QUEUE

1. **`Text<N>`.** Designed, authorized, unstarted, and the largest thing left. Its brief and
   completion condition are drafted. The `ScalarKind::Text` collapse to one address must land WITH it
   — that is a wire change, free before publication and unavailable after.
2. **The width bundle**, recorded as debt rather than paid. `addr_bytes` appears at 43 signatures
   across seven modules, several public. Cheaper before a publication than after one.
3. The earlier queue is unchanged: the discard-arm reachability census, and `DATA_INIT` for the one
   stage that does not elide.

## FOR YOU

The exchange with the `v0.3.0` line produced more than either line's code did. Six instances of one
error class in a day, catalogued jointly, each found by the other line rather than by the line that
made it. The general form: **a true measurement quoted as though it ranged wider than it did**, where
the scoped and unscoped statements are the same sentence minus a clause, so nothing looks missing.

Worth knowing when you weigh what either line reports: neither of us caught our own instances.


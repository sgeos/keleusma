# Brief — the combination neither gate has run

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X. **Drafted 2026-09-01**, at the close of a long session.

---

## The goal set

| goal | state |
|---|---|
| **G11** run the full gate, because a combination exists that neither line has tested | **unblocked, and the subject of this brief** |
| absorption 44 | gated: `origin/v0.2.3` has not moved; the arithmetic width is in their gate |
| `f16`, `Text<N>`, `Opaque`, publication | not mine |

## The finding, and it is not what I set out to check

I set out to argue that the workspace needed no gate run here, on the ground that this line does not
own `src/`. **The first half of that argument holds and was measured**: `src/` and `tests/` are
**byte-identical** to `origin/v0.2.3` after absorption 43, so the library and its tests are exactly
what the other line's gate verifies.

**The second half is false.** The diff outside `native_codegen/` and `docs/` is *not* empty:

- **seven corpus files this line ADDED** under `examples/scripts/` — the witness programs written for
  backend coverage — plus their `README.md`
- **`examples/scripts/rogue/rogue_dungen.kel`, MODIFIED**, one line changed
- **`scripts/release-gate.sh`**, 29 lines added by this line
- `.gitignore`

**And six workspace tests read `examples/scripts/`**, among them `rogue_scripts.rs`,
`example_index_claims.rs` and `corpus_pattern_coverage.rs`.

## So there is a combination neither gate has run

| | source | corpus | run? |
|---|---|---|---|
| the other line's gate | theirs, new | **theirs** | yes |
| this line's gate at session start | older | **this branch's** | yes |
| **now** | **theirs, new** | **this branch's** | **no** |

**Identical `src/` is not the same as verified.** Their gate paired that source with *their* corpus.
This branch pairs it with a corpus carrying seven extra programs and one modified line, read by six
workspace tests.

**This is this line's own widest-input rule**, recorded after a pin failed here: *before pinning a
value, ask what the widest input to it is and whether that input is pinned too.* The invariant "we do
not own `src/`" protects a region; the workspace tests' widest input lies outside it.

## Why this and not another increment

**Eleven increments have landed today, every one verified against the `native_codegen` package
only.** Each push was justified by scope — a detached package, or documentation — and each
justification was sound in isolation. **What none of them checked is the combination above.**

Adding a twelfth increment on an unverified base would be worse than verifying the base.

## The wrong turns

**1. Do not report the gate's exit code as the result.** This line has recorded both directions:
`cargo test | tee log` yields tee's status and is green on a red tree; a trailing `grep` is red on a
green one. **Read the status of the thing being asked about.**

**2. Do not treat a red canary as a regression without a solo re-run.** The perf canary reads
**69.04s under concurrent load against a 30s tripwire and 1.20s alone**, and the regression row in its
own reference table is **67.3s** — three per cent away. A contended false red is numerically
indistinguishable from the true positive it exists to catch.

**3. The other line's gate is running.** Warn them before starting, as they warned me, and give them
the numbers rather than a decision.

**4. Do not silently fix a failure this reveals.** If the untested combination is red, the finding is
that it was untested — reporting the failure matters more than repairing it quickly.

**5. `--all-features` is not one of the tested configurations and does not pass.** It cascades
mutually exclusive narrow-width selectors and pulls in an SDL3 build. Running it and reporting the
failure would be a manufactured finding.

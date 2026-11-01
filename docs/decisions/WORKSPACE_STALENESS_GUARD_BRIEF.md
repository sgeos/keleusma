# Brief — make the routine change executable, because the prose version already failed once

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X. **Drafted 2026-09-02, evening.**

---

## The present goals

| goal | state |
|---|---|
| **G21** workspace suite, and the routine change it exists to motivate | suite in flight; **the routine change is the deliverable** |
| `f16` | **blocked, no oracle.** The reference refuses widths 3 and 4 at load, so a binary16 module never runs on the reference side |
| publication | held |
| absorption 48 | **nothing unabsorbed.** `origin/v0.2.3` still at `29460378`, verified by fetch |
| the gate hole | deliberately open; the obvious fix is measured not to work |

**The recommendation is to finish G21 by changing the routine, and to change it in a form that
cannot quietly stop working.**

## Why the obvious form of the deliverable is the wrong one

The gap is real and already stated. Every absorption prediction on this line names only
`native_codegen` figures, so **none of them can express workspace staleness**, and the check is
missed by construction rather than by oversight. This is the absorption-46 build-clause lesson in a
second place, **not a new species**.

The obvious remedy is a paragraph in the handoff saying "also check the workspace". **That remedy is
made of the same material that already failed.** This line has recorded, in its own documents, that

- a handoff banner read "after absorption 18" for three days while the body described absorption 38,
- a blocker row kept a reason that had expired, inviting the inference that the blocker cleared,
- an ancestry anchor was written against a moving ref and went false without anyone noticing until
  the block was actually run.

Prose in a long document is the least-refreshed thing in the repository. **A remedy written in prose
for a defect whose mechanism is "the written thing did not fire" is not a remedy.**

So the deliverable is an **executable** check, and the prose points at it rather than replacing it.

## What the check must be careful about

**It must not fire by default.** A test asserting that workspace coverage is current would go red at
the instant of every absorption and stay red until someone spends ninety minutes on a workspace run.
**A guard that is red by default is suppressed, not obeyed**, and suppressing it would leave the tree
worse than having no guard at all. This is why the artifact is a script answering on demand rather
than a test.

**It must fail safe.** An unrecognised path is classified as compiled, the strongest class. The
guard may over-report staleness and may never under-report it. Putting a path on the inert list is a
claim that requires evidence, and the one entry there has it.

**Its reach must be proven before it is trusted.** A guard returning CURRENT on a clean tree has
demonstrated nothing. Each verdict needs a case whose answer is known independently, and the inert
class specifically needs a case where it suppresses a **real** change rather than an empty one.

## A finding that came out of building it, and corrects the framing

The handoff describes the gap as **absorption 47, seven `src/` files, 191 insertions**. That figure
is true about absorption 47.

**It is not the size of the coverage gap.** The last workspace run was at `03e4917f`, which predates
several absorptions, so the tree has moved by **twenty-four files the workspace suite compiles**, not
seven. The seven-file figure is a true measurement about a narrower population, read as though it
described the gap.

**That is the SCOPE_DELETION shape again, in my own handoff, written by me.** The guard reports the
range it spans, which is why the discrepancy surfaced at all.

## The wrong turns

**1. Do not call the workspace run a gate pass.** It is one suite in one configuration. The gate is
fourteen steps and includes formatting, linting, documentation, links and the subprojects.

**2. Do not write into `docs/` while the workspace suite runs.** `tests/documentation_links.rs` walks
`docs/` with `read_dir` and asserts a reach minimum, so a file appearing mid-run is exactly the
absorption-40 hazard, where a suite reported a failure naming the editing session's own change.
**Note the correction**: an earlier claim in this session that fifteen tests read `docs/` was an
overcount from a grep that matched doc comments. About five read a path. The count was wrong; the
constraint was not.

**3. Do not test the guard by checking out a historical commit.** Mutating the working tree to test a
guard would corrupt any suite running beside it. This is why the guard accepts an explicit revision
rather than always reading `HEAD`.

**4. Report every figure with its population.** Several are available here and they differ.

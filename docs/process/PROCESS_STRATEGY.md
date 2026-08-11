# Process Strategy

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

This document describes the development process for Keleusma, adapted for library engineering with agentic AI-driven development.

## Engineering Classification

This project is classified as **Library Engineering**.

Libraries occupy a middle ground between FMCG (Fast-Moving Consumer Goods) and high-assurance engineering. Correctness matters more than in a game, because users depend on the library for their own projects. However, the cost of a bug is lower than in high-assurance systems. This classification informs the level of rigor applied to testing, documentation, and code review throughout the project.

| Dimension | Library | FMCG | Mission-Critical |
|-----------|---------|------|-------------------|
| Ship criteria | Tests pass, API stable | Tests pass, playable | Formal verification |
| Testing | Unit + integration + property | Unit + integration | Unit + integration + property + fuzz + formal |
| Failure cost | User projects break | Player experience degraded | Safety or financial loss |
| Iteration speed | Moderate | Fast | Slow |
| Documentation | API docs + guides required | Internal docs sufficient | Exhaustive specification |
| Code review | Thorough review of public API | Rapid review | Multi-reviewer with sign-off |

### Higher Rigor Subsystems

The compiler and VM require careful correctness. Bytecode execution must not panic or produce undefined behavior. The lexer and parser should produce clear, actionable error messages. These subsystems receive additional scrutiny during development and review, including edge case testing and defensive validation at module boundaries.

## Agentic AI Development Loop

The AI agent operates within a structured loop that balances autonomy with human oversight.

```
1. Identify blockers
       |
       v
2. Research (read docs, explore code)
       |
       v
3. Clear blocker (ask human or resolve independently)
       |
       v
4. Advance development (implement, test, refactor)
       |
       v
5. Update process files (TASKLOG.md, REVERSE_PROMPT.md)
       |
       v
6. Commit
       |
       v
   (return to step 1)
```

### Autonomy Boundaries

The AI agent **may proceed** autonomously with:

- Adding dependencies to Cargo.toml
- Making design decisions within the documented specification
- Creating new files and modules
- Resolving technical blockers through research and implementation

The AI agent **should stop** and consult the human pilot when:

- A decision would change the language semantics
- A technical approach has significant tradeoffs requiring human judgment
- The token limit is approaching and work is incomplete
- An assumption is unclear and cannot be resolved from existing documentation

## Tiered Verification

The full gate must be green before every **merge**. It does not need to run after every
**change**. Those are different questions, and conflating them costs hours: roughly twenty
full gates were run across one session, one per increment, where four would have given an
identical answer.

| Tier | When | Cost | Command |
|---|---|---|---|
| **0 — inner loop** | every edit | seconds | `scripts/fast-check.sh 'test(<filter>)'` |
| **1 — pre-commit** | every increment | ~3 min | the three checks below |
| **2 — pre-merge** | once per merge, batching three or four increments | ~2 h | `scripts/release-gate.sh` |

**Tier 1 is the one that is easy to skip and should not be.** These three catch defects that
targeted tests are structurally incapable of seeing, and each has drawn blood:

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p keleusma --no-default-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

A broken intra-doc link is invisible to `test` and `clippy`; that is how V0.2.1 shipped with a
red CI Doc job, and how broken `src/selfhost/` links survived four releases. A test importing
behind a feature gate breaks the runtime-only build while every targeted test passes.

**Batch increments per merge.** A feature branch may sit red across many commits; only its tip
must be green. Running the full gate per increment buys nothing, because the gate answers a
question about the merge, not about the increment.

### The full gate re-runs the heaviest suite per feature configuration

`cargo test` runs once per feature config, so `selfhost_codegen` — whole-stage self-compiles,
the most expensive suite by far — executes four times. That is most of the two hours.

This is **deliberate and has not been narrowed.** Skipping it under `signatures` or
`signatures,shell` would probably be safe, since those affect signing and the docs.rs surface
rather than codegen. "Probably safe" is how the two coverage holes above were made, and the
saving is roughly two-fold where batching gives five-fold at no cost to coverage. Take the
batching. Narrowing the matrix is an operator decision, not the loop's.

### Run the gate in a worktree, not in the tree you are working in

`scripts/release-gate.sh` reads the working tree, so running it directly freezes development
for its whole duration. At ~2h33m per merge that was the largest single calendar-time cost in
the loop, and it bought nothing: the gate is answering a question about one commit.

`scripts/gate-in-worktree.sh <commit>` runs it in a **detached worktree pinned to that
commit**, with its own `CARGO_TARGET_DIR`. Two things improve at once:

- **The main tree stays free.** Slice N+1 is developed while slice N gates.
- **The result is pinned by construction.** "A gate result is valid only for the tip it ran
  against" stops being a discipline someone has to remember and becomes a property of the
  mechanism — the same mechanism-over-procedure argument made below about CI. The script
  re-checks that the tree is at the requested commit and refuses if it is dirty.

**THE SAME `pgrep` CALL IS RIGHT HERE AND FATAL IN `gate-status.sh`, AND THE DIFFERENCE IS WORTH
STATING** so nobody "fixes" the working one. `gate-in-worktree.sh` refuses to start a second gate
with `pgrep -f "release-gate.sh"`, which is the literal pattern `gate-status.sh` warns against in
capitals. Both are correct:

- **In a WAITER LOOP it is fatal.** `until ! pgrep -f "release-gate.sh"` matches the shell running
  the loop, so it never exits. That deadlocked a session for hours.
- **In a ONE-SHOT REFUSAL it is fine.** `gate-in-worktree.sh` does not have that string in its own
  command line, and the worst failure mode is a false positive that declines to start a gate —
  which is the safe direction for a guard whose job is to decline.

The rule is therefore about the *shape of the use*, not the call: **never let a `pgrep` for a script
name gate a loop's exit; a one-shot refusal may use one.**

**THE REFUSAL IS OVER-BROAD FOR `--setup-only`, noted and NOT fixed.** The running-gate check sits
above the `--setup-only` early exit, so a queued session cannot even prepare its worktree while
another session's gate runs — although setup touches only that session's own named directory and
runs no cargo at all. Preparing ahead is exactly what a queued session should be able to do. Left
alone deliberately: this is shared gate infrastructure, and changing it while a gate runs and
another is imminent puts risk into the mechanism both sessions are about to depend on. The narrow
fix, when someone does it, is to scope the check to `$GATE_DIR` for the setup-only path rather than
to move it.

**On the canary, which is the obvious objection.** This deliberately introduces concurrent
load, and `tests/perf_canary.rs` wants a quiet machine. Accept it, because the error is
directional: **load can only make the canary slower.** It can therefore produce a false
positive, costing one re-run, and it cannot produce a false negative, which is the failure
that would matter. A real regression stays visible under load.

**`--setup-only` prepares and verifies the worktree without running the gate.** It exists so
the setup path is testable without a 2.5-hour run, and it earned that on its first use: the
reuse check compared an unnormalised path against git's resolved one, so the second
invocation — the common case — tried to re-create an existing worktree and died. A guard that
has not been shown to work is not a guard, and that applies to the tooling around the gate as
much as to the gate.

### A green suite cannot see a performance regression

On 2026-08-08 the wire-format v2 cutover was merged only after this was learned the hard way:
the port was functionally perfect and roughly **forty times slower**, and every tier reported
green. One stage self-compile went from 54 seconds to over 37 minutes. Nothing in the gate
measures time, so nothing failed.

`tests/perf_canary.rs` is the answer — a tripwire, not a benchmark, with a deliberately loose
ceiling. It runs in every tier that runs the test suite and costs about two seconds.

**If it fails, profile before touching the ceiling.** The defect class it guards is a hot-path
read that has become proportional to the whole module: a rebuilt view, a re-parsed table, or a
whole-pool decode behind what should be a single-record fetch. Correctness tests will keep
saying the answers are right, because they are.

The canary was validated against the real regression rather than assumed to work: reverting the
repair takes it from 1.7 s to 67.3 s, tripping the ceiling. A performance guard that has not
been shown able to fail is not a guard.

### Do not build a strong gate and a weak gate for a human to choose between

The obvious response to a 2h33m gate is a fast variant for routine work and the
full one before anything that matters. **Reject that shape.** It produces a
procedure that is sound on paper and catastrophic when not followed, and the
deviation is silent — the cheap path passes, nothing announces that the expensive
path was skipped, and the discipline erodes under exactly the schedule pressure
the fast path was created to relieve.

The safe form is not a better procedure. It is **removing the choice**:

- **The complete check must be a MECHANISM, not a procedure.** CI runs on every
  push to `main` and `v*`, unconditionally, in parallel jobs, with no one deciding
  whether to invoke it. It cannot be forgotten, hurried, or skipped under
  pressure. That is what makes it safe to be the authority.
- **CI must be a strict SUPERSET of the local gate.** If the local gate checks
  something CI does not, the local gate is load-bearing and cannot be trimmed
  without losing coverage. Keeping that containment is the whole precondition.
- **The local gate is then a fast pre-check, not an authority.** Trimming it costs
  no coverage, only the latency of finding a failure in CI instead of locally.
- **A deviation must be loud.** `merge-to-trunk.sh --skip-gate` exists and prints
  a warning; that is the right shape. An escape hatch that is silent is the
  dangerous kind.

**This containment was NOT holding when it was checked on 2026-08-09.** CI lacked
the `self-host` feature and every `keleusma-wire` configuration, and its Doc job
lacked both new crates — while the local gate had all of them. The two checks had
diverged in both directions, nobody had chosen that, and the informal two-tier
system the design above forbids had accreted on its own. Closed in the same
change that recorded this.

### Prefer a pattern to an enumeration; a by-name list is a latent hole

The same defect has now produced **five** separate failures in this repository, and
in each case the fix is a rule that matches rather than a list that remembers.

| Enumeration | What it missed | Cost |
|---|---|---|
| `release-gate.sh` lists crates by name | `keleusma-wire` | four days of gate coverage with no CI coverage |
| CI Doc job lists crates by name | `src/selfhost/`, then both new crates | broken intra-doc links survived four releases |
| Root `.gitignore` listed nested `target/` dirs by name | `native_codegen/target/` | 571 build artifacts swept into a commit |
| A gate-progress regex bounded to 70 characters | the 71-character twelfth step | every "step N of 12" report was wrong for a day |
| A gate-progress regex anchored to line start | the same header wrapped in ANSI escapes | reported `steps=0` for a gate that had run thirteen |

**A HAND-WRITTEN BOUND IS A BY-NAME ENUMERATION.** The last two rows are the same
defect wearing different clothing, and recognising that took embarrassingly long:
`{5,70}` enumerates the acceptable lengths and `^` enumerates the acceptable
prefixes, both over a set that was free to grow. A list enumerates members; a
bound enumerates a range. Neither is a rule that matches.

**A by-name list is correct on the day it is written and silently wrong the moment
the set grows.** Nobody is at fault when it fails, which is exactly why it keeps
happening. Where a pattern can express the intent — `**/target/` rather than one
line per package, `[^=]+` rather than `[^=]{5,70}` — use the pattern. Where
enumeration is unavoidable, put a comment at the point of failure saying what must
be added, and expect that to work only sometimes.

**Every one of the five failed silently and read as success**, which is the
property that makes the class worth naming rather than just fixing case by case.
A gate with no coverage passes; a doc job with no crate reports green; a regex
that matches nothing reports zero.

The `.gitignore` case carries an extra lesson for parallel work: **a guard that
lives on one branch does not protect another.** A package's ignore rule in its own
subdirectory is absent on any branch lacking that subdirectory, and untracked
files survive a branch switch, so `git add -A` after switching sweeps whatever the
new branch does not know to ignore. A rule at the repository root exists on every
branch that has the root file and cannot go missing that way.

### Gate visibility: `scripts/gate-status.sh`, and the status-line hook

A gate runs two to three and a half hours in a detached worktree, and with two sessions on one
machine there may be several. "Is it still going, and where" was being answered by ad-hoc `pgrep`
and `grep` at the prompt, which produced **three defects in a single day**, all of the same family
— a convenience that quietly answers a different question:

| Ad-hoc form | What it actually did |
|---|---|
| `pgrep -f "release-gate.sh"` | **matched its own shell**, so a waiter loop never exited and gating deadlocked for both sessions |
| a header regex capped at 70 characters | silently skipped the 71-character last step; progress read "11 of 12" forever |
| `cargo test … \| tail` in a background job | reported **tail's** exit status, not cargo's |

`scripts/gate-status.sh` replaces all of it. Two properties are load-bearing:

- **Liveness comes from the log's modification time and its verdict line, never from a process
  lookup.** That makes the self-matching `pgrep` failure *unreachable by construction* rather than
  by remembering, and it distinguishes RUNNING from STALLED, which a process check cannot.
- **The header pattern is unanchored and unbounded.** Anchoring missed the ANSI escape that wraps
  each header — this script reported `steps=0` for a twelve-step gate on its first run — and a
  length cap is the same defect in another dress. The verdict line matches the header pattern too,
  so it is excluded from the count; before that, gate summaries over-reported by one.

**The bar is WEIGHTED by measured test time, not by step count.** Four of the twelve steps carry
**91%** of the wall clock and eight carry about 9%, so a uniform bar would read ~50% within three
minutes and then crawl for three hours — worse than no bar, because it invites exactly the wrong
estimate of time remaining. Weights come from a completed run (12,594 s of test time). A finished
gate is full; otherwise completed steps count in full and the current step counts a half, since
intra-step progress is not observable from the log. If the step count ever exceeds the weight table
the bar **degrades to uniform rather than mis-weighting**, which is this file's own rule applied to
itself.

The weights cover test time only, not compilation, so `Clippy` and `Docs` are under-weighted. That
is a known and stated limitation rather than a hidden one.

**The status line shows it automatically.** `~/.claude/statusline.sh` appends one line of stdout
from an executable `scripts/statusline-segment.sh`, if the project has one. The contract is
deliberately defensive, and every guard is tested: a hard timeout so a slow script degrades to
silence, stderr discarded, a non-zero exit ignored, newlines stripped, and the output truncated.
A project without the file contributes nothing.

`scripts/` rather than `.claude/`, because `.claude/` is gitignored here and an integration point
that is not version-controlled silently differs between machines.

### Reap orphans before timing anything

An interrupted gate leaves its test binary reparented to PID 1, still at full CPU. One was found
burning four cores for ten hours and halving the machine. They accumulate, one per interrupted
run, and they corrupt exactly the timing signal the canary depends on. `release-gate.sh` now
reaps them as a preflight; do the same by hand before any measurement.

## Milestone-Based Development

Development follows milestone sprints. Each milestone represents a coherent unit of work with defined entry criteria, exit criteria, and success criteria. See [COMMUNICATION.md](./COMMUNICATION.md) for the bidirectional communication protocol and work item coding system.

## Related Documents

- [COMMUNICATION.md](./COMMUNICATION.md) for the bidirectional human-AI communication protocol
- [GIT_STRATEGY.md](./GIT_STRATEGY.md) for version control conventions

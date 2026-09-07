# A measurement taken while its inputs move is not a measurement

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: implemented 2026-09-06 on the V0.3.X line. `native_codegen/tools/frozen-run.sh`, pinned
by `native_codegen/tests/frozen_run_guard.rs`.

---

## The failure this closes, and it is a repeat offence

| when | what moved | consequence |
|---|---|---|
| absorption 40 | increment edits landed mid-run | the attribution had to be **argued**, which is the one thing the discipline exists to prevent |
| 2026-09-02 | a documentation commit landed at 18:11 during a workspace run | quotable only with a caveat |
| 2026-09-06 | documents edited under a running backend suite | the run was **discarded** rather than quoted |
| 2026-09-06 | a commit landed during a push | the push's own local-vs-remote check printed a mismatch that was not one |

**Each time the rule was already written down.** Two of the four were committed in a single session,
after that session had recorded the rule against them.

## Why a rule was the wrong repair

This tree already carries the lesson, from the twelve-tests-one-witness fault: **a coupling that keeps
rotting is repaired structurally, not by resolving to be careful.** A convention lives in whoever
remembers it; a verdict attached to the output is met by whoever reads the result.

So `frozen-run.sh` hashes `HEAD` plus `git status --porcelain` before and after a command, and prints
**FROZEN** or **NOT FROZEN** beside the exit status and wall clock.

## What it covers, and what it does not

**Catches**: an edit, a commit, a merge, a stash, or a checkout landing mid-run — anything that moves
tracked content or `HEAD`.

**Does not catch**: untracked files a run reads, environment changes, machine load, writes outside the
worktree, or anything at all about whether the measurement was *correct*.

> ⚠ **A FROZEN VERDICT RULES OUT ONE WAY OF BEING WRONG.** It is the way this project keeps being
> wrong, which is why it is worth automating — but it is not a validity claim. This line has
> overclaimed an instrument's reach three times, so the limit is stated beside the guard rather than
> left to be inferred.

## It labels; it does not block

A legitimate edit to an unrelated file is not an error, and failing a run for it would be a new
failure mode. The wrapped command's exit status passes through unchanged, and a test pins that — **a
wrapper that could turn a failing measurement into a passing one would be far worse than the problem
it was built for.**

## It also records WHAT the run measured, and the reason is a mistake of the author's

A verdict saying **FROZEN** without saying what the run found is half a provenance record: it tells a
later reader the tree was still and nothing about the result.

**The prompt was two consecutive runs quoted as `exit 0` with no test count**, because the invocation
piped the guard through `tail -10` and the summary line fell outside the window. **The guard was not
losing it — the caller was.** Checking that before changing anything avoided "fixing" a tool that
worked.

But the observation survived the correction, so the last lines of the wrapped command are now printed
**adjacent to the verdict**. A truncated log fragment then carries both or neither.

> ⚠ **STREAM AND CAPTURE, NEVER BUFFER.** The output goes to the terminal as it happens *and* to a
> file, via `tee`. Buffering until exit is a recorded failure here: a 12h51m sweep was piped through
> `tail`, so progress was invisible and the continue-or-kill decision was blind for hours.

**The wrapped exit status still passes through unchanged**, verified directly and by the test that
pins it. `tee` moves the status into `PIPESTATUS`, and getting that wrong would have made every failed
measurement read as a pass — the exact failure mode this guard must never introduce.

## An opt-in guard is still a rule, so the common case now wraps itself

`frozen-run.sh` made the tree check structural **for runs that remember to use it**, and the handoff's
instruction to *"wrap long runs in it"* is exactly the form that has already failed four times.
Measured after building it: the guard was referenced by its own test and its own documentation and
**nowhere else** — no script, no routine.

`native_codegen/tools/backend-gate.sh` closes that. It runs formatting, lints and both suite halves
**through the guard**, so the backend's most common operation cannot forget.

**It also captures three facts that lived only in a session's scrollback:**

1. **The suite must be split.** `corpus_differential` alone is ~390 s and the rest ~140 s; together
   they exceed the harness's background ceiling, and two runs were killed mid-flight before the cause
   was understood.
2. **There are two float configurations.** `narrow-float-32` is selectable only because the manifest
   forwards it, and it went unmeasured until 2026-08-31, when it was found RED.
3. **Every run should be frozen-checked.**

**What it is not**: not the release gate, which covers the workspace and is mandatory before a merge;
and not a continuous-integration job, because that is a per-push cost and the `v0.2.3` line has
recorded such costs as the operator's call.

## The guard's own test was the defect it guards against

The first version had three `#[test]` functions. The harness runs them concurrently, so the probe that
deliberately moves the tree ran while the case asserting a still tree was measuring, and that case
failed.

**The tests raced each other through the very state the guard observes.** It is the same class as the
`linkage_symbol_census` scratch-directory race, except no per-test isolation can fix it: the shared
resource is not a directory the test chose, it is the worktree. **A guard that observes global state
cannot be tested in parallel with anything that mutates global state**, so the cases are sequential
within one test — a property of what is being checked, not a workaround.

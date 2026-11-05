# Decisions Open with the Operator, 2026-11-04

> **Navigation**: [Kaizen](./README.md) | [Process](../README.md)

The operator asked on 2026-11-04 for a review of the messages from other lines and an
updated list of open decisions and obvious choices. This list follows
[the list of 2026-11-01](./2026-11-01-open-decisions.md), which holds that day's three
rulings verbatim, and items keep their numbers. New items are E1 to E3. Every figure
below was read at about 02:40 UTC on 2026-11-05. The other lines' questions come from
`docs/process/REVERSE_PROMPT.md` on `origin/v0.2.3` at `f437bc31` and
`docs/process/handoffs/v0.3.0-BRIEF.md` on `origin/v0.3.0` at `ee56203b`, which are the
authority for every peer question.

An item is marked **obvious** when the recommendation follows from a ruling or a
measurement already on record and no option trades one stated goal against another.
Otherwise it is marked **judgment**. Each list gives the recommendation first.

## Messages from other lines

| Sequence | From | Content | Action |
|---|---|---|---|
| 248 | V0.3.X | Merged `v0.3.0` into `main` as `badd3e5c` through pull request 714, before a release, with no tag or publication | Read and acknowledged. Nothing owed |
| 280 | V0.3.X | Asked whether the worktree `keleusma-worktrees/wire-corpus` is Kaizen's | Answered in sequence 284. It was the `v0.2.3` mainline line's, by commit `b383a838` of 2026-08-15, and V0.2.X removed it, by V0.3.X's own commit `36546b77`. It no longer exists, and its calibration logs survive under `keleusma-worktrees/archive-logs-20260831` |

This line sent sequence 285 to V0.2.X about E1 below.

## Closed since the list of 2026-11-01

| Item | Outcome |
|---|---|
| D2 | `idleCompaction` is false from 05:42 UTC on 2026-11-01. The trial runs to about 2026-11-15 |
| D3 | The proofs line holds, on the operator's word |
| D4 | **Fact.** GitHub now answers alert 6 with HTTP 410, "withdrawn". The advisory itself is not withdrawn. It was updated at 16:50 UTC on 2026-11-04 and names `>= 7.0.0, < 11.0.1`, which still covers the `minimatch` 10.2.6 that `main` and `v0.2.3` hold. The latest `vscode-languageclient`, 11.0.0, still requires `^10.2.6`. The exposure analysis of 2026-11-01 is unchanged |
| O3 | The merged agent tree `agent-ab1f9687f23f80567` is gone, and V0.3.X moved its archive folder to `.archive/` |
| O1 | **Fact.** The fuzz schedule runs. The runs of 2026-11-01 to 2026-11-04 all passed, starting at 16:49, 20:24, 17:52 and 18:02 UTC against a cron time of 10:17, so GitHub starts them six to ten hours late |
| Release signing | Adopted. `scripts/release_manifest.py` merged in pull request 710. No public key is committed on any branch yet |

## Decisions

**E1. The scratch volume is nearly full.** *Obvious, but deletion is the operator's.*
**Fact.** `/Volumes/Scratch` has 12 GiB free of 233 GiB, 95 percent used. `worktree.sh
list` names seven folders that no live tree uses, 25.8 GB in all, each named after a
V0.2.X agent branch. Most of each is `target`, but three also hold logs, diffs or
archives that may be evidence. A build that fills the volume fails partway.
(a) V0.2.X keeps what it needs from its seven folders and removes the rest, which this
line requested in sequence 285. A standing rule follows. The line that made a scratch
folder removes it once no live tree uses it and anything it needs is kept, with
`worktree.sh list` as the check.
(b) The operator removes them.
(c) Leave them, and accept failed builds when the volume fills.

**E2. Withdraw D5 (c), the agent trees' `build-dir` file.** *Judgment.* **Fact.** The
file was not created. **Fact.** V0.2.X now links each agent tree's `target` by hand to
a scratch folder named after its branch. The live tree `agent-a85a1fe47e1e1c3a9` links
to `agent-v02-unify-native-gates`, and the seven folders of E1 came from the same
practice. **Inference.** Hand linking already moves V0.2.X's agent output, with folder
names a person can attribute, which a hashed `build-dir` folder lacks. Both leave a
folder behind when Claude Code removes the tree, so neither avoids E1.
(a) Withdraw D5 (c) and rely on hand linking, which `worktree.sh list` now recognizes.
(b) Create the file as ruled. It also covers trees no line links by hand, but its
folders cannot be attributed.

**E3. When release signing starts.** *Judgment.* The ruling begins signing at the
first release after the operator has made a key, and no key exists yet. V0.2.X lists
the release last among its next steps, for the operator to take up.
(a) Make the key before V0.2.3 is published, so that V0.2.3 is the first signed
release. The operator's one-time steps are in
[the signing proposal](./2026-11-01-release-signing-proposal.md#ruling-and-procedure).
(b) Publish V0.2.3 unsigned and begin with the next release.

## The operator's own checks

Unchanged since 2026-10-30. K6, look in the repository settings for "Require merge
queue". K9, the account plan. K10, whether the self-host archive storage is billed.
K11, the blog's fixes. K14, the pydromon changeover, held.

## Obvious choices

- **O6.** Merge the fix that makes `worktree.sh list` count a scratch folder as used
  when a live tree's `target` links into it under another name. Without it, the
  report named the live 15 GB folder of `agent-a85a1fe47e1e1c3a9` as unused, which
  invites deleting a live tree's output.
- **O7.** Accept the fuzz schedule's late start. It runs nightly and passes, and an
  earlier cron time would not change GitHub's queueing.
- **O8.** Rerun `context_quality.py` on about 2026-11-15 for the idle compaction
  trial.
- **O9.** Keep watching for a `vscode-languageclient` that accepts `minimatch` 11, and
  take it when Dependabot proposes it.
- **O10.** Rewrite this line's state file within its budget. It stood at 11,974 of
  12,000 bytes after pull request 711.
- **O11.** Leave the main checkout's 182 MB `target` and `arena-composites`'s 261 MB
  `compiler/target` with their lines. Both are small.

## Questions held by other lines

**V0.2.X.** The seven questions of
[`NESTED_LOOP_ARENAS.md`](../../decisions/NESTED_LOOP_ARENAS.md) on `v0.2.3`, each with
its recommendation first. In brief, Q1 asks what a `loop` calling a `loop` means in
V0.2.x, recommending terminal delegation as measured. Q2 asks for the arena mechanism
once independent resets exist. Q3 asks about hot reload at an inner Reset. Q4 asks
whether a stream chunk's lack of `Return` becomes a verifier check. Q5 asks for the
nesting depth bound. Q6 asks about second and later loop parameters. Q7 asks whether
`replace_module` refuses a swap away from a Reset. The record is the authority, and
this line has no view that differs from its recommendations.

**V0.3.X.** Its brief states that a further merge of `v0.3.0` into `main` waits for the
operator to ask for it. Its questions 29 to 34 are answered.

## Verification limits

The scratch figures are from `df` and `du` at about 02:40 UTC on 2026-11-05, and a
running build changes them. Whether the seven folders' extra files matter is V0.2.X's
judgment, not this line's. The fuzz start times are from the runs interface. The cause
of the late starts is GitHub's queueing by inference, not by any statement from it.

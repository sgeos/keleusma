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

## Operator ruling, 2026-11-04

The operator ruled first-hand in this line's session on 2026-11-04, in these words.

> Re: Decisions.
>
> - E1. Everything stale should be removed. I think Rust updates cause build bloat.
> - E2. A.
> - E3. A. Key creation will happen later.
> - K-series. Defer.
> - O-series. Accept.
>
> Re: Other line's questions. Will follow up directly.

Shortly after, the operator added, "Note that I updated the toolchain shortly after
issuing my response to you."

| Item | Disposition |
|---|---|
| E1 | Carried out for what this line may remove, below. Applied to other lines' live trees only by their owners, who were told in sequences 286 to 289 as a relay |
| E2 | (a). D5 (c) is withdrawn. The `agents` exception left `worktree.sh`, and `/Volumes/Scratch/keleusma/agents`, empty, was deleted |
| E3 | (a). V0.2.3 is the first signed release, and its publication waits for the operator's key. V0.2.X was told |
| K-series | Deferred |
| O6 to O11 | Adopted. O6 and O10 merged in pull request 731 |
| Other lines' questions | The operator follows up with those lines directly |

**E1, what was removed.** At about 03:00 UTC on 2026-11-05, each of the seven folders
was rechecked with `worktree.sh list` as used by no live tree, and every file in them
that was not build output, 82 MB of logs, diffs, `before`, `after`, `base` and
`base.tar`, was first copied to
`/Volumes/Scratch/keleusma/.archive/agent-scratch-20261105/`. The seven folders, 25.8 GB,
were then deleted, and so was this line's own scratch `target`, 3.4 GiB, which was
rebuilt. The scratch volume went from 12 GiB free to 40 GiB free. Nothing of another
line's live tree was touched.

**E1, the hypothesis.** **Fact.** Before the removals, by file modification time
against the stable toolchain's installation at 10:24 local time on 2026-10-28, 11.7 of
the 120.3 GiB in the live scratch folders predated that update, 3.2 of it this line's
own. `arena-composites` held 47.7 GiB and `v0.2.x` 21.3 GiB. **Fact.** The update the
operator made on 2026-11-04 rewrote only the nightly toolchain's manifest, at 02:52 UTC
on 2026-11-05. Stable is still rustc 1.99.1, and no scratch folder holds Miri output.
**Inference.** A toolchain update does leave the old build output in place beside the
new, because Cargo names artifacts by compiler, so the hypothesis is right in kind. On
this measurement, though, most of the space is each tree building its own full output,
several profiles and feature sets each, and a stable update adds about a tenth.
Modification time is a proxy. A file older than the update may still be current if a
pinned or nightly toolchain built it.

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

## Work on open issues, 2026-11-05

The operator asked on 2026-11-04 for every known open issue that needs no further
input to be addressed. This section records what was done and the one new question
that arose.

- **The state file budget.** `tests/state_file_budget.rs` fails when a file under
  `docs/process/handoffs/` that states "within a budget of N KB" exceeds N thousand
  bytes, and fails when no file opts in, so it cannot pass by checking nothing. It runs
  on the light path, which `tests/ci_light_path_coverage.rs` required. A file over
  budget and a renamed phrase each fail it.
- **The fuzz crash path.** The "Run each target" step was run locally under GitHub's
  default shell flags, with a stand-in for `cargo fuzz` whose second of three targets
  crashed. Every target ran, the step failed naming the crashing target, and the
  crashing input was under `fuzz/artifacts/`. The upload action itself is still
  untested. A job stopped by its 75-minute limit is cancelled rather than failed, so
  `if: failure()` would not upload its inputs.
- **The gate saving of pull request 508**, the Jev selection task
  `kaizen-release-gate-timing`. That pull request filtered the `selfhost_*` binaries out
  of the gate's workspace test step, because the self-host step runs them as the run of
  record. nextest builds every binary whatever the filter, so the saving is run time
  alone. **Fact.** On 2026-11-05 at `e0488c25`, with the binaries built first,
  `cargo nextest run --profile ci --workspace -E 'package(keleusma) & binary(~selfhost)'`
  ran 1,024 tests in 71 binaries, all passing, in 4,355 seconds. The load average was 40
  to 63 during the run, against about 8 before it, partly from this run's own build.
  **Inference.** The step now saves about 73 minutes on this machine under that load and
  less when it is quiet. One run under contention is not a quiet-machine figure. The
  release line was asked to cite this where the release procedure describes the local
  gate, since that document changes through it.

**E4. The interline checkpoint carries other projects' messages.** *Judgment.*
**Fact.** The shared store under `secret/interline/` now registers the lines
`pydromon`, `pykeleusma` and `radiate` beside Keleusma's, and on 2026-11-05 it holds
293 messages. Six from `radiate` to `pydromon` match kept disclosure term 3.
**Fact.** The tracked checkpoints, which a public repository serves, carry messages
between `radiate` and `pydromon`, 23 on `main`, 47 on `v0.2.3` and 28 on `v0.3.0`.
None of those match a term or name a home directory. **Fact.**
[The protocol](../INTERLINE_PROTOCOL.md) describes the store as shared by the lines of
one repository and forbids private material in checkpoints. This line holds its own
export and asked V0.2.X, V0.3.X and proofs in sequences 294 to 296 to hold theirs.
(a) Export only messages whose sender and recipient are both Keleusma lines, by a
change to `scripts/interline.py` that this line would make with tests. The messages
already published stay in history, since removing them would need a history rewrite
and force pushes.
(b) As (a), and also remove the other projects' messages from the current tracked
checkpoints by an ordinary commit on each branch, leaving history as it is.
(c) Give the other projects their own store, so Keleusma's holds only its own lines.
(d) Keep exporting everything, after removing the six matching messages' bodies.

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

# Decisions Open with the Operator, 2026-11-01

> **Navigation**: [Kaizen](./README.md) | [Process](../README.md)

The operator asked on 2026-11-01, at about 03:00 UTC, for an updated list of open
decisions and obvious choices. This list follows
[the list of 2026-10-31](./2026-10-31-open-decisions.md), which holds the three rulings
of that day verbatim, and items keep their numbers. The new kaizen item is N8. The
other lines' questions were read on 2026-11-01 from `docs/process/REVERSE_PROMPT.md` on
`origin/v0.2.3` at `2d5444b3` and `docs/process/handoffs/v0.3.0.md` on `origin/v0.3.0`
at `5457bb5d`, and those records are the authority for every peer question.

An item is marked **obvious** when the recommendation follows from a ruling or a
measurement already on record and no option trades one stated goal against another.
Otherwise it is marked **judgment**. Each list gives the recommendation first.

## Closed since the list of 2026-10-31

| Item | Outcome |
|---|---|
| Scratch volume | The proposal merged as pull request 697, `79665b2b`, on the operator's "Merge.", and the notices went to the three other lines in sequences 213 to 215 |
| Gate build output | Pull request 703, `10b8985c`, defaults the gate's target to the scratch volume when `SCRATCH_BASE` is set |
| Gate folders | 19.4 GB of gate output last written on 9 and 10 August deleted on the operator's "Delete." at 00:36 UTC |
| Interline checkpoint | Exported with the bodies of sequences 193 and 194 redacted, merged as pull request 698. Sequences 190 to 192 stay as they are, on the operator's decision |
| V0.2.X decision 2 | Dropped by V0.2.X against V0.3.X's record of question 28 as (a) |

## Decisions

**D1. N6, keep the pinned shard lists.** *Judgment.* Pull request 689 pins 43 self-host
tests to shards by measured duration and hashes the rest. The table compares every
successful CI run with all four shard jobs, from the four-shard layout to the merge of
pull request 689 at 20:02 UTC on 2026-10-31, and from that merge to 01:42 UTC on
2026-11-01. The figures are job minutes from the jobs interface, in
[the evidence file](./evidence/ci-practices/2026-11-01-shard-minutes.tsv).

| Runs | Count | Longest shard, median | Longest shard, range | Longest less shortest, median | Four shards summed, median |
|---|---|---|---|---|---|
| Before | 47 | 37.4 | 25.4 to 47.9 | 17.2 | 122 |
| After | 19 | 35.9 | 29.0 to 39.5 | 12.6 | 124 |

**Inference.** The change shortens the median critical path by about 1.5 minutes,
against about 3 predicted, and removes the slowest tail, with runner minutes about the
same. The difference is small against runner variance, so it is not clearly shorter in
the sense the third ruling's disposition set. The before group mixes pull-request and
branch runs over a day of changing tests, and the after group is one evening.
(a) Keep the lists. The cost is the guard in `tests/ci_shard_coverage.rs`, which fails
a stale entry rather than dropping a test, and an occasional refresh of the lists.
(b) Return to plain hashing and remove the lists and their guard.
(c) Keep the lists and add a fifth shard, which shortens each shard by about a fifth
and adds a job.

**D2. Apply `idleCompaction: false` for a trial.** *Judgment.* The setting is absent
from every settings file, and the sandbox denies this line writes to them. By
[the trial report](./2026-10-31-idle-compaction-trial.md), a compacted resumption shows
about 0.8 points more tool errors in its next 50 calls than an uncompacted one past the
cache lifetime, and writes about 294,000 fewer cache tokens on its first call. That is
suggestive only.
(a) Apply it in the global settings for two weeks, after which this line reruns
`context_quality.py` and reports.
(b) Apply it in one project's settings only. Whether the key is honoured at project
scope is a hypothesis, not checked.
(c) Leave compaction as it is and close the trial.

**D3. K13, the proofs branch.** *Obvious, but only the operator can act.* F1 (a)
released the proofs branch, yet `origin/proofs` is unchanged at `6537a36f`, four of its
files match kept disclosure terms, and the proofs inbox holds the release unread.
(a) Start a proofs session when convenient, which reads its inbox and corrects the
branch.
(b) Leave the branch as it is until the proofs line resumes for its own reasons.

## The operator's own checks

These need an action, not a decision, and are unchanged since 2026-10-30. K6, look in
the repository settings for "Require merge queue". K9, the account plan, which bears on
the job-log egress limit met on 2026-10-30. K10, whether the self-host archive storage
is billed. K11, the blog's fixes, since the blog's Pages build holds a shared runner for
much of a busy day. K14, the pydromon changeover, held.

## Obvious choices

Each follows from an adopted next increment or an earlier ruling.

- **O1.** Check the first scheduled fuzz run at 10:17 UTC on 2026-11-01, including its
  crash path and artifact upload, and report a failure to the operator.
- **O2.** Keep the pinned lists in place until D1 is ruled, and add the after-change
  runs to the evidence as they arrive.
- **O3.** N8, the agent worktrees. Claude Code makes its agent trees under
  `.claude/worktrees/` without `worktree.sh`, so they build on the data volume, four of
  them 11.7 to 11.9 GB on 2026-10-31. Have `worktree.sh list` report a tree whose
  `target` is not a link, deleting nothing, and check whether Claude Code offers a
  worktree creation hook that could link them. A hook is a hypothesis until checked, and
  a hook would live in a settings file, which is the operator's to apply. The four trees
  are V0.2.X's live trees and are left alone, and moving or deleting their output is the
  operator's decision.
- **O4.** Rewrite this line's state file to record the closures above, within its
  budget.
- **O5.** Leave V0.2.X's concerns with V0.2.X. Its record lists the host field-order
  contract, the native supplement copy and the `Text<N>` trust assumption as accepted or
  owned there, and none needs a kaizen ruling.

## Questions held by other lines

**V0.2.X.** None. Its record states that no decision waits on the operator. Its last
listed next step is the release, which the operator takes up once that line is
otherwise idle.

**V0.3.X.** None. Its handoff records the operator's first-hand answers to questions 22,
24, 25, 27 and 28 and states that no question is open.

## Verification limits

The shard figures are job durations from start to completion, which exclude queue time
and include checkout and setup. Runs that failed or were cancelled are excluded, and so
is any run with fewer than four successful shard jobs. Test-seconds were not measured
for the after group, so the split between pinned and hashed runs inside a shard is not
known. The peer questions were read on 2026-11-01 as cited above, and the marks are the
kaizen line's judgment, not the owning lines'.

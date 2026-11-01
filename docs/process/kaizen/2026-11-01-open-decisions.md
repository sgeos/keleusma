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

## Operator ruling, 2026-11-01

The operator ruled first-hand in this line's session on 2026-11-01, in these words.

> Re: Decisions.
>
> D1. A.
> D2. A.
> D3. A. I started a proofs session.
> D4. A.
>
> Re: K-series. We will circle back.
>
> Re: Obvious choices. Accept all obvious choices and take appropriate action.

| Item | Disposition |
|---|---|
| D1 | (a). The pinned lists stay, and nothing changes |
| D2 | (a). **Fact.** The Claude Code settings reference lists `idleCompaction` as a Boolean, default `true`, honoured in any settings file, from version 2.1.290, and this machine runs 2.1.316. The sandbox denies this line writes to settings files, so the operator adds `"idleCompaction": false` to the global `settings.json`. The two weeks start on the day it is applied, after which this line reruns `context_quality.py` |
| D3 | (a). The proofs session merged its branch into `v0.2.3` as pull request 708, `b24d05c1`, and the remote `proofs` branch is gone. Its last commit, `dc1b784b`, and `origin/v0.2.3` at `b24d05c1` match no disclosure term on this line's scan. K13 is closed |
| D4 | (a) was tried and abandoned. `minimatch` 11.0.3 removes the `Minimatch.prototype.makeRe` method, which `vscode-languageclient` 11.0.0 calls in `globPattern.js` and `fileOperations.js`, so the override would throw at run time when the language server registers a file watcher or a file operation filter. The extension's CI job parses the manifest, checks the client's syntax and runs `npm ci`, and all three passed with the override, so that job cannot detect this break. Nothing was pushed. The alert stays open, which is option (b), until the operator chooses otherwise |
| K-series | Open. The operator will return to them |
| O1 to O5 | Adopted. O3 is carried out in the pull request that records this ruling. D5 below follows from it |

**Correction to D4.** The list said the lockfile holds `minimatch` 10.2.6. That is true
of `v0.2.3`, which uses `vscode-languageclient` 11.0.0. `main`, where the alert opened,
holds `minimatch` 5.1.9 under `vscode-languageclient` 8.1.0. The advisory names no
patched version below 11.0.1, so neither line has an in-range fix.

**D5. A worktree hook for Claude Code's agent trees.** *Judgment, new.* O3 adds to
`scripts/worktree.sh list` a report of every package `target` in a live tree that is a
directory and not a link, deleting and moving nothing. On 2026-11-01 it named the main
checkout's `target`, 182 MB, and one agent tree's, 2.4 GB. The four agent trees of
2026-10-31 are no longer listed. **Fact**, from the Claude Code hooks reference: a
`WorktreeCreate` hook replaces Claude Code's own creation of a worktree, receives a
name and no path, must create the tree itself and print its absolute path, and is not
run for `git worktree add` in a shell. A `WorktreeRemove` hook receives the tree's path.
Neither runs when a subagent's tree is cleaned up at the end of the subagent.
(a) Write a tracked hook script that creates the tree where Claude Code would and links
its targets through `scratch.sh`, with a removal hook that deletes its scratch folder,
both tested on throwaway repositories as `worktree.sh` is. The operator then adds the
hooks to the project settings, which this line cannot write. Because the hook replaces
the default creation, `.worktreeinclude` would no longer be read, and the subagent
cleanup path would leave scratch folders for `worktree.sh list` to report.
(b) Keep the report only, and let the operator move or delete large agent output.

## Operator ruling, 2026-11-01, second

The operator ruled first-hand in this line's session later on 2026-11-01, in these
words.

> D2. Where exactly do I add `"idleCompaction": false`?
> D3. The proofs line is now properly in a holding state.
> D4. Take appropriate action under the circumstances.
> O3. Investigate and advise.
> O1. Acknowledged.
> D5. A.
> Minisign. I meant signing public releases. MCP Agent Mail uses minisign, as I
> understand it.

| Item | Disposition |
|---|---|
| D2 | Answered in the session. The key goes at the top level of the global `settings.json`, or through the "Idle compaction" row of `/config`, which the Claude Code changelog lists from version 2.1.302. On 2026-11-01 the key was absent from that file |
| D3 | The proofs line holds, on the operator's word. Nothing remains for this line |
| D4 | The alert stays open, with the reasons below. It is not dismissed |
| O3 | Investigated, with the advice below |
| D5 | (a) chosen, and paused before implementation on a finding that bears on it, below |
| Minisign | [The release signing proposal](./2026-11-01-release-signing-proposal.md) |

**D4, the action taken.** **Fact.** Upstream tracks the fix in
`microsoft/vscode-languageserver-node` issue 1938, "Release with minimatch 11 to fix
GHSA-g3hv-75wg-m249", and Dependabot pull request 1935 in that repository, both open
since 2026-10-31. **Fact.** The advisory's vulnerable path is the preprocessing of a
glob pattern, so the attacker must supply the pattern. **Fact.** `keleusma-lsp`
registers no file watchers and declares no file operation filters, and the extension's
document selector names a language and a scheme with no pattern. **Inference.** No
pattern from outside the project reaches `minimatch` in this extension, so the
exposure is low. Leaving the alert open keeps it visible and lets Dependabot propose
the update once a compatible `vscode-languageclient` is released, which a dismissal
would hide. When V0.2.X's pull request 707 merges, `main` moves from
`vscode-languageclient` 8.1.0 to 11.0.0, which is still affected, so the alert stays
until upstream releases.

**O3, the investigation.** `worktree.sh list` on 2026-11-01 reported three things.

- `target` in the main checkout, 182 MB, the main checkout's own
  build output. That checkout is on `v0.2.3` and belongs to V0.2.X. *Advice.* Link it
  with `scripts/scratch.sh link target` from inside it when nothing builds there, at
  V0.2.X's or the operator's convenience. The saving is small.
- `.claude/worktrees/agent-ab1f9687f23f80567`, 2.4 GB, on
  `fix/v02-arena-capacity-inputs`. **Fact.** Its head `972c9c50` of 2026-10-30 is an
  ancestor of `origin/v0.2.3`, merged through `8a86b7ca`. Its status is clean. Its
  build output was last written at 13:18 on 2026-10-30 local time. Git records it as
  locked by a Claude agent with process 83575, started on 2026-10-24. This line cannot
  list processes inside the sandbox, so whether that process is alive is not known.
  *Advice.* If `ps -p 83575` shows nothing, the lock is stale. Then
  `git worktree unlock` and `git worktree remove` on that path, and deleting its local
  branch, lose no work. That is the operator's or V0.2.X's to do.
- `/Volumes/Scratch/keleusma/removed-worktree-tmp-20261101`, 52 MB. **Fact.** It holds
  ten `.tgz` archives named after removed worktrees, the largest
  `retcon-delegation-tmp.tgz` at 49 MB, all written at 20:27 local time on 2026-10-31.
  It is a deliberate archive of temporary folders, not build output, and this line did
  not make it. *Advice.* Keep it under a folder name that cannot match a tree, such as
  `/Volumes/Scratch/keleusma/.archive/`, so the report stops naming it, or delete it
  once its maker confirms it is no longer needed.

Two more trees of this repository live in the blog project, at
`tmp/keleusma-021` and `tmp/keleusma-022` of the blog's checkout, detached at the
tags `v0.2.1` and `v0.2.2`. They hold no build output. They are the blog's to keep or
remove.

**D5, the finding that paused it.** **Fact**, from the Claude Code hooks and worktrees
references read on 2026-11-01. A `WorktreeCreate` hook replaces Claude Code's own
creation entirely. Claude Code then keeps every worktree the hook made for a subagent
or a workflow agent, does not run `WorktreeRemove` for it, and states that such trees
must be removed by hand. Without a hook, a periodic sweep removes subagent worktrees
older than `cleanupPeriodDays`, and Claude Code holds a worktree lock while an agent
runs. The hook would also have to reproduce the default base, the remote's default
branch kept current through `origin/HEAD`, and the checks Claude Code makes before
creating a tree. **Inference.** The hook trades build output on the data volume for
whole source trees that accumulate on it, which is a regression for the purpose D5
serves.

An alternative leaves creation to Claude Code. Cargo's `build.build-dir` setting,
available in the pinned Cargo 1.99.1, takes the template `{workspace-path-hash}`, and
Cargo reads configuration from every parent directory of the working directory. On a
test of 2026-11-01 with a throwaway crate, a `build-dir` set in a parent directory's
`.cargo/config.toml` sent `deps`, the test executables, incremental data and build
scripts to the scratch path and left 4 KB in the tree's `target`. The tracked
`.cargo/config.toml` sets neither `build-dir` nor `target-dir`, so it would not
override it.

(c) Create one untracked file, `.claude/worktrees/.cargo/config.toml` in the main
checkout, holding

```toml
[build]
build-dir = "/Volumes/Scratch/keleusma/agents/{workspace-path-hash}"
```

The sandbox does not let this line write in the main checkout, so the operator creates
it. No settings file and no hook change. Its limits are these. The folders on scratch
are named by hash, so a swept tree leaves a folder that only its age identifies, and
`worktree.sh list` cannot attribute it. The path-scoped `pkill` pattern, which matches
`target/debug/deps` inside the tree, would not match an agent tree's test binaries.
The build output of the trees under `.worktrees/` and `../keleusma-worktrees/` is
unaffected, since they are not below `.claude/worktrees/`.
(a) The hook pair, as chosen, with the regression above.
(b) The report only.

This line recommends (c) and asks the operator to confirm it, or to keep (a) knowing
the regression.

## Operator ruling, 2026-11-01, third

The operator ruled first-hand in this line's session later on 2026-11-01, in these
words, and pasted V0.2.X's reply to the O3 advice, which is summarized below.

> D2. Nominally complete.
> D5. C. Recommendation accepted.
> Minisign. Recommendation accepted.

| Item | Disposition |
|---|---|
| D2 | Applied. **Fact.** The global `settings.json` holds `"idleCompaction": false`, written at 05:42 UTC on 2026-11-01. The trial runs to about 2026-11-15, when this line reruns `context_quality.py` |
| D5 | (c). The operator creates `.claude/worktrees/.cargo/config.toml` in the main checkout, and this line has made `/Volumes/Scratch/keleusma/agents`. `worktree.sh list` now skips a `target` under 1 MB, which is what such a tree keeps. Whether the file takes effect is checked on the next agent tree that builds |
| Release signing | (a) with (c) of [the proposal](./2026-11-01-release-signing-proposal.md), adopted, beginning at the first release after the operator has made a key. `scripts/release_manifest.py` builds the manifest |

**O3, V0.2.X's reply and a correction.** V0.2.X reports that process 83575 is alive. It
is the long-running Claude Code process, started at 22:21 local time on 2026-10-23,
which locks every agent worktree it creates and held the locks on three agent trees
V0.2.X unlocked on 2026-11-01 after their agents finished. **Correction.** The test
this line gave, whether the locking process is alive, cannot show that a lock is
stale, because the lock names the harness and not the agent. The tree's branch is
merged and its status is clean, which is the evidence that no work would be lost.
V0.2.X took none of the three actions. It removes the agent tree on the operator's
word. It names V0.3.X as the likely maker of the archive folder, because the archives
are named after V0.3.X worktrees. It does not touch the main checkout, so linking its
`target` needs the operator's authorization. Those three remain the operator's.

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

**D4. Dependabot alert 6, `minimatch` in the editor extension.** *Judgment.* The alert
opened at 22:07 UTC on 2026-10-31 against `editors/vscode/package-lock.json` on `main`,
advisory GHSA-g3hv-75wg-m249, a moderate denial of service through glob pattern
amplification, fixed in `minimatch` 11.0.1. The lockfile holds 10.2.6 as a runtime
dependency of `vscode-languageclient`. **Fact.** Every published `vscode-languageclient`
up to the latest, 11.0.0 of 2026-10-22, requires `minimatch` `^10.2.6`, so no upstream
release admits the fix. **Inference.** The vulnerable patterns reach the client from the
language server's file watcher registrations, which the project's own `keleusma-lsp`
sends, so an attacker-supplied pattern is unlikely. That is not verified.
(a) Force `minimatch` 11.0.1 through an npm `overrides` entry on a branch, and keep it
only if the extension builds and its tests pass. A green security-only pull request
into `main` is pre-authorized. The override puts a major version under a library that
did not ask for it.
(b) Leave the alert open and check for an upstream release weekly.
(c) Dismiss the alert as tolerable risk, giving the reason above. A dismissal is
visible on GitHub, so it is the operator's.

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

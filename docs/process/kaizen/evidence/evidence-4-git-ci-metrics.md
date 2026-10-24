# Evidence 4: git and continuous-integration metrics (miner report, Sonnet, 2026-09-08, all EXECUTED)

Raw data files in the session scratchpad: ci_runs_full.jsonl, prs_merged_fresh.json, subjects_v0.2.3.txt, subjects_v0.3.0.txt, numstat_*.txt, unmerged_*.txt, cadence.txt. Copied below unedited.

EXECUTED metrics report. Repository sgeos/keleusma. Working tree
/Users/bsechter/projects/rust/keleusma-worktrees/kaizen. All commands ran read only. No
repository file was edited, no cargo command was run, no state changing git command was issued.

## 1. Cadence

EXECUTED for `origin/v0.2.3`, `origin/v0.3.0`, `origin/proofs`, `origin/main`.

```
git log --format=%H "$b" | wc -l
git log --format="%ad" --date=short "$b" | tail -1   # oldest
git log --format="%ad" --date=short "$b" | head -1   # newest
git log --merges --format=%H "$b" | wc -l
git log "$b" --format=%ad --date=format:%Y-%W | sort | uniq -c
git log "$b" --format=%ad --date=short | sort | uniq -c | sort -rn | head -10
git log "$b" --format="%b" | grep -i "^Co-Authored-By:" | sed -E 's/^Co-Authored-By:\s*//I' \
  | sed -E 's/\s*<[^>]*>$//' | sort | uniq -c | sort -rn
```

| branch | first commit | last commit | total commits | merge commits | distinct days |
|---|---|---|---|---|---|
| v0.2.3 | 2026-03-02 | 2026-09-08 | 2124 | 328 | 106 |
| v0.3.0 | 2026-03-02 | 2026-09-08 | 2851 | 518 | 106 |
| proofs | 2026-03-02 | 2026-08-29 | 1860 | 219 | 94 |
| main | 2026-03-02 | 2026-07-24 | 781 | 3 | 52 |

All four branches share root history through 2026-07-09, so early weekly counts are identical
by construction. main stops after 2026-07-24. v0.2.3, v0.3.0, and proofs diverge after that and
run as separate lines. main carries only 3 merge commits against 781 total commits, a fact this
audit did not trace to a mechanism.

Commits per ISO week, `%Y-%W`.

| week | v0.2.3 | v0.3.0 | proofs | main |
|---|---|---|---|---|
| W10 | 5 | 5 | 5 | 5 |
| W19 | 98 | 98 | 98 | 98 |
| W20 | 63 | 63 | 63 | 63 |
| W21 | 169 | 169 | 169 | 169 |
| W22 | 61 | 61 | 61 | 61 |
| W23 | 67 | 67 | 67 | 67 |
| W24 | 64 | 64 | 64 | 64 |
| W25 | 6 | 6 | 6 | 6 |
| W26 | 35 | 35 | 35 | 35 |
| W27 | 70 | 70 | 70 | 70 |
| W28 | 288 | 288 | 288 | 141 |
| W29 | 84 | 84 | 84 | 1 |
| W30 | 123 | 123 | 123 | 1 |
| W31 | 55 | 55 | 55 | 0 |
| W32 | 133 | 197 | 133 | 0 |
| W33 | 319 | 552 | 319 | 0 |
| W34 | 207 | 351 | 201 | 0 |
| W35 | 119 | 278 | 19 | 0 |
| W36 | 150 | 270 | 0 | 0 |
| W37 | 6 | 14 | 0 | 0 |

v0.3.0 pulls ahead of v0.2.3 from W32, consistent with periodic `merge/absorption-N` merges
(named in section 6) folding v0.2.3 history into v0.3.0. proofs stops after W35, matching its
last commit date above.

Heaviest single days, top 3 of a measured top 10, full top 10 in scratch file `cadence.txt`.

| branch | day 1 | day 2 | day 3 |
|---|---|---|---|
| v0.2.3 | 2026-07-11 (71) | 2026-08-10 (69) | 2026-07-12 (65) |
| v0.3.0 | 2026-08-09 (126) | 2026-08-10 (117) | 2026-08-14 (110) |
| proofs | 2026-07-11 (71) | 2026-08-10 (69) | 2026-07-12 (65) |
| main | 2026-05-31 (49) | 2026-05-19 (49) | 2026-07-09 (47) |

Co-Authored-By trailer occurrences, by distinct name string, scanned over the full commit
message body since the trailer lives in the body, not the subject. A commit can carry more than
one trailer, so a name's count is not a commit count.

| name | v0.2.3 | v0.3.0 | proofs | main |
|---|---|---|---|---|
| Claude | 873 | 1457 | 743 | 135 |
| Claude Opus 4.8 (1M context) | 607 | 607 | 607 | 342 |
| Claude Opus 4.7 (1M context) | 291 | 291 | 291 | 291 |
| Claude Opus 5 (1M context) | 38 | 36 | 0 | 0 |
| Claude Fable 5 | 37 | 41 | 41 | 0 |
| Claude Opus 4.6 | 5 | 5 | 5 | 5 |

The bare `Claude` label most plausibly aggregates several early, unversioned sessions predating
the per version trailer convention. Inference, not a measured fact.

## 2. Pull Requests

EXECUTED. The reused scratch `prs_merged.json` held 374 rows, one short of a live count, so it
was refreshed.

```
gh api "search/issues?q=repo:sgeos/keleusma+is:pr+is:merged&per_page=1" --jq '.total_count'
# -> 375
gh pr list --state merged --limit 500 \
  --json number,title,createdAt,mergedAt,baseRefName,additions,deletions,changedFiles
  # -> 375 rows after refresh, matching the live count
gh pr list --state open --json number,title,createdAt,baseRefName
gh pr list --state closed --limit 500 --json number,title,mergedAt
```

| baseRefName | merged PR count |
|---|---|
| v0.2.3 | 255 |
| v0.3.0 | 120 |

| state | count |
|---|---|
| open | 1 (#376, base v0.2.3, opened 2026-09-08T08:25:54Z) |
| closed, not merged | 0 |
| merged | 375 |

Cycle time, createdAt to mergedAt, all 375, hours: median 0.84 (about 50 min), p90 2.30, mean
1.42, min 0.002 (about 8 s), max 26.68. The near instant minimum is consistent with either
an operator merging within seconds of opening or a scripted merge, not distinguishable here.

Size, additions plus deletions, all 375: min 0, q1 109, median 265, q3 496, p90 1084, max 18815.
changedFiles: median 3, p90 10, max 66.

Title prefix classification, leading conventional scope such as `docs(` or `docs:`.

| prefix | count | share |
|---|---|---|
| test | 89 | 23.7% |
| docs | 85 | 22.7% |
| feat | 83 | 22.1% |
| fix | 31 | 8.3% |
| merge | 9 | 2.4% |
| perf | 2 | 0.5% |
| sync | 2 | 0.5% |
| chore | 1 | 0.3% |
| refactor | 0 | 0.0% |
| unmatched | 73 | 19.5% |

docs alone is 22.7% of 375. feat, fix, test, and refactor combined is 203, 54.1%. The 73
unmatched titles are full sentence titles in house style, for example "Lift the artifact
ceiling, and rewrite a handoff that lied," most reading as process work by content though
carrying no scope prefix. Not independently coded, so the docs share above is a lower bound.

## 3. Continuous Integration

EXECUTED. `gh run list --workflow CI --limit 500` caps at 500 rows against a true total of
1387, so the full history was pulled through the REST API instead.

```
gh api "repos/sgeos/keleusma/actions/workflows/271683428/runs?per_page=1" --jq '.total_count'
# -> 1387
gh api --paginate "repos/sgeos/keleusma/actions/workflows/271683428/runs?per_page=100" \
  --jq '.workflow_runs[] | {databaseId: .id, conclusion, status, createdAt: .created_at, \
  updatedAt: .updated_at, headBranch: .head_branch, event, runStartedAt: .run_started_at}'
  # -> 1387 lines, full history
```

Conclusion, all 1387 runs: success 1209 (87.2%), failure 110 (7.9%), cancelled 67 (4.8%), one
run still in_progress at audit time (0.1%).

Conclusion by head branch pattern, top patterns by volume.

| branch pattern | success | failure | cancelled |
|---|---|---|---|
| v0.2.3 | 436 | 8 | 4 |
| v0.3.0 | 279 | 5 | 4 |
| feat/* | 198 | 7 | 15 |
| main | 95 | 90 | 0 |
| docs/* | 95 | 0 | 12 |
| fix/* | 45 | 0 | 5 |
| test/* | 22 | 0 | 23 |
| merge/* | 21 | 0 | 2 |

main is a near even split, 95 success against 90 failure of 185 runs, a 48.6% failure rate,
markedly above every other branch pattern. This audit records the figure and does not
attribute a cause, and did not examine which jobs failed.

Duration, successful runs, createdAt to updatedAt, minutes, n = 1209: median 47.0, p90 59.7,
max 138.5, min 0.4.

Longest runs overall, any conclusion, runStartedAt to updatedAt.

| run id | branch | conclusion | duration | date |
|---|---|---|---|---|
| 32211341993 | v0.2.3 | cancelled | 360.8 min | 2026-08-19 |
| 32272247534 | v0.2.3 | cancelled | 360.6 min | 2026-08-19 |
| 32091414949 | v0.2.3 | cancelled | 360.3 min | 2026-08-18 |
| 32089347195 | v0.2.3 | cancelled | 360.3 min | 2026-08-18 |
| 31850697785 | v0.3.0 | success | 98.3 min | 2026-08-14 |

The four cancelled runs cluster near 360 minutes, consistent with a 6 hour job or workflow
timeout ceiling rather than organic length. The workflow YAML's `timeout-minutes` was not
inspected to confirm this, so it remains an inference.

Failure followed by success on the same headBranch within 24 hours: 54 recovery episodes of 110
total failures, a 49.1% within window recovery rate.

Five sampled successful v0.2.3 runs, spread across history, `gh run view <id> --json jobs`.

```
30123197802  2026-07-24
31395747431  2026-08-10
31903663386  2026-08-15
32743794220  2026-08-24
34197639348  2026-09-08
```

Per job average duration across the five, top entries.

| job | n | avg | min | max |
|---|---|---|---|---|
| Test | 5 | 39.0 min | 24.4 min | 59.8 min |
| Test (self-host feature) | 4, absent from oldest run | 37.7 min | 16.8 min | 59.6 min |
| Test (signatures feature) | 5 | 30.0 min | 17.3 min | 46.7 min |
| Test (broad features) | 5 | 28.9 min | 21.2 min | 36.6 min |
| Self-hosted compiler subproject | 5 | 21.8 min | 12.5 min | 30.0 min |
| RTOS demonstrator build | 5 | 3.8 min | 3.5 min | 4.4 min |
| remaining jobs | 5 each | under 2 min each | | |

The bottleneck job is `Test`, the default features job, at 39.0 min average and 59.8 min on
2026-09-08, with `Test (self-host feature)` close behind. These test jobs run in parallel within
one workflow run, so the run's wall clock tracks the slowest job plus queue and setup overhead,
consistent with the roughly 47 minute median measured above.

## 4. Rework

EXECUTED. `git log --grep` searches the full message body, not the subject, and grossly
inflated matches when tried first, 483 v0.2.3 subjects flagged for `correct` alone, most with no
such text in the subject itself. The corrected method extracts `%s` and matches the subject
string only.

```
git log "$b" --format="%H|%ad|%s" --date=short > subjects_$name.txt
# then a subject only, case insensitive substring match per keyword
```

Per keyword subject counts, subject line only.

| keyword | v0.2.3 | v0.3.0 |
|---|---|---|
| retract | 16 | 30 |
| correct | 61 | 92 |
| revert | 4 | 4 |
| was wrong | 2 | 3 |
| stale | 21 | 41 |
| carried | 2 | 4 |
| fabricat | 0 | 3 |
| distinct subjects, at least one hit | 90 (4.2%) | 157 (5.5%) |

v0.3.0's history is a superset of v0.2.3's through the absorption merges in section 1, so these
two columns are not independent samples and a higher v0.3.0 rate should not be read as a higher
correction rate per unit of original work.

The three `fabricat` hits, all v0.3.0, all `docs(process)` or adjacent, are dated 2026-08-26
"absorption 11 re-derives every census, and the provenance audit had fabricated a number",
2026-09-02 "record how to run the sweep, and why contention fabricates coverage", and 2026-09-06
"fix(native): the mutation sweep's instrument was fabricating coverage, and was 400x too slow".

Process versus code classification, subject line only.

| class | v0.2.3 | v0.3.0 |
|---|---|---|
| `docs(process)` prefixed | 261 (12.3%) | 398 (14.0%) |
| mentions handoff, mailbox, or REVERSE_PROMPT/HANDOFF | 136 (6.4%) | 183 (6.4%) |
| feat, fix, test, or refactor prefixed | 1085 (51.1%) | 1357 (47.6%) |

Lines added and removed under three path prefixes, non merge commits since 2026-07-10,
`--numstat`, which is empty for merge commits by default.

| path | v0.2.3 + | v0.2.3 - | v0.3.0 + | v0.3.0 - |
|---|---|---|---|---|
| docs/process/ | 33,643 | 15,506 | 52,084 | 21,126 |
| src/ | 29,407 | 4,302 | 29,414 | 4,303 |
| tests/ | 65,445 | 10,906 | 65,454 | 10,910 |
| everything else | 50,783 | 11,909 | 133,351 | 16,581 |
| branch total | 179,278 | 42,623 | 280,303 | 52,920 |

src/ and tests/ totals are nearly identical between branches, consistent with the same
implementation work landing on both through absorption. The gap in "everything else," 38,874
net lines on v0.2.3 against 116,770 net on v0.3.0, most plausibly sits in the self-hosted
compiler tree and other docs/ subdirectories, a hypothesis this audit did not verify by further
path breakdown.

## 5. Gate Cost

EXECUTED. `grep -rn -E '[0-9]+m[0-9]+s|minutes|wall-clock|hours' scripts/ docs/process/
.github/` returned mostly narrative incident accounts in `DESIGN_JOURNAL.md` and `TASKLOG.md`
rather than a stated policy figure. Load bearing lines:

```
scripts/gate-status.sh:6:  A full gate runs 2 to 3.5 hours in a detached worktree
docs/process/PROCESS_STRATEGY.md:258:  A gate runs two to three and a half hours in a detached
                            worktree
docs/process/HANDOFF.md:506:  verified strict superset and runs in ~48 minutes against ~2h30m
docs/process/PROCESS_STRATEGY.md:172:  One stage self-compile went from 54 seconds to over 37
                            minutes
docs/process/GIT_STRATEGY.md:198:  Feature branches should be short-lived, ideally under 24
                            hours
```

`scripts/release-gate.sh`'s header states it is a superset of the CI workflow plus the detached
`compiler/` subproject and instructs the operator to run it whole, never cut down before a
release. It states no numeric time budget itself, the 2 to 3.5 hour figure lives in
`gate-status.sh` and `PROCESS_STRATEGY.md`. `scripts/gate-summary.sh` states no run time at all,
its header instead documents a past incident where summing test counts across the gate's
several `test result:` sections produced a wrong total that reached a commit message and a
merged pull request body.

Gate log location, from `scripts/gate-status.sh`.

```
TREES_DIR="${KEL_WORKTREES_DIR:-"$REPO_ROOT/../keleusma-worktrees"}"
done < <(ls -t "$TREES_DIR"/*.log 2>/dev/null)
```

Gate logs are one `.log` file per worktree, in the parent `keleusma-worktrees` directory next
to the repository, or wherever `KEL_WORKTREES_DIR` points. `STALE_AFTER` for a running looking
gate defaults to 240 seconds without a log write. No stated tiered structure, for example a fast
tier against a full tier with distinct budgets, was found in any of the three script headers.
The 2 to 3.5 hour figure covers one whole `release-gate.sh` run, with no per step breakdown
found in the text searched.

## 6. Branch Sprawl

EXECUTED.

```
git branch -r | grep -v 'HEAD ->' | wc -l                        # -> 207
git branch -r --merged origin/v0.2.3 | grep -v 'HEAD ->' | wc -l  # -> 150
git branch -r --merged origin/v0.3.0 | grep -v 'HEAD ->' | wc -l  # -> 195
git branch -r --no-merged origin/v0.2.3 | grep -v 'HEAD ->'       # -> 57 branches
git branch -r --no-merged origin/v0.3.0 | grep -v 'HEAD ->'       # -> 12 branches
```

Raw `git branch -r | wc -l` reads 208, one above the 207 distinct branches, because it includes
the `origin/HEAD -> origin/main` alias line, excluded from every count above.

| | into v0.2.3 | into v0.3.0 |
|---|---|---|
| merged | 150 (72.5%) | 195 (94.2%) |
| unmerged | 57 (27.5%) | 12 (5.8%) |

v0.3.0 has absorbed nearly all branch tips, consistent with its role as the newer, wider
integration line. v0.2.3's larger unmerged set concentrates in `feat/native-*` and
`merge/absorption-N` branches, both families that by name feed v0.3.0 rather than v0.2.3, so
their being unmerged into v0.2.3 specifically is expected rather than anomalous. Branch content
was not read to confirm this reading.

Unmerged into v0.2.3, 57 branches, oldest and newest three of the full list, which is recorded
in scratch file `unmerged_v023.txt`.

```
2026-08-09  origin/feat/llvm-backend-spike
2026-08-09  origin/feat/native-byte-and-bounds
2026-08-09  origin/feat/native-coverage-spike
...
2026-09-08  origin/fix/opaque-flat-field-width
2026-09-08  origin/kaizen
2026-09-08  origin/v0.3.0
```

Unmerged into v0.3.0, all 12 branches.

```
2026-08-09  origin/feat/llvm-backend-spike
2026-08-09  origin/feat/native-byte-and-bounds
2026-08-09  origin/feat/native-coverage-spike
2026-08-10  origin/feat/native-data-segment
2026-08-11  origin/feat/native-bounds-transfer
2026-08-14  origin/feat/native-stream-refusals
2026-08-29  origin/proofs
2026-09-07  origin/docs/arm-11-closed
2026-09-08  origin/docs/handoff-session-63-close
2026-09-08  origin/fix/opaque-flat-field-width
2026-09-08  origin/kaizen
2026-09-08  origin/v0.2.3
```

Six branches sit unmerged into both lines: `feat/llvm-backend-spike`,
`feat/native-byte-and-bounds`, `feat/native-coverage-spike`, `feat/native-data-segment`,
`feat/native-bounds-transfer`, `feat/native-stream-refusals`, last commits between 2026-08-09
and 2026-08-14, roughly four weeks stale against the 2026-09-08 audit date. Whether these are
abandoned spikes or work still queued for absorption was not determined, since that would
require reading commit content outside this audit's scope.

## Figures not obtained or only partially obtained

No fresh gate timing was measured directly, per the read only, no cargo constraint, only the
stated figures in script headers and process documents. No per step timing breakdown of a full
`release-gate.sh` run was found anywhere searched, only the whole run figure and the one CI
comparison figure of about 48 minutes against about 2 hours 30 minutes. The "everything else"
path bucket in section 4 is not broken out further, it includes at minimum `docs/` outside
`docs/process/`, `compiler/`, and configuration files. Section 3's per job CI timing rests on a
5 run sample per the task instruction, not the full 1387 run population, so those averages carry
unquantified sampling uncertainty.

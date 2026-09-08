# Evidence 1: the documented process (miner report, Sonnet, 2026-09-08)

Scope: kaizen worktree, branch `kaizen` (= v0.2.3 tip 802e72d3 + one handoff commit). Read-only.

## 1. The process in one paragraph

Keleusma is developed under a documented "agentic AI development loop" (READ, PROCESS_STRATEGY.md:26-50) layered onto a four-level git release-branch model (main -> vX.Y.Z -> feature -> sub-feature; READ, GIT_STRATEGY.md:9-18). Work is tracked through three single-writer channels, PROMPT.md (human->AI), REVERSE_PROMPT.md (AI->human, overwritten), DESIGN_JOURNAL.md (AI->human, append-only), plus a shared TASKLOG.md, with HANDOFF.md as a periodically refreshed self-validating resume prompt for compaction (READ, CLAUDE.md:106-157, COMMUNICATION.md:9-18). An "autonomous implementation loop" (AUTONOMOUS_IMPLEMENTATION_LOOP.md) lets the agent iterate on the self-hosted-compiler frontier without per-increment approval, surfacing only at an enumerated list of forks (opcode/wire changes, non-converging divergence, irreversible actions, exhausted frontier, token budget). Verification is tiered: seconds-scale fast-check.sh, a ~3-minute pre-commit tier, and a full release-gate.sh (~2h30m local vs ~48min on CI) mandatory before any merge into a version branch or main, and with --miri before a crates.io publish, governed by a separate Go/No-Go release procedure (RELEASE_PROCESS.md). Parallel agents coordinate by isolated worktrees and per-branch mailbox files, no live message bus (PARALLEL_DEVELOPMENT.md).

| Artifact | Purpose | Writer | Cadence | Size |
|---|---|---|---|---|
| CLAUDE.md | agent instructions, status, session protocol | human + AI | as needed | 227 lines |
| CONTRIBUTING.md | external contributor workflow, test tiers | human + AI | as needed | 134 lines |
| docs/process/PROMPT.md | human->AI staging, read-only for AI | human only | per prompt | 11 lines |
| docs/process/REVERSE_PROMPT.md | AI->human bounded latest state | AI | overwritten per task | 307 lines / 19,096 B |
| docs/process/DESIGN_JOURNAL.md | append-only increment reasoning, newest first | AI | every increment | 11,096 lines / 1,056,884 B |
| docs/process/TASKLOG.md | shared sprint source of truth | human + AI | incrementally | 2,165 lines / 434,064 B |
| docs/process/HANDOFF.md | self-validating resume prompt | AI | before planned compaction | 1,439 lines / 93,081 B |
| docs/process/GIT_STRATEGY.md | branch model, merge mechanics | human + AI | as needed | 270 lines |
| docs/process/PROCESS_STRATEGY.md | classification, tiered verification, gate lessons | human + AI | as needed | 314 lines |
| docs/process/COMMUNICATION.md | channel protocol | human + AI | as needed | 105 lines |
| docs/process/AUTONOMOUS_IMPLEMENTATION_LOOP.md | bounded-autonomy cycle and stop list | human + AI | as needed | 269 lines |
| docs/process/AUTONOMOUS_RESEARCH_LOOP.md | unattended research-firing protocol | human + AI | as needed | 168 lines |
| docs/process/PARALLEL_DEVELOPMENT.md | worktree/mailbox/gate-serialization protocol | human + AI | as needed | 527 lines |
| docs/process/RELEASE_PROCESS.md | Go/No-Go publish checklist | human + AI | as needed | 608 lines |
| docs/process/handoffs/README.md | mailbox convention | human + AI | as needed | 27 lines |
| handoffs/{kaizen,proofs,v0.2.3}.md | per-line mailboxes | AI per line | per sync | 160 / 84 / 2,504 lines |
| .cargo-husky/hooks/pre-push | push gate | human (versioned) | every push | 53 lines |
| .github/workflows/ci.yml | 22-job CI matrix | human | every push/PR to main, v* | 433 lines |
| scripts/*.sh (15) | gate, worktree, merge, status tooling | human + AI | as needed | 1,578 lines |

## 2. Pilot touchpoints

1. Session startup wait. CLAUDE.md:110 "Wait for human prompt before proceeding." Hard block, no default. (COMMUNICATION.md:74-78 has a different 3-step version, see section 4.)
2. Blocked/uncertain stop. CLAUDE.md:116. No default.
3. Semantics-changing decisions. PROCESS_STRATEGY.md:63. No default.
4. Significant-tradeoff decisions. PROCESS_STRATEGY.md:64. No default.
5. Approaching token limit. PROCESS_STRATEGY.md:65; default given in AUTONOMOUS_IMPLEMENTATION_LOOP.md:228-229 (checkpoint the three channels and stop).
6. Unresolvable assumption. PROCESS_STRATEGY.md:66. No default.
7. New opcode / record-node kind / wire field / BYTECODE_VERSION bump. AUTONOMOUS_IMPLEMENTATION_LOOP.md:174-178; CLAUDE.md:9. Workaround default (tag reuse + module tables), bump itself no default.
8. Oracle diverging and not converging. AUTONOMOUS_IMPLEMENTATION_LOOP.md:179-197; converging default is keep going.
9. Full gate red for pre-existing reason, or shared protocol / wire format change. AUTONOMOUS_IMPLEMENTATION_LOOP.md:198-200.
10. No remaining bounded roadmap candidate. AUTONOMOUS_IMPLEMENTATION_LOOP.md:201-222; NOT a stop when merely choosing among bounded tasks (loop orders by context then priority, :71-87); IS a stop when a candidate needs information only the operator holds. The list explicitly rejects rationalizations ("differ by an order of magnitude in cost", "this one wants a dedicated run", "which does the operator want first", "the cheap work is exhausted", :207-222), suggesting those evasions were observed.
11. Irreversible or outward-facing action (publish, force-push, tag). AUTONOMOUS_IMPLEMENTATION_LOOP.md:223-225 "a prior 'keep going' does not license these." Per-instance confirmation.
12. Frontier exhausted. :226-227 report and hand back.
13. CI-green confirmation (release step 6). RELEASE_PROCESS.md:110,325-343,355-362, human-confirmed; doctrine rule 3 (:36-41) bars self-certification.
14. Publish authorization (release step 8). RELEASE_PROCESS.md:112-113,355-386, enumerates what does NOT authorize (a prior "expedite", authorization for a different artifact or prior release). Hardest gate; V0.2.2 incident.
15. External release audit GO (step 7). RELEASE_PROCESS.md:345-353; maintainer-discretion default for delta-scoped review on small patches.
16. Yank-vs-supersede call. RELEASE_PROCESS.md:525-542.
17. Prose style, from the user-level CLAUDE.md. No project default.
18. PROMPT.md read-only for the AI. PROMPT.md:5, COMMUNICATION.md:22.
19. Kaizen-line rulings route through the operator only. handoffs/kaizen.md:89-92.
20. Roadmap ordering of v0.2.3 vs v0.3.0 into main. PARALLEL_DEVELOPMENT.md:354-356, deferred, "does not need answering to proceed" (default exists).

Pattern: defaults exist for ordering and pacing (proceed, keep going, checkpoint-and-stop), never for irreversibility, semantics, or self-certification.

## 3. Gates

| Gate | Runs | Mandatory when | Cost (documented) | Overlaps |
|---|---|---|---|---|
| Tier 0 fast-check.sh | fmt, clippy on touched crate, one test filter | every edit | "seconds" (PROCESS_STRATEGY.md:77); memoized self-host cache "from ~102s to ~0.02s" (fast-check.sh:19-22) | subset of tiers 1/2 |
| Tier 1 pre-commit (clippy workspace all-targets, test -p keleusma --no-default-features, cargo doc) | manual, every increment | ~3 min (PROCESS_STRATEGY.md:75-88) | subset of pre-push and release-gate |
| Pre-push hook .cargo-husky/hooks/pre-push | nextest --profile quick (self-host EXCLUDED, line 19), cargo test --doc, fmt --check, clippy -D warnings, cargo doc (docs.rs feature set), check-md-links.kel | every push (bypass --no-verify) | not timed; installed by cargo-husky on next dev-dep compile, not on clone (CONTRIBUTING.md:102-104) | duplicates CI default-feature jobs and CONTRIBUTING checklist by design (pre-push:3-4) |
| Tier 2 release-gate.sh | fmt, clippy, test matrix (default, no-default, signatures, signatures+shell), doc -D warnings all crates, doc-links, detached compiler/ subproject | once per merge into version branch or main, mandatory (GIT_STRATEGY.md:254-269) | ~2h30m local (GIT_STRATEGY.md:172), "2h33m ... the largest single calendar-time cost in the loop" (PROCESS_STRATEGY.md:112-113); heaviest suite run 4x (:98-101) | strict subset of CI (GIT_STRATEGY.md:177-182); per :162-168 CI, not the local gate, gates feature-branch merges since 2026-08-11 |
| CI ci.yml, 22 jobs (check, test x6 feature configs, examples-sdl3, doc, clippy, fmt, 2x MSRV, no-std, rtos-n6-build, lsp, selfhost-compiler, vscode-extension, wasm, miri x2, docs-links x2) | every push/PR to main, v* | ~48 min wall, 23 parallel jobs (GIT_STRATEGY.md:172) | verified strict superset of local gate "checked step by step rather than asserted" |
| Release local gate --miri | adds Miri (nightly, Tree Borrows) | before publish (RELEASE_PROCESS.md:239,247) | not timed | superset of local gate |
| gate-in-worktree.sh | release-gate against a pinned commit in an isolated worktree | recommended | same ~2h30m, non-blocking (gate-in-worktree.sh:6-9) | wraps tier 2 |
| docs-links / check-md-links.kel | relative link resolution + book drift check | pre-push, gate, CI | not timed | triply duplicated by design |
| Release steps 6-8 | CI-green confirm, external audit, publish authorization | mandatory Go/No-Go | human latency, "hold publication until the review is satisfied" (RELEASE_PROCESS.md:353) | unique to release |

## 4. Contradictions and drift

- Git model: CONTRIBUTING.md:47 "The project follows trunk-based development. Short-lived feature branches fast-forward into main" with an ff-only example (:49-58). GIT_STRATEGY.md:9-18 supersedes exactly this. CONTRIBUTING.md, the external-facing document, describes a workflow no longer in force.
- scripts/merge-to-trunk.sh vs GIT_STRATEGY.md:143-151, self-reported: "The script implements the linear form ... The script and this document have differed since both existed ... it will flatten a branch onto the spine without saying so." PARALLEL_DEVELOPMENT.md:319-329 repeats for v0.3.0.
- Mailbox scope: handoffs/README.md:11-12 and PARALLEL_DEVELOPMENT.md:45-49 say "A mailbox lives on the VERSION branch, never on a feature branch", path pattern handoffs/<version-branch>.md. EXECUTED: kaizen and proofs are not vX.Y.Z branches yet carry mailboxes under that convention. kaizen.md:62,71-73 calls itself a "top-level integration branch"; proofs.md:9 asserts the rule's spirit. A third, undocumented branch category ("top-level line") exists that the canonical text does not acknowledge.
- Session-startup protocol: CLAUDE.md:106-110 four steps beginning with HANDOFF.md; COMMUNICATION.md:74-78 under the same heading gives three steps and never mentions HANDOFF.md. Neither cross-references the other.
- Giant-file read burden undocumented as a cost: startup requires reading HANDOFF.md (93,081 B / 1,439 lines) and TASKLOG.md (434,064 B / 2,165 lines) every session with no excerpt mechanism, although COMMUNICATION.md:16 records REVERSE_PROMPT.md was split off DESIGN_JOURNAL.md because it "had accreted to ~362 KB (process-audit item 5)"; TASKLOG.md (434 KB) and HANDOFF.md have no comparable size discipline; HANDOFF.md:12-89 shows the accreted-prose pattern recurring.
- --all-features claim self-corrected in place, CLAUDE.md:9 "this file claimed the opposite until 2026-08-16."
- Test counts: CONTRIBUTING.md:14 "roughly 1,150 unit tests" vs CLAUDE.md:226 "1279 ... re-derive them rather than trusting the number."

## 5. Reactive rules

| Date | Failure | Rule/change |
|---|---|---|
| 2026-07-22 | REVERSE_PROMPT.md "accreted to ~362 KB" (process-audit item 5) | DESIGN_JOURNAL.md split out (COMMUNICATION.md:16, DESIGN_JOURNAL.md:9-13) |
| 2026-08-08 | Wire-format v2 port "functionally perfect and roughly forty times slower", every tier green | tests/perf_canary.rs tripwire (PROCESS_STRATEGY.md:168-185) |
| 2026-08-09 | Package .gitignore only on a feature branch; git add -A "staged 571 build artifacts" | root ignore files rule (GIT_STRATEGY.md:97-117) |
| 2026-08-09 | Mailbox on a feature branch returned "a day-old orientation document" | mailboxes pinned to version branch (PARALLEL_DEVELOPMENT.md:45-58) |
| 2026-08-09 | pgrep -f "release-gate.sh" "matched its own shell", deadlocked a session "for hours" | gate-status.sh uses log mtime; "never let a pgrep for a script name gate a loop's exit" (PROCESS_STRATEGY.md:124-153,256-277) |
| 2026-08-09 | CI and local gate silently diverged both directions | "CI must be a strict SUPERSET of the local gate" (PROCESS_STRATEGY.md:198-216) |
| 2026-08-11 | "gate time is the project's bottleneck and two sessions were serialising on one machine" | CI authoritative for feature-branch merges (GIT_STRATEGY.md:162-168) |
| V0.2.1 | shipped with red CI Doc job; pre-push bypassed with --no-verify | cargo doc -D warnings mandatory in gate, hook, CI (CLAUDE.md:193-197; RELEASE_PROCESS.md:268-272,570-572,584-586) |
| V0.2.2 | agent "treated an earlier 'V0.2.2 should be expedited' as standing authorization and published all four crates" | doctrine rule 3, enumerated non-authorizations (RELEASE_PROCESS.md:364-385,573-579) |
| process audit item 4 | detached compiler/ "gated nowhere"; stale decoder shipped "unknown op tag 62" | selfhost-compiler CI job; gate runs subproject (GIT_STRATEGY.md:269-270, ci.yml:340-353) |
| 2026-08-31 | awk one-liner summed test counts across feature sections, "179 binaries and 2904 tests" for a 113/2708 run; "reached a commit message and a merged pull-request body" | scripts/gate-summary.sh "one correct reader" |
| undated | interrupted gate left a test binary "reparented to PID 1 ... burning four cores for ten hours" | release-gate.sh reaps orphans (PROCESS_STRATEGY.md:300-305) |
| undated | CI concurrency grouping would cancel a version branch's own run, "~20 minutes ... asked three times" | concurrency group scoped PR-ref vs run_id (ci.yml:13-38) |

## 6. Passages on calendar time, pilot involvement, autonomy, compaction, coordination

- GIT_STRATEGY.md:172-176: wall clock ~2h30m local vs ~48 min CI (23 parallel jobs); local contends for the shared machine exclusively; two sessions at once impossible locally, yes on CI.
- PROCESS_STRATEGY.md:109-113: "running it directly freezes development for its whole duration. At ~2h33m per merge that was the largest single calendar-time cost in the loop, and it bought nothing."
- PARALLEL_DEVELOPMENT.md:16-19: "Two sessions ran in parallel for a day and the arrangement worked ... It also cost the operator an afternoon of relaying prose between them."
- PARALLEL_DEVELOPMENT.md:83-87: "the measured bottleneck is the gate at ~2h33m, and it is repetition-bound. A channel's ceiling is 1 / (1 - f_c) ... a perfect channel buys almost nothing while f_c is small and the gate dominates."
- AUTONOMOUS_IMPLEMENTATION_LOOP.md:15-19: "The operator does not have to say 'continue' each time, and operator silence means continue."
- AUTONOMOUS_IMPLEMENTATION_LOOP.md:245-250: HANDOFF.md before planned compaction; on resume validate by HEAD~1; "A resume familiarizes and reports first; the keep-going default is for an active session, not a cold resume."
- AUTONOMOUS_RESEARCH_LOOP.md:9,13,23,107: self-firing loop while operator away; per-task review checkpoint suspended for the duration; ScheduleWakeup at thirty-minute intervals; "If the next firing would feel like filler, stop instead."
- PROCESS_STRATEGY.md:70-73: "roughly twenty full gates were run across one session, one per increment, where four would have given an identical answer."
- RELEASE_PROCESS.md:37-41: two mandatory gates rendered by the human maintainer; "Preparation and go-ahead are separated hands."

Note: GUIDE_OUTLINE.md (344 lines) is a book outline, out of scope.

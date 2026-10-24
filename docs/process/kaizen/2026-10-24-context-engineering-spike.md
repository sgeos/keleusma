# Context engineering follow-up

Research spike completed on 2026-10-24 by the kaizen line. It answers the operator's
response to the [token economy spike](./2026-10-24-token-economy-spike.md) and asks
what the current state of the art says about context engineering for this project.
It revises one proposal of that spike, implements one, and records the operator's
positions on the others. No gate, setting or instruction file was changed. One
tracked tool changed, and its activation is left to the operator.

## Summary

| # | Finding | Evidence class |
|---|---|---|
| 1 | The literature does not support "more context gives better work" in general. It supports "relevant, uncompressed context gives better work". Length alone degrades performance even when retrieval is perfect. | Published, five claims re-fetched |
| 2 | Compaction is lossy and the loss compounds. One study measured 53 percent retention of safety rules after one Claude Code compaction round and 10 percent after five. | Published, re-fetched |
| 3 | In this project's transcripts the tool error rate in the 50 calls after a compaction was 2.85 percent, against 1.16 percent before it and 1.43 percent at a fresh session start. Errors rose in 16 of 31 compactions and fell in 4. | Measured, weak proxy |
| 4 | The operator's hypothesis therefore splits. Full context beats compacted context, and that half has support. More context beats less context, and that half does not. | Inference |
| 5 | The first proposal of the token spike, a lower auto-compact window, would add compactions. It is withdrawn in favor of fewer compactions and deliberate session ends. | Inference from 2 and 3 |
| 6 | Deliberate session ends require small startup files, because a fresh session pays for them again. The proposals on instruction files, startup files and session boundaries are one strategy, not three. | Inference |
| 7 | The prompt reminder hook now emits a short line per prompt and stays silent in a nested worktree configured for another line. | Implemented and tested, not activated |

## The operator's positions

The operator responded first-hand in this session on 2026-10-24. The positions are
paraphrased here, with the operator's own qualifiers kept.

| Proposal | Position |
|---|---|
| 1. Lower auto-compact window | Hypothesis that full context has produced better work, provided conversations stay focused. Asked for the state of the art in context engineering |
| 2. Wait without re-sending context | Agreed to the extent reasonably possible. Claude Code, Codex, Grok Build and untested harnesses differ |
| 3. Clear at task boundaries | Depends on the answer to question 1 |
| 4. Trim `CLAUDE.md` | Agreed, for `CLAUDE.md` and the equivalent files of other harnesses |
| 5. Bound the startup reads | Agreed. History needs to live only in git. The startup surface should hold what is in flight and why |
| 6. Shorten the hook banner | Agreed, deferring to the analysis |
| 7. Summarize gate output | Agreed, provided no failure is hidden |

The operator also proposed two mechanisms. Some files could be overwritten whole every
time they are touched, so that they cannot accumulate. Jev could make decisions that
need many input tokens but can be expressed as a multiple choice. The operator stated
that this line exists to spend tokens on improvements that affect every line, and that
any improvement above the polishing level is nominally worth its cost.

These positions are recorded as positions. They do not by themselves rule which line
makes each change, and the plan below says where a peer line owns the surface.

## Question 1, the state of the art

The full survey with sources is
[`context-engineering-survey.md`](./evidence/token-economy/context-engineering-survey.md),
saved verbatim from a research subagent. The kaizen line re-fetched the five claims
the conclusions rest on and found each as stated.

| Claim | Source | Re-fetched |
|---|---|---|
| Accuracy fell 13.9 to 85 percent with input length alone, even with irrelevant tokens replaced by whitespace or masked | [Du et al., arXiv 2510.05381](https://arxiv.org/abs/2510.05381) | Yes |
| Successful SWE-bench agent trajectories typically stay under 20k to 30k tokens, and longer accumulated context correlates with lower success | [Raju et al., arXiv 2602.16069](https://arxiv.org/abs/2602.16069) | Yes |
| Claude Code's compaction prompt kept 53 percent of safety rules after one round and 10 percent after five | [Zerhoudi et al., arXiv 2608.22752](https://arxiv.org/abs/2608.22752) | Yes |
| Repository context files did not generally improve task success and raised inference cost by over 20 percent, while their instructions were followed | [Gloaguen et al., arXiv 2602.11988](https://arxiv.org/abs/2602.11988) | Yes |
| "Sometimes you should let context accumulate because you're deep in one complex problem and the history is valuable" | [Claude Code best practices](https://code.claude.com/docs/en/best-practices) | Yes |

The field organizes context engineering as four operations, write, select, compress
and isolate [LangChain](https://www.langchain.com/blog/context-engineering-for-agents).
Against each, the evidence for this project reads as follows.

- **Write state outside the context.** Progress files, structured task lists and git
  as memory are the recommended substrate for long-running agents
  [Anthropic](https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents).
  The project already does this, at a scale that defeats the purpose, because the
  state files are too large to read whole.
- **Select only what is relevant.** On SWE-bench, more relevant files helped and
  trimming them to the edited lines helped further. Raising retrieval recall with
  irrelevant text lowered success [SWE-bench](https://arxiv.org/abs/2310.06770).
  Relevance density, not volume, carries the gain.
- **Compress with care.** Compaction loses information on every round. A full rewrite
  of a curated context once shrank it from 18,282 tokens to 122 and fell below the
  no-context baseline [ACE](https://arxiv.org/abs/2510.04618). Trained compaction
  recovers several points on SWE-bench, which indicates that untrained compaction
  leaves quality behind.
- **Isolate reading, not writing.** Subagents that read widely and return short
  summaries keep the main context clean. Practitioners keep writes single-threaded and
  give an escalation model the full context
  [Cognition](https://cognition.com/blog/multi-agents-working).

Inference. The operator's observation is consistent with this literature if the
benefit came from relevant facts surviving, rather than from volume. A long focused
session keeps its facts verbatim. A compacted session keeps a lossy summary of them.
A session that starts by reading 700 KB of history keeps volume without relevance.
The three are different situations, and the project currently produces all three.

No published study tests this project's configuration. The only way to settle the
hypothesis here is to hold a task class fixed and vary session length and startup
size, then count outcomes.

## The local measurement

[`context_quality.py`](./evidence/token-economy/context_quality.py) reads the same
transcripts as the earlier spike and emits only counts. Its output is
[`context-quality.json`](./evidence/token-economy/context-quality.json). It counts
tool results the harness marked as errors and excludes one permission denial.

| Window | Calls | Errors | Rate |
|---|---|---|---|
| First 50 tool calls of a fresh session | 350 | 5 | 1.43 percent |
| 50 tool calls before a compaction | 1,550 | 18 | 1.16 percent |
| 50 tool calls after a compaction | 1,473 | 42 | 2.85 percent |

Errors were more frequent after the compaction in 16 of 31 compactions, less frequent
in 4, and tied in 11. A two-sided sign test on the 20 untied pairs gives p of about
0.012. Of the 42 post-compaction errors, 29 were Bash, 11 were Edit and 2 were Read.

By context size the error rate falls, from 2.81 percent at 50k to 100k tokens to 0.47
percent at 700k and above. Inference. This does not show that large contexts help.
The small-context calls of Opus 5 sessions are mostly the calls just after a
compaction, since those sessions dropped to about 55k tokens, so the gradient is
consistent with the compaction effect.

Limits. A tool error is not a measure of work quality. A failing test, a non-zero
grep and a rejected edit all count, and a wrong action that succeeds does not count.
Context size is confounded with session age, task and model. The fresh-start window
holds only five errors, so its rate is imprecise. The sample is the same seven
surviving transcripts as the earlier spike. Errors are clustered within sessions,
which the sign test respects and a pooled rate does not.

## The revised strategy

The evidence supports one strategy across proposals 1, 3, 4 and 5 of the token spike.
Keep the always-loaded prefix small and stable, let a focused session use the large
window, avoid compaction where possible, and end sessions deliberately.

1. **Withdraw the lower auto-compact window.** A lower window means more compactions,
   and compaction is where the measured error rate rises. Keep the default for now.
2. **End sessions at task boundaries instead of compacting through them.** Write the
   line's state file, commit, and start fresh with `/clear` or a new session. The
   fresh-start error rate was close to the pre-compaction rate. A session that is
   deep in one problem continues, as the vendor guidance itself allows. A session
   that has compacted twice is a candidate for a deliberate end, because each round
   loses more.
3. **Make the fresh start cheap.** A deliberate end is only economical when the
   startup read is small. That makes the instruction files and the startup files the
   precondition for the session policy, which is why they come first in the plan.
4. **Put what must survive where the harness re-reads it.** Claude Code re-injects the
   root `CLAUDE.md` from disk after compaction and summarizes path-scoped rules away
   [context window](https://code.claude.com/docs/en/context-window). A rule that must
   survive belongs in the root file, which is a reason to keep that file short enough
   that its rules are followed.

## Overwrite-only state files

The operator's proposal has one hazard the literature names. ACE found that a
monolithic rewrite of a curated context can collapse it. A file that is overwritten
whole can lose items silently. Git removes the irrecoverability, because the previous
version is one command away, but not the silence. The proposal below keeps the
overwrite and adds an accounting rule.

- **State files** are overwritten whole and hold only what is in flight, why, what is
  owed and the next step. They carry a byte budget. Candidates are
  `REVERSE_PROMPT.md`, `HANDOFF.md`, each per-line handoff, and a current-sprint task
  file replacing the startup role of `TASKLOG.md`.
- **History** lives in commit messages, pull request bodies and dated reports, which
  are written once and never edited. The operator's statement that history needs to
  live only in git fits this exactly.
- **Accounting.** A rewrite that drops an item says in its commit message whether the
  item was completed, moved or abandoned. Hypothesis. This keeps the overwrite from
  becoming the silent collapse ACE measured.
- **Enforcement.** A guard test in the style of `documentation_links` fails when a
  state file exceeds its budget. It can land only after the owning lines have trimmed
  their files, or it fails at once.
- **Existing accumulations.** `TASKLOG.md` is 701 KB, `DESIGN_JOURNAL.md` 1.49 MB,
  `HANDOFF.md` 154 KB and the `v0.2.3` per-line handoff 161 KB. Each would be frozen
  and stop being a startup read. Whether a frozen file stays in the tree or leaves it
  for git history is the owning line's choice. Links into it would need repair either
  way.

This line owns one such file, its own handoff, which had grown to 50 KB. It is
rewritten in this increment to the proposed form as a worked example.

## Jev for large-input multiple-choice decisions

Small-model reading with large-model deciding is well supported when the large model
sets the question and the answer space is bounded. Minions kept 97.9 percent of
performance at 5.7 times lower cost [arXiv 2502.15964](https://arxiv.org/abs/2502.15964).
A weak model deciding when to escalate did not work
[Cognition](https://cognition.com/blog/multi-agents-working). Jev returns typed
choices and probabilities, accepts up to 64,000 tokens per request, and was listed at
USD 0.042 per million input tokens on the earlier research date
[pilot](../JEV_SHADOW_PILOT.md).

Candidate decisions, each a selection rather than a verdict:

| Decision | Input | Choices | Cost of a wrong answer |
|---|---|---|---|
| Which sections of a large file bear on a task | the task statement and up to 64k tokens of a file | section headings | a missed section, detectable only later |
| What kind of failure a gate log shows | the failing portion of a log | compile error, assertion, timeout, infrastructure, other | a misdirected first look, corrected by reading |
| Whether a peer diff touches tests, continuous integration or hooks | a pull request diff | yes or no per category | a missed notice, which V0.3.X explicitly asked for |
| Whether a peer message requires action | the message body | act now, act later, record only | a delayed response |

Constraints. Jev's answer never gates, consistent with the pilot rule. The main agent
writes the question and the options. The option order is randomized, because the
earlier Jev spike found order sensitivity. The main agent reads the selected material
itself before acting. Adoption waits on shadow measurement, in which the pilot records
Jev's choice and the agent's own choice blind and compares them over enough cases to
estimate a miss rate. The section-selection decision has the largest saving and the
least visible failure, so it should be measured first and adopted last.

## Waiting without re-sending context

The harness survey is
[`harness-waiting-survey.md`](./evidence/token-economy/harness-waiting-survey.md),
saved verbatim from a second research subagent. The kaizen line re-fetched the
Claude Code limits and the Codex hook input below.

Every wake-up costs one request carrying the full context, in every harness. Inside
the cache lifetime the prefix is billed as a cache read. Past it the prefix is written
again. The question is therefore how many wake-ups a wait takes, not which primitive
produces them.

| Harness | Exit notification for a background command | Bounded watch | Headless fresh session |
|---|---|---|---|
| Claude Code | Yes. Local background commands have no time limit, and this session's tool contract re-invokes the agent at exit | Monitor, 5 minutes by default and 30 at most, one wake-up per output line | `claude -p`, with `--bare` to skip hooks and instruction files |
| Codex CLI | Not documented. The configuration describes polling of background terminals | Not documented | `codex exec`, with `--ephemeral` |
| Grok Build | Not documented | Monitors, one notification per output line | `grok -p` |

Inference. A portable pattern follows from the table.

1. **Put the wait in a script, not in the model.** A waiter such as `gh run watch`
   or a loop on a file blocks, prints one summary line, and exits with a status that
   distinguishes pass, fail and timeout.
2. **In Claude Code, run the waiter in the background and end the turn.** That is
   one wake-up at completion. A Monitor over a 90-minute run needs at least three
   re-arms and wakes on every output line, so it suits short waits with sparse
   output. A `/loop` that polls pays one full-context request per iteration.
3. **Where no exit notification exists, or the wait outlasts the cache lifetime,
   end the session instead.** Write the state file, then let an external supervisor
   run the waiter and start a fresh headless session that reads the state file and
   the result. After a long wait, a fresh session costs its startup read, while a
   resume re-sends the whole transcript uncached. This is the same deliberate session
   end as the revised strategy, which again depends on small startup files.

The Claude Code hook documentation states that `UserPromptSubmit` also fires on turns
Claude Code starts on its own. Inference. The full banner was therefore added on every
wake-up as well as every prompt, and the shorter reminder below reduces both.

Untested. Grok Build reads Claude Code hook files and, per its documentation, ignores
the output of passive hooks. The reminder would then not reach a Grok session at all,
and the manual inbox check in the protocol remains the authoritative route there.

## The prompt reminder hook

The hook text comes from the tracked `scripts/interline.py`, so the fix belongs there.
Two changes landed in this increment.

- On `UserPromptSubmit` the hook now prints one line, with the line name, the pending
  count, the trust rule and the protocol path. That is 206 characters for `kaizen`
  against about 750 before. At session start and after compaction it prints the full
  reminder. When the client payload is absent or unreadable it prints the full
  reminder, so a failure errs toward more instruction, not less.
- The second banner came from the primary checkout's configuration firing inside this
  nested worktree. The hook now finds the session's own worktree and stays silent when
  that worktree configures interline for a different line.

Both Claude Code and Codex document `hook_event_name` and `cwd` among the fields every
command hook receives on stdin, which the change relies on. The installed command is
unchanged, so existing configurations need no edit. Four
tests were added and the installer test now checks both forms. All 23 interline tests
pass. Run against the live store, read-only, the `kaizen` reminder was 206 characters
and the inherited `v0.2.3` hook printed nothing.

Not activated. Hooks run the installed copy in the shared store, and this line does
not reinstall over shared stores. Activation is the operator's step, by running the
installer from an updated checkout or copying `scripts/interline.py` to
`secret/interline/interline.py`. The change does not alter the mailbox schema.

## Gate output

Agreed by the operator with the condition that no failure is hidden. The shape is a
wrapper that writes the full output to a log, prints the exit status, the failing
test names and the summary lines, and prints the full tail whenever it cannot find a
summary. Its tests must use real captured failures, including a compile error, a
panic, a timeout and a failing doctest. It is the next increment of this line and is
not started here.

## Plan and ownership

| Step | Owner | State |
|---|---|---|
| Shorter reminder and nested-worktree silence | kaizen | implemented, activation with the operator |
| Rewrite of the kaizen handoff as a state file | kaizen | done in this increment |
| Draft of a trimmed `CLAUDE.md` and `AGENTS.md` | kaizen drafts, V0.2.X reviews | next, after notifying both version lines |
| State-file conversion of `HANDOFF.md`, `TASKLOG.md`, `REVERSE_PROMPT.md` and the version handoffs | V0.2.X and V0.3.X | proposed |
| State-file budget guard | kaizen, after the conversions | proposed |
| Gate output wrapper with failure fixtures | kaizen | next |
| Jev shadow tasks for the four decisions | kaizen with the pilot lines | proposed |
| Session-end policy after task boundaries | operator ruling | proposed |

## Limits

- The survey's labels are the subagent's own. Five claims were re-fetched. The others,
  including every vendor figure about its own product, were not.
- The local measurement is a proxy over seven transcripts. It supports a hypothesis,
  that compaction raises tool errors, and does not establish it.
- No outcome measure of work quality exists for this project. The hypothesis in
  question 1 remains open here until one is defined.
- The harness survey found no documentation for exit notification in Codex or Grok
  Build. Absence of documentation is not absence of the feature.
- The two research subagents used about 143,000 and 183,000 tokens.

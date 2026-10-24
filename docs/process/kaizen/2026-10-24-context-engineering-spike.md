# Context engineering follow-up

Research spike completed on 2026-10-24 by the kaizen line. It answers the operator's
response to the [token economy spike](./2026-10-24-token-economy-spike.md) and asks
what the current state of the art says about context engineering for this project.
It revises one proposal of that spike, implements one, and records the operator's
positions on the others. A second pass the same day, at the operator's request,
extended the measurement, established the cause of the Opus 5.5 compactions, built
the gate output summary, and added a continuous-integration job for the process
tooling. No setting was changed. The `CLAUDE.md` draft is prepared and waits on a
peer merge, as described under the plan.

## Summary

| # | Finding | Evidence class |
|---|---|---|
| 1 | The literature does not support "more context gives better work" in general. It supports "relevant, uncompressed context gives better work". Length alone degrades performance even when retrieval is perfect. | Published, five claims re-fetched |
| 2 | Compaction is lossy and the loss compounds. One study measured 53 percent retention of safety rules after one Claude Code compaction round and 10 percent after five. | Published, re-fetched |
| 3 | In this project's transcripts, mid-task threshold compactions raised the tool error rate in the next 50 calls from 0.38 percent to 1.91 percent. Errors rose in 11 of 21 such compactions and fell in 1. The rate decays over about 100 calls. | Measured, weak proxy |
| 4 | The operator's hypothesis therefore splits. Full context beats compacted context, and that half has support. More context beats less context, and that half does not. | Inference |
| 5 | The first proposal of the token spike, a lower auto-compact window, would add compactions. It is withdrawn in favor of fewer compactions and deliberate session ends. | Inference from 2 and 3 |
| 6 | Deliberate session ends require small startup files, because a fresh session pays for them again. The proposals on instruction files, startup files and session boundaries are one strategy, not three. | Inference |
| 7 | The prompt reminder hook now emits a short line per prompt and stays silent in a nested worktree configured for another line. After compaction it names the line's state file. | Implemented and tested, not activated |
| 8 | Every Opus 5.5 compaction in the sample was an idle compaction, 54.5 to 54.8 minutes after the last activity. Every Opus 5 compaction was a threshold compaction. The unexplained early compactions of the first spike are therefore explained, and waiting is a cause of compaction as well as a cost. | Measured, cause established |
| 9 | A deliberate fresh start costs about the same context as a compaction. After 30 calls both stood near 120k tokens. | Measured |
| 10 | Codex sessions spent 7.25 percent of their input tokens at Codex's own `sleep` and `wait` calls, a lower bound because shell sleeps are not counted. | Measured |
| 11 | A gate output summary that hides no failure reduced a real 1,297-line library test log of 83,076 characters to 4 lines. | Implemented, measured on one run |
| 12 | The Python tests of the process tooling now run in continuous integration. | Implemented, rehearsed locally |

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

**Two kinds of compaction.** A compaction that follows the last activity by minutes is a
threshold compaction, made because the context reached its limit. One that follows 30
minutes or more of idle time is the harness's idle compaction, which Claude Code runs
on the one-hour cache lifetime "shortly before the cache expires"
[prompt caching](https://code.claude.com/docs/en/prompt-caching). The measured gaps
separate cleanly. All 19 Opus 5 compactions came 1.2 to 2.2 minutes after activity, at
768k to 1M tokens. All 10 Opus 5.5 compactions came 54.5 to 54.8 minutes after
activity, at 212k to 616k tokens. The first spike left the earlier Opus 5.5
compactions unexplained. They were idle compactions.

**The harness's edit guard.** All 30 failed edits in the sample were the harness's own
refusal to edit a file not read, or modified since read, in the current context.
Compaction clears that record, so part of the post-compaction rise is this mechanical
guard rather than a model error. The table excludes it.

| Window, excluding the edit guard | Calls | Errors | Rate | Errors rose, fell, tied |
|---|---|---|---|---|
| First 50 tool calls of a fresh session | 350 | 5 | 1.43 percent | |
| 50 calls before a threshold compaction | 1,050 | 4 | 0.38 percent | |
| 50 calls after a threshold compaction | 1,049 | 20 | 1.91 percent | 11, 1, 9 |
| 50 calls before an idle compaction | 493 | 7 | 1.42 percent | |
| 50 calls after an idle compaction | 453 | 12 | 2.65 percent | 6, 3, 1 |

For threshold compactions a two-sided sign test on the 12 untied pairs gives p of about
0.006. For idle compactions it gives about 0.5, which is not evidence of an effect.
Inference. The harmful case is the mid-task compaction, where work continues on a
summary of itself. An idle compaction falls at a natural break, which is where the
vendor advises compacting, and the work that follows is often a new task.

| Calls since a compaction | Calls | Errors | Rate |
|---|---|---|---|
| 0 to 9 | 310 | 14 | 4.52 percent |
| 10 to 24 | 465 | 13 | 2.80 percent |
| 25 to 49 | 738 | 16 | 2.17 percent |
| 50 to 99 | 1,431 | 19 | 1.33 percent |
| 100 to 199 | 2,524 | 27 | 1.07 percent |

The rate decays monotonically over about 100 calls. Inference. The session rebuilds
what the summary lost by re-reading and by failing, which is the re-derivation cost the
first spike anticipated.

**What a restart costs.** The first 30 calls after a compaction read 68k characters of
tool results on average, 25k of them from process and instruction documents, and the
context grew from 68k to 120k tokens. The first 30 calls of a fresh session read 97k
characters, 35k of them from those documents, and the context grew from 57k to 119k
tokens. A fresh session therefore starts about 11k tokens lighter, reads more, and
arrives at the same size. Its error rate is close to the pre-compaction rate, on only
five errors.

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
4. **Treat a long wait as a session boundary.** Every Opus 5.5 compaction in the sample
   followed an hour of idle time. A session that is about to wait an hour for CI, a
   peer or the operator can write its state file first, so that the summary it is about
   to receive matters less. Setting `idleCompaction` to `false` keeps the full context
   across the wait instead, at the price of re-writing the whole prefix to the cache on
   return. The measured effect of idle compaction is not significant, so this is the
   operator's choice of cost and not a correction.
5. **Put what must survive where the harness re-reads it.** Claude Code re-injects the
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

The Claude Code hook documentation states that `UserPromptSubmit` also fires when a
session-scoped scheduled task or `/loop` iteration fires, when a background subagent
reports back, and when another session sends a message
[hooks](https://code.claude.com/docs/en/hooks). The full banner was therefore added on
each of those wake-ups as well as on every prompt, and the shorter reminder below
reduces both.

Codex shows the same pattern. Across 72 Keleusma Codex sessions, 301 `sleep` calls and
366 `wait` calls were followed by requests carrying 144.0 million input tokens, 7.25
percent of all Codex input. Sleeps run through Codex's shell tool are not counted, so
this is a lower bound. The Codex configuration documents a five-minute maximum poll
window for background terminals and no completion notification.

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
- At session start and after compaction, the full reminder names the line's state file,
  `docs/process/handoffs/<line>.md`, when it exists. A `SessionStart` hook's output is
  added to the compacted context
  [context window](https://code.claude.com/docs/en/context-window), so this is the
  vendor's documented route for re-injecting state. Only the path is printed, so no
  tracked text enters hook output.

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

Agreed by the operator with the condition that no failure is hidden. Implemented as
`scripts/gate_summary.py`. It runs a command with its merged output in a log file and
prints a bounded summary under six rules.

1. The command, exit status, duration and log path are always printed, and the wrapper
   exits with the command's own status, reporting a signal as a shell does.
2. Every failure marker line is printed with a few lines of context after a panic or a
   compiler error, under the most recent gate step or test-binary header.
3. Summary lines such as `test result:` and the nextest `Summary` are printed.
4. A failing run with no recognised marker prints the last lines of the log.
5. A passing run whose output contains failure markers says so.
6. When the marker lines exceed the limit, the elided count and the log's last lines
   are printed.

The tests replay real output captured from cargo, nextest, clippy and rustfmt against a
throwaway crate. The fixtures cover a failed assertion, a panic, a compile error, a
failed doctest, two clippy lints, a formatting diff, a test binary killed by `SIGABRT`
and a nextest timeout, with two passing controls. `capture.sh` beside them regenerates
them. One test checks every fixture line that an independent pattern recognises as a
failure. It found a real gap on its first run, because nextest indents each test's
captured output and the first patterns were anchored at the line start.

On the library tests of this repository, 1,297 lines and 83,076 characters of output,
about 21,000 tokens at four characters per token, became four lines. Inference. The
saving grows with the gate, because passing lines are the bulk of every gate log.

Limits. The markers cover the formats captured. A tool that changes its output format
can stop matching, and rule 4 is the guard for a failing run in that case. A passing run
that hides a failure in an unrecognised format would not be flagged by rule 5.

## Plan and ownership

| Step | Owner | State |
|---|---|---|
| Shorter reminder, nested-worktree silence and state-file pointer | kaizen | implemented, activation with the operator |
| Rewrite of the kaizen handoff as a state file | kaizen | done |
| Gate output summary with real failure fixtures | kaizen | implemented |
| Python tooling tests in continuous integration | kaizen | implemented, job `python-tooling` |
| Draft of a trimmed `CLAUDE.md` and `AGENTS.md` | kaizen drafts, V0.2.X reviews | drafted, waiting on a V0.2.X merge |
| Release gate runs the heavy tier twice | V0.2.X | reported with a proposed fix |
| State-file conversion of `HANDOFF.md`, `TASKLOG.md`, `REVERSE_PROMPT.md` and the version handoffs | V0.2.X and V0.3.X | proposed, awaiting the operator's first-hand confirmation to those lines |
| State-file budget guard | kaizen, after the conversions | proposed |
| Jev shadow tasks for the four decisions | kaizen with the pilot lines | proposed |
| Session end before a long wait or at a task boundary | operator ruling | proposed |
| `idleCompaction` setting | operator choice | options stated |

**The `CLAUDE.md` draft.** V0.2.X accepted that Kaizen drafts it and asked that the
draft be based on `v0.2.3` after its Order 3 pull request, which edits the status
block, and that four things be kept or their new place named. The draft keeps the
Appendix B constraint, the session startup protocol and the git workflow rules. It
moves the two verification catalogues verbatim to `docs/process/VERIFICATION_PITFALLS.md`
and keeps their two correctives in `CLAUDE.md`. The release history it drops is already
in `CHANGELOG.md`. It is 17,944 bytes against 42,802, about 58 percent smaller. Swapped
into the tree, it passed the four tests that read `CLAUDE.md` or its links,
`claimed_counts`, `documentation_links`, `gate_ci_correspondence` and
`release_process_crate_list`. Its first version failed six guards in `claimed_counts`,
each a phrasing that a figure-checking test reads, which is how the draft came to
restate the opcode count and the `Value` slot width.

**Withdrawn.** The earlier concern that `scripts/merge-to-trunk.sh` conflicts with the
no-fast-forward model is already documented in `GIT_STRATEGY.md` and
`PARALLEL_DEVELOPMENT.md`, so it was not a new finding.

## Limits

- The survey's labels are the subagent's own. Five claims were re-fetched. The others,
  including every vendor figure about its own product, were not.
- The local measurement is a proxy over seven transcripts. It supports a hypothesis,
  that compaction raises tool errors, and does not establish it.
- No outcome measure of work quality exists for this project. The hypothesis in
  question 1 remains open here until one is defined.
- The harness survey found no documentation for exit notification in Codex or Grok
  Build. Absence of documentation is not absence of the feature.
- The idle threshold of 30 minutes separates the measured gaps, which fall either under
  three minutes or near 55. A compaction triggered manually after a long pause would be
  misclassified as idle.
- Codex tool outputs record an exit status in only 944 of about 25,000 cases, so no
  Codex error rate is reported.
- The two research subagents used about 143,000 and 183,000 tokens.

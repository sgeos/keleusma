# Token economy research spike

Research spike completed on 2026-10-24 by the kaizen line at the operator's request.
It measures where this project's agent tokens go and reviews current token
economization techniques. It proposes measured changes and adopts none. No gate,
setting, hook or instruction file was changed.

## Findings

| # | Finding | Evidence class |
|---|---|---|
| 1 | Context re-reading dominates. Over 16,269 Claude Code API calls, cache reads were 7.06 billion tokens and output was 13.3 million. | Measured |
| 2 | Context size per call is set mainly by the compaction threshold. Opus 5 sessions compacted at 767k to 999k tokens and averaged 466k to 486k tokens per call. | Measured |
| 3 | Waiting is expensive at large context. Loop wakeups and polling commands were sent with 14.5 percent of all context tokens. | Measured, with a pattern classifier |
| 4 | Current Opus 5.5 sessions already run smaller, at 178k to 285k tokens per call on average. The remaining saving from a lower threshold is therefore smaller than the historical figures suggest. | Measured |
| 5 | The always-loaded and startup-read files grew between three and six times since June. `CLAUDE.md` is 41 KB against a vendor target of 200 lines. | Measured |
| 6 | Output is the second lever, not the first. Under list-price weighting it is about 7 to 11 percent of the cost-equivalent volume. | Inference from measured counts and published prices |

The figures cover only transcripts that survived harness retention on this machine.
They span 2026-08-31 through 2026-10-24. Subscription plan limits are not published
as token weights, so every cost share below uses API list-price ratios as a proxy.
That proxy is an assumption, not a measurement of plan usage.

## Method

[`measure.py`](./evidence/token-economy/measure.py) reads the Claude Code transcripts
under `~/.claude/projects/` whose directory names `keleusma` and the Codex session
logs whose working directory names `keleusma`. It emits only counts, token totals,
tool names, sizes and timestamps. Its output is
[`measurements.json`](./evidence/token-economy/measurements.json). Context per call
means input plus cache-write plus cache-read tokens, which is the size of the
prompt sent. Assistant records are deduplicated by message identifier.
[`surface-sizes.md`](./evidence/token-economy/surface-sizes.md) records the growth
of the instruction files from git history.

The literature review was delegated to one subagent and saved verbatim as
[`literature-survey.md`](./evidence/token-economy/literature-survey.md), with its own
fetched and unfetched marks. The kaizen line re-fetched the three Claude Code pages
the recommendations depend on, the
[cost guide](https://code.claude.com/docs/en/costs), the
[memory guide](https://code.claude.com/docs/en/memory) and the
[model configuration guide](https://code.claude.com/docs/en/model-config).
It also checked the survey's cache lifetime claim against the measurements. Main
conversation cache writes were all one-hour writes, 95.2 million tokens, and
subagent writes were all five-minute writes, 3.0 million tokens. That matches the
documented behavior. Other survey figures were not independently re-verified.

## Where the tokens go

### Volume

| Source | Calls or sessions | Cache read | Cache write | Uncached input | Output |
|---|---|---|---|---|---|
| Claude Code main conversations | 15,679 calls | 7,002 M | 95.2 M | 0.04 M | 13.1 M |
| Claude Code subagents | 555 calls | 46.9 M | 2.9 M | 0.01 M | 0.2 M |
| Codex | 72 sessions | 1,953 M cached of 1,987 M input | not reported | 34 M | 6.2 M |

Subagents are under one percent of Claude Code context volume. Codex input was
98.3 percent cached.

### Cost-equivalent shares

The pricing page lists a one-hour cache write at twice base input, output at five
times base input, and a cache read at 0.1 times by default or 0.05 times on
Opus 5.5 [pricing](https://platform.claude.com/docs/en/about-claude/pricing). Most
historical calls ran on `claude-opus-5`, whose read multiplier was not separately
confirmed, so both read multipliers are shown.

| Component | Share at 0.1 read | Share at 0.05 read |
|---|---|---|
| Cache reads | 72.8 % | 57.3 % |
| Cache writes | 20.3 % | 31.9 % |
| Output | 6.9 % | 10.8 % |

Inference. Context volume per call, multiplied by the number of calls, is the
dominant cost under either assumption. Cache writes come mostly from content newly
added to context, and they recur after every compaction.

### Context size follows the compaction sawtooth

| Model era | Compaction point | Floor after compaction | Mean context per call |
|---|---|---|---|
| `claude-opus-5`, two sessions, 19 compactions | 767k to 999k | 53k to 62k | 466k and 486k |
| `claude-opus-5-5`, five sessions, 9 compactions | 211k to 616k | 80k to 99k | 178k to 285k in the four sessions run mainly on it |

The documentation states that models with a native one-million-token window compact
at about 967k tokens unless an auto-compact window is set
[model configuration](https://code.claude.com/docs/en/model-config). No such setting
exists in the user, project or local settings on this machine. A context that
climbs linearly from a floor F to a threshold T has a mean of (F + T) / 2. For the
Opus 5 sessions that predicts about 462k, which matches the measured 466k and 486k.
Why the Opus 5.5 sessions compacted earlier is not established. Some of those
compactions may have been manual.

In the two long Opus 5 sessions, 95 percent of context tokens were sent in calls
above 200k tokens. One of them stayed on one transcript for 54 days.

### Waiting at full context

| Activity | Calls | Context tokens sent | Share of all context tokens |
|---|---|---|---|
| `ScheduleWakeup` loop turns | 787, of which 96 marked no-op | 421 M | 5.9 % |
| Bash `sleep` commands | 614 | 330 M | 4.6 % |
| GitHub run and check status queries | 595 | 288 M | 4.0 % |

Each wakeup or poll re-sends the whole conversation. The mean context at a wakeup
was 517k and 551k in the two loop sessions. The polling classifier matches command
text and both misses and over-matches some commands. The count excludes the
follow-up calls a wakeup triggers, so the true cost of waiting is likely higher.

### Instruction surface

| File on `v0.2.3` | June 1 | October 24 | Role |
|---|---|---|---|
| `CLAUDE.md` | 12 KB | 41 KB | loaded in every session and subagent |
| `docs/process/HANDOFF.md` | absent | 154 KB | startup read |
| `docs/process/TASKLOG.md` | 119 KB | 692 KB | startup read |
| `docs/process/DESIGN_JOURNAL.md` | absent | 1,490 KB | resume channel after compaction |
| `docs/process/handoffs/v0.3.0.md` on `v0.3.0` | not measured | 768 KB | per-line resume |

The status paragraph of `CLAUDE.md` alone is 10,021 characters of release history.
At an assumed four characters per token, the whole file is about 10k tokens.
Inference. A file of that size, present in all 16,269 calls at its historical sizes,
accounts for roughly two percent of context volume. Its larger cost is plausibly
adherence. The vendor states that files over about 200 lines "consume more context
and reduce adherence" [memory guide](https://code.claude.com/docs/en/memory). That
claim is unquantified.

The startup files cannot be read whole. `TASKLOG.md` alone would be about 170k tokens
by the same estimate. Agents read prefixes of them, and what they read stays in
context until the next compaction.

Two `UserPromptSubmit` hooks each inject a 750-character banner on every prompt, one
from the worktree settings and one from the primary checkout settings. That is about
375 tokens per prompt, accumulating until compaction.

### Tool output

Tool results totaled 17.4 million characters. Bash produced 86 percent of that over
13,938 calls, with a mean of about 1,070 characters. Results over 10,000 characters
made up 24 percent of tool-result volume. Inference. Tool output is small next to
the re-read volume, and it matters because it persists and is re-read on every
later call.

## State of the art, condensed

The full survey with sources is in
[`literature-survey.md`](./evidence/token-economy/literature-survey.md). The items
that bear on this project follow.

| Technique | Strongest evidence found | Applies here |
|---|---|---|
| Prompt caching | Vendor documented. Reads at 0.05 to 0.1 times input, invalidated by model, effort or tool changes | Already in effect. Avoid mid-session model and effort switches |
| Lower auto-compact window | Vendor documented setting `autoCompactWindow`, from 100k to 1M | Yes, by setting |
| `/clear` between unrelated tasks | Vendor says it "costs nothing" [cost guide](https://code.claude.com/docs/en/costs) | Yes, with the per-line handoffs |
| Observation masking | SWE-bench Verified, cost down 52.7 percent and solve rate 53.4 to 54.8 percent on one model, lower on a thinking model [arXiv 2508.21433](https://arxiv.org/abs/2508.21433) | Partly. The harness clears tool results itself |
| Bounded observations | SWE-agent ablations favored a 100-line viewer and collapsed history [arXiv 2405.15793](https://arxiv.org/abs/2405.15793) | Yes, as a reading habit |
| Small instruction files and skills | Vendor guidance, under 200 lines, specialist text moved to skills or path-scoped rules, HTML comments stripped | Yes |
| Test-output filtering hooks | Vendor example, unquantified | Yes, with a correctness check |
| Long-context degradation | Accuracy falls as input grows [Context Rot](https://www.trychroma.com/research/context-rot), [Lost in the Middle](https://arxiv.org/abs/2307.03172) | Supports smaller contexts on correctness grounds |
| Model cascades and routing | FrugalGPT and RouteLLM abstracts report large savings | Only at the subagent boundary |
| Prompt compression | LLMLingua family, two to twenty times compression on natural language | No. Unsafe for code and identifiers, and needs a gateway |
| Semantic response caching | False-hit rates reported, adversarial collisions | No. A correctness hazard under changing repository state |
| Batch API | Fifty percent discount, asynchronous | No for interactive agents |

Agentic coding token use varies up to thirty times between runs of the same task,
and accuracy often saturates at intermediate cost
[arXiv 2604.22750](https://arxiv.org/abs/2604.22750). That variance means one
before-and-after comparison cannot establish a saving here.

## Recommendations

Ordered by expected effect. Each needs an operator ruling before adoption, and each
should be measured with `measure.py` and the `/usage` cache line before and after.

1. **Set an auto-compact window for one-million-token models.** Hypothesis. A window
   near 300k, with a floor near 85k, gives a mean near 190k. For the historical
   Opus 5 pattern that is about 60 percent fewer context tokens per call. For the
   current Opus 5.5 pattern, with a window near 250k, the estimate is about 20
   percent. A rough extra compaction cost, using the measured growth of about 1.6k
   tokens per call, is about one twentieth of the saving. The risk is correctness.
   Each compaction loses detail, and re-deriving it costs tokens. Pilot it on one
   line for a week and compare re-reads, corrections and gate failures.
2. **Wait without re-sending the context.** Prefer background commands and monitor
   notifications to `sleep` loops and repeated status queries. Schedule loop wakeups
   at long intervals. Compact or hand off before an idle loop. Historically about
   14.5 percent of context volume was spent at waiting calls.
3. **Clear at task boundaries.** One transcript ran 54 days. The per-line handoffs
   already make `/clear` safe at a task boundary. Inference. This removes stale
   context without a compaction request.
4. **Trim `CLAUDE.md` toward 200 lines.** Move the release-history status paragraph
   to the roadmap or the changelog, move line-specific procedure into skills or
   path-scoped rules, and keep maintainer rationale in HTML comments. The token
   saving is about one to two percent. The main expected benefit is adherence, and
   that is unmeasured.
5. **Bound the startup reads.** Replace "read `TASKLOG.md`" with reading its current
   section, and rotate dated history out of `TASKLOG.md`, `HANDOFF.md` and the
   per-line handoffs into archive files that are not startup reads. This is a
   process change for the owning lines, not for Kaizen to apply.
6. **Remove the duplicate hook banner and shorten it.** Emit the full banner at
   session start and after compaction, and a one-line pending count otherwise. The
   pending count changes, so it must stay per-prompt. Saving about 375 tokens per
   prompt is small but nearly free.
7. **Summarize gate output.** Write full gate output to a log file and return the
   exit status, the failing test names and the summary line. Any filter must be
   validated against real failure output so that a panic or a compile error is
   never hidden.

Not recommended. Prompt compression, semantic response caching and routing the main
conversation to a weaker model. Each trades correctness for a saving the measurements
do not show to be the bottleneck.

## Limits and open questions

- Plan usage weights are not public. The cost shares are list-price proxies.
- Transcript retention removed earlier sessions. The sample is seven Claude Code
  transcripts with 28 subagent transcripts, and 72 Codex sessions.
- The four characters per token figure is an estimate, not a tokenizer count.
- The report's figures come from a run earlier on 2026-10-24. The committed
  `measurements.json` was regenerated later the same day and differs slightly,
  mainly by this session's own growth. Live transcripts change on every rerun.
- The cause of the earlier compaction on Opus 5.5 is not established.
- The effect of any recommendation on correctness is untested. Recommendation 1
  carries the largest correctness risk and the largest expected saving.
- The survey's `[F]` marks are the subagent's own claims. The kaizen line verified
  three pages and the cache lifetime split only.
- This spike itself used tokens. The subagent reported 165,881 tokens.

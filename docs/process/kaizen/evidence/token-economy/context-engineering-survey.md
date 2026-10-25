<!-- Saved verbatim from a research subagent of the kaizen line on 2026-10-24. Its labels are the subagent's own. The kaizen line re-fetched five claims, listed in the report. -->

# Context Engineering for Long-Running Coding Agents, State of the Art as of 2026-10-24

I fetched every source listed below unless the closing section says otherwise. Claim labels are FACT, which means a cited source states it directly, INFERENCE, and HYPOTHESIS. The tag VENDOR-SELF means a vendor is reporting a figure about its own product.

**Bottom line on the operator hypothesis.** The literature mostly argues against the unconditional form "full context gives better work even at higher token cost". It partly supports a narrower form. Relevant, detailed, and uncompressed context helps. Volume beyond what the task needs hurts, even when the conversation stays focused. Lossy rewriting also hurts. "Focused" is necessary but not sufficient, because several studies show degradation from length alone (INFERENCE).

## 1. Long-context quality degradation

**Evidence against the hypothesis**
- FACT. Chroma's "Context Rot" study (Hong, Troynikov and Huber, 14 July 2025) tested 18 models. Reliability fell as input length grew, even on simple tasks. On LongMemEval, every model did significantly better with a focused prompt of about 300 tokens than with the full prompt of about 113,000 tokens. Shuffled haystacks beat logically ordered ones for all 18 models. The study does not test agentic coding.
- FACT. Du et al. (arXiv 2510.05381, October 2025) report accuracy drops of 13.9 to 85 percent as input grew, even with perfect retrieval and well inside the claimed context windows. The drop persisted when the irrelevant tokens were replaced by whitespace or masked. This is the strongest result against the idea that a focused but long context is harmless. Having the model recite the evidence before answering recovered up to 4 percent.
- FACT. Lost in the Middle (Liu et al., Transactions of the Association for Computational Linguistics, 2023). Performance is highest when the relevant information sits at the start or end of the context.
- FACT. NoLiMa (ICML 2025). At 32K tokens, 11 of 13 models fell below half of their short-context baseline. GPT-4o went from 99.3 to 69.7 percent.
- FACT. RULER (COLM 2024). Only about half of 17 models kept satisfactory performance at 32K tokens.
- FACT. LongBench v2 (December 2024). The best direct-answer model scored 50.1 percent and o1-preview scored 57.7 percent, against 53.7 percent for time-limited human experts.

**Agentic coding specifically**
- FACT. Raju et al. (arXiv 2602.16069, ICLR 2026 workshop) studied mini-SWE-agent on SWE-bench Verified. Successful trajectories usually stayed under 20,000 to 30,000 tokens, and longer accumulated context correlated with lower success. In single-shot patching with 64,000 tokens of input, Qwen3-Coder-30B-A3B resolved 7 percent of tasks and GPT-5-nano resolved none. The authors conclude that agentic gains come from decomposition into short steps, not from long-context reasoning.
- FACT. The SWE-bench paper (ICLR 2024) found that Claude 2's performance "drops considerably" as total context length grows. Raising the BM25 retrieval budget improved recall of the oracle files yet lowered performance. Oracle retrieval scored 4.8 percent against 1.96 percent for BM25. Collapsing the oracle files to the edited lines plus or minus 15 raised the score to 5.9 percent. So more relevant context helped and less irrelevant context helped further.
- FACT. LongCodeBench (arXiv 2505.07897) reports Claude 3.5 Sonnet falling from 29 percent to 3 percent between 32K and 256K on LongSWE-Bench. I saw this in a search snippet and did not open the paper.
- FACT. SlopCodeBench (arXiv 2603.24755, 2026). Structural erosion rose in 77 percent of iterative trajectories and verbosity in 75.5 percent. Explicit quality guidance lowered the starting level but not the rate of degradation. This measures drift over iterations, not context length as such.

**Evidence for the hypothesis**
- FACT, VENDOR-SELF. Anthropic reports that Opus 4.6 scored 76 percent on the 8-needle, 1M-token MRCR v2 test, against 18.5 percent for Sonnet 4.5. Anthropic calls this a qualitative shift on context rot (5 February 2026).
- FACT. The Opus 5 announcement (24 July 2026) and the Opus 5.5 announcement (22 September 2026) publish no numeric long-context results. Opus 5.5 offers only customer anecdotes, such as one session staying on task for more than 18 hours.
- INFERENCE. Newer models probably degrade more slowly. I found no independent 2026 measurement of agentic coding quality as a function of context fill for Opus 5.x, GPT-5.x or Gemini 3.x.

**Implication for a multi-line agent project with large startup files.** Startup files take token budget at every position of every session. The Du et al. result suggests they lower reasoning quality even when they are on topic. The SWE-bench oracle-collapsed result suggests the gain comes from relevance density, not from volume. Treat the 1M window as headroom, not as a target. HYPOTHESIS: if the operator's observed gains are real, they more likely come from relevant facts being present than from the total amount of context.

Sources: https://www.trychroma.com/research/context-rot ; https://arxiv.org/abs/2510.05381 ; https://arxiv.org/abs/2307.03172 ; https://arxiv.org/abs/2502.05167 ; https://arxiv.org/abs/2404.06654 ; https://arxiv.org/abs/2412.15204 ; https://arxiv.org/abs/2602.16069 ; https://arxiv.org/abs/2310.06770 ; https://arxiv.org/html/2505.07897v2 ; https://arxiv.org/abs/2603.24755 ; https://www.anthropic.com/news/claude-opus-4-6 ; https://www.anthropic.com/news/claude-opus-5 ; https://www.anthropic.com/news/claude-opus-5-5

## 2. Compaction and summarization

- FACT. Anthropic, "Effective context engineering for AI agents" (29 September 2025). It treats context as "a finite resource with diminishing marginal returns" and advises finding "the smallest set of high-signal tokens". Claude Code compaction keeps a summary plus the five most recently accessed files. Anthropic calls tool-result clearing "one of the safest lightest touch forms of compaction".
- FACT. Current Claude Code documentation:
  - After compaction, the project-root CLAUDE.md, auto memory and the plan are re-injected from disk.
  - Path-scoped rules and nested CLAUDE.md files are summarized away.
  - Skill bodies are capped at 5,000 tokens each and truncated from the end.
  - `/autocompact` sets the threshold.
  - The documentation states that "LLM performance degrades as context fills".
- FACT. The API offers several context-management tools.
  - Context editing provides `clear_tool_uses_20250919`, which by default triggers at 100,000 input tokens and keeps the last 3 tool uses, and `clear_thinking_20251015`.
  - Clearing invalidates the cached prefix.
  - When clearing is near, the memory tool warns Claude so it can save state first.
  - Server-side compaction exists in two forms, on-demand (`compact-2026-09-04`) and threshold-based. The documentation says compaction keeps active context small "because response quality degrades as a conversation grows".
- FACT, VENDOR-SELF. Anthropic reports that context editing improved an internal agentic-search evaluation by 29 percent, and by 39 percent when combined with the memory tool. It cut token use by 84 percent in a 100-turn web-search evaluation (29 September 2025).
- FACT. OpenAI provides `POST /responses/compact`. A forum staff reply distinguishes it from `truncation: "auto"`. Launch coverage says GPT-5.1-Codex-Max was trained to work across context windows through compaction. These come from search snippets because the OpenAI blog returned 403.

**Information loss**
- FACT. ACE (ICLR 2026) found that a fully rewritten context shrank from 18,282 tokens to 122 tokens in one step. Accuracy fell from 66.7 to 57.1, below the 63.7 baseline with no context.
- FACT. The Compaction Cliff (arXiv 2608.22752, CIKM 2026) tested Claude Code's /compact prompt on Sonnet 4.6. It kept 53 percent of safety rules after one round and 10 percent after five, across 20 production configurations.
- FACT. CompactionRL (arXiv 2607.05378) and AutoCompact (arXiv 2610.02163) gain 5 to 9 points on SWE-bench Verified by training the compaction behaviour. INFERENCE: untrained inference-time compaction leaves measurable performance unrealized.
- FACT. Cognition (June 2025) says LLM compression is hard to get right and eventually reaches limits.

**Implication.** Compaction loses information, and each round loses more. Anything that must survive should live on disk in a file the harness re-injects, which in Claude Code means the root CLAUDE.md, not path-scoped rules. It should not depend on the summary. Repeated compaction of a long session is the case most at risk. This argues for ending a session deliberately with a handoff rather than compacting many times (INFERENCE).

Sources: https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents ; https://code.claude.com/docs/en/context-window ; https://code.claude.com/docs/en/best-practices ; https://platform.claude.com/docs/en/build-with-claude/context-editing ; https://platform.claude.com/docs/en/build-with-claude/compaction ; https://claude.com/blog/context-management ; https://developers.openai.com/api/reference/resources/responses/methods/compact ; https://community.openai.com/t/gpt-5-1-codex-max-is-now-available-in-the-api/1368399 ; https://arxiv.org/abs/2510.04618 ; https://arxiv.org/abs/2608.22752 ; https://arxiv.org/abs/2607.05378 ; https://arxiv.org/abs/2610.02163 ; https://arxiv.org/abs/2605.23296 ; https://cognition.com/blog/dont-build-multi-agents

## 3. Structured note-taking and external memory

- FACT. Anthropic, "Effective harnesses for long-running agents" (26 November 2025).
  - An initializer agent writes `init.sh`, a progress log, a feature list in JSON with every feature marked failing, and a first git commit.
  - JSON was chosen because the model is "less likely to inappropriately change or overwrite JSON files compared to Markdown files".
  - Each session works on one feature, then commits and updates the progress file.
  - Failure modes observed include declaring victory too early and marking features done without verification.
  - The article reports no benchmark numbers.
- FACT. Manus (Yichao Ji, 18 July 2025).
  - KV-cache hit rate is "the single most important metric". Cached Sonnet input cost $0.30 per million tokens against $3 uncached.
  - The input-to-output ratio is about 100 to 1.
  - Manus masks tools instead of removing them, and uses the file system as recoverable compressed context.
  - It recites `todo.md` to keep the plan near the end of the context, keeps errors in context, and varies serialization to avoid few-shot ruts.
  - These are practitioner claims, not controlled measurements.
- FACT. ACE treats context as a playbook updated by itemized deltas that are merged without an LLM, rather than by monolithic rewrites. It reports gains of 10.6 points on agents and 8.6 on finance. It argues that LLMs, "unlike humans", benefit from long, detailed contexts. This is the strongest published support for the "rich context" half of the hypothesis. Note that it concerns curated, structured detail, not raw history.
- FACT. Cao et al. (arXiv 2603.20432, March 2026) report that coding agents that keep long text in files and work on it with tools beat published state of the art by 17.3 percent on average.
- FACT. MemGPT (2023) proposed OS-style tiered virtual context.

**Implication.** The evidence favours three things.
- Append-only or delta-structured logs, because rewriting collapses content.
- Machine-readable state files for status flags, which models corrupt less.
- Pointers to files instead of inlined content.

An overwrite-only file such as a reverse prompt is exactly the "monolithic rewrite" pattern ACE warns about, unless an append-only journal backs it. A bounded overwrite file paired with an append-only journal matches the literature (INFERENCE). A stable startup prefix also preserves cache hits.

Sources: https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents ; https://manus.im/blog/Context-Engineering-for-AI-Agents-Lessons-from-Building-Manus ; https://arxiv.org/abs/2510.04618 ; https://arxiv.org/abs/2603.20432 ; https://arxiv.org/abs/2310.08560

## 4. Sub-agent isolation

- FACT, VENDOR-SELF. Anthropic's research system (13 June 2025).
  - Agents use about 4 times the tokens of chat, and multi-agent systems about 15 times.
  - The multi-agent design beat single-agent Opus 4 by 90.2 percent on an internal research evaluation.
  - Token usage explained 80 percent of variance on BrowseComp.
  - Coding is a weaker fit because it has "fewer truly parallelizable tasks".
- FACT. Anthropic, September 2025. Subagents return 1,000 to 2,000 tokens of summary from tens of thousands of tokens of exploration.
- FACT. Cognition, "Don't Build Multi-Agents" (June 2025). It gives two principles, share full traces and remember that actions carry implicit decisions.
- FACT. The April 2026 revision, "Multi-Agents: What's Actually Working", keeps writes single-threaded.
  - The patterns that work are a clean-context reviewer, a "smart friend" escalation that receives a fork of the full context, and manager-child map-reduce.
  - Devin Review catches an average of 2 bugs per PR, about 58 percent of them severe (VENDOR-SELF).
  - The post credits part of the reviewer's gain to context rot.
- FACT. LangChain (July 2025) proposes the write, select, compress and isolate taxonomy, and cites Breunig's failure modes, namely poisoning, distraction, confusion and clash.

**Implication.** Isolate reading and reviewing, not writing. For escalation, Cognition finds that full shared context is the practical 80/20 choice. This partly supports the hypothesis, since a decision-maker should see the decision-relevant history. Expect multiplied token cost.

Sources: https://www.anthropic.com/engineering/multi-agent-research-system ; https://cognition.com/blog/dont-build-multi-agents ; https://cognition.com/blog/multi-agents-working ; https://www.langchain.com/blog/context-engineering-for-agents

## 5. Session boundaries and instruction-file length

- FACT. Claude Code best-practices guidance on sessions:
  - Run `/clear` between unrelated tasks.
  - After two failed corrections, clear the session and re-prompt, because "a clean session with a better prompt almost always outperforms a long session with accumulated corrections".
  - Write a specification, then start a fresh session to implement it.
  - The same page also says "Sometimes you should let context accumulate because you're deep in one complex problem and the history is valuable." This vendor statement supports the focused-conversation clause of the hypothesis.
- FACT. Laban et al. (arXiv 2505.06120) report a 39 percent average drop in multi-turn versus single-turn settings across six tasks. Most of the drop is unreliability rather than lost aptitude.
- FACT. Claude Code best-practices guidance on instruction files:
  - "Bloated CLAUDE.md files cause Claude to ignore your actual instructions".
  - If Claude disobeys a rule, "the file is probably too long".
  - Emphasizing many lines makes none stand out.
- FACT. IFScale (Distyl). The best frontier models reached about 68 percent accuracy at 500 simultaneous instructions, with a bias toward earlier instructions. Related 2025 papers report falling all-instructions accuracy as the instruction count rises.
- FACT. Gloaguen et al. (arXiv 2602.11988, ETH, 2026). AGENTS.md-style context files did not generally improve task success and raised inference cost by more than 20 percent. The result held for both LLM-generated and developer-written files. Agents follow the instructions, and repository overviews were not helpful. The files did help for non-standard practices.
- FACT. Codex stops loading AGENTS.md content past `project_doc_max_bytes`, which defaults to 32 KiB, so later guidance can be dropped silently.
- No source I found directly compares overwrite-only and append-only state files for agents. ACE's collapse result is the nearest evidence (INFERENCE).

**Implication.** Large startup files are the configuration this evidence most directly targets. Expected effects are cost growth of at least 20 percent, crowding out of rules, and early-position bias (INFERENCE from Gloaguen and IFScale).
- Move rarely needed material behind pointers or on-demand skills.
- Keep non-standard, high-consequence rules near the top.
- Measure adherence rather than assuming it.

Sources: https://code.claude.com/docs/en/best-practices ; https://arxiv.org/abs/2505.06120 ; https://distylai.github.io/IFScale ; https://arxiv.org/pdf/2509.21051 ; https://arxiv.org/abs/2602.11988 ; https://learn.chatgpt.com/docs/agent-configuration/agents-md

## 6. Cheaper models making bounded decisions over large inputs

- FACT. FrugalGPT (2023) matched GPT-4 at up to 98 percent lower cost through an LLM cascade.
- FACT. RouteLLM (2024) cut cost by more than 2 times in some cases without losing quality.
- FACT. Minions (Stanford, February 2025) had a small local model read document chunks while a frontier model decomposed the task and decided. This cut cost 5.7 times while keeping 97.9 percent of performance. A naive chat protocol kept only 87 percent.
- FACT. Cognition (2026) found that a weak primary model did not know when to escalate. Escalation worked when both models were strong.
- FACT. LLM judges show position, verbosity and self-enhancement biases. GPT-4 agreed with humans more than 80 percent of the time (Zheng et al., 2023).
- FACT. Claude Code's auto mode already uses a separate classifier model to gate actions.

**Implication.** "Small model reads, big model decides" is supported, provided the big model sets the question and the answer space is bounded. Calibrate the judge against held-out labels and randomize option order to control position bias (INFERENCE). Do not let the cheap model decide when to escalate.

Sources: https://arxiv.org/abs/2305.05176 ; https://arxiv.org/abs/2406.18665 ; https://arxiv.org/abs/2502.15964 ; https://cognition.com/blog/multi-agents-working ; https://arxiv.org/abs/2306.05685 ; https://code.claude.com/docs/en/best-practices

## 7. What 2026 changes

- FACT. Compaction is now productized and trained into models. Examples are Anthropic's server-side compaction, OpenAI's compact endpoint and Codex-Max, and RL-trained compaction in CompactionRL, which feeds GLM-5.2 training, and AutoCompact.
- FACT. 1M-token windows are standard on Claude models from Opus 4.6 onward. Anthropic published no new long-context numbers for Opus 5 or 5.5.
- FACT. The new benchmarks SWE-Marathon, with an average of 27.2 million tokens per attempt and fewer than 30 percent of tasks solved, and SlopCodeBench target long-horizon degradation.
- INFERENCE. The field is moving toward *managed* long context, meaning large windows plus trained compaction plus external state. It is not moving toward "keep everything". It is also not moving toward minimal context, as ACE and Cognition's full-context fork show.

**Implication.** Re-test the hypothesis empirically on the project's own tasks. Hold the task fixed and vary startup size and session length, and count outcomes, not impressions. No public study answers it for this configuration.

Sources: https://platform.claude.com/docs/en/build-with-claude/compaction ; https://arxiv.org/abs/2607.05378 ; https://arxiv.org/abs/2610.02163 ; https://arxiv.org/abs/2606.07682

## Unverified claims and unfetched sources

- **OpenAI.** https://openai.com/index/gpt-5-1-codex-max/ returned 403. The Codex-Max compaction claims come from search snippets and the forum only. I did not verify Codex CLI compaction thresholds.
- **Fiction.LiveBench.** The page returned only site navigation. Current Gemini 3 and GPT-5.x results at 192K and above are unverified. A January 2026 snippet says gemini-3-pro-preview and gpt-5.2 are "neck and neck" near perfect at the top.
- **Model-specific MRCR.** I found no primary MRCR or long-context figure for Opus 5, Opus 5.5, GPT-5.x or Gemini 3.x. Third-party aggregators such as morphllm, codingfleet and benchlm disagree with each other, so I did not use them.
- **Per-condition AGENTS.md figures.** The success-rate drops for AGENTS.md files, about 0.5 to 2 percent, come from a secondary blog. The abstract I saw states only "no general improvement" and "more than 20 percent cost".
- **LongCodeBench.** The 29 to 3 percent figure comes from a search snippet. I did not open the paper.
- **IFScale.** I did not open the paper, and the figures come from the project-page snippet. I could not resolve its arXiv identifier.
- **Parallel Context Compaction.** The claim in arXiv 2605.23296 that coding agents over-compress recent context does not appear in its abstract. I treat it as unverified.
- **References only.** I did not fetch CliffCompaction (arXiv 2609.26779) or "structured context eviction" (arXiv 2606.11213).
- **Codex AGENTS.md.** I did not open the main openai/codex AGENTS.md documentation beyond the redirected OpenAI page.
- **Overwrite versus append.** I found no controlled study of overwrite-only versus append-only agent state files. The conclusions in sections 3 and 5 on this point are inferences from ACE.
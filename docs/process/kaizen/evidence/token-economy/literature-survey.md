<!-- Provenance: the final report of a general-purpose subagent spawned by the kaizen line on 2026-10-24, saved verbatim. It is model output. Its [F] marks are the agent's own claims of having fetched a source. The kaizen line independently re-fetched https://code.claude.com/docs/en/costs, https://code.claude.com/docs/en/memory and https://code.claude.com/docs/en/model-config and checked the TTL split against measurements.json. Other figures are unverified by the kaizen line. -->

# Token Economization for LLM Coding Agents: Literature Survey (as of 2026-10-24)

Epistemic legend. **[V]** means a vendor claim from the vendor's own documentation or blog. **[P]** means a measured result in a paper, with the paper's own conditions. **[I]** means my own inference and is not sourced. **[F]** means I fetched the source in this session. **[M]** means I cited it from memory and did not re-verify it here.

Some Anthropic documentation now names models such as Opus 5.5, Sonnet 5.5, Fable 5.1, and Haiku 5.5. I report those figures exactly as the fetched pages state them.

---

## 1. Prompt caching (input-side, cached tokens)

### 1.1 Anthropic API [V][F]
Source: https://platform.claude.com/docs/en/build-with-claude/prompt-caching and https://platform.claude.com/docs/en/about-claude/pricing

- **Mechanism.** The API matches an exact prefix in the order `tools` → `system` → `messages`. A change at one level invalidates that level and every later level.
- **Price multipliers.** A 5-minute write costs 1.25× base input. A 1-hour write costs 2×. A read costs 0.1× by default, 0.05× on Opus 5.5 and Sonnet 5.5, and 0.025× on Fable 5.1 and Mythos 5.1.
- **Break-even.** The pricing page says caching pays off "after one cache read for the 5-minute duration … or after two cache reads for the 1-hour duration."
- **List prices for Opus 5.5.** Input is $4/MTok, a 5-minute write $5, a 1-hour write $8, a cache hit $0.20, and output $20. Sonnet 5.5 is $2 / $2.50 / $4 / $0.10 / $10.
- **TTL.** The default is 5 minutes and is refreshed free on every hit. A 1-hour option is set with `"ttl": "1h"`. The lifetime is measured from the *start* of the request, so a 4-minute stream leaves about 1 minute for the follow-up.
- **Minimum cacheable length.** It is 512 tokens on Opus 5.5, Sonnet 5.5, and the Fable models. It is 1,024 on Opus 4.8 and Sonnet 4.5/4.6, and 4,096 on Opus 4.5/4.6 and Haiku 4.5. A shorter prompt is silently left uncached.
- **Breakpoints.** There are at most 4 breakpoints, and the lookback window is 20 blocks.
- **Invalidation triggers.**
  - Any change to tool definitions invalidates everything.
  - Toggling web search or citations, or changing the speed setting, invalidates system and messages.
  - Changing `tool_choice`, or adding or removing an image, invalidates messages.
  - Changing the thinking mode or budget invalidates messages.
  - An effort change is model-specific.
- **Other effects.** Caching has "no effect on output token generation." Cache hits are not counted against rate limits.
- **Failure modes.**
  - Silent non-caching below the minimum length.
  - A gateway that strips `cache_control` markers "bills as uncached input on every turn" (https://code.claude.com/docs/en/prompt-caching [F]).
- **Applicability.** On the API you control breakpoints and TTL. In Claude Code you control only the harness settings, which §1.2 covers.

### 1.2 Claude Code's caching behavior (harness the user does not control internally) [V][F]
Source: https://code.claude.com/docs/en/prompt-caching

- **Layering.** The system prompt and tools come first, then project context (CLAUDE.md, auto memory, unscoped rules), then the conversation.
  - Editing CLAUDE.md mid-session neither invalidates the cache nor takes effect until `/clear`, `/compact`, or a restart.
  - Skills, plan mode, output-style switches, and subagent spawns append to the conversation and keep the prefix.
- **Actions that invalidate the cache.**
  - Switching models. This includes `opusplan` toggles and a skill whose frontmatter names a different `model`.
  - Changing effort on most models. On Opus 5.5, Sonnet 5.5, Haiku 5.5, and Fable 5.1 with an API key or subscription, the cache is kept.
  - Enabling fast mode, the first time only.
  - Connecting or removing an MCP server, but only when tools are not deferred.
  - Bare-name deny rules, again only without tool search.
  - Compaction, which rebuilds the conversation layer.
  - Accumulating images.
  - Upgrading Claude Code.
- **TTL buckets.**
  - On a subscription within plan usage, the main conversation gets a 1-hour TTL.
  - Subagents, workflows, teammates, forks, compaction, and titles get **5 minutes**.
  - Under an API key, usage credits, or a cloud provider, everything gets 5 minutes.
  - The defaults can be overridden with `promptCacheTtl` / `CLAUDE_CODE_PROMPT_CACHE_TTL` and with `subagentPromptCacheTtl` / `CLAUDE_CODE_SUBAGENT_PROMPT_CACHE_TTL`. These need v2.1.242 or later.
- **Cache scope.** The cache is "effectively scoped to one machine and directory." The system prompt embeds the auto memory paths and the working directory. Sequential sessions share a prefix only if the startup git-status snapshot matches.
- **Subagents.** A subagent does not read the parent's cache. A **fork** inherits the parent prefix, so its first request reads the parent's cache.
- **Compaction cost.** A warm `/compact` costs "a fraction of what the context size suggests." A cold one, after the TTL has lapsed, reprocesses the whole history as uncached input. `/rewind` truncates back to an already-cached prefix.
- **Diagnostics.** `/usage` shows a "Prompt cache (main)" hit ratio and the likely cause of misses (v2.1.251 or later).
- **[I] Implications for this project.**
  - Parallel worktrees in different directories cannot share each other's cache.
  - A local test gate longer than 5 minutes run by a subagent, or a background agent that waits on one, will likely let that agent's cache go cold. A main conversation idle for more than 1 hour goes cold on the next turn.
  - Setting `subagentPromptCacheTtl: 1h` is worth measuring for agents that poll long gates. The higher write price makes it a trade-off, not a guaranteed saving.

### 1.3 OpenAI [V][F]
Source: https://developers.openai.com/api/docs/guides/prompt-caching

- **Minimum length.** On GPT-5.6 and later the prefix must be at least 1,024 visible tokens.
- **Discount.** Cached input costs "up to 95% less." Reads cost 0.1× on GPT-5.6 and later, and 0.05× on GPT-6.1 Sol.
- **Retention.**
  - Older models in memory: about 5 to 10 minutes of inactivity, up to 1 hour.
  - The 24-hour option on older models.
  - GPT-5.6 and later: a 30-minute minimum by default, with a 12-hour option in the Responses API.
- **Routing.** Routing uses a hash of the initial tokens plus `prompt_cache_key`, with about 15 requests per minute per key before traffic overflows to other machines.
- **What breaks a match.** Changes to `model`, `tools`, `parallel_tool_calls`, `text.format`, `reasoning.effort`, `text.verbosity`, or `context_management` can break it.
- **Applicability to Codex.** Codex users do not set these values directly. **[I]** Stable AGENTS.md content and a constant reasoning effort within a session should preserve hits.

---

## 2. Context reduction inside a session (input-side)

### 2.1 Tool-result clearing and context editing (Anthropic API) [V][F]
Source: https://platform.claude.com/docs/en/build-with-claude/context-editing

- **What it does.** `clear_tool_uses_20250919` replaces the oldest tool results with placeholders.
  - The defaults are a trigger at 100,000 input tokens and keeping the last 3 tool uses.
  - `clear_at_least` exists so that clearing is worth the cache rewrite it causes.
  - `exclude_tools` exempts named tools.
- **Thinking blocks.** `clear_thinking_20251015` drops thinking blocks from earlier turns.
- **Beta header.** `context-management-2025-06-27`.
- **Caching cost.** Clearing *invalidates* the cached prefix at the clear point.
- **Vendor numbers.** Context editing improved an internal agentic-search evaluation by 29%, and by 39% when combined with the memory tool. It cut token consumption by **84%** in a 100-turn web-search evaluation (https://claude.com/blog/context-management [F], 2025-09-29). The post gives no baseline, sample size, or variance.
- **Claude Code.** Claude Code does its own tool-result clearing and counts it as an "expected rebuild" (https://code.claude.com/docs/en/costs [F]).

### 2.2 Observation masking vs. LLM summarization (paper) [P][F]
Source: https://arxiv.org/abs/2508.21433 (HTML: https://arxiv.org/html/2508.21433)

- **Setup.** SWE-agent on SWE-bench Verified (n=500). Masking replaces observations older than the last M=10 turns with a placeholder.
- **Results on Qwen3-Coder 480B.**

| Strategy | Solve rate | Cost per instance |
|---|---|---|
| Raw agent | 53.4% | $1.29 |
| Observation masking | 54.8% | $0.61 (−52.7%) |
| LLM summarization | 53.8% | $0.64 (−50.4%) |

- **Results on other models.** Gemini 2.5 Flash with masking cost 56% less, and its solve rate rose from 32.8% to 35.6%. With thinking enabled, however, masking *lowered* the solve rate from 40.4% to 36.4%, and summarization lowered it to 31.4%. Qwen3-32B with masking lost 2 points.
- **Trajectory length.** Summarization lengthened trajectories by about 15%.
- **Hybrid.** A masking-plus-summary hybrid cost 7% less than masking alone, tested on only 50 instances.
- **Caveats.** Open models only, and one scaffold. The correctness effect is model-dependent.

### 2.3 SWE-agent ACI ablations (paper) [P][F]
Source: https://arxiv.org/html/2405.15793v3, Table 3, SWE-bench Lite, GPT-4 Turbo, default resolve rate 18.0%.

| Component | Variant | Resolve rate |
|---|---|---|
| File viewer | 100-line window (default) | 18.0% |
| File viewer | 30-line window | 14.3% |
| File viewer | Full file | 12.7% |
| Context | Keep last 5 observations, collapse the rest (default) | 18.0% |
| Context | Full history | 15.0% |
| Search | Summarized search (default) | 18.0% |
| Search | Iterative search | 12.0% |
| Edit | Edit with linting (default) | 18.0% |
| Edit | Edit without linting | 15.0% |

- **Takeaway.** Bounded observations improved correctness, not only cost. The evidence is old (2024) and limited to one model.

### 2.4 Compaction [V][F]
- **Claude Code.** `/compact [focus]` is available, and `/autocompact <tokens>` sets the threshold (https://code.claude.com/docs/en/context-window).
  - After compaction, Claude Code re-injects the project-root CLAUDE.md, auto memory, and the plan file.
  - It re-reads up to 5 recent files. Files over 5,000 tokens become path references only.
  - Skill bodies are re-injected, capped at 5,000 tokens each and 25,000 tokens total.
  - Hook-added context is summarized away. SessionStart hooks with the `compact` source re-run.
- **Anthropic API.** Server-side compaction is recommended. Client-side SDK compaction defaults to a 100,000-token threshold and overcounts when server-side tools are in use: the page's example shows 334,400 counted against about 63,000 real tokens (https://platform.claude.com/docs/en/build-with-claude/context-editing).
- **OpenAI.** Responses `context_management` takes `compact_threshold` (examples use 200,000), and compaction produces an opaque encrypted item (https://developers.openai.com/api/docs/guides/compaction.md).
- **Codex CLI.** Third-party posts report a `model_auto_compact_token_limit` clamped to 90% of the window. I could not verify this in official docs.
- **Failure mode.** Compaction is lossy, and instructions given only in the conversation are lost. **[I]** Compaction also costs one large request, so it should run at task boundaries while the cache is warm.

### 2.5 `/clear`, subagents, rewind [V][F]
Source: https://code.claude.com/docs/en/costs

- `/clear` "costs nothing" and is recommended between unrelated tasks.
- Delegating verbose work such as tests, log processing, or documentation fetches to subagents keeps that output out of the main context. The subagent's own tokens still count toward usage.
- Agent teams use "approximately 7x more tokens than standard sessions when teammates run in plan mode."
- Anthropic's context-engineering post says subagents typically return condensed summaries of "1,000-2,000 tokens" (https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents [F]).
- **[I] Failure modes.**
  - Subagents start cold, with no parent cache and a 5-minute TTL.
  - Summaries can drop detail that the parent then re-derives.
  - A subagent handed a hypothesis tends to confirm it.

---

## 3. Always-loaded instruction economy (input-side, recurring)

### 3.1 CLAUDE.md sizing [V][F]
Source: https://code.claude.com/docs/en/memory

- "Target under 200 lines per CLAUDE.md file. Longer files consume more context and reduce adherence."
- `@imports` "don't reduce its context cost." They load together with the importing file, up to 4 hops deep.
- Path-scoped `.claude/rules/` files and nested CLAUDE.md files load on demand.
- Block-level HTML comments are stripped before injection, so they cost zero tokens. This is a free channel for maintainer notes.
- For auto memory, only the first 200 lines or 25KB of MEMORY.md load. Topic files load on demand.
- The costs page adds: "move instructions from CLAUDE.md to skills" (https://code.claude.com/docs/en/costs).
- **[I] Cost model.** Under caching, a large CLAUDE.md costs about 0.05× to 0.1× of input per turn after the first write. The context-occupancy and adherence costs are not discounted by caching.

### 3.2 AGENTS.md (Codex) [V][F]
Source: https://learn.chatgpt.com/docs/agent-configuration/agents-md

- Files are concatenated from the root down to the current directory, with one file per directory. `AGENTS.override.md` takes precedence.
- `project_doc_max_bytes` defaults to **32 KiB**. Codex "stops adding files once the combined size reaches the limit," so the tail is silently truncated.
- **[I]** That truncation is a correctness risk for long instruction files shared between harnesses.

### 3.3 Skills progressive disclosure [V][F]
Source: https://code.claude.com/docs/en/skills

- Only the name and description load until the skill is invoked. The combined description is truncated at 1,536 characters in the listing.
- The recommended SKILL.md length is under 500 lines.
- Once invoked, the body stays in context across turns. A re-invoke with identical content adds only an "already loaded" note.

### 3.4 Hook-injected text [V][F]
Source: https://code.claude.com/docs/en/hooks

- UserPromptSubmit fires on every prompt. Its stdout or `additionalContext` is added "alongside the submitted prompt."
- The cap is 10,000 characters per string. Overflow goes to a file with a 2,000-character preview.
- **[I]** Repeated reminder text is appended each turn rather than placed in the prefix. It therefore accumulates linearly in the conversation. It is cached on later turns but still occupies context and is summarized at compaction. Moving static text to CLAUDE.md, or emitting it only when its condition changes, removes the accumulation.

### 3.5 Tool definitions and deferred tool search [V][F]
- **Claude Code.** MCP tools are deferred by default, so only names enter context. CLI tools such as `gh` remain "more context-efficient than MCP servers" (https://code.claude.com/docs/en/costs).
- **Tool Search Tool.** Anthropic reports upfront tool context falling from about 77K to about 8.7K tokens. MCP-evaluation accuracy rose from 49% to 74% on Opus 4 and from 79.5% to 88.1% on Opus 4.5. One deployment saw 134K tokens of tool definitions before optimization (https://www.anthropic.com/engineering/advanced-tool-use).
- **Programmatic tool calling.** Average tokens fell from 43,588 to 27,297, a 37% reduction, on the vendor's internal tasks (same source).
- **Code execution with MCP.** Anthropic gives an illustrative drop from 150,000 to 2,000 tokens (98.7%) for a single workflow, with no stated measurement method. Running agent code requires sandboxing (https://www.anthropic.com/engineering/code-execution-with-mcp).

---

## 4. Observation economy (tool output)

- **Hook-side filtering.** Anthropic's example is a PreToolUse hook that rewrites test commands to `grep -A 5 -E '(FAIL|ERROR|error:)' | head -100`. The doc says a log grep reduces "tens of thousands of tokens to hundreds" (https://code.claude.com/docs/en/costs [V][F]).
  - **[I] Correctness risk.** Filters can hide warnings, panics that match none of the patterns, or summary lines such as Rust's `test result:` line. A whitelist must be validated against the gate's real failure output.
  - **[I] For long local gates.** Writing full output to a log file and returning only the exit status, the failing test names, and a tail is the analogue. It keeps the full output auditable on disk.
- **Bounded views and line ranges.** SWE-agent's 100-line window outperformed full-file viewing (§2.3, [P][F]).
- **Code intelligence over grep-then-read.** Claude Code recommends code-intelligence (LSP) plugins: "a single 'go to definition' call replaces … a grep followed by reading multiple candidate files" (https://code.claude.com/docs/en/costs [V][F]). No quantitative figure is given.
- **Agentic search vs. vector retrieval.** Secondary reports say Claude Code dropped vector-index retrieval for grep-style agentic search, citing simplicity, staleness, and security. I did not retrieve the primary post, so this is unverified (https://smartscope.blog/en/ai-development/practices/rag-debate-agentic-search-code-exploration/ [F, secondary]). Cursor took the opposite route with a semantic index. I found no peer-reviewed head-to-head on token cost in this session.
- **Where the tokens go.** Bai et al. analyzed eight frontier LLMs on SWE-bench Verified with OpenHands [P][F] (https://arxiv.org/abs/2604.22750).
  - Agentic coding uses "1000x more tokens than code reasoning and code chat."
  - Input tokens, not output tokens, drive the cost.
  - Runs on the same task vary up to 30× in total tokens.
  - Accuracy "often peaks at intermediate cost and saturates." This is direct evidence that spending more tokens does not reliably buy correctness.
  - Models predict their own usage poorly, with correlations of at most 0.39.

---

## 5. Output-token economy

- **Effort and reasoning budgets (Anthropic)** [V][F] (https://platform.claude.com/docs/en/build-with-claude/effort).
  - Effort affects "all tokens," meaning text, tool calls, and thinking. Lower effort produces "fewer and terser tool calls."
  - Opus 5.5 defaults to `medium`, and its thinking cannot be disabled. The doc recommends `low` for "simpler tasks … such as subagents."
  - A top-level effort change invalidates the cache. A per-message effort change (beta header `mid-conversation-output-config-2026-07-01`) preserves it.
  - In Claude Code, thinking is billed as output, and the default "can be tens of thousands of tokens per request." `MAX_THINKING_TOKENS` applies only to fixed-budget models (https://code.claude.com/docs/en/costs).
  - The docs give no quantitative quality-versus-effort curve. Anthropic says to run your own evaluation sweep.
- **Reasoning effort (OpenAI)** [V][F] (https://developers.openai.com/api/docs/guides/reasoning).
  - Levels run from `none` to `max`, and reasoning tokens are billed as output.
  - The guide recommends reserving at least 25,000 tokens for reasoning and output.
  - Passing reasoning items back, including `encrypted_content`, is described as the "most token-efficient" way to continue.
- **Output prices.** Output is 5× input on Opus 5.5 ($20 vs. $4) and 100× a cache read ($0.20) (pricing page [V][F]). **[I]** Per token, verbose prose and whole-file rewrites are the most expensive things an agent emits. Because Bai et al. find input dominates in aggregate, output is the second lever, not the first.
- **Diff edits vs. whole-file rewrites.** In Aider's laziness benchmark on GPT-4 Turbo (89 tasks), unified diffs scored 61% against 20% for SEARCH/REPLACE, with lazy comments on 4 tasks versus 12 (https://aider.chat/docs/unified-diffs.html [P, practitioner benchmark][F]). The benchmark measures correctness by edit format. It does not measure tokens. **[I]** String-replace edits emit only the changed span plus anchors, but a failed unique match costs a retry.
- **Concise-output instructions.** Opus 5 documentation notes that "changing effort does not reliably shorten responses, so prompt for length instead" (effort page [V][F]).

---

## 6. Prompt compression (input-side, API only)

All figures below are abstract-level claims [P][F].

| Method | Source | Reported result |
|---|---|---|
| LLMLingua | https://arxiv.org/abs/2310.05736 | "up to 20x compression with little performance loss" on GSM8K, BBH, ShareGPT, and Arxiv |
| LongLLMLingua | https://arxiv.org/abs/2310.06839 | Up to +21.4% on NaturalQuestions with GPT-3.5-Turbo at about 4× fewer tokens; 94.0% cost reduction on LooGLE; latency 1.4× to 2.6× faster at 2× to 6× compression |
| LLMLingua-2 | https://arxiv.org/abs/2403.12968 | 2× to 5× compression; 3× to 6× faster than prior compressors; end-to-end latency 1.6× to 2.9× lower |

- **[I] Applicability.**
  - These methods drop tokens. That is unsafe for code, identifiers, file paths, and specification text, where one dropped token changes meaning. They suit natural-language retrieval context.
  - They cannot be inserted into Claude Code without a gateway.
  - Compressed text that changes from turn to turn defeats prefix caching, and caching may already give a 10× to 20× discount on the same tokens.

---

## 7. Model cascades and routing

| Method | Source | Reported result |
|---|---|---|
| FrugalGPT | https://arxiv.org/abs/2305.05176 [P][F] | Matches GPT-4 at "up to 98% cost reduction," or about +4% accuracy at equal cost; datasets not named in the abstract |
| RouteLLM | https://arxiv.org/abs/2406.18665 [P][F] | Savings "over 2 times in certain cases" without quality loss |

- **Vendor guidance in Claude Code.** Use Sonnet for most coding, reserve Opus, set `model: haiku` for simple subagents, and use Sonnet for teammates (https://code.claude.com/docs/en/costs [V][F]).
- **[I] Failure modes.**
  - A weaker model's confident wrong answer is the core risk, especially for verification or review tasks.
  - In a harness, a model switch inside a conversation invalidates the cache (§1.2). Routing at the subagent boundary is cheaper than mid-conversation switching.

---

## 8. Batch and asynchronous pricing (API only) [V][F]
Source: https://platform.claude.com/docs/en/build-with-claude/batch-processing and the pricing page

- **Discount.** 50% off input and output. Opus 5.5 batch prices are $2 input and $10 output.
- **Turnaround.** Most batches finish within 1 hour, with a hard expiry at 24 hours.
- **Stacking.** The discount stacks with caching, but batch cache hits are "best-effort … 30 percent to 98 percent." The doc recommends the 1-hour TTL for batches.
- **Interactive use.** There is no batch mode for interactive Claude Code sessions.
- **[I] Applicability.** Offline audits, documentation sweeps, and labelling jobs could run through the batch API if the operator holds an API key. Interactive agents cannot use it.

---

## 9. Semantic caching (response reuse)

- **MeanCache.** On 250 queries that should all miss, GPTCache produced 54 false hits and MeanCache produced 3 (https://arxiv.org/pdf/2403.02694 [P], from the search summary, PDF not fetched).
- **vCache.** Proposes per-prompt thresholds with user-set error bounds (https://arxiv.org/html/2502.03771v3 [P], not fetched).
- **Adversarial collisions.** A key-collision attack paper argues adversarial false hits are structural (https://arxiv.org/pdf/2601.23088 [P], not fetched).
- **[I] Applicability.** Returning a cached answer to a "similar" coding question is a correctness hazard whenever repository state differs. It is not recommended for coding agents. Exact prefix caching (§1) carries no such risk.

---

## 10. Long-context degradation (why less context can be more correct)

- **Lost in the Middle** [P][F] (https://arxiv.org/html/2307.03172).
  - In multi-document QA, GPT-3.5-Turbo scored 75.8% with the answer document first and 53.8% with it at position 10 of 20. The closed-book score was 56.1%, so mid-context accuracy fell *below* the no-document baseline.
  - The curve is U-shaped, favoring the beginning and the end.
  - The models are from 2023, and newer models may differ.
- **Context Rot** (Chroma, 2025-07-14) [P, technical report][F] (https://www.trychroma.com/research/context-rot).
  - 18 models, including Claude Opus 4 and Sonnet 4, GPT-4.1, o3, and Gemini 2.5.
  - Performance "degraded consistently as input grew," even on simple tasks.
  - Distractors compounded the loss.
  - On LongMemEval, every model scored significantly higher on about 300-token focused prompts than on about 113k-token full prompts.
  - Needle position showed no notable effect in their NIAH setup, which partly contradicts the Lost-in-the-Middle finding.
- **Vendor framing.** Anthropic calls context "a finite resource with diminishing marginal returns" and recommends just-in-time retrieval and structured notes (NOTES.md-style) outside the window (https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents [V][F]).
- **[I] Implication.** Trimming always-loaded process text can serve correctness and cost at once. This rests on the general degradation results; no study measured instruction adherence against CLAUDE.md length beyond Anthropic's unquantified "reduce adherence."

---

## 11. Applicability summary

**[I]** This summary is my synthesis, not sourced.

**Controllable within Claude Code** (settings, files, and hooks):
- Instruction size: CLAUDE.md under 200 lines, path-scoped rules, skills, and HTML comments.
- Hook output size and frequency.
- TTL settings: `promptCacheTtl` and `subagentPromptCacheTtl`.
- `/autocompact`, plus `/compact` and `/clear` at task boundaries.
- Model and effort choice for subagents.
- Holding the model and effort fixed within a session.
- Test-output filtering hooks.
- LSP plugins.
- Using forks rather than fresh subagents when parent context is needed, since forks share the cache.

**Not controllable** without an API key or gateway:
- Breakpoint placement.
- Context-editing parameters.
- LLMLingua-style compression.
- The batch API.
- Routing inside a turn.

**Codex:**
- Keep AGENTS.md under 32 KiB, or the tail is truncated.
- Keep reasoning effort fixed within a session.
- Compaction settings could not be verified from official sources.

**Measurement before change:**
- The Claude Code `/usage` cache line, its attribution breakdown, and `/context` provide baselines.
- Bai et al.'s 30× run-to-run variance means a single-run comparison cannot establish a saving. Repeated runs are required.

---

## 12. Could not verify
- The official Codex `/compact` and `model_auto_compact_token_limit` defaults. Only third-party blogs report them.
- The primary source for Claude Code's move away from RAG.
- RouteLLM numbers beyond the abstract.
- The MeanCache, vCache, and key-collision papers were not fetched; their figures come from search summaries.
- Exact Claude Code auto-compact thresholds per model. I did not fetch https://code.claude.com/docs/en/model-config.
- Anthropic's tool-search and programmatic-calling numbers are internal evaluations without published methodology.
- Any study isolating the token cost of per-prompt hook reminders. I found none.

---

## URL table

| URL | Status |
|---|---|
| https://platform.claude.com/docs/en/build-with-claude/prompt-caching | Fetched |
| https://platform.claude.com/docs/en/about-claude/pricing | Fetched |
| https://platform.claude.com/docs/en/build-with-claude/context-editing | Fetched |
| https://platform.claude.com/docs/en/build-with-claude/effort | Fetched |
| https://platform.claude.com/docs/en/build-with-claude/batch-processing | Fetched |
| https://code.claude.com/docs/en/costs | Fetched |
| https://code.claude.com/docs/en/prompt-caching | Fetched |
| https://code.claude.com/docs/en/memory | Fetched |
| https://code.claude.com/docs/en/skills | Fetched (first 100k characters) |
| https://code.claude.com/docs/en/hooks | Fetched (first 100k characters) |
| https://code.claude.com/docs/en/context-window | Fetched |
| https://claude.com/blog/context-management | Fetched |
| https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents | Fetched |
| https://www.anthropic.com/engineering/advanced-tool-use | Fetched |
| https://www.anthropic.com/engineering/code-execution-with-mcp | Fetched |
| https://developers.openai.com/api/docs/guides/prompt-caching | Fetched |
| https://developers.openai.com/api/docs/guides/reasoning | Fetched |
| https://developers.openai.com/api/docs/guides/compaction.md | Fetched |
| https://learn.chatgpt.com/docs/agent-configuration/agents-md | Fetched |
| https://arxiv.org/abs/2307.03172 and https://arxiv.org/html/2307.03172 | Fetched |
| https://www.trychroma.com/research/context-rot | Fetched |
| https://arxiv.org/abs/2310.05736 | Fetched (abstract) |
| https://arxiv.org/abs/2310.06839 | Fetched (abstract) |
| https://arxiv.org/abs/2403.12968 | Fetched (abstract) |
| https://arxiv.org/abs/2305.05176 | Fetched (abstract) |
| https://arxiv.org/abs/2406.18665 | Fetched (abstract) |
| https://arxiv.org/abs/2508.21433 and https://arxiv.org/html/2508.21433 | Fetched |
| https://arxiv.org/abs/2405.15793 and https://arxiv.org/html/2405.15793v3 | Fetched |
| https://arxiv.org/abs/2604.22750 | Fetched (abstract) |
| https://aider.chat/docs/unified-diffs.html | Fetched |
| https://smartscope.blog/en/ai-development/practices/rag-debate-agentic-search-code-exploration/ | Search result only, secondary |
| https://arxiv.org/pdf/2403.02694 (MeanCache) | Search result only |
| https://arxiv.org/html/2502.03771v3 (vCache) | Search result only |
| https://arxiv.org/pdf/2601.23088 (semantic-cache key collisions) | Search result only |
| https://codex.danielvaughan.com/2026/04/16/codex-cli-context-compaction-tuning-long-sessions/ | Search result only, third-party, unverified |

No repository files were written. Tool-output overflow files went only to the harness's own session directory under ~/.claude/projects/.../tool-results/.

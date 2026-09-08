# Evidence 6: external survey of autonomous multi-agent development workflows (Sonnet research agent, 2026-09-08)

Auditor's caveat: the agent graded and, where it says so, fetched these sources. The auditor has not independently re-fetched them. Figures the agent marked UNVERIFIED are reproduced as such. Grades: A controlled experiment or peer-reviewed measurement; B documented practice with reported outcomes or telemetry; C documented practice, no outcome data; D opinion.

## 1. Controlled and quasi-controlled evidence

- METR, "Measuring the Impact of Early-2025 AI on Experienced Open-Source Developer Productivity", 2025-07-10, metr.org/blog/2025-07-10-early-2025-ai-experienced-os-dev-study. 16 maintainers, 246 real issues, randomized AI allowed/disallowed (mostly Cursor with Claude 3.5/3.7). Forecast +24%, believed +20% afterward, measured 19% SLOWER. Grade A. Single-agent chat/autocomplete setting; authors disclaim wider generalization. Bears on (c) and (d): self-reported speed gains unreliable; humans misjudged their own oversight burden in real time.
- Peng, Kalliamvakou, Cihon, Demirer (Microsoft Research), "The Impact of AI on Developer Productivity: Evidence from GitHub Copilot", Feb 2023. 70 programmers, HTTP server in JS, treatment 55.8% faster. Grade A. Narrow single-file single-session task.
- A larger field RCT (Microsoft, Accenture, a Fortune 100 firm; >4,000 developers; +26% concentrated among junior/short-tenure): primary not fetched. UNVERIFIED.
- DORA, "State of AI-assisted Software Development 2025", dora.dev/dora-report-2025. Survey-based. 90% daily AI use; epics per developer +66.2%; AI adoption correlated with HIGHER delivery instability. Grade B. Central claim: AI amplifies existing strengths and weaknesses.
- DORA, Accelerate State of DevOps 2024. >1/3 report moderate-to-extreme gains; 75.9% daily use. Grade B.
- GitClear, AI Assistant Code Quality Research (Jan 2026 edition; earlier Feb 2025), gitclear.com/ai_assistant_code_quality_2025_research. 211M changed lines 2020-2024: copy-pasted code 8.3% -> 12.3% of changed lines; refactoring ~25% -> <10%; copy-pasted lines exceeded moved lines for the first time. Grade B. AI attribution is inferred, not demonstrated.
- Uplevel Data Labs, "AI for Developer Productivity", ~800 developers telemetry: no significant cycle-time/throughput change from Copilot; 41% more bugs among Copilot users. Grade B.
- Faros AI telemetry (~22,000 developers, >4,000 teams), "AI productivity paradox": individual output up, company-level productivity flat. Derived figure (PR review time +91%, PR size +154%, DORA metrics flat) UNVERIFIED on the page fetched; shape corroborated by DORA 2025.
- MetaGPT (ICLR 2024): role-based multi-agent framework, 100% completion on its own benchmark, beat GPT-Engineer. Grade B; self-selected self-scored benchmark.
- ChatDev: outperformed MetaGPT and GPT-Engineer on its quality metric. Grade B, same caveat.
- SWE-agent / OpenHands (arxiv 2407.16741) / SWE-EVO (arxiv 2512.18470v5): capability benchmarks, not workflow evidence. GPT-5 under OpenHands: 21-25% on SWE-EVO's 48 long-horizon tasks (avg 21 files, 874 validating tests) vs 65-73% on SWE-bench Verified. Grade B for the measurement. Bears on (b): short-task success is a poor predictor of long-horizon multi-file reliability.
- Sakana AI, "The AI Scientist-v2" (arxiv 2504.08066, 2025-04-10): fully autonomous pipeline; 1 of 3 workshop submissions scored above the acceptance threshold. Grade B. The agent reports the paper was later withdrawn for ethical/procedural reasons (auditor: not independently verified).

## 2. Vendor and practitioner documentation

- Anthropic, "Building Effective Agents" (Schluntz, Zhang, 2024-12-19). Workflows vs agents; start with the simplest composition. Grade C. The project's worktree/mailbox architecture sits at the workflow end.
- Anthropic, "How we built our multi-agent research system" (2025-06-13). Orchestrator + 3-5 parallel subagents beat single Opus 4 by 90.2% on an internal research eval at ~15x token cost; documented failure modes fixed by prompting: excessive spawning, duplicated work under vague instructions, over-investment in simple queries. Grade B.
- Cognition, "Don't Build Multi-Agents" (Walden Yan, 2025-06-12). Parallel subagents without shared context make incompatible decisions (Flappy Bird example); recommends single-threaded agents with context compression. Grade D. Later partially revised by its author: multi-agent works when a single main loop carries state and subagents are stateless narrow workers.
- Anthropic, Claude Code best practices (code.claude.com/docs/en/best-practices): worktrees, subagents, headless mode, hooks. Grade C.
- Anthropic Research, "How Claude Code is used in practice" (2026-06-16, anthropic.com/research/claude-code-expertise): ~400,000 interactive sessions Oct 2025-Apr 2026; users make ~70% of planning decisions, Claude ~80% of implementation decisions; ~4 exchanges and ~10 actions per prompt per session. Grade B. EXCLUDES non-interactive/headless/autonomous usage, called "substantial" but unmeasured. The "80% of Anthropic's internal code / 8x productivity" claim appeared only on aggregators: UNVERIFIED.
- OpenAI Codex cloud agents, Google Jules, Cursor background agents: clone, plan, implement, test, open a PR; "weeks of work in days" claims unmeasured. Grade C.
- Factory.ai Code Droid: "four-month migration in 3.5 days". Grade D.
- Amazon Kiro, GitHub Spec Kit: spec-driven development (spec -> design -> tasks -> implementation). Grade C.
- Ralph Wiggum loop (Geoffrey Huntley, 2025; documented by codecentric.de, Thoughtworks): same prompt, fresh context each iteration, reads its own git history and a persisted plan file, until a stop condition. Grade C. Structurally close to this project's handoff-before-compaction pattern; convergent practice, unmeasured.
- Steve Yegge, Gas Town and Beads (2025-26; Pragmatic Engineer coverage): 20-30 parallel agents under one human with an agent-oriented issue-tracking memory layer. Grade D.
- Simon Willison, "Embracing the parallel coding agent lifestyle" (2025-10-05): up to four agents in separate checkouts; human review speed is the binding constraint; cognitive exhaustion by late morning. Grade D but directly on (d).
- Harper Reed, "My LLM codegen workflow atm" (2025-02-16): brainstorm -> spec -> stepwise prompt plan -> execute. Grade D.
- Thoughtworks Technology Radar vol. 34 (2026): "cognitive debt" from AI code volume; return to fundamentals. Grade C.

## 3. Process literature

- DORA four keys (Forsgren, Humble, Kim; dora.dev/insights/dora-metrics-history): deployment frequency, lead time, change failure rate, time to restore; >39,000 professionals over a decade; elite performers get both speed and stability. Grade B.
- Google engineering practices, code review (google.github.io/eng-practices): small focused changes; one-business-day maximum review latency. Grade C. Not obvious the norm survives agent output arriving many times faster than a human author's.
- Verification debt / review bottleneck: Sonar "AI Code Verification Debt", plus MetaCTO, Signadot, Codacy. Sonar cites He et al. 2025 (CMU): 3-5x velocity spike in month one reversing within three months, issues +30%, complexity +41%. He et al. not fetched: UNVERIFIED. Grade C for Sonar, D for the marketing pieces; the qualitative convergence with GitClear, Uplevel, DORA 2025 stands without it.
- AutoGen limitations (secondary commentary): no formal coordination guarantees, no principled conflict detection between subagent plans. Grade C.

## Finding i: grade A and B table

| Source | Grade | Core finding |
|---|---|---|
| METR RCT 2025 | A | 19% slower with AI despite believing 20% faster |
| Microsoft Copilot RCT 2023 | A | 55.8% faster on a narrow single-file task |
| DORA 2025 | B | individual throughput +66.2%, delivery instability up |
| DORA 2024 | B | >1/3 report moderate-to-extreme gains |
| GitClear 2020-24 | B | copy-paste 8.3% -> 12.3%; refactoring ~25% -> <10% |
| Uplevel ~800 devs | B | no efficiency gain; 41% more bugs |
| Anthropic multi-agent research | B | +90.2% over single agent at ~15x tokens |
| Anthropic Claude Code usage 2026 | B | humans ~70% of planning, agents ~80% of execution, interactive only |
| MetaGPT / ChatDev | B | role-structured multi-agent beats single agent on self-defined benchmarks |
| SWE-bench vs SWE-EVO | B | 65-73% short tasks -> 21-25% long-horizon |
| DORA four keys | B | speed and stability are not traded off by elite performers |
| AI Scientist-v2 | B | 1 of 3 accepted, later withdrawn |

## Finding ii: practices corroborated by multiple independent B-or-better sources

1. AI raises individual/component throughput without a matching gain, sometimes with a loss, in quality or organization-level stability (DORA 2025, GitClear, Uplevel; three methodologies).
2. Self-reported and benchmark-reported speed/capability are unreliable predictors of measured outcomes (METR; SWE-bench-to-SWE-EVO gap).
3. Parallel agents introduce a distinct failure surface (duplicated work, conflicting decisions, excess spawning) that must be engineered against (Anthropic from the inside; Cognition and AutoGen from outside, lower grade).
No B-or-better source validates this project's specific combination (worktree-isolated concurrent lines, markdown mailboxes, adversarial fresh-context audits) as a system; only components in isolation.

## Finding iii: clearest documented failure modes

1. Perception gap: pilots overestimate AI help in real time (METR, grade A).
2. Quality erosion under volume (GitClear, Uplevel, DORA 2025).
3. Review bottleneck: verification capacity did not scale with generation capacity in any source (Faros pattern, unverified percentages; Willison testimony at four agents).
4. Multi-agent coordination collapse absent deliberate design (Anthropic, Cognition, AutoGen).
5. Long-horizon capability collapse (SWE-EVO): session-level success over-predicts roadmap-level reliability.
6. Passing a nominal gate is not passing scrutiny (AI Scientist v2): argues for adversarial audit; no source found a gate that catches everything first pass.

## Finding iv: what the field has not established

- No study evaluates this project's architecture as an integrated system. Every A/B source studies a narrower slice; Anthropic's largest sample excludes autonomous usage.
- No source quantifies the minimum sustainable human check-in cadence for a pilot supervising multiple autonomous lines.
- No source measures whether adversarial audit by a fresh LLM context catches defects at a rate comparable to human review.
- No source establishes whether DORA's four keys keep their meaning when the delivering unit is one human plus several concurrent agents.
- No source reports rework or revert rates for autonomous multi-agent branches merged after an automated gate, which is exactly the measure needed to know rather than assume the gate is sufficient.

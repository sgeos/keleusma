# Evidence 7: context and state management for long-running, multi-session agents (Sonnet research agent, 2026-09-08)

Grades: A controlled or peer-reviewed; B documented practice with outcomes; C documented practice without outcomes; D opinion. Fetched directly unless marked.

## Anthropic guidance
- "Effective context engineering for AI agents", Anthropic Applied AI, 2025-09-29, anthropic.com/engineering/effective-context-engineering-for-ai-agents. Context is finite with diminishing returns; three techniques for long horizons: compaction into a fresh context, structured note-taking outside the window, sub-agents returning 1,000-2,000-token condensed summaries. Grade C. Finding 1.
- "Effective harnesses for long-running agents", Anthropic, 2025-11-26, anthropic.com/engineering/effective-harnesses-for-long-running-agents. Two-agent harness: an initializer sets up a progress file and feature list; a coding agent reads the progress file, checks git history, runs baseline tests at the start of every session. Durable state lives in a small file plus git history. Grade C. Finding 1.
- "Managing context on the Claude Developer Platform", 2025-09-29, claude.com/blog/context-management. Server-side context editing plus a memory tool; internal agentic-search eval: context editing +29%, editing + memory +39%; 100-turn eval: 84% fewer tokens consumed. Grade B (internal benchmark). Finding 1.
- Multi-agent research system (secondary): isolate-and-summarize beat accumulate-and-forward by 90.2%. Grade B.

## Claude Code memory mechanics
- "How Claude remembers your project", code.claude.com/docs/en/memory (fetched 2026-09-08). CLAUDE.md is human-authored, loaded whole, target UNDER 200 LINES, hard skip at 4 MiB. Auto-memory: a MEMORY.md index plus topic files; ONLY THE FIRST 200 LINES OR 25 KB of the index loads; beyond that is silently dropped; Claude Code warns and eventually errors when a write nears the cap so the index is rewritten rather than grown. Grade C. A shipping system that solved unbounded growth for one file by an enforced cap and forced rewrite. Finding 1.
- "Orchestrate teams of Claude Code sessions", code.claude.com/docs/en/agent-teams (fetched). Agents communicate through a JSON mailbox file per agent, a shared claimable task list, and direct messaging; the receiving agent is TOLD a message came from another Claude session, not the human, so a relayed approval claim is untrusted input. Grade C. Finding 5: a shipping system that refuses to let a peer's relay stand in for a human ruling.
- Claude Code Routines (April 2026, secondary, UNVERIFIED): each run is a fresh isolated cloud session; state must live in the repository.

## Other conventions
- AGENTS.md, Agentic AI Foundation under the Linux Foundation, agents.md (fetched). Plain markdown README for agents; nested files, closest wins; OpenAI's repo carried 88 nested files. Grade C. Decomposition into many small scoped files rather than one large one.
- Cursor Rules, cursor.com/docs/rules (fetched). Versioned .mdc rules scoped always / by description / by glob / manual; recommends UNDER 500 LINES per rule, split larger ones. Memories (Cursor 1.0, June 2025): background model proposes facts, developer approves each. Grade C.
- Manus, "Context Engineering for AI Agents", Yichao Ji, 2025-07-18, manus.im/blog. File system as persistent memory; "recitation": the agent continuously REWRITES a small todo.md in place and re-reads it, pushing the plan into recent attention; keep failed actions visible. Grade C. Finding 1: current state rewritten in place, not appended.
- Ralph Wiggum loop, Geoffrey Huntley, 2025, ghuntley.com/ralph. Shell loop re-invokes the same prompt file against a fresh context each iteration; progress in progress.txt and git commits. Grade D.

## Long-context degradation
- Liu et al., "Lost in the Middle", TACL 12, 2024, aclanthology.org/2024.tacl-1.9. U-shaped retrieval: performance highest at start and end of input, degrades in the middle, even for long-context models. Grade A.
- Hong, Troynikov, Huber (Chroma), "Context Rot", 2025-07-14, trychroma.com/research/context-rot. 18 frontier models; accuracy degrades non-uniformly well before stated limits, sometimes by 30-50%; lower question-answer similarity accelerates decline. Grade A (vendor, public toolkit). Together with Lost in the Middle: reading 93 KB + 434 KB + 1 MB at startup degrades reliability even below the window.

## Decision records, single writer, multi-agent state
- Cognition, "Don't Build Multi-Agents" (secondary here). Fragment relay produces conflicting unverifiable decisions. Grade C. Finding 5.
- Geng et al. (CMU), "Effective Strategies for Asynchronous Software Engineering Agents", 2026, arxiv.org/html/2603.21489. CAID (Centralized Asynchronous Isolated Delegation): a central manager builds a dependency graph, delegates to agents in isolated git worktrees, integrates by test-gated merges; +25.6 points on PaperBench, +14.7 on Commit0 over single-agent and looser multi-agent baselines. Grade A. Supports worktree branch-and-merge; a single manager is a de facto single place to record a ruling.
- MCP Agent Mail, mcpagentmail.com. Persistent identities, threaded git-backed messaging, glob-scoped advisory file reservations with expiry, searchable archive; ~49 req/s across 40-50 agents; 9.1x fewer git writes by coalescing; explicitly does NOT centralize human arbitration. Grade C. Finding 5: makes the first answer searchable, does not force it to be given once.
- "Architecture Decision Records for AI Coding Agents", braingrid.ai blog. Five fields (title, status, context, decision, consequences), one page, versioned, immutable once accepted, superseded not edited. Grade D. Finding 4/5.

## Bounded-state practice, a measured case
- Zhang and Song, "Written by AI, Managed by AI: Semantic Space Control and Index Sickness Elimination Across 391 Consecutive Sessions", arXiv 2606.19121, 2026-06-17. "Index sickness": adding structure, identifiers, rules, and context to a long-running AI-managed documentation set makes the model reason self-referentially inside the symbolic layer ("phantom legislation"). Fix: physical separation of a compact BASELINE document from an APPEND-ONLY LOG; reduced AI-instruction volume ~75% across 391 sessions; no recurrence over the next 150. Grade B (single self-reported case study). Best-matched source for Findings 1 and 4 together.

## Mapping
| Finding | Technique | Grade | Cost / failure mode |
|---|---|---|---|
| 1 | baseline + log physical separation (391-session study) | B | single case; discipline to keep the split |
| 1 | enforced size cap with rewrite-on-overflow (Claude Code MEMORY.md 200 lines / 25 KB; Cursor 500 lines) | C | product-specific; must be reimplemented as a script or test; one capped file does not stop sprawl across uncapped ones |
| 1 | context rot / lost-in-the-middle as rationale | A | says why, not how big |
| 4 | baseline + log applied to a lessons ledger; one-page immutable ADR entries superseded not edited | B, D | depends on distilling, not appending |
| 5 | treat inter-agent messages as untrusted relay (Claude Code agent teams) | C | prevents fabricated approval; does not create the durable first artifact |
| 5 | single-writer / full-trace (Cognition); CAID central manager | C, A | single writer does not scale to multi-day async sessions; CAID measured on coding benchmarks not ruling consistency |
| 5 | git-backed searchable archive (Agent Mail) | C | discoverable after the fact; no single answer forced |

Not found: any study relating handoff size to error rate; a cross-tool hard cap on top-level instruction files (only Claude Code's auto-memory index is enforced); any published "verification traps" ledger pattern by that or a similar name; replication of the 391-session study; any measurement of a human answering the same relayed question differently across sessions.

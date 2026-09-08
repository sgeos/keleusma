# Evidence 9: human-in-the-loop decision workflows, progress measurement, and CI cost control (Sonnet research agent, 2026-09-08)

Grades: A controlled or peer-reviewed; B documented practice with outcomes; C documented practice without outcomes; D opinion.

## (a) Decision workflows
- Claude Code permissions, code.claude.com/docs/en/permissions. Modes default / acceptEdits / plan / bypassPermissions / auto (a classifier reviews actions); no notion of a decision timing out or defaulting. Grade C. Finding 5.
- Claude Code hooks (33 events per Anthropic's reference dated 2026-09-06, via claudefa.st). A PreToolUse deny blocks even under bypass; allow cannot loosen a deny. Grade C. Hard rules like "never publish without confirmation" belong in hooks; a default-on-timeout policy would have to be built there.
- OpenAI Codex approvals, learn.chatgpt.com/docs/agent-approvals-security. Policies on-request / untrusted / never plus per-category overrides, layered on sandbox modes. Grade C.
- LangChain, "Making it easier to build human-in-the-loop agents with interrupt", 2025. interrupt() pauses a graph, persists state via a checkpointer, resumes at the same node; pre-interrupt code re-runs so side effects must be idempotent. Grade C. Finding 5: questions could surface mid-session rather than at handoffs, given resumable checkpoints Claude Code sessions do not expose.
- "Design Patterns for Approval Processes" (ACM 2023) and "Even More Design Patterns for Approval Processes" (Springer 2026): ~14 named patterns incl. delegated and default decisions. Fetch blocked; UNVERIFIED beyond abstract. Grade C.
- "The Two-Person Rule for AI Agents", Medium, April 2026. Separate decide and execute for irreversible actions; policy-as-code as the second person. Grade D. Matches the project's existing no-default stance.
- Focused Labs, "Approval Queues Are the Runtime for Agentic AI Workflows", 2026. Queue item fields: action, arguments, risk reason, owner, allowed decisions, checkpoint pointer, trace link, TIMEOUT, ESCALATION PATH, approval receipt. Grade C. Supplies the missing schema for medium reversible decisions.
- Pan et al., "Measuring Agents in Production", arXiv 2512.04123, Dec 2025 (submitted ICLR 2026). 20 interviews + 86 practitioners, 26 domains: 68% of production agents take ten or fewer steps before human intervention; static workflows preferred over open-ended autonomy for reliability. Grade B. Frequent short human checkpoints are the norm; "the operator is the long pole" is a known trade-off, not an anomaly.
- ADRs, adr.github.io, MADR 4.0 (2024): decision drivers, considered options with pros/cons, outcome. Grade C. The standard container for "options with a recommended default". UK Government Digital Service adopted an ADR framework across the public sector, 2025-12-08 (technology.blog.gov.uk). Grade C.

## (b) Measuring progress and quality
- DORA 2025 and its AI Capabilities Model (dora.dev/dora-report-2025; dora.dev/research/2025/ai-capabilities-model). ~5,000 professionals, 100+ interview hours; "a mirror and a multiplier"; seven capabilities reported by secondary summaries (clear AI stance, healthy AI-accessible data, strong version control, SMALL BATCH SIZES, user-centric focus, quality internal platforms, ...), list UNVERIFIED against the PDF (services.google.com/fh/files/misc/2025_dora_ai_capabilities_model.pdf). Grade B. Findings 6, 7: batch size and platform quality predict whether AI helps.
- DX Core 4 (Abi Noda, getdx.com): speed, effectiveness, quality, impact; Booking.com case, 3,500 engineers: AI adoption +65%, throughput +31%, +16% productivity attributed to AI. Grade B (vendor-reported). Finding 7: separates output volume from quality and impact.
- Forsgren et al., "The SPACE of Developer Productivity", ACM Queue / CACM, 2021-03-06. Five dimensions; a single metric cannot capture productivity. Grade A. Merge and test counts are pure Activity metrics.
- GitClear 2025/2026 (see evidence-8): two-week churn 3.3% pre-AI -> 7.1% in 2025. Grade B. Baseline for a churn/rework metric.
- Larridin, "AI Code Quality: 0.2% vs 15.16% Revert Rates", 2026: one customer's repo; 30-day revert 0.2% AI-assisted vs 15.16% human; 90-day 0.94% vs 27.8%. Grade C; adopt the metric category, not the numbers.
- "Pre-registration for Predictive Modeling", arXiv 2311.18807, 2023. Declaring an analysis plan before running it. Grade A. Finding 7: the V0.3.X predicted-vs-measured practice is pre-registration; could extend from a number to a process.
- Exit criteria / Definition of Done (Wikipedia; rexblack.com). Explicit agreed conditions before a release is complete. Grade D (decades-old consensus). Finding 7: the missing lifting criterion.
- ShiftMag summarizing an Index.dev study, 2025: PRs per engineer +98% with AI; controlling for PR size, AI has negligible independent effect on defect rate, size drives it. Grade C, UNVERIFIED primary. Findings 6, 7.

## (c) CI cost control
- dorny/paths-filter (GitHub Marketplace, v3). Job- and step-level path filter that outputs which filters matched; downstream jobs skip via `if` while the workflow, and its required check, still completes. Grade C. Finding 6: the documented fix for the stuck-pending-check failure.
- GitHub docs, "Troubleshooting required status checks" and "Skipping workflow runs". A required check whose workflow is skipped by paths-ignore stays PENDING FOREVER; a job skipped by an `if` condition reports success and satisfies the requirement. Grade C. Naive paths-ignore on the slow job would strand every docs-only PR.
- cargo-nextest partitioning, nexte.st/docs/ci-features/partitioning. Slice-based (even, may reassign as the suite changes) or hash-based (stable, less even) sharding of a test binary across CI shards. Grade C. Finding 6: drop-in mechanism to split the 60-minute job.
- Depot.dev, "Fast Rust Builds with sccache and GitHub Actions", 2025-26. sccache starts immediately and fetches only what changed, BUT cites an independent benchmark finding no configuration where sccache beat a cold baseline except after clearing its own cache. Grade C, with a B-grade caution. Measure before trusting.
- Mergify, CI bill posts, 2025-26. Merge-queue batching cuts queue CI minutes 60-80% in month one for high-traffic repos; two-tier lightweight-then-full CI plus test selection 50-70%; ten queued PRs batched: five hours -> thirty minutes. Grade C (vendor). Upper bound.
- GitHub Actions timeout (multiple practitioner sources, e.g. leimao.github.io). No timeout-minutes means the 360-minute platform default. Grade D on a documented fact. One-line fix.

## Mapping
| Finding | Technique | Grade | Cost / failure mode |
|---|---|---|---|
| 5 | approval-queue schema with timeout and default field + ADR-style packet with recommended default | C | default-approve trades speed for risk; default-deny stalls; the timeout is itself a decision |
| 5 | interrupt-and-resume checkpointing | C | needs idempotent pre-interrupt logic and a checkpoint store Claude Code does not expose |
| 6 | dorny/paths-filter at job level + nextest partitioning of the slow job | C | must pair with the required-check semantics or it reintroduces stuck-pending |
| 6 | explicit timeout-minutes at job and step | D | near zero cost; too tight cancels healthy slow runs |
| 7 | DX Core 4 dimensions; pre-registration applied to predicted-vs-measured | B, A | needs instrumentation; predictions must be recorded before and audited after |
| 7 | written exit criteria / Definition of Done in ADR form | D | no technical cost; someone must commit to a threshold in writing, a Finding-5-shaped problem |

Not found: an independent audited before/after of GitHub Actions cost reduction; the DORA capabilities PDF text; a multi-org peer-reviewed AI revert-rate study; any literature using "roadmap honesty"; a worked two-person-rule inside a coding-agent merge pipeline; independent nextest partitioning wall-clock data; a GitHub-native fix for required-check vs path-filter; default-on-timeout language adapted to engineering decision logs.

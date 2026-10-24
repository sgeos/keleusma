# Kaizen audit working notes (unversioned; tmp/ is gitignored)

## Observations recorded during evidence mining, 2026-09-08

- EXECUTED 2026-09-08 ~01:30 PT: six parallel Fable-model mining subagents were all terminated by an API session limit (HTTP 429, 'session limit, resets 2:30am America/Los_Angeles') before reporting. Relaunched on Sonnet. Calendar cost: one interruption plus a full relaunch; no partial reports survived, only raw exports. Bears on the calendar-time and autonomy objectives: session limits are a real constraint the process documents do not mention.

- EXECUTED sizes of process documents a resuming session is told to read (bytes / lines):
  - v0.2.3 docs/process/DESIGN_JOURNAL.md: 1056884 bytes, 11096 lines
  - v0.2.3 docs/process/TASKLOG.md: 434064 bytes, 2165 lines
  - v0.2.3 docs/process/HANDOFF.md: 93081 bytes, 1439 lines
  - v0.2.3 docs/process/REVERSE_PROMPT.md: 19096 bytes, 307 lines
  - v0.2.3 docs/process/handoffs/v0.2.3.md: 160773 bytes, 2504 lines
  - v0.3.0 export v030.md: 524326 bytes, 8032 lines
  - v0.3.0 export dj030.md: 1315214 bytes, 15024 lines
  - v0.3.0 export tl030.md: 510599 bytes, 3084 lines
  - v0.3.0 export rp030.md: 37165 bytes, 609 lines

## Draft status, 2026-09-08 ~03:00 PT

- process-audit-draft.md, DRAFT 1, ~8000 words, blog format (front matter, bold-lede paragraphs, Source Base / Epistemic State / Out of Scope / Conclusion / References). Style-scanned clean on prose lines. Article number placeholder. Not reviewed by a second context.
- evidence-1..6 saved; raw/ holds the metrics exports. Two miner figures NOT independently checked by the auditor: the blog post word count (459,117) and the incident classification totals.
- Next: second-context review of the draft (a fresh session should attack findings 2, 4, 5 and the trade-off inference); then operator reads; then integrate proposals as decisions/ documents on a feature branch off kaizen.

## Review and revision, 2026-09-08 ~03:40 PT

- review-1.md: fresh-context adversarial review of DRAFT 1, 20 findings (4 blocking, 9 major, 7 minor), all dispositioned FIXED in DRAFT 2 (process-audit-draft.md). DRAFT 1 kept as process-audit-draft.v1.md.
- EXECUTED by the auditor during revision: incident classes recounted over the 43 tabulated rows (tooling 9, stale 8, checker 6, gate 6, overconfidence 6, relayed 5, spec 3); net numstat by bucket since 2026-07-10 on v0.2.3 (docs/process +33,643/-15,506 net 18,137; src Rust +20,899 net 17,669; src/selfhost/kel +8,508 net 7,436; compiler/ +25,968 net 16,964; tests +65,445 net 54,539).
- EXECUTED earlier, now recorded: .github/workflows/ci.yml has no timeout-minutes (grep), so the four ~360-minute cancelled runs match GitHub's default job limit; main-branch failures all between 2026-05-08 and 2026-07-11, last main run 2026-07-24 (from raw/ci_runs_full.jsonl).
- Source for "routine feature merges proceed without asking": the auditor's session memory file routine-feature-merges-authorized.md (2026-08-20), not a project document. The draft labels the no-review claim as an inference.
- Timing: first (Fable) mining pass launched ~01:00 PT, terminated by session limit ~01:30; Sonnet relaunch ~01:35, last report ~02:47; draft 1 ~03:05; review ~03:25; draft 2 ~03:40.

## Research pass two and DRAFT 3, 2026-09-08 ~04:40 PT

- Four Sonnet research agents (context/state; verification; decisions/metrics/CI; coordination + source verification) reported; saved as evidence-7..10. All ran in parallel without hitting the session limit.
- Five primaries verified first hand by the agent: Faros 2025 (91%/154%, 10k devs) and the distinct 2026 "Acceleration Whiplash"; He et al. CMU (arXiv 2511.04427: +281% month 1, warnings +29.7%, complexity +40.7%); Anthropic claude-code-expertise (2026-06-16, 70/20 split); Sakana withdrawal was BY PROTOCOL, not misconduct (evidence-6 wording corrected in the draft); Cui et al. SSRN 4945566 reached only via abstract summaries (+26.08%, N=4,867).
- DRAFT 3 = DRAFT 2 + "The State of the Art, by Finding" section (eight paragraphs, one per finding, with grades), a verified-primaries paragraph in the literature section, proposals 1,3,4,5,6,7,9,10 amended with technique evidence, Epistemic State and Source Base updated, ~40 references added. DRAFT 2 kept as process-audit-draft.v2.md. Prose style scan clean. Not yet reviewed by a second context (review-1 covered DRAFT 1 only).
- Strongest new results: Cross-Context Review (arXiv 2603.12123, grade A: isolated review F1 28.6 vs 24.6, p=0.008; second same-session pass no help); LLM-as-judge consistency-bias paradox (arXiv 2606.19544); PBT-Bench (agent properties catch 30-40% vs 70-80% human); 391-session index-sickness case study (arXiv 2606.19121); Claude Code auto-memory 200-line/25 KB enforced cap; agent teams treat peer messages as untrusted; GitHub required-check pending-forever semantics for workflow-level path filters; CAID (arXiv 2603.21489) worktree+manager +25.6 pts; coordinator null result (arXiv 2608.16801); shared task state 78%->0% duplicate work (arXiv 2606.19616, agents real or simulated unknown).

## Equation-density pass, DRAFT 4, 2026-09-08 ~05:10 PT

- Operator prompt: "Please review for equation density, and add all candidate equations." (the blog's pass two). DRAFT 3 kept as process-audit-draft.v3.md.
- 18 display equations added in the blog's convention (own $$ lines, lead-in sentence before, numeric consequence after); front matter mathjax: true. Coverage: startup token estimate (4 bytes/token heuristic, stated as heuristic); incident class shares; mutation sweep projection and null-control fraction; reactive-rule mechanism share; operator cost per decision under a packet; run time = slowest job + overhead (8 min); test-job growth rate and 90-minute extrapolation (five points, stated); sharding estimate; docs-PR runner hours (67 h) and stuck-run hours (24 h); gross/net prose-to-source ratios; the parallel-development queueing identity with f_c = 0.14 -> 1.16; historical gate waste (40 h); boundary/green/recovery fractions; F1 and Cohen's kappa definitions; message pairs n(n-1)/2 and goodput ratio 3.4; sign vector for regime one; default-admissibility inequality for regime two; absorption rate 54/30 days = 1.8 per day (EXECUTED: v0.3.0 cut 2026-08-08, absorption 54 on 2026-09-07; 76 merge commits mention absorption, i.e. some absorptions span several merges).
- Verifier-style checks run locally on the draft: no pipes inside math, no $$ sharing a line with prose, even delimiter count, no bare double subscripts, prose style scan clean. The blog's own _verify.py was NOT run (it would require placing the draft in the blog repository, which this line does not write to).
- Not yet reviewed by a second context since DRAFT 1.

## Reference-density pass, DRAFT 5, 2026-09-08 ~05:40 PT

- Operator prompt: "Please review for reference density, specifically primary references, and add all identified references." (the blog's pass three). DRAFT 4 kept as process-audit-draft.v4.md.
- EXECUTED: sgeos/keleusma is PUBLIC; sgeos/sgeos.github.io (the blog) is PUBLIC; a permalink at 802e72d3 resolves via the GitHub API. So the primary references are commit-pinned permalinks to the project's own documents.
- 108 anchors, all cited inline in the blog's `[[Label][anchor]]` form and all defined under ## References in three blocks with a visible bulleted list each and alphabetized definitions: Primary Documents (34, `primary_*`, permalinks at 802e72d3 / b8d78c3f / 6537a36f / 7f87d03f, the PR and workflow-run indexes, and four blog repo files at 68182f3), Reference (44, `ref_*`, vendor docs, reports, practitioner posts, and Wikipedia for definitions), Research (30, `research_*`, arXiv/DOI/SSRN).
- Label colons replaced by commas (prose rule); the one contraction is the verbatim title "Don't Build Multi-Agents" (inline and in the bullet list), an exemptible verifier warning, left verbatim.
- Not cited: sources in the evidence files that the draft's prose does not use (e.g. Google eng-practices, Thoughtworks radar, LangGraph interrupt, MCP Agent Mail, AGENTS.md, Claude Code permissions, fuzzing-with-agents, LLM mutation study). They remain in evidence-6..10.
- Checks: inline == defined (108/108), no unused, no undefined, sorted per block, no duplicates; prose style scan clean apart from the title contraction; equations untouched.
- Still not reviewed by a second context since DRAFT 1.

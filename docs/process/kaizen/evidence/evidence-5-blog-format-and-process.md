# Evidence 5: the blog repository, format and process (miner report, Sonnet, 2026-09-08)

Repository ~/projects/blog (Jekyll, master). Read-only. EXECUTED: cat, sed, grep, wc, ls, git log. READ: the scripts, _verify.py, the newest X-planes draft, draft_summary.md, the newest post, CLAUDE.md, STYLE_GUIDE.md, template.markdown, _docs/process/{HANDOFF,TASKLOG,REVERSE_PROMPT,CONTENT_WORKFLOW,COMMUNICATION,VERIFICATION_TRAPS,PROMPT}.md, tmp handoff prompts.

## Goal 1: deliverable format

Tooling: _new_draft.sh copies _drafts/template.markdown; _publish.sh does the git mv into _posts (BROKEN on macOS BSD sed, documented at _publish.sh:20-24; CONTENT_WORKFLOW.md:34 says use git mv directly); _preview.sh (drafts off by default); _check.sh runs exactly what CI runs: _verify.py, a production jekyll build, _lib/render.py on rendered HTML.

_verify.py (695 lines) checks: front matter needs layout, title, date (:249-250); date offset +0000 (:266-268); filename date == front-matter date (:259-262); first category not "keleusma" (:52-53); reference anchors defined, used, non-duplicate, sorted per block (:288-316); a References block with definitions but no visible bulleted list is an error (:322-330); MathJax/kramdown traps; prose scanned for em/en dashes (error) and contractions (warning, exemptible) (:499-507); WATCH_WORDS rate-checked at 5.0/1000 words above 400 words (:511-533); corpus-level one article per calendar day, one debug tag per article number, category-slug collisions (:600-604). Drafts run the whole battery downgraded to warnings (:606-627). _verify_exemptions.yml records documented false positives. _verify_citations.py (307 lines) is network-dependent, resolves every DOI against Crossref and matches title/author/year, motivated by a 2026-08-02 incident of 13 citations whose DOI resolved to an unrelated paper.

Newest draft: _drafts/x_planes_lockheed_martin_x55_acca.markdown, modified 2026-09-08 01:30, untracked, 59,700 words (matches TASKLOG.md:111 "59,698 words").

Front matter verbatim (lines 1-10):
```yaml
---
layout: post
mathjax: true
comments: true
title:  "X-Planes: Lockheed Martin X-55 ACCA"
date:   2025-11-30 09:00:00 +0000
categories: aerospace history engineering
series: x_planes
series_title: X-Planes
series_index: 56
---
```
then `<!-- A352 -->` and `<script>console.log("A352");</script>` (article-number debug tag, _verify.py:286).

Heading skeleton: unheaded bold-lede opening prose (no "Introduction"); "This is the Nth article in the series" backward-linking paragraph (:15); topic sections; "## What a Single Aeroplane Can and Cannot Demonstrate"; "## What Followed"; "## The Source Base" (with ### subsections incl. "Primary Documents"); "## The Contemporary Literature" (per-cluster ###); "## What the Data Changed"; "## Epistemic State" (fixed rhythm: what is documented / what is computed here and stated as such / what is uncertain / what could not be verified / what the corpus excludes, :475-483); "## Out of Scope"; "## Conclusion"; "## References" with "### Books" (OpenLibrary work links), "### Reference", "### Related Post" ({% post_url %} back-references only), "### Research" (DOI links). HANDOFF.md:2110-2113 says the genre carries three sections beyond a twelve-section standard, enforced by a separate check_any.py (not examined).

Citation convention: reference-style links `[label][anchor]` in prose; definitions under ## References in typed sub-lists, each followed by its own alphabetized `[anchor]: url` block.

Voice: no contractions (possessive 's exempt), no em/en dashes, no prose colons/semicolons, no prose parentheticals (math and Wikipedia disambiguators exempt), acronyms spelled out on first use, bold (never caps) for emphasis (HANDOFF.md:2105); most paragraphs open with a bold-lede sentence (observed house convention, not named as a rule). Sources: _verify.py:499-507, STYLE_GUIDE.md:15-19.

Most recent post: _posts/2026-08-10-when_error_correction_meets_a_signature.markdown (published 2026-08-13, commit 1cd62b4), identical front-matter shape, debug tag, closing-section order. The miner reports 459,117 words for it under a "no length limit, no reference limit" directive (HANDOFF.md:145-155); this figure is as reported by wc -w and was not independently checked by the auditor.

draft_summary.md (847 KB) is a per-draft status ledger, one ## per draft, ending in a salvage assessment and a "Candidate Future Post Topics" table (:9010-9038).

TEMPLATE:
```
---
layout: post
mathjax: true|false
comments: true
title:  "<Series>: <Subject>"
date:   YYYY-MM-DD HH:MM:SS +0000
categories: <first NOT "keleusma">
series: <slug>
series_title: <Human Title>
series_index: <N>
---
<!-- A<nnn> -->
<script>console.log("A<nnn>");</script>

<bold-lede opening paragraph>

## <topic sections>
## The Source Base
## The Contemporary Literature
## Epistemic State
## Out of Scope
## Conclusion
## References
### Books / ### Reference / ### Related Post / ### Research
```

## Goal 2: the blog's process as comparison

| File | Role | Size |
|---|---|---|
| _docs/process/HANDOFF.md | AI-to-AI snapshot, parent-commit stamped, self-validating (mirrors keleusma) | 2,370 lines / 175 KB |
| _docs/process/TASKLOG.md | Current Task + append-only History table | 591 lines / 1.04 MB |
| _docs/process/REVERSE_PROMPT.md | overwritten per task | 131 lines / 6.3 KB |
| _docs/process/PROMPT.md | human staging, read-only for AI | 58 lines |
| _docs/process/VERIFICATION_TRAPS.md | 21 catalogued method-level failures, cause + check + habit | 444 lines |
| COMMUNICATION, CONTENT_WORKFLOW, GIT_STRATEGY, CROSS_LINKED_SERIES, FORWARD_DATED_POSTS, PUBLICATION_REVIEW, STYLE_VERIFICATION, URL_VERIFICATION, PR_STRATEGY, RESEARCH_AGENT, SISTER_SESSION | supporting protocols | 105-206 lines each |

No DESIGN_JOURNAL equivalent; its function is split between VERIFICATION_TRAPS.md and the very verbose TASKLOG History rows.

Human required: every one of four per-article passes is a separate human prompt (draft, equation-density review, reference-density review, publication review; HANDOFF.md:130-142); "No article is mid-rhythm. Wait for the pilot's prompt and do not start A352 unprompted." (:42-43); pushing authorised only by the publication-review prompt (:2124-2126); publishing (git mv to _posts) has NEVER been authorised for the 56-article X-Planes series (:19-21, TASKLOG.md:9-10), once for the Keleusma research-spike series (TASKLOG.md:172); open decisions deferred to the pilot (deploy gate ~35 min growing to 3+ hours, tmp/handoff_msg.txt:26-31; a malformed citation label "the repair belongs to the pilot", TASKLOG.md:104-106); "Irreversible or outward-facing actions need confirmation." (:2123).

Publication record: TASKLOG.md History row, e.g. :172 (2026-08-13) "A373 PUBLISHED and PUSHED ... The publication interlock was verified before the move ... _verify.py 0 errors and 0 warnings across 301 posts, ./_check.sh clean at 465 pages"; corroborated by commit 1cd62b4. CONTENT_WORKFLOW.md:38-45 codifies a two-commit pattern (draft commit, publish commit).

Verification: _check.sh mirrors CI (asserted CLAUDE.md:36-58, not independently confirmed); _verify_citations.py excluded from the gate (network). No hooks examined.

Cadence (git log by day): sporadic single-digit days per month Feb-Jun 2026; dense from 2026-08-02 (16-64 commits/day); 2-14/day through 2026-09-07. Four-pass rhythm completes one article per 1-4 calendar days when a series is active (A346-A352 each 1-2 days). Series: X-Planes 72 planned, 56 drafted, 0 published; keleusma_research_spikes 5/5 published; SBIR/STTR 13/13 published; SpaceX 12.

Lessons recorded:
| File:line | Brief | Class |
|---|---|---|
| VERIFICATION_TRAPS.md:15-22 | "'0 errors, 0 warnings' was measured against a directory that no longer exists" (sticky cd) | checker reach / tooling |
| :29-40 | a checker rewritten three times; v2 "masked the first error" | checker reach |
| :250-268 | "The checker was right and the first reading called it noise" | model overconfidence |
| :303-320 | pgrep -f wait loop matches its own command line, "structurally unable to terminate", 22 leaked shells | tooling (same trap as keleusma 2026-08-09) |
| :328-345 | TASKLOG stated "forty-eight" then "forty-seven" on consecutive lines; a presence check "goes green precisely when a number goes stale" | stale doc |
| TASKLOG.md:147 | 2,295 citation labels silently truncated to four words | overconfidence corrected by measurement |
| CLAUDE.md:56-58; HANDOFF.md:2100-2103 | A342 "shipped a survey paragraph wrong in all six of its statistics past a verifier that confirmed the stale string" | checker reach |
| TASKLOG.md:172 | four already-live pages renumbered "Part N of 4" to "of 5", "authorised by the pilot" | coordination, human-gated |

Autonomy statements: HANDOFF validity protocol identical to keleusma's; the pilot quotes a standing directive verbatim on every publication-review prompt ending "do not yet publish it" with "No length limit and no reference limit are permissions, not instructions." (:145-155); tmp/post_compaction_handoff_prompt.md is a literal paste-after-compaction script naming five files incl. the agent's persistent memory file; TASKLOG.md:222 (2026-08-04) a handoff written ahead of a planned compaction with reconnaissance pre-done and two findings "recorded as pilot decisions rather than unilateral fixes".

Comparison note (miner's): near-identical machinery in both repos; the blog additionally enforces a four-pass human-gated rhythm per article and a standing separation between committed/pushed and published, with publication requiring explicit logged authorization even after a push.

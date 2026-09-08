# Evidence 8: verifying AI-generated code and the agents' own instruments (Sonnet research agent, 2026-09-08)

Grades: A controlled or peer-reviewed; B documented practice with outcomes; C documented practice without outcomes; D opinion.

## Self-correction
- Huang et al. (DeepMind, UIUC), "Large Language Models Cannot Self-Correct Reasoning Yet", arXiv 2310.01798, ICLR 2024. Self-revision without an external signal frequently degrades performance. Grade A. Finding 8: a session grading its own harness has no external signal.
- Lu, Teehan, Jin, Ren (NYU), "When Does Verification Pay Off? A Closer Look at LLMs as Solution Verifiers", arXiv 2512.02304, late 2025. 37 models, 9 benchmarks. Grade A design; numbers UNVERIFIED beyond abstract.
- 2025-26 error-localization preprints: models cannot reliably find their own errors unsupported but can fix one once its location is pointed out. Grade B-C.

## Fresh context and independent review
- "Cross-Context Review: Improving LLM Output Quality by Separating Production and Review Sessions", arXiv 2603.12123, 2026. 30 artifacts, 150 injected errors, four conditions, 360 reviews. Fully isolated cross-context review F1 28.6% vs same-session self-review 24.6% (p = 0.008); critical errors 40% vs 29%; a second same-session pass gave no improvement (p = 0.11), ruling out "reviewing twice". Single model (Claude Opus 4.6), synthetic errors, no human baseline. Grade A. The most direct evidence that context separation itself is the active ingredient. Finding 8.
- "LLMs as Code Review Agents: A Rapid Review and Experimental Evaluation with Human Expert Judges", Computational Collective Intelligence series, 2025-26. 23 articles reviewed plus an experiment against human judges. Grade B.
- "Measuring and Exploiting Contextual Bias in LLM-Assisted Security Code Review", arXiv 2603.18740, 2026. Framing a PR as a security improvement changes detection; metadata redaction partially mitigates. Grade A. Even an independent reviewer is steerable by framing.

## Review tools and catch rates
- Greptile, "AI Code Review Benchmarks 2025", greptile.com/benchmarks. 50 real bugs from bug-introducing/fixing commit pairs in Sentry, Cal.com, Grafana, Keycloak, Discourse; catch = line-level comment naming the fault. Greptile 82%, Cursor Bugbot 58%, GitHub Copilot 54%, CodeRabbit 44%, Graphite 6%; critical bugs: Greptile and Bugbot 58%. Grade B (vendor-run, transparent). Range 44-82%.
- Martian Code Review Bench via CodeRabbit blog, 2026: ~300,000 real PRs over two months plus a 50-PR gold set; CodeRabbit F1 51.2%, precision 49.2%, recall 53.5%, best of ten tools. Grade B; other tools' numbers not obtained (UNVERIFIED cross-tool). Precision near 50% means half of flags are not acted on.
- Cursor, "Building a Better Bugbot", cursor.com/blog/building-bugbot, through early 2026. 40 experiments Jul 2025-Jan 2026; bugs flagged per review 0.4 -> 0.7; resolution rate (flagged bugs fixed by merge) 52% -> >70%, measured by an LLM classifier spot-checked with authors; >2M PRs/month. Grade B. A team measuring whether its instrument's verdicts are meaningful, the discipline the audited harness lacked. Findings 3, 8.
- GitHub Copilot code review: independent blog eval 16 of 23 seeded bugs, 4 false positives (Grade C); arXiv 2509.13650 security study: detection drops sharply for privilege escalation, architectural, business-logic flaws, and when a PR is framed as a security fix (Grade A). Automated review catches shallow pattern-matchable bugs and misses cross-file reasoning.
- CR-Bench, arXiv 2603.11078, 2026: SWE-bench issue/PR pairs converted into a review task; strong models leave a substantial detection gap; per-tool numbers UNVERIFIED. Grade A.
- Anthropic, "Code Review for Claude Code", claude.com/blog/code-review and code.claude.com/docs, research preview 2026. Several specialized agents review a diff in parallel with a VERIFICATION STEP that checks candidate findings against actual code behavior before posting; internally under 1% of findings marked incorrect; substantive comments on 54% of internal PRs vs 16% before. Grade B (first-party). The vendor's product for exactly Finding 8; its verify-before-surfacing step is a "must-fire against reality" discipline.

## Mutation testing
- cargo-mutants docs, mutants.rs. Outcomes: caught, missed, unviable (did not compile), timeout; only missed and timeout surfaced. Exit status counts as detection ONLY after the mutant compiled and ran. Grade C. Finding 3: the audited harness skipped exactly this check; the standard tool guards against it.
- "A Comprehensive Study on LLMs for Mutation Testing", arXiv 2406.09843, 2024. LLM mutants score >0.7 vs ~0.5 for PIT/Major. Grade A. Says nothing about harness trustworthiness.
- "Do Coverage and Mutation Scores of LLM-Generated Test Suites Correlate with Their Effectiveness? A Replicability Study", arXiv 2607.22880, 2026. For LLM-written suites, size correlates only weakly with mutation score; mutation score is a weaker proxy for LLM-written tests. Grade A. Finding 3: even a correct scorer is a soft signal here.
- Meta, "LLMs Are the Key to Mutation Testing and Better Compliance", engineering.fb.com, Sept 2025; arXiv 2501.12862. Automated Compliance Hardening: LLM generates fault-class mutants from a text description, then tests to kill them, with an LLM equivalence detector filtering unkillable mutants; Oct-Dec 2024 trial: engineers accepted 73% of generated tests, 36% privacy-relevant. Grade B. Human remains the verifier of the verifier.

## Property-based testing and fuzzing
- PBT-Bench, arXiv 2605.15229, 2026. Agent-written properties catch ~30-40% of seeded bugs vs 70-80% human baseline; failure modes: over-constrained (reject valid inputs) and under-constrained properties. Grade A. Findings 3, 4.
- "Fuzzing with Agents: Generators Are All You Need", arXiv 2604.01442, and related. LLM agents synthesize generators and grammars for coverage-guided fuzzers; bugs found in compilers and DL libraries that traditional fuzzers missed. Grade A per study. Finding 8: an LLM-directed grammar fuzzer could have produced the non-ASCII literal case the corpus lacked.

## Differential testing
- Practice literature 2025-26 incl. SmartOracle (arXiv 2601.15074): differential testing shifts the burden to corpus/generator coverage; it exposes divergence only on inputs actually produced; false positives from environmental drift and implementation-defined behavior. Grade B-C. Finding 8: the zero-non-ASCII-literal gap is structural; the fix is generator breadth, not comparison logic.

## LLM-as-judge
- "Reliability without Validity: A Systematic, Large-Scale Evaluation of LLM-as-a-Judge Models Across Agreement, Consistency, and Bias", arXiv 2606.19544, 2026. 21 models, 9 providers, ~541,000 judgments; raw agreement overstates discriminative ability by 34-41 points after kappa correction; a consistency-bias paradox: run-to-run reproducibility >0.95 alongside severe positional bias. Grade A. Findings 3, 8: an instrument that always gives the same answer is not thereby correct.

## Adversarial multi-agent review
- "Adversarial Review: Structured Disagreement for Grounded Agentic Code Review", arXiv 2608.18167, 2026. Author, reviewer, critic-of-reviewer; inner text-only argument loop, outer edit loop. SWE-bench Verified 75.2% vs 71.6% zero-shot; on SWE-PRBench naive adversarial review UNDERPERFORMED a simpler baseline (F1 0.457) until disagreement was forced to carry explicit evidence (0.533). ~4.5x tokens. Failure mode: false consensus, the critic yielding to a confident reviewer. Grade A. Finding 8.

## Telemetry after automated gates only
- GitClear, "The Maintainability Gap", 2026, 623M changes 2023-2026: block duplication +81%; cross-file reuse -35%; refactoring commits 21% (2022) -> 3.8% (2026); maintenance of >1-year-old code -74%. Grade B; causal attribution to AI is inference.
- DORA 2025 (secondary): more AI use correlates with more change failures, rework, longer resolution; AI as amplifier. Grade B; exact figures UNVERIFIED here.
- Veracode GenAI Code Security Report via larridin.com: security flaw in 45% of tests across 150+ models; 55% secure absent guidance. Grade C, UNVERIFIED primary.

## Mapping
| Finding | Technique | Grade | Cost / failure mode |
|---|---|---|---|
| 3 | mutation scoring only after compiled-and-ran (cargo-mutants convention) plus a judge-reliability check before trusting consistency | C to A | still needs a negative control that breaks the code to prove the harness can fail; tooling does not force it |
| 3 | property-based tests benchmarked against a human baseline (PBT-Bench) | A | agent properties catch ~half of what human ones do |
| 4 | longitudinal telemetry on refactoring/duplication; Meta's human accept/reject loop | B | needs sustained measurement and periodic human judgment |
| 8 | fully isolated cross-context review | A | modest absolute detection (F1 ~29%); needs a session boundary |
| 8 | adversarial author/reviewer/critic with evidence-forced disagreement | A | ~4.5x tokens; false consensus |

Not found: a replicated portable "catch rate of fresh-context LLM review of code" (closest: Cross-Context Review F1 28.6 vs 24.6, one model, synthetic errors, no human baseline); any named "harness self-test" or "must-fire check" literature (only the cargo-mutants convention functioning as one); a controlled study of metamorphic testing on AI code; primary Veracode or New Relic reports; a March 2026 outage attributed to AI code appeared in search results and could not be traced to a fetchable source, excluded.

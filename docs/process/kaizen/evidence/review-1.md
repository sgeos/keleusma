# Review 1 of process-audit-draft.md (fresh-context adversarial reviewer, Fable, 2026-09-08)

Disposition column added by the auditor. v1 of the draft is kept as process-audit-draft.v1.md.

| # | Severity | Finding (reviewer) | Disposition |
|---|---|---|---|
| 1 | BLOCKING | "Forty two incidents" and the class table describe different populations; the table mixed evidence-2's approximate sweep tallies (sum 67, tildes dropped, "other" dropped) with evidence-3's tally (sum 23 vs its 20 rows), so it summed to 90 under a heading saying 42 | FIXED. Recounted over the 43 tabulated rows (22 + 21) by primary class: tooling 9, stale 8, checker 6, gate 6, overconfidence 6, relayed 5, spec 3. Sweep tallies stated as separate and approximate |
| 2 | MAJOR | "Largest single failure class" not robust to the count method | FIXED. Heading and text now name stale and instrument classes together, ranking within imprecision |
| 3 | BLOCKING | "Fails toward clean, in every one" conflates two HANDOFF passages; the five-constructed-statuses table has two false alarms; the awk "sixteen still running" and the nine non-placing mutations are alarms of the other kind; the lede's "instruments" overstates 42 mostly non-instrument rows | FIXED. Finding Three retitled "mostly fail silent", two sets separated, false alarms named; lede restated as "record of itself was wrong" |
| 4 | BLOCKING | The 6/7 mechanism-sentence split does not partition evidence-1's thirteen rows; three named "sentences" are not among them; pgrep rule shipped with a script | FIXED. Rebuilt from the thirteen rows: 9 mechanisms, 4 sentences, with the classification rule stated; handoff-resident rules named separately |
| 5 | MAJOR | Recurrence evidence concerns rules outside the thirteen; mechanisms recurred too (twice-stated-figure test defeated 09-03; perf canary could not fail); "in capitals" wrong, rule dated 08-27 so only the 09-02 relay incident follows it | FIXED. Finding Four retitled "and both recur"; distinction narrowed to detectability; dates corrected; adopted as hypothesis |
| 6 | MAJOR | The 2026-08-24 packet's ruling 8 was the relayed rebase; three operator sessions ruled merge/rebase/sync, an operator-consistency failure the two-regime model omitted; ruling 3 still unruled | FIXED. Packet presented with both benefit and hazard; operator-consistency failure added to Finding Five and to the trade-off section |
| 7 | MAJOR | "The operator does not review" and "the record of merges shows none" rest on a private memory note, not evidence; no review data pulled | FIXED. Restated as an inference from cycle time and the 2026-08-20 standing authorization in the auditor's session memory; added to Epistemic State |
| 8 | MAJOR | "Changed only documentation" promotes a title classification | FIXED. "Titled as documentation" throughout; file lists not checked, stated |
| 9 | MAJOR | "A third of the commit stream" traces to nothing | FIXED. Replaced with measured 12.3/14.0 percent and 6.4 percent |
| 10 | MAJOR | "More process prose than source" depends on gross additions that count churn and the 362 KB relocation; net docs/process 18,137 vs src 25,105; compiler/ excluded; src includes 8,508 Keleusma lines | FIXED. Net figures by bucket added to the table and the paragraph rewritten to "same order of magnitude, depends on the measure" |
| 11 | MAJOR | PR figures cover 2026-08-11 to 09-08 (28 days), CI from 2026-05-05 | FIXED. Windows stated in the table and the lede |
| 12 | MAJOR | Two-regime inference contradicted proposals: relayed rulings are mechanizable; group one has costs; DORA applicability caveat | FIXED. Relay placed in regime one; "trades no objective against another, though each costs work and a ruling"; DORA caveat added |
| 13 | BLOCKING (proposals) | Proposal five's docs path filter would skip claimed_counts and the handoff boundary test, reproducing the 2026-09-03 blind spot | FIXED. Filter keeps the document-reading binaries; ruling scope widened |
| 14 | MINOR | Proposal eight: single revert fails after absorption | FIXED. Sentence added |
| 15 | MINOR | "Three times wrong" is a factor | FIXED |
| 16 | MINOR | TASKLOG has a same-day consolidation rule (COMMUNICATION.md:72), not followed | FIXED. Added to Finding One |
| 17 | MINOR | Completeness overclaims: per-row labels absent; timeout check unrecorded; "about thirty minutes" not in NOTES | FIXED in text; timeout check and timing recorded in NOTES.md |
| 18 | MINOR | "No equivalent" overstated; proofs.md retractions section, SCOPE_DELETION.md, STALE_COUNT_TRIAGE.md exist | FIXED. "No single equivalent" with those named |
| 19 | MINOR | "three merges" were commits; Sakana reliance; blog untracked draft; timing | FIXED |
| 20 | MINOR | Acronyms ASCII, AI, METR, DORA unspelled; four self-regarding sentences | FIXED. Spelled out; sentences removed or marked as unestimated |

Attacks that failed (reviewer): all five document sizes reproduce; PR totals/medians/docs share reproduce; CI totals, durations, main window, four 360-minute cancellations reproduce; ci.yml has no timeout-minutes, no paths filter, no partitioning; numstat buckets reproduce; release-branch model entered GIT_STRATEGY.md 2026-07-23 after the last main failure; CONTRIBUTING.md:47 still trunk-based; two startup protocols differ; the 2026-07-22 split at COMMUNICATION.md:16; v0.3.0 banner-stale passage at its lines 6-9; 116 vs 118 at absorption 47; Theorem B2 unruled; ABI_RULINGS.md and OPERATOR_DECISIONS_OPEN.md on v0.3.0 only; no PR-template negative-control item; no lessons ledger under v0.2.3 docs/process.

# Evidence 3: the V0.3.X and proofs lines' own records (miner report, Sonnet, 2026-09-08)

Read-only; no cargo, no state-changing git. Note from the miner: the scratchpad export v030.md (8032 lines) was stale against live origin/v0.3.0 (8136 lines, 530,846 B); citations re-verified against a fresh git show. dj030/tl030/proofs.md byte-identical to live.

Sources: v0.3.0 handoffs/v0.3.0.md (8136 lines / 530,846 B); v0.3.0 DESIGN_JOURNAL.md (15,024 lines / 1,315,214 B); v0.3.0 TASKLOG.md (3,084 / 510,599 B); proofs handoff (168 lines); three audit records; v0.3.0 docs/decisions/ABI_RULINGS.md and OPERATOR_DECISIONS_OPEN.md (the ABI rulings live under docs/decisions, not docs/process); git logs (730 commits unique to v0.3.0; 5 unique to proofs).

## A. Failures, retractions, corrections, lessons

| Date | What went wrong | Class | Cost | Remedy | Cite |
|---|---|---|---|---|---|
| 2026-08-21 | Told the operator src/verify.rs had no owner, reading a peer's handoff backwards on indexical phrasing | relayed-ruling | | read the governing text directly | v0.3.0 DJ:8069-8075 |
| 2026-08-21 | Claimed Op::IsStruct had no producer (removal candidate under the opcode constraint); it had four producers, two still trapping | overconfidence | | method used to find a defect not reapplied to validate the repair | DJ:8082-8085 |
| 2026-08-23 to 27 | v0.2.3's relay of the proofs operator's ruling ("rebase") recorded as settled in the proofs mailbox and Appendix E; actual ruling was merge | relayed-ruling | Appendix E correction queued | never record a relayed ruling as settled | proofs.md:81-90 |
| 2026-08-27 | The proofs line's "rebase onto the result" instruction was a relay of a relay; this line merged instead on its own operator's "sync" | relayed-ruling | none, correctly declined | a peer cannot grant escalation | v0.3.0.md:1179-1200 |
| 2026-08-24 to 26 | Three adversarial audit rounds on the proof: nine converging findings round one (four theorems/corollaries not established as written), a defective round-one repair found in round two, two soundness holes plus consistency breaks round three; ~40 failed attacks also recorded | spec-misread | three full rounds | axiom additions, statement-box discipline | AUDIT_2026-08-24.md:17; ROUND2:24; ROUND3:27,37 |
| round one | Theorem A2's statement box kept the refuted hypothesis after the body was patched | spec-misread | one extra round | "a repair applied inside a proof body while the statement is left unchanged is not a repair" | AUDIT_2026-08-24.md:105; proofs.md:64 |
| undated | Proofs line's style scans excluded blockquotes; passed three times over a live violation | checker-reach | | "a checker's clean report is evidence about its reach before it is evidence about the tree" | proofs.md:104-105 |
| undated | A directional claim measured wrong (reachability motivator existed only because of dispatch scopes) | overconfidence | | measure the direction | proofs.md:106-108 |
| pre-08-27 | Appendix D backend-row discharge retracted by its reporter; region_nonreuse.rs bounds distinct-site sharing, not loop-iteration reuse | spec-misread | row stands "required for soundness and not discharged" | two guarantees, one enforced, not conflated | proofs.md:94-99 |
| s. ending 08-31/09-01 | "239" composite sites was a carried number with no producer; real 256 across 35 chunks, cross-checked by two methods over the same 69-module population | stale-count | | corroboration = different methods over the SAME population | v0.3.0.md:586-600 |
| same | Corpus population quoted as 91 modules / 1117 chunks; three test files listed examples/scripts/rogue explicitly AND recursed into it, double-counting 24 files; real 67 / 1074 | stale-count | published coverage figures unaffected | derive the watched scope from what consumers read | v0.3.0.md:663-690 |
| same | bound_transfer.rs reported "74 examined / 71 compared" vs "69 compiling modules" elsewhere; the reconciliation probe itself had two defects (two prelude.kel collapsed under file-name keying; a fmt-reformatted line caused a silent no-op substitution) | stale-count | second silent-substitution in one session | "three numbers, all correct, none labelled"; state the population every time | v0.3.0.md:715-780 |
| through 09-02/06 | NATIVE_MUTATION_CENSUS.md said "no hole open"; mutation_sweep.py classified on process exit status, any non-zero = DETECTED, so after a 2026-08-21 test change it would score every module as detecting every mutation | tooling-fabrication | 12h51m attempt (5 of 25 mutations) projecting ~60h before the defect was found; repaired sweep 42 min | separate population guards from consistency guards; prove the fix both directions | v0.3.0.md:326-360, :3 |
| same repair | Nine mutations (52 sites under Return) silently stopped placing after an emitter signature change | tooling-fabrication | would have been misreported as a real hole | placement checked on every run | v0.3.0.md:326-358 |
| "six times in one day" (both lines) | A true measurement quoted with its scope silently deleted ("the branch compiles" under default features only; "the gate is green" by wrapper exit code; "eleven tests cover the sites" when four did; "requires strictly more" by count not containment); worst instance had three falsifiers that all passed because they shared the claim's framing | checker-reach | docs/decisions/SCOPE_DELETION.md written | write the population into the sentence | v0.3.0.md:465-472 |
| four times / three sessions | Measurement taken while its inputs moved (absorption 40; the 09-02 workspace run; twice on 09-06, after the rule was recorded) | gate blind spot | absorption 40: false-attributed failure 390/1/77 vs predicted 391/0/77 | structural repair frozen-run.sh then backend-gate.sh, "rather than a resolution to be careful" | v0.3.0.md:379-397, 32-38 |
| s. ending 08-31/09-01 | Five process mistakes in one session: pipeline-status trap fired three times; git add -A swept a 152-line test file into a docs commit, pushed unverified; duplicate suite run contended at load 21; a mutation perturbed the subject not the lowering; git push died SIGPIPE (141) after the hook passed, three times | tooling / gate blind spot | one baseline turned red | stage explicitly; verify a push by ls-remote vs rev-parse HEAD | v0.3.0.md:517-535 |
| 2026-09-06/07 | Two commits claimed "the recorded count is updated to match"; the file carries no such count, so replace() matched nothing and failed silently, unguarded | tooling-fabrication | correction note in the file | | v0.3.0.md:7-13 |
| 2026-09-06 | Own handoff undercounted an absorption by four commits (14 vs 18) because the peer advanced during the work | stale-count | | "a count in a handoff is a timestamp, not a fact" | DJ:14-16 |
| 2026-09-02 night | Relayed a peer's f64->f32->binary16 narrowing chain as settled, against this tree's own FLOAT_LADDER.md and FLOAT_ARITH_WIDTH_BRIEF.md forbidding it | relayed-ruling | caught before landing | | DJ:472-490 |
| 08-20 vs 08-29 | Mailbox recorded a 2026-08-20 string-ABI ruling ("ratify the current shape"); ABI_RULINGS.md recorded 2026-08-29 Option B; tip carried both contradictory records | stale-doc / relayed | flagged, not reconciled | flag rather than pick | DJ:2996-3000 |

Totals (20 incidents, primary class): relayed-ruling/provenance 6, stale-count/stale-doc 5, tooling-fabrication 4, spec-misread 3, checker-reach 2, model-overconfidence 2, gate-or-CI-blind-spot 1.

## B. Pilot dependencies

| Date | Question | Resolution / elapsed | Direct vs relayed | Cite |
|---|---|---|---|---|
| 2026-08-29 | Float entry ABI (OPERATOR_DECISIONS_OPEN item 2) | Option A, real FP ABI; width detail flagged as the line's own reading; RESOLVED 2026-08-30, one day | DIRECT | ABI_RULINGS.md:15-38; v0.3.0.md:930-935 |
| 2026-08-29 | String ABI (item 5) | Option B "make the two embeddings agree"; NOT IMPLEMENTABLE by this line (marshalling in src/, owned by v0.2.3) | DIRECT, but contradicted by a 2026-08-20 record on the same tip | ABI_RULINGS.md:44-52; DJ:2996-3000 |
| open | Fixed shared-slot ABI (item 4): three readings of one ruling sentence | open; conditional on an unstated interop goal | DIRECT, underspecified | ABI_RULINGS.md:54-72; ODO:101-124 |
| open | Text shared-slot ABI | the ruling's own supposition was incorrect for Text; preserved, not silently corrected | DIRECT, internally wrong | ABI_RULINGS.md:96-105 |
| open | Opaque shared-slot ABI | intent already met by existing handle design; literal reading conflicts with narrow-word builds | DIRECT, satisfied by measurement | ABI_RULINGS.md:107-118 |
| open | Unit | "Not sure what unit is", a question not a ruling; line's inference marked as inference | not a ruling | ABI_RULINGS.md:120-127 |
| 08-23, corrected 08-27 | Topology mechanism across three lines | three operators' words converge on merge; 17+ absorptions used merge | RELAYED initially, corrected | proofs.md:81-90; v0.3.0.md:1179-1200 |
| predating proofs | Float ABI "one word" in v0.2.3's REVERSE_PROMPT | open at proofs' 08-29 stamp; v0.3.0's related float-ABI question resolved 08-30 | | proofs.md:117-119 |

## C. Coordination

Mailbox convention exists because relayed prose loses information; its first day (2026-08-09) produced the failure it exists to prevent (hreadme.md). Messages crossed over Text<N>: the operator authorized it directly in one session and asked whether it had been communicated; it had not; both lines wrote simultaneously (DJ:2611-2614). R2 (flat composite) adopted on merit (DJ:2622-2625); R5 (Opaque by address width) turned out a ~60-site public API change touching four KeleusmaType trait methods, not one line (DJ:2664-2673).

Gate serialization on one shared machine: one session killed a gate at step 7 of 13 to restart on a newer commit, discarding a mergeable result and costing the peer ~40 minutes of queued waiting; operator's correction: bank a green result (v0.3.0.md:7776-7796). gate-in-worktree.sh correctly refused a second gate at 91% (:7770-7775). CI concurrency keyed on github.ref would have let a second PR's merge cancel the first's run; caught by a peer's review (:6230-6249).

Absorption mechanism (numbered merge of v0.2.3 into v0.3.0 with predicted test-count movement stated before merging): highest 54 (:2870-2871). Frozen-tree discipline caught a conflation at 13-15, was skipped at 16 (caught after the fact), violated again at 40 (:32-38). Absorptions 52-54 each report predicted vs measured conflicts (52: 1 vs predicted 3; 53, 54: 0 vs 0).

## D. Calendar and cost

cadence.txt: origin/v0.3.0 has commits on 106 distinct days; heaviest 2026-08-09 (126 commits), 08-10 (117), 08-14 (110); weekly volume ~60-90 in W19-W27 rising to 552 in W33. 730 commits unique to v0.3.0; 5 unique to proofs (2026-08-23 to 08-29), almost entirely bookkeeping.

Gate figures: a feature PR merge on CI green ~48 min across 22 checks (:7491). Mutation-sweep control: one real module 385 s unfiltered vs 387 s for a nonexistent module name, i.e. nearly all per-module cost was tests incapable of detecting a mutation (400x-too-high cost finding, :346-352). 12h51m attempt (2026-09-02) bought 5 of 25 mutations, projecting ~60 h; repaired harness 42 min (:326-360). Frozen discipline adopted after four violations across three sessions; it labels a run, catching only in-flight edits, not incorrectness or load (:379-397).

## E. Roadmap honesty

Corrected counts, all self-reported: 239 -> 256 sites; 91 -> 67 modules; three-way population split (74 / 71 / 69) reconciled after being quoted unlabelled; absorption commit count 14 vs 18; the mailbox banner read "after absorption 18" for three days while the body described 38, "the least-refreshed line in the file" (:1-6). Progress is measured by paired prediction-then-measurement (test counts, conflict counts, populations stated before an absorption or run, then checked); partial holds flagged (e.g. predicted 116 binaries vs actual 118, :37-44). Publication held by the operator throughout; no start date or lifting criterion found (:183,294; TASKLOG.md:11,34,61,78; OPERATOR_DECISIONS_OPEN.md:176-179 "not a request to change that").

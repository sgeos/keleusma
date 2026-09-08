# Evidence 2: the V0.2.X line's own record (miner report, Sonnet with forks, 2026-09-08)

Sources: handoffs/v0.2.3.md (2504 lines / 160,773 B), DESIGN_JOURNAL.md (11,096 lines / 1,056,884 B), TASKLOG.md (2165 / 434,064 B), REVERSE_PROMPT.md (307 / 19,096 B, read in full), HANDOFF.md (1439 / 93,081 B, read in full). Keyword sweep (EXECUTED): 140+ hits in the mailbox, 233 in TASKLOG, dozens per journal session (66 dated sections, session 3 2026-06-09 to session 63 2026-09-05). Table is a representative sample, not exhaustive.

## A. Failures, retractions, corrections, lessons

| Date | What went wrong | Class | Cost | Remedy | Cite |
|---|---|---|---|---|---|
| 2026-09-05 | Three of the session's own instruments reported clean verdicts about things they never touched (a sweep column listed WARNING under PASSING; a mutation test aimed at the wrong call site passed; an operand-range probe mutated an opcode absent from its program and reported it "admitted") | tooling-fabrication | "none was caught by review" | "an explanation is also a guess, and so is a measurement" | HANDOFF.md:41-46; REVERSE_PROMPT.md:158-166 |
| 2026-09-04 | A float-using module verifies, loads, traps InvalidBytecode on a no-floats build; verify.rs has no floats gating | gate-or-CI-blind-spot | repair prototyped then reverted on a since-corrected premise | pinned by tests/float_opcode_without_floats.rs, not repaired; deferred to operator | HANDOFF.md:12-24; REVERSE_PROMPT.md:14-46 |
| repeated | "Bitten SEVEN times" by carrying a stale figure forward instead of re-deriving; latest a wire.kel comment citing offsets an order of magnitude wrong | stale-count | | "derive numbers; do not copy them forward" | HANDOFF.md:296-299 |
| unspecified | Boundary census count wrong three times running by adjusting rather than re-summing; re-summing exposed a missing group of 3 sites | stale-count | | derive from the test, never restate in prose | HANDOFF.md:37-40,300-303 |
| 2026-08-21 | Shipping self-hosted compiler and its test-file copy diverged: five defects because the boundary exercised only the copy | checker-reach | limited exposure | three guards, none sufficient alone | HANDOFF.md:806-833 |
| 2026-08-20 | Self-hosted compiler silently mis-lowered true/false to GetLocal(0), a miscompile not a refusal | tooling-fabrication | | fixed | TASKLOG.md:1157-1163 |
| 2026-08-30/31 (s58) | Reference lexer corrupted every non-ASCII string literal; corpus had zero non-ASCII literals; "the reference was the divergent side" | checker-reach / spec-misread | | 49-probe lexical divergence census | DESIGN_JOURNAL.md:960-979 |
| 2026-08-19 | Two operator rulings answered questions already stale (ECC plane reported open though built; token-array framed as capacity though streaming solved it) | stale-doc / coordination | "a wrong question costs a ruling" | "read the tree before putting a question to the operator" | TASKLOG.md:1261-1266; DJ:4915-4938 |
| 2026-08-11 | "A residency refutation published and retracted": units error claimed a 77% projection refuted by 40x; original 58.3% was right | model-overconfidence | two commits withdrawn by a third | commit-based retraction on a shared branch | TASKLOG.md:1723 |
| 2026-08-05 | Guessed the cause of a 782s regression three times before instrumenting; near-miss of hiding two corpus stages behind #[ignore] | model-overconfidence | ~30 min | instrumentation found Names::intern linear scan; 782s -> 2.45s | TASKLOG.md:1754; DJ:8626-8646 |
| 2026-08-21 | Indexical "they/their" caused v0.3.0 to misread its mailbox, escalate a false "no owner" finding; this line relayed the same misreading to its operator without checking both texts | relayed-ruling/provenance | escalation on both lines | ownership now a table naming lines absolutely | HANDOFF.md:709-716,1405-1424 |
| 2026-08-21 | Overclaim that Op::IsStruct has no producer, validated by guessing three constructs; peer found four counterexamples within an hour | model-overconfidence | a load-time hole survived | "a method used to FIND a defect is not automatically valid for validating its REPAIR" | HANDOFF.md:737-767 |
| 2026-08-26 | wire.kel compile failure misdiagnosed twice (capacity bound; 256-declaration cap); real cause missing hex/binary literals | model-overconfidence | two wrong publications same day | corrected | TASKLOG.md:512,1902-1903 |
| 2026-08-26/27 | Byte-identity debugging: 17 guess-based repairs failed; bisection succeeded 3 of 3 | tooling/method | 17 wasted attempts | method-cost lesson recorded | TASKLOG.md:1856 |
| 2026-08-31 | Local release-gate run twice under load, neither finished, step 3 of 12 in 110 minutes | gate blind spot | ~3.5h machine time, no verdict | rely on CI under load | HANDOFF.md:150-159; DJ:293-299 |
| 2026-08-31 | awk split on whitespace reported "sixteen still running" for 90 minutes after completion | tooling-fabrication | merges delayed | | DJ:302-305 |
| one session | Seven silent instrument failures listed by name (timeout absent on macOS silently no-op'd a gate; unconditional echo "CLIPPY OK"; tail-piped exit code masking failure 101; two waiters misreading "still running" as "failed"); none erred toward a false alarm | tooling-fabrication | | "derive a population, never pick one"; capture exit status in the log | HANDOFF.md:130-166 |
| 2026-08-09 | Local gate abandoned at step 13, then step 7, each after hours of exclusive machine time; a waiter read "ABANDONED" as complete and stayed silent ~66 hours | gate blind spot | 2h30m per false trip; ~66h blind spot | CI gates feature branches (~48 min) | v0.2.3.md:1987-2012,2038-2150 |
| 2026-08-10 | WCMU soundness hole: verify() admitted a chunk that runs off the end without Return; first repair broke 37 lib tests | checker-reach | 37 tests | reverted, repaired via Op::Reset | TASKLOG.md:1736 |
| 2026-09-03 | A handoff refresh deleted one of two required occurrences of the boundary figure, breaking its own self-pinning test; invisible because the pre-push routine tier excludes selfhost_* binaries | gate blind spot | | must-fire control | HANDOFF.md:159-166; TASKLOG.md:82-87 |
| 2026-09-04 | A perf canary asserted its ceiling only after the timed call returned, so could not fail; spun 57 minutes at 99% CPU | gate blind spot | 57 min | bounded on a channel, mutation-tested | REVERSE_PROMPT.md:105-108; TASKLOG.md:28-29 |
| 2026-08-15 | Op::cost() covered 17 of 66 opcodes; analyze_class silently fell through to (0,0) | checker-reach | "the largest gap between what is asserted and what is measured" on a WCET-headline project | exhaustive match | TASKLOG.md:1406-1409 |

Totals by class (miner's tally of distinct incidents, lower bounds): model-overconfidence ~12, stale-count/stale-doc ~14, tooling-fabrication ~11, gate-or-CI-blind-spot ~9, checker-reach ~9, relayed-ruling/provenance ~4, coordination-overhead ~3, shared-checkout ~2, spec-misread ~3, other ~2.

## B. Pilot dependencies

| Date | Question | Answered? | Elapsed | Direct/Relayed | Cite |
|---|---|---|---|---|---|
| 2026-08-24 | Eight numbered rulings (FP entry ABI, confinement x2, Theorem B2, publication, GRAMMAR.md cross-ref, CI Doc coverage, merge sequence) | seven ruled; #3 "UNRULED IN EITHER DIRECTION" | same session | direct, numbered table | HANDOFF.md:1190-1207 |
| ongoing | Publication authorization | held; "a prior 'expedite' is not authorization" | open at s63 | direct | HANDOFF.md:1192 |
| 2026-09-04 | Land the ~10-line float-verification repair? | deferred | open at s63 | direct | REVERSE_PROMPT.md:56-62,300-303 |
| | How a Text<N> value is constructed | "one of the two questions that block everything large" | open | direct | HANDOFF.md:63-72; RP:296-299 |
| | Whether addr_bytes justifies a breaking API change (33 signatures, 14 public) | unresolved | open | direct | HANDOFF.md:73-78 |
| 2026-08-11 | Local gate vs CI policy | answered same day | same day | direct | HANDOFF.md:505-506; TASKLOG.md:1710 |
| 2026-08-29/30 | Whether a string-ABI ruling on v0.3.0 bound this line | read, deliberately NOT acted on; operator confirmed directly | one session arc | direct after refusing relay | DJ:1041,1335-1345 |
| 2026-08-21 | verify.rs ownership | misread, escalated by v0.3.0, relayed by this line without checking | same day | relayed (error) | HANDOFF.md:1405-1424 |
| 2026-08-08 relays | Four items incl. dynamic Text<N> | confirmed in session 2026-08-31 | ~23 days from relay to confirmation | direct after relay | v0.2.3.md:3-10,42-44,544 |
| various | Cross-line float ABI item | "THEIRS to bring the operator" | unresolved | cross-line | TASKLOG.md:1832-1833 |

No case found where a ruling acted on arrived only by relay without later direct confirmation; the discipline emerged over time (explicit refusal to act on a relayed reading DJ:1335-1345; "neither of us is a reliable narrator about the other's code" HANDOFF.md:1417-1424).

## C. Coordination

Pull-only mailboxes, "the mailbox has no wake ... boundary polling is enough because coordination only matters at boundaries" (v0.2.3.md:2481-2492). ~55 distinct numbered exchange headings (EXECUTED); "ANNOUNCING BEFORE LANDING" pattern x5 (:717,755,798,832,867). Crossed/missed: "Our messages crossed" (:5); "YOU OFFERED THIS AND I MISSED IT" (:370); "why seventeen attempts across both our lines missed it" (:613); a peer's abandoned gate unnoticed 66h21m (:1712-1738). Owed ledger entries (:1235, :1048; HANDOFF.md:1424). Ownership became a table after the 2026-08-21 indexical misread (HANDOFF.md:1388-1424). Three named cases of the lines being unreliable narrators about each other's code (HANDOFF.md:1417-1421).

## D. Calendar

Session 3 = 2026-06-09 (DJ:10984); s6 = 06-11; s11 = 06-14; s50 = 08-21; s51 close = 08-23; s53 close = 08-25; s57-59 = 08-29/30/31 (s57 alone: eight named increments, DJ:1061-1472); s60 = 08-31; s61 = 09-01; s62 close = 09-03/04; s63 close = 09-05. Roughly 88 calendar days for 60 numbered sessions, not one per day. Durations: CI ~48 min vs local ~2h30m (HANDOFF.md:505-506); a local run "GREEN, 13 steps, ~3h35m" (v0.2.3.md:2303); stuck at "step 3 of 12 in 110 minutes"; abandoned gate stale 66h21m; perf-canary 57 min; 782s -> 2.45s; a detached ~2h gate run lost its exit-status capture (DJ:307-311).

## E. Roadmap honesty

Measures: merge/PR counts at session close ("TEN MERGES THIS SESSION", TASKLOG.md:1836); test counts per gate step; the construct-support boundary table, 83 SOk (DJ:4662) through 88, 90, 91, 94 to 96 SOk / 1 Refuses / 3 Diverges / 1 RefRejects of 101 (HANDOFF.md:216-219). HANDOFF.md:216-224 warns "Do not trust the counts in this file without re-deriving them"; a test forces the figure to appear twice so a stale copy turns the suite red (:227-234), itself defeated once on 2026-09-03 (:159-166).

"THE MACRO POSITION": none of the five V0.2.x success criteria hold yet (HANDOFF.md:930-931); two recorded CONSTS obstacles were themselves wrong (:932-940). "179 binaries and 2904 tests" claimed, corrected to 113/2708 (DJ:701-720). RELEASE_PROCESS.md once omitted two of seven publishable crates from all four enumerations (DJ:320-333). A roadmap cell stale by 38 tests (TASKLOG.md:1637). CLAUDE.md carried a wrong --all-features claim until 2026-08-16 (TASKLOG.md:1483).

Reading protocol: DESIGN_JOURNAL.md says only "newest-first, NOT overwritten" (DJ:7), no guidance on how much of ~1 MB to read. HANDOFF.md:498-503 gives the one explicit reading order (secret/notes/APPENDIX_B.md, peer mailbox to the end, own mailbox, the three channels, AUTONOMOUS_IMPLEMENTATION_LOOP.md). HANDOFF.md:317-320 warns it is "long and largely historical", only the top banner current, with a "SUPERSEDED duplicate of this very heading further down" left as an example of drift.

# Reverse prompt

## Current state

Order 2 is complete for the full first-pass validator scope on line v0.2.3.
The branch is feat/v02-order2-validator. This increment starts at a12c45c9.
The scoped commit includes this handoff and the completion audit at
docs/decisions/ORDER2_COMPLETION_AUDIT.md. Read that audit for the requirement
matrix, exact verification scope and retained limits.

Order 3, trap analysis in the validator, is the next V0.2.X roadmap priority.
Do not retire reference verification or runtime guards. The typed stage retains
its selected control-flow widening profile, with a concrete disagreement control.
Full-language expansion remains Order 6. No push, merge or publication is included
in this increment.

The legacy HANDOFF.md is invalid-and-stale. It was 55 commits behind the starting
head and still listed twelve unchanged stage sources. There are fourteen changed
sources. Its ancestry anchor passed, and the fingerprint remains 0x4327_63E1.
Appendix B was read before tracked writes. The V0.3.X worktree, .codex/ and nvim.log
were not changed. Public bytecode version remains 2, instruction count 66, selected
type capacities 640 and the depth limit 32.

## Implemented work

The public admission entries combine structural, typed and resource results.
Keleusma owns graph traversal, resource summary storage, text-allocation transitions,
yield fixpoint scheduling, productivity and final arena admission. Native attestations
have a separate admission entry. External native costs are deduplicated and charged
per chunk in the stage. Heap arithmetic saturates at the unsigned 32-bit ceiling.
The analyzer uses a 64-bit host Word, independently of subject widths.

The failure boundary keeps unavailable analysis separate from completed rejection.
Missing callee summaries and unknown completion codes are protocol errors. Bounded
walk exhaustion is an explicit error. The valid controls and stage-decision
interventions pass. Rust transports opcode facts and declared metadata, allocates
storage and sequences the stage calls.

The typed audit repaired branch-prefix widening, checks after terminated branches,
incompatible break heights and the reference post-loop fallback. The last control
first demonstrated a complete-admission false acceptance. After the repair, the
same malformed scalar projection rejects and its valid control accepts. Eight
permanent tests cover those remaining families and the exact 12288-call-shape
capacity. The detached structural corpus now invokes both verifier entry points.

The shipping command is keleusma validate with --validator rust or self-hosted.
The self-hosted result is computed before the reference comparison. Exit zero
means acceptance, one rejection, and two unavailable analysis or disagreement.
The command executes no subject instructions and does not authenticate signatures.
It uses empty native attestations, matching the reference offline entry.

## Verification

Every verification process from this increment is terminal. Do not resume old
session 25978 or corrected session 30602. The corrected gate's command ledger is
tmp/order2-corrected-gate-results.json. Its recorded source hashes did not change
through any completed step.

| Check | Result |
|---|---|
| Full workspace nextest | 3281 pass, two skipped. Separate doctests have twelve passes and three ignored. |
| No-default nextest | 329 pass. Doctests pass. |
| Signatures nextest | 2350 pass. Doctests pass. |
| Signatures plus shell nextest | 2367 pass. Doctests pass. |
| Isolated self-host nextest | 3031 pass, two skipped. Doctests pass. |
| Wire configurations | 57 tests with all features and 20 without defaults pass. |
| Detached compiler | 86 tests pass, including 46 structural controls through both entries. Clippy and formatting pass. |
| Compatibility and documentation | Rust 1.88 test checking, thumbv7em-none-eabihf build, warnings-denied documentation and 85 book assertions pass. |
| Additional checks | Debug iteration overflow control, ShellCheck, fresh workspace Clippy and formatting pass. |

The logs use the prefix tmp/order2-corrected-. The full matrix used release mode,
the full nextest ci profile and separate doctests. RUSTC_WRAPPER was empty and
KEL_SELFHOST_CACHE was zero. The source inventory is 1682 integration test attributes
across 169 root files. Library counts are 1407 under workspace unification and 1357
under isolated self-host.

The first workspace run was red on stale residency pins. The remaining sequence
was stopped before the post-loop repair. New measurements are analyzer total 3215
and peak 367, typed validator total 1438 and peak 395. Both retain the existing
threefold requirement, and the corrected full runs pass the updated pins.

The corrected gate's only failed step was the link checker reading generated
copies under the ignored secret directory. The checker now prunes that local
store. One new command test proves that a public broken link still fails and a
restored public link passes. The validator implementation was unchanged after
the full feature runs. This new test has its own receipt in
tmp/order2-doc-link-controls.log. The real public-tree link check passes in
tmp/order2-corrected-public-doc-links.log. Final documentation refresh receipts
are tmp/order2-handoff-doc-links.log, tmp/order2-handoff-citations.log,
tmp/order2-handoff-roadmap.log and tmp/order2-handoff-fmt.log. Fresh Clippy after
the new test is in tmp/order2-handoff-clippy.log.

Memory-interpreter, advisory and unrelated platform checks were not repeated.
No unsafe arena code, dependency manifest, lockfile or platform implementation changed.
Remote continuous integration is not claimed. No processes remain intentionally running.

## Pilot and interline

Task v023-order2-resource-a12c has one diagnostic and three bounded claims. The
diagnostic is labelled compiler_or_test. Protocol failure and post-loop repair
claims are labelled supports. The boundary claim is labelled unsupported because
its short payload omits proof that the limit is positive. The actual command
corpus asserts positive capacity and passes on all fourteen stages.

All labels have independent false. The increment outcome is resolved. External
model evaluation remained unavailable under the earlier automatic approval rejection.
There was no retry, provider judgment, independent accuracy or saved-reasoning claim.
Usage values remain null.

The reviewed export is tmp/jev-shadow-order2-resource-a12c-final.json, with 193
samples and 49 task records. All four current request digests and source hashes
match. The peer-owned tracked pilot checkpoint remains untouched. The assigned
inbox is empty. The interline checkpoint is exported and reviewed before commit.
No peer message was sent.

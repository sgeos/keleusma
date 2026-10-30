# Kaizen Line Outputs

> **Navigation**: [Process](../README.md) | [Documentation Root](../../README.md)

Dated reports of the kaizen line, the project's continuous-improvement line, with the evidence each
rests on. The line's mailbox and resume prompt is [`../handoffs/kaizen.md`](../handoffs/kaizen.md).

| report | state |
|---|---|
| [`2026-10-29-changelog-fragments.md`](./2026-10-29-changelog-fragments.md) | changelog entries moved to one fragment file per change, assembled at release, on the operator's ruling of 2026-10-29 after V0.2.X reported merge conflicts in `CHANGELOG.md`, and the operator's ruling the same day to keep branch runs uncancelled |
| [`2026-10-28-pydromon-team-setup.md`](./2026-10-28-pydromon-team-setup.md) | proposal for running every Keleusma line in pydromon, six lines with a fresh-context reviewer and a blind Jev labeller, one full-gate lease, a validated project file and team rules, two phases, nothing installed, changeover timed by the operator |
| [`2026-10-24-context-engineering-spike.md`](./2026-10-24-context-engineering-spike.md) | follow-up to the token spike at the operator's response, state of the art and harness waiting surveyed, mid-task compaction measured as a likely cost to correctness, every Opus 5.5 compaction found to be idle, the lower auto-compact window withdrawn, gate summary and Python CI job built, reminder changes not activated |
| [`2026-10-25-ci-load-proposal.md`](./2026-10-25-ci-load-proposal.md) | reply to the admin session's CI load review, compilation found to dominate runner time, documentation found to be read by at least 17 test binaries, a light path for pull requests, a book filter, tree-identity deduplication, a build-once shard layout and job consolidation proposed and applied in pull request 533, its first run measured at 173 runner-minutes against about 200 |
| [`2026-10-24-token-economy-spike.md`](./2026-10-24-token-economy-spike.md) | token use measured from local transcripts and git history, state of the art surveyed, seven measured proposals, none adopted, evidence under `evidence/token-economy/` |
| [2026-10-04-jev-shadow-pilot-setup.md](./2026-10-04-jev-shadow-pilot-setup.md) | locally installed advisory pilot for V0.2.X and optional V0.3.X, blind comparisons, concurrent budget reservations and portable evidence |
| [2026-10-04-jev-development-flow-spike.md](./2026-10-04-jev-development-flow-spike.md) | bounded Jev experiment with tracked receipts, low service cost verified, subscription savings and overall error reduction unmeasured |
| [2026-10-04-interline-portability-receipt.md](./2026-10-04-interline-portability-receipt.md) | project-local checkout, tracked queue checkpoint, fresh-clone restoration and harness-independent routes |
| [2026-10-04-interline-protocol-receipt.md](./2026-10-04-interline-protocol-receipt.md) | shared local transport and both client configurations installed, live receiver activation unverified |
| [`2026-10-04-policy-propagation-proposal.md`](./2026-10-04-policy-propagation-proposal.md) | nine-file patch for the V0.2.X owning line, tested on a trunk export, no peer changes applied |
| [`2026-10-04-process-delta-audit.md`](./2026-10-04-process-delta-audit.md) | delta review through trunk commit `89fc6aac` on 2026-09-28, five proposals partly implemented, no full adoption demonstrated, with measured evidence and verification limits |
| [`2026-09-08-process-audit.md`](./2026-09-08-process-audit.md) | the first process audit, sixth draft, publication-reviewed, in the operator's blog format, not published, describing the process as of 2026-09-08 |
| [`2026-10-04-harness-and-mailbox-notes.md`](./2026-10-04-harness-and-mailbox-notes.md) | design notes on a self-handoff harness, a mailbox interface for lines and the operator, and Keleusma's place in both, nothing built, nothing ruled |

The `evidence/` directory holds the ten mining and research reports the audit cites, the
adversarial review of its first draft with every finding's disposition, the working notes, the
URL check, and under `evidence/raw/` the two exports that cannot be regenerated identically,
the full workflow-run history and the merged pull request list at the time of measurement.
Git-derived exports were not kept because the commands in the fourth evidence file regenerate them.

These reports do not adopt policy. The original proposals state the ruling each needs.
The delta review distinguishes subsequent trunk mechanisms from the full proposals.

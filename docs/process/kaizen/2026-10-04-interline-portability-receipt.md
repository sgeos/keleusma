# Interline Portability Receipt

Measured on 2026-10-04 on `feat/kaizen-interline-protocol`, based on `806e0afa`.
The operator identified `docs/process/INTERLINE_PROTOCOL.md` in the previous
system-temporary checkout as the document to move. The whole isolated checkout
now resides at `.worktrees/kaizen/` under the primary project. The document retains
its normal tracked path and therefore travels with clones containing this branch.
The primary peer branch was not switched or edited.

## Continuation state

Protocol version 2 stores a reviewed queue checkpoint at
[`../interline/checkpoint.json`](../interline/checkpoint.json). It contains line
registrations and any messages and receipts present at export. The installer
recreates the ignored local store from that checkpoint on a fresh clone.
No old `tmp/`, `secret/`, absolute interpreter path or hook configuration is needed.
Machine-specific hooks, client trust and sandbox grants are established locally.
Other private project prerequisites, including Appendix B, remain separately
provisioned and are not copied into this protocol.

The current production checkpoint contains four lines and zero messages. No live
peer traffic was copied or manufactured. Public checkpoint contents must be
reviewed before committing. A snapshot does not capture subsequent messages.
Stop communication and commit the final checkpoint before moving machines.
Concurrent independent machine histories require reconciliation and are not
automatically merged by this protocol.

## Harness independence

Routing uses sender and recipient line identifiers. Sending and acknowledging
work without a harness argument. Optional metadata accepts other clients and is
excluded from retry identity, so a line can retry after switching harnesses.
The original metadata is preserved. Receivers remain self-reported identities
within the existing same-user trust boundary.

## Verification

The Python suite passed eighteen cases, including the previous transport and
installation cases plus harness-independent retries, checkpoint round trips,
malformed-checkpoint rejection and a fresh Git clone. The clone fixture contains
only tracked protocol assets and a checkpoint. It starts with neither ignored
directory, runs the installer with only a line argument, recovers a pending
message and an earlier receipt, and acknowledges the pending message without
naming a harness. No model session is launched.

```sh
python3 -B scripts/test_interline.py -v
```

The documentation link, comment citation and claimed-count guards were recompiled
with the moved worktree as their manifest directory and the compile feature
enabled. All fourteen cases passed. Old binaries embedded the former temporary
path and were not reused as evidence about the moved checkout.
Installed client and protocol copies were refreshed and compared with source.
The production checkpoint was exported from the actual database.

No Rust runtime gate or remote continuous integration was run. No push, merge
or publication occurred. Live receiver consumption and client hook activation
remain unverified. A remote clone can resume this work only after the branch's
commits are made available on that remote.

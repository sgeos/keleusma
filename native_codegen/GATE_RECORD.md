# Backend gate record

Written by `tools/backend-gate.sh`. One row per float configuration; a run updates
its own row and leaves the other's alone. Rows are machine-read by the guard in
`tests/gate_record.rs` and by `tools/gate-status.sh`.

**What a row attests.** That a gate run completed against the named commit, with
the worktree in the stated condition, and reached the stated verdict. The
`worktree` column excludes this file, whose own state says nothing about the code.

**What a row does NOT attest.** That the backend is green *now*. A row describes
the tree at one commit and nothing after it. `tools/gate-status.sh` reports how
many backend sources have changed since, which is the question actually worth
asking; a bare commit mismatch is not a defect, because committing a record
necessarily produces a commit the record cannot name.

**`dirty` is a disclosure, not a failure.** A dirty run's verdict belongs to a tree
that was never committed and cannot be reproduced from history.

| configuration | commit | worktree | verdict | run (UTC) |
|---|---|---|---|---|
| default features | f13f56e27e422d8f3ec3f1f36cfefa7d8e4a2ffb | clean | PASS | 2026-09-25T05:56:58Z |
| narrow-float-32 | f13f56e27e422d8f3ec3f1f36cfefa7d8e4a2ffb | clean | PASS | 2026-09-25T06:10:49Z |

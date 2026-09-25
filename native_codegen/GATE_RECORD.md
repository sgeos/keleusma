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
| default features | d2178ec77612db72e34b8b2557c6638e87a0277d | clean | PASS | 2026-09-25T00:14:03Z |
| narrow-float-32 | d2178ec77612db72e34b8b2557c6638e87a0277d | clean | PASS | 2026-09-25T00:29:37Z |

# Absorption 87

Computed against native `a97a79d4` and upstream `018ad48b` on 2026-10-24, with
5 unabsorbed commits measured after fetching. The merge base is `e4c1b4c3`.
The separate conflict command `git merge-tree --write-tree HEAD origin/v0.2.3`
exited one and produced tree `30f878de3908c6e5f4e6abec75a65342d7446133` with
three conflicting files, `.gitignore`, `docs/process/TASKLOG.md` and
`docs/process/interline/checkpoint.json`. The merge was prepared on
`feat/v3-absorption-87` with `--no-ff --no-commit`, cut from
`feat/v3-order2-arena-frames`, pull request 509, so that 509 merges first
without a conflict.

## What was absorbed

Kaizen pull requests 505 and 507, merged into `v0.2.3` as `9ec88d0a` and
`018ad48b`. They add the token-economy and context-engineering reports under
`docs/process/kaizen/`, `scripts/gate_summary.py` with real failure fixtures,
a shorter per-prompt reminder in `scripts/interline.py`, and one CI job,
`python-tooling`, which runs the `scripts/` unit tests and fails below 50 tests
or on any skip. `CLAUDE.md` changes its CI job count from 27 to 28. No Rust
source, Rust test or native workflow changed. Kaizen announced the job in
`kaizen-20261024-pr507-ci-notice`.

## Conflict resolution

- `.gitignore`: both sides ignore `__pycache__/`. This line's side also ignores
  `*.py[cod]` with its reason, so it is kept.
- `docs/process/TASKLOG.md`: both sides inserted at the same point again. Both
  blocks are kept, this line's first.
- `docs/process/interline/checkpoint.json`: regenerated from the live store,
  26 messages.

## Verification

Run locally with `RUSTC_WRAPPER=` on the staged merge. Logs are under ignored
`tmp/absorption-85/`.

| Check | Result |
|---|---|
| Root `cargo test --locked --no-fail-fast` with `--test documentation_links --test comment_citations --test gate_ci_correspondence --test ci_shard_coverage` | 16 passed, status zero |
| Native `cargo test --locked --no-fail-fast` with `KEL_REQUIRE_LLVM_TOOLS=1` over eleven document-reading binaries, `native_workflow` among them | 51 passed, status zero |
| `python3 -B -m unittest discover -s scripts -p 'test_*.py'`, the new job's command | 68 passed, none skipped, status zero |

The Python suite first ran inside this session's sandbox and `test_jev_shadow`
failed in `setUpClass`, because `git init` there exited 128. The sandbox
denies writes to a new repository's configuration. Outside the sandbox the same
command passed all 68 tests. The hosted job runs without that restriction.

These are targeted checks. Hosted CI on the pull request is the integration check. Publication is
held and no merge into `main` is authorized.

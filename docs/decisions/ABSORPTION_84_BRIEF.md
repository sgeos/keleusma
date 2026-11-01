# Absorption 84

Computed against native `db3d8a51` and upstream `d7461857` on 2026-10-24,
with 4 unabsorbed commits measured after fetching. The merge base is
`f213f421`. The separate conflict command
`git merge-tree --write-tree origin/v0.3.0 origin/v0.2.3` exited zero and
produced tree `f0772e12c89d1abdf3333ee1c4f9d9b7e08cdcf9`. The merge was
prepared on `feat/v3-absorption-84` with `--no-ff --no-commit` so the
absorption and handoff could share the prompt's single commit.

A first conflict check in this session ran against a stale `origin/v0.3.0`
that still named `99023f8f`, because the fetch preceded the merge of pull
request 492. It reported 15 commits and 42 changed files. That measurement
described no current state and was discarded. The figures above come from the
fetch made after the merge.

## What was absorbed

Two Kaizen pull requests, both build and test infrastructure. Neither changes
root `src/`, a stage source, an opcode or the bytecode version.

- Pull request 493, merged as `b2ccc099`, sets `opt-level = 1` in
  `[profile.dev]` of the root `Cargo.toml` and of the detached
  `compiler/Cargo.toml`. Debug assertions and overflow checks stay on.
- Pull request 494, merged as `d7461857`, collapses the workspace CI test job
  into one unpartitioned job named `Test` that excludes the `selfhost_*`
  binaries of `keleusma`. The four-shard self-host job is now the only place the
  heavy tier runs. `tests/ci_shard_coverage.rs` pins the filter and fails if the
  self-host job gains one. The everyday command in `CLAUDE.md` now ends in
  `cargo tnq`, and `scripts/fast-check.sh` now enables `self-host`.

The changed files are `.github/workflows/ci.yml`, `CLAUDE.md`, `Cargo.toml`,
`compiler/Cargo.toml`, `docs/process/GIT_STRATEGY.md`, `scripts/fast-check.sh`
and `tests/ci_shard_coverage.rs`. `CLAUDE.md` auto-merged without conflict.

## Effect on this line

Inspected, not inferred from the notice. `.github/workflows/native.yml` names
no root CI job, and no `native_codegen` test reads `ci.yml` or
`tests/ci_shard_coverage.rs`. The removed `Test (shard k/4)` names therefore
break nothing on this line. The `partition` matches in native tests concern the
opcode partition, not CI partitioning.

`native_codegen/Cargo.toml` declares its own `[workspace]` and
`[profile.dev]`, so the root `opt-level = 1` does not reach the native package.
Applying the same setting there is a separate decision, not part of this
absorption. The native corpus fingerprint is unaffected because no stage source
changed.

## Verification

Run locally with `RUSTC_WRAPPER=` on the staged merge. Logs are under ignored
`tmp/absorption-84/`.

| Check | Result |
|---|---|
| Root `cargo test --locked --no-fail-fast` with `--test ci_shard_coverage --test gate_ci_correspondence --test documentation_links --test comment_citations` | 16 passed, exit zero |
| Native `cargo test --locked --no-fail-fast` in `native_codegen/` with `KEL_REQUIRE_LLVM_TOOLS=1` and `--test comment_citations --test handoff_figures --test test_population_guard --test gate_record --test shared_channel_discipline`, first run | 22 passed, 2 failed, exit 101 |
| Cause of those failures | `handoff_figures` found the archive banner at absorption 84 while its state table still said 83, and the resume stamp no longer had the `Covers the tree at` form. Both were refreshed |
| Same native command, rerun | 24 passed, exit zero |

The first native run was launched from a subshell whose last command was an
`echo`, so the background task reported exit zero. The log's own captured
status read 101. The figures above are read from the logs' summary lines and
captured statuses, not from the launcher. A first root run also passed, but
the handoff files were checked out while it was in flight, so it was discarded
and repeated on the final tree.

These are targeted checks in the default feature configuration. They do not run
the self-host tier, the other four root feature sets or either complete native
gate. The hosted CI run on the absorption pull request is the version-branch
integration check. No new full-gate record was written. Publication is held and
no merge into `main` is authorized.

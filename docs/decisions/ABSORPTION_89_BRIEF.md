# Absorption 89

Computed against native `1de44fa0` and upstream `1b8e9ea4` on 2026-10-24, with
5 unabsorbed commits measured after fetching. The merge base is `ea6f3c8c`. The
only conflict was `docs/process/interline/checkpoint.json`, regenerated from the
live store with 46 messages. The merge was prepared on `feat/v3-absorption-89`,
cut from `feat/v3-absorption-88`, pull request 513, so 513 merges first.

## What was absorbed

- Kaizen pull request 511, `82bc87eb`. `PilotTests` builds its fixture
  repository in the system temporary directory, so it no longer fails in a
  sandbox that denies writes to a new repository's configuration. This line
  reported that failure in absorption 87.
- Kaizen pull request 508, `1b8e9ea4`. `scripts/release-gate.sh` filters the
  self-host binaries out of its workspace step, as CI does since pull request
  494, runs documentation tests separately and requires `cargo-nextest`. Its
  header now calls the gate a subset of CI.

`scripts/release-gate.sh` auto-merged. This line's detached `native_codegen/`
step is unchanged in the result, with the same six references. No Rust source,
Rust test or workflow changed.

## Verification

Run locally with `RUSTC_WRAPPER=` on the staged merge.

| Check | Result |
|---|---|
| Root `documentation_links`, `comment_citations`, `gate_ci_correspondence` and `ci_shard_coverage` | 16 passed, status zero |
| Eleven native document-reading binaries with `KEL_REQUIRE_LLVM_TOOLS=1` | 51 passed, status zero |
| `python3 -B -m unittest discover -s scripts -p 'test_*.py'`, inside this session's sandbox | 68 passed, none skipped, status zero |

The last row is the fix pull request 511 claims: the same command failed in the
sandbox at absorption 87. Hosted CI is the integration check. Publication
is held and no merge into `main` is authorized.

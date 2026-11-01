# Absorption 90

Computed against native `6c3a382c` and upstream `ec536b28` on 2026-10-24, with
3 unabsorbed commits measured after fetching. The merge was prepared on
`feat/v3-absorption-90`, cut from `feat/v3-absorption-89`, pull request 514.

## What was absorbed

Kaizen pull request 512, merged upstream as `ec536b28`. It trims `CLAUDE.md` to
about 270 lines and moves its two verification catalogues verbatim to
`docs/process/VERIFICATION_PITFALLS.md`. Kaizen and V0.2.X both asked that this
line's routing paragraph survive the absorption.

## Conflict resolution

`CLAUDE.md` conflicted in two hunks.

- **Startup.** Upstream's trimmed text is taken, and the paragraph headed
  "V0.3.X routing takes precedence over the shared protocol below" is restored
  immediately above "Session startup protocol", where it stood. It is the only
  difference between this line's `CLAUDE.md` and upstream's.
- **Catalogues.** Upstream's two correctives are taken. This line's copy of the
  first catalogue carried rows the moved file lacks: five rows, from the targeted
  run selected by binary name to the launcher's exit status, and a longer form
  of the truncated-log row. They are added to
  `docs/process/VERIFICATION_PITFALLS.md` with a sentence saying where they came
  from. Upstream's longer cached-clippy row is kept. Nothing is dropped.

`AGENTS.md` auto-merged and keeps this line's section on the two development
lines.

## Verification

Run locally with `RUSTC_WRAPPER=` on the staged merge.

| Check | Result |
|---|---|
| Root `documentation_links`, `comment_citations`, `gate_ci_correspondence`, `ci_shard_coverage`, `claimed_counts` and `release_process_crate_list`, the binaries that read `CLAUDE.md` or the gate | 36 passed, status zero |
| Eleven native document-reading binaries with `KEL_REQUIRE_LLVM_TOOLS=1` | 51 passed, status zero |

`tests/selfhost_codegen.rs` names `CLAUDE.md` only in a comment and was not run
locally. Hosted CI is the integration check. Publication
is held and no merge into `main` is authorized.

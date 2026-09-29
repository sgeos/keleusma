# Codex development-line readiness audit

Measured 2026-09-28. This is a setup and handoff audit, not a release certification.

## Decision

The Codex installation can perform development on this machine. The V0.2.X handoff is valid under its stated checks and has no demonstrated missing runtime prerequisite. V0.3.X is accessible for investigation but does not have a verified green handoff at the audited tip. Its inherited workflow failure and stale native gate results must be resolved before treating it as green.

The audited starting commits were `f4090bbc3047643b299b3b8985f80c7b8fb6e4ab` on `v0.2.3` and `7ebcd1e89c9bc25fcb5f7ffd2135677e638be312` on `v0.3.0`. Both working trees were clean and both tips matched a live `git ls-remote origin` query. No pull requests were open in that query. Subsequent audit documentation changes are outside these baseline measurements.

## Confirmed setup

- Codex command-line interface 0.158.0 passed all 21 `codex --strict-config doctor --json` checks when run outside the execution sandbox. Authentication, provider connectivity and local database integrity passed. The initial sandboxed diagnostic failed on network access and opening the memories database. These failures did not reproduce outside the sandbox, so they do not establish database corruption or broken account access.
- Global Codex instructions match the global Claude instructions apart from the generated-file notice. No global override exists. Each line has its own root `AGENTS.md`. Global plus root instruction sizes were 11,422 and 11,533 bytes, below the [documented default 32 KiB discovery limit](https://learn.chatgpt.com/docs/agent-configuration/agents-md). `CLAUDE.md` remains an explicit read through the repository instruction, not a second automatically loaded file from the same directory.
- Git author identity resolves and GitHub authentication succeeds. Remote reads and the earlier status-line feedback submission establish working GitHub access. No push, merge or publication was performed during this audit.
- Rust 1.98.1, Cargo, Clippy, rustfmt, nextest 0.9.138, cargo-audit 0.22.1, ShellCheck, GitHub CLI, jq, mdBook, Node and wasm-pack are installed. Nightly has Miri. The pinned Rust 1.92 toolchain has Clippy and the declared embedded target.
- LLVM 22.1.8 is installed at the location specified by the native package configuration. Its absence from the ordinary executable search path does not prevent that package from building and executing native differential tests.
- No configured Claude lifecycle hooks were found in the inspected global and project settings. The Claude status renderer is a separate integration. The shared Git pre-push hook exists and remains applicable regardless of the agent harness.

## Required attention

### 1. V0.3.X has a reproducible workflow failure

The workflow at the V0.3.0 tip already had failed no-default-features and signatures jobs when inspected. Its preceding workflow failed all five root test configurations with the same test name. A local reproduction on the current tip returned one pass and one failure.

```sh
cargo test --test gate_ci_correspondence --no-default-features
```

The failing test is `nothing_claims_the_gate_is_a_superset_or_that_ci_skips_the_subproject` in `tests/gate_ci_correspondence.rs`. It searches an entire file for a forbidden phrase. On this branch the matching label belongs to the detached `native_codegen/` gate, but the diagnostic refers to the separate `compiler/` package and its `selfhost-compiler` workflow job. No `native_codegen` reference was found in this branch's `ci.yml`.

Inference from the source and reproduction is that this is a package-scoping defect in the documentation guard, not evidence of incorrect native lowering. It still makes the workflow red. Correct the guard on the V0.2.X line that owns root tests, then absorb the repair into V0.3.X. Do not remove the native verification requirement or relabel native checks as covered by the root workflow merely to satisfy the text scan.

Evidence includes [the preceding failed workflow](https://github.com/sgeos/keleusma/actions/runs/36448666122) and [the current workflow](https://github.com/sgeos/keleusma/actions/runs/36461733977). The current workflow subsequently completed with failure in all five root test configurations. Any repair still requires successful checks on its new commit before a merge decision.

### 2. V0.3.X gate records do not cover the current tree

Both native gate rows record a clean, frozen pass at `cf52d29b0e3ea9a0f22c7f594241e122c167fd04`. The package's own `tools/gate-status.sh` reports two changed inputs since that measurement, `docs/decisions/WORKSTREAM_B_FIRST_INCREMENT.md` and `docs/process/handoffs/v0.3.0.md`. Documentation is an input to this suite.

The reporter exits zero even while printing `UNVERIFIED`. Read its reported verdict, not merely the shell exit status. The current state-table claim that the record is green must not substitute for this freshness check.

Five commits from the observed `origin/v0.2.3` were not yet absorbed. This includes the latest agent-facing instruction reconciliation. Run the documented absorption preparation, incorporate the required changes, then run `native_codegen/tools/backend-gate.sh` and its `--narrow` variant separately on a stationary tree. Neither full gate was rerun in this diagnostic audit.

### 3. A fresh V0.3.X session needs the correct workspace and private prerequisite

Launch its Codex session in the `arena-composites` worktree and begin from `docs/process/handoffs/v0.3.0-BRIEF.md`. This session's writable workspace covers the primary checkout, not that sibling worktree or the parent directory used by the worktree-creation helper. Reading is possible here, but native build writes required explicit execution escalation. A separate session rooted there is the appropriate normal development context.

Appendix B was readable in the primary checkout and was read for this audit. It is absent from the V0.3.X worktree. Make that prerequisite privately available to the next session before tracked edits. Its contents must remain outside tracked documentation. Absorbing tracked commits alone cannot restore an untracked prerequisite.

### 4. Compilation requires a deliberate sandbox workflow

The global Cargo configuration selects `sccache`. The unmodified `cargo nextest run --workspace` failed immediately inside this session's sandbox with an operation-not-permitted error from that wrapper. The approved execution outside the sandbox compiled and ran tests.

The installation is usable with approved build commands. Do not interpret these denials as Rust failures, and do not disable the sandbox globally merely to avoid them. A future decision to use a cache-free sandboxed build should be tested independently before it is documented as working. It was not tested here.

### 5. Instruction and memory reconciliation remains incomplete

Both lines' `CLAUDE.md` startup sections correctly defer to the handoff's own ancestry and content checks, while their compaction sections still prescribe comparing a stamp with `HEAD~1`. A future compacted session can therefore receive contradictory instructions. Reconcile that paragraph before relying on unattended continuation.

The cross-line channel rules also need reconciliation. `PARALLEL_DEVELOPMENT.md` describes the root process channels as single-writer, while the native `shared_channel_discipline` test explicitly preserves a V0.3.X addendum in the reverse prompt. During absorption, retain that addendum and its outstanding reports. Do not treat a blanket rewrite as harmless.

Both copies also prescribe a Claude coauthor trailer for all assisted commits. Update the attribution convention to name the actual assistant. The V0.3.X `AGENTS.md` still contains the old generic no-commit rule alongside a line-specific exception. Absorbing upstream must preserve its valuable native-line routing while incorporating the reconciliation.

At the baseline, the primary handoff was about 140 KB, the reverse prompt about 200 KB and the task log about 532 KB. Use bounded current-state sections and targeted reads. A truncated whole-file tool result does not establish that the complete file was read. Preserve the V0.3.X brief as its entry point.

Claude's project memory is not part of the inspected Codex instruction chain. Selected entries were reviewed as audit evidence. They include operational authorizations and known gate behavior, but also a stale explanation that attributes virtual-machine behavior to an archive path that the current repository says is retired. Do not copy that memory wholesale. Preserve current operator rulings with provenance and verify technical claims against the tree.

## Optional tooling and parity gaps

Rust 1.85 and 1.88, the two minimum-supported compiler versions exercised in continuous integration, are not installed locally. This does not prevent normal development, but local reproduction of those jobs will require those toolchains. The stable toolchain lacks `wasm32-unknown-unknown`, although Rust 1.92 has it. A stable playground build may therefore need that target installed.

The default `rust-analyzer` launcher reports that the component is absent. Claude has language-server plugins enabled, but no equivalent language-server tool is available in this Codex session. Cargo-based checks work without it. Treat editor or language-server integration as optional setup rather than evidence of a broken Rust installation.

The custom project status segment and selectable used quota percentages remain unsupported by the configured Codex footer. This is a visibility gap, not a development blocker. The existing project status scripts can still be invoked explicitly. The [maintainer feedback](https://github.com/openai/codex/issues/17827#issuecomment-5875971013) records these limitations.

## Verification and limits

`cargo nextest run --workspace` passed 2,930 tests, skipped two and exited zero. The runner reported 2,045.697 seconds of test execution. All five content checks in the V0.2.X handoff therefore passed on the baseline tree. Its handoff is valid. The V0.3.X handoff is invalid-and-stale as a claim of current green status because its gate evidence no longer covers the tree and its root workflow fails.

- V0.2.X handoff stamp is two commits behind the audited tip. Its ancestry check passes, the fingerprint is `0x4327_63E1`, the stage-source count is 12, and the specified stage diff is empty.
- `cargo audit` completed successfully after updating its advisory data and scanning 102 dependencies. This is a dependency-advisory result, not a general security assessment.
- `shellcheck scripts/*.sh` passed. The pinned real-time operating system example host suite passed six tests.
- V0.3.X handoff stamp is two commits behind its audited tip. All 150 ancestry anchors extracted from its primary validity block passed.
- Native `differential`, `gate_record`, `handoff_figures`, `kind_arm_census`, `prediction_stamp` and `reach_census` passed 80 tests in each float configuration. These checks establish toolchain operation and selected handoff invariants. They do not replace either complete backend gate.
- The V0.2.3 tip has successful [continuous integration](https://github.com/sgeos/keleusma/actions/runs/36450164249) and book workflows. Root verification does not certify the detached native backend.
- The documentation-link and comment-citation checks passed eight tests after the audit records were written.
- The complete local release gate, all five local feature configurations, Miri, minimum-supported-version builds, hardware execution and publication were not performed. No new security, semantics, opcode or bytecode-version approval is implied.

Raw diagnostic output and captured command exit statuses are retained locally under the ignored `tmp/codex-setup-audit/` directory. They are not portable audit artifacts. The commands, commit stamps and public workflow links above provide reproducible anchors.

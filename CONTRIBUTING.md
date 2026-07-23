# Contributing to Keleusma

Contributions are welcome. Issues, pull requests, and documentation patches are all in scope. This document describes the workflow conventions; the architectural rationale and language-design reference live under [`docs/`](docs/README.md).

## Quick start

```sh
git clone https://github.com/sgeos/keleusma
cd keleusma
cargo install cargo-nextest --locked   # recommended: parallel test runner
cargo nextest run --workspace          # or: cargo test --workspace
```

The keleusma library carries roughly 1,150 unit tests plus two dozen integration-test binaries. See [`docs/process/PROCESS_STRATEGY.md`](docs/process/PROCESS_STRATEGY.md) for the broader development process and [`docs/process/GIT_STRATEGY.md`](docs/process/GIT_STRATEGY.md) for the branching model.

## Fast local iteration

The full gate (every crate, every feature set, clippy, fmt, doc) is correct but heavy. For the inner edit-compile-test loop, use the tiered [`cargo` aliases](.cargo/config.toml), fastest to slowest:

```sh
cargo qc     # type-check the whole workspace, no codegen (compile-error feedback)
cargo tl     # run only the keleusma library unit tests (seconds after compile)
cargo lint   # the exact clippy invocation CI gates on
cargo tn     # the full workspace suite under nextest (parallel; needs cargo-nextest)
cargo tw     # the full workspace suite under cargo's serial runner
```

Install [`cargo-nextest`](https://nexte.st) (`cargo install cargo-nextest --locked`) to run the integration-test binaries in parallel rather than one at a time; the workspace has two dozen of them. nextest does not run doc-tests, so the full check pairs `cargo nextest run --workspace` with `cargo test --workspace --doc`. Reserve the full gate below for before opening a pull request; the pre-push hook runs it automatically.

For the self-hosted-compiler inner loop, [`scripts/fast-check.sh`](scripts/fast-check.sh) bundles fmt, clippy on the touched crate, and a test filter you pass in — the one construct or the one changed self-host stage in flight — so you get a seconds-scale signal without the full gate:

```sh
scripts/fast-check.sh 'test(self_host_compiles_word_bnot)'                  # one construct
scripts/fast-check.sh 'test(self_host_compiles_parse_kel_byte_identically)' # only the changed stage
```

Re-running only the changed stage's self-compile (rather than all five) is the fast form; the full five-stage self-compile stays in the pre-push hook and the merge gate. Deliberately no stage-bytecode memoization: a stale cache could mask a real byte-identity divergence, the worst failure for a differential oracle.

The default nextest profile caps the run at four concurrent test processes (`test-threads` in [`.config/nextest.toml`](.config/nextest.toml)). nextest runs one process per test, so the full suite would otherwise spawn many at once; on a memory-constrained host the combined footprint plus parallel compilation can exhaust RAM and swap and wedge the run. Four processes keep peak memory modest while still finishing the library suite in a few seconds. The `ci` profile omits the cap because the CI runners have ample memory.

## Branching and commits

The project follows trunk-based development. Short-lived feature branches fast-forward into `main`; force-pushes to `main` are blocked by branch protection.

```sh
git checkout main
git pull --rebase
git checkout -b feat-my-change
# work, commit, push branch
git checkout main
git merge --ff-only feat-my-change
git push origin main
git branch -d feat-my-change
```

Commits follow the Conventional Commits convention with a scope. Common scopes are `feat`, `fix`, `docs`, `refactor`, `chore`, `test`. Examples:

- `feat: add information-flow labels`
- `fix(vm): correct off-by-one in operand-stack reset`
- `docs(guide): clarify hot-swap discipline`

When AI assistance contributed, add the `Co-Authored-By` trailer.

## Before opening a pull request

```sh
cargo nextest run --workspace   # parallel; or: cargo test --workspace
cargo test --workspace --doc    # nextest does not run doc-tests
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings -A rustdoc::redundant-explicit-links" \
    cargo doc -p keleusma --no-deps --features signatures,encryption,shell
cargo run -q -p keleusma-cli -- run scripts/check-md-links.kel
```

All of these must pass. The last verifies that relative links between
Markdown files resolve to existing files. It is itself a Keleusma
script, [`scripts/check-md-links.kel`](scripts/check-md-links.kel), run
through the CLI; it orchestrates POSIX tools through `shell::run` and
drives its exit code from their result. The earlier pure-POSIX
implementation remains in the git history at `scripts/check-md-links.sh`. CI runs the same set plus per-feature variants, no_std targets, Miri on the arena crate, and the bare-metal STM32N6570-DK build through `examples/rtos`. The doc command uses the docs.rs feature set; the `encryption` feature carries the `crate::encryption` module that the wire-format documentation links to, so omitting it produces an unresolved-link failure.

The commands above and the pre-push hook run only the **default** feature set of
the root workspace. They do not exercise the configurations where feature-gated
code diverges — `--no-default-features`, the `signatures` and broad
`signatures,shell` sets — nor the detached `compiler/` self-hosting subproject,
which is a separate workspace gated by neither the hook nor the root CI job. That
gap is how a `--no-default-features` compile break or a subproject regression
reaches CI unseen. Run [`scripts/verify.sh`](scripts/verify.sh) to close it: it
mirrors CI's runnable feature matrix plus the subproject's build, tests, clippy,
and fmt, and reports every failure in one pass. It deliberately does not use
`--all-features` (unsuitable for this workspace, as the CI "broad features" job
documents — it selects mutually-degenerate narrow-word widths), and it skips the
toolchain- or target-specific jobs (Miri, MSRV, cross-builds, WASM) with a note
when the required toolchain is absent. Run it before pushing changes that touch
feature-gated code or the subproject.

## Automated pre-push hook

These checks also run automatically before every `git push` through a [cargo-husky](https://github.com/rhysd/cargo-husky) pre-push hook. The hook is version-controlled at [`.cargo-husky/hooks/pre-push`](.cargo-husky/hooks/pre-push) and cargo-husky installs it into `.git/hooks/` the next time the dev-dependencies compile, for example on the first `cargo test --workspace` after cloning. No manual setup step is required. The hook runs the test step under `cargo nextest` (parallel) when it is installed and falls back to `cargo test --workspace` otherwise, so installing `cargo-nextest` makes the push gate substantially faster.

The hook fails the push if any check fails. To push a work-in-progress branch without running it, bypass with `git push --no-verify`. If the CI feature set or the checklist above changes, update the hook script to match.

If your change is user-visible, add an entry to [`CHANGELOG.md`](CHANGELOG.md) under `[Unreleased]`.

If your change touches the surface language (new keyword, new operator, new construct), update the relevant section of [`docs/spec/GRAMMAR.md`](docs/spec/GRAMMAR.md) and consider whether the syntax highlighters under [`editors/`](editors/README.md) need to learn the new construct.

## Filing an issue

[`.github/ISSUE_TEMPLATE/`](.github/ISSUE_TEMPLATE/) holds the bug-report and feature-request templates. The bug report asks for a minimal reproducible Keleusma script plus the host code that exercises it; the feature request asks for the motivating use case and an indication of which static guarantee (totality, productivity, bounded-step, bounded-memory, safe-swap) the feature should preserve.

## Verifier rejections

If your program lexes and parses but `Vm::new` or `compile` rejects it, the rejection is intentional under the conservative-verification stance. Read [`book/src/WHY_REJECTED.md`](book/src/WHY_REJECTED.md) for the rejection taxonomy and proposed rewrites. If you believe the rejection is a verifier bug rather than a design choice, file a bug report with the rejecting program and the diagnostic message.

## Patches that touch verifier soundness

Soundness is the load-bearing property of the language. Any change to `verify::wcet_stream_iteration`, `verify::wcmu_stream_iteration`, `verify::verify`, or the surrounding analysis machinery should:

- Carry tests in the affected module that exercise the soundness boundary.
- Be paired with a CHANGELOG entry that names the soundness invariant the change preserves or relaxes.
- Be reviewed against the rationale in [`docs/architecture/LANGUAGE_DESIGN.md`](docs/architecture/LANGUAGE_DESIGN.md#conservative-verification) for the conservative-verification stance.

## Code of conduct

Interactions on issues, pull requests, and other project communication channels are expected to be civil and focused on the technical substance. There is no formal code-of-conduct document at V0.2.0; the project will adopt one if and when external contributor activity grows to a point where a written policy is useful.

## License

By contributing, you agree that your contributions are licensed under the BSD Zero Clause License (`0BSD`) — the same license as the rest of the project. See [`LICENSE`](LICENSE).

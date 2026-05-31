# Contributing to Keleusma

Contributions are welcome. Issues, pull requests, and documentation patches are all in scope. This document describes the workflow conventions; the architectural rationale and language-design reference live under [`docs/`](docs/README.md).

## Quick start

```sh
git clone https://github.com/sgeos/keleusma
cd keleusma
cargo test --workspace
```

Approximately 826 library tests run in around twenty seconds on a modern laptop. See [`docs/process/PROCESS_STRATEGY.md`](docs/process/PROCESS_STRATEGY.md) for the broader development process and [`docs/process/GIT_STRATEGY.md`](docs/process/GIT_STRATEGY.md) for the branching model.

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
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings -A rustdoc::redundant-explicit-links" \
    cargo doc -p keleusma --no-deps --features signatures,encryption,shell
cargo run -q -p keleusma-cli -- run scripts/check-md-links.kel
```

All five must pass. The last verifies that relative links between
Markdown files resolve to existing files. It is itself a Keleusma
script, [`scripts/check-md-links.kel`](scripts/check-md-links.kel), run
through the CLI; it orchestrates POSIX tools through `shell::run` and
drives its exit code from their result. The earlier pure-POSIX
implementation remains in the git history at `scripts/check-md-links.sh`. CI runs the same set plus per-feature variants, no_std targets, Miri on the arena crate, and the bare-metal STM32N6570-DK build through `examples/rtos`. The doc command uses the docs.rs feature set; the `encryption` feature carries the `crate::encryption` module that the wire-format documentation links to, so omitting it produces an unresolved-link failure.

## Automated pre-push hook

These five checks also run automatically before every `git push` through a [cargo-husky](https://github.com/rhysd/cargo-husky) pre-push hook. The hook is version-controlled at [`.cargo-husky/hooks/pre-push`](.cargo-husky/hooks/pre-push) and cargo-husky installs it into `.git/hooks/` the next time the dev-dependencies compile, for example on the first `cargo test --workspace` after cloning. No manual setup step is required.

The hook fails the push if any check fails. To push a work-in-progress branch without running it, bypass with `git push --no-verify`. If the CI feature set or the checklist above changes, update the hook script to match.

If your change is user-visible, add an entry to [`CHANGELOG.md`](CHANGELOG.md) under `[Unreleased]`.

If your change touches the surface language (new keyword, new operator, new construct), update the relevant section of [`docs/spec/GRAMMAR.md`](docs/spec/GRAMMAR.md) and consider whether the syntax highlighters under [`editors/`](editors/README.md) need to learn the new construct.

## Filing an issue

[`.github/ISSUE_TEMPLATE/`](.github/ISSUE_TEMPLATE/) holds the bug-report and feature-request templates. The bug report asks for a minimal reproducible Keleusma script plus the host code that exercises it; the feature request asks for the motivating use case and an indication of which static guarantee (totality, productivity, bounded-step, bounded-memory, safe-swap) the feature should preserve.

## Verifier rejections

If your program lexes and parses but `Vm::new` or `compile` rejects it, the rejection is intentional under the conservative-verification stance. Read [`docs/guide/WHY_REJECTED.md`](docs/guide/WHY_REJECTED.md) for the rejection taxonomy and proposed rewrites. If you believe the rejection is a verifier bug rather than a design choice, file a bug report with the rejecting program and the diagnostic message.

## Patches that touch verifier soundness

Soundness is the load-bearing property of the language. Any change to `verify::wcet_stream_iteration`, `verify::wcmu_stream_iteration`, `verify::verify`, or the surrounding analysis machinery should:

- Carry tests in the affected module that exercise the soundness boundary.
- Be paired with a CHANGELOG entry that names the soundness invariant the change preserves or relaxes.
- Be reviewed against the rationale in [`docs/architecture/LANGUAGE_DESIGN.md`](docs/architecture/LANGUAGE_DESIGN.md#conservative-verification) for the conservative-verification stance.

## Code of conduct

Interactions on issues, pull requests, and other project communication channels are expected to be civil and focused on the technical substance. There is no formal code-of-conduct document at V0.2.0; the project will adopt one if and when external contributor activity grows to a point where a written policy is useful.

## License

By contributing, you agree that your contributions are licensed under the BSD Zero Clause License (`0BSD`) — the same license as the rest of the project. See [`LICENSE`](LICENSE).

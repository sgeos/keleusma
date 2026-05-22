<!--
Pull request template for Keleusma.

Replace this comment block with a short summary of what the
change does and why. Reference any related issue (`Fixes #N` or
`Closes #N`) to auto-link.
-->

## Summary

<!-- One or two sentences. -->

## Type of change

- [ ] Bug fix
- [ ] New feature
- [ ] Documentation
- [ ] Refactor
- [ ] Test
- [ ] Chore (CI, build, tooling)

## Pre-merge checklist

- [ ] `cargo test --workspace` passes locally
- [ ] `cargo fmt --all -- --check` clean
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] `RUSTDOCFLAGS="-D warnings -A rustdoc::redundant-explicit-links" cargo doc -p keleusma --no-deps --features signatures,shell` clean
- [ ] `CHANGELOG.md` updated under `[Unreleased]` if the change is user-visible
- [ ] If the change touches the surface language, `docs/spec/GRAMMAR.md` and the editor syntax files under `editors/` are updated
- [ ] If the change touches verifier soundness, the rationale is recorded in the PR description and the CHANGELOG entry names the soundness invariant preserved or relaxed

## Notes for the reviewer

<!--
Anything the reviewer should pay particular attention to:
non-obvious design decisions, alternate approaches considered
but rejected, areas where you would like a second opinion, etc.
-->

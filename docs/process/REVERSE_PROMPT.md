# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

## Current state

2026-09-29. V0.2.X work remains on `feat/v02-order1-next`, following roadmap Order 1.
The preceding capacity increment is `3fefccc0`. Its six 640-row caps remain in place.

`verify_types.kel` now compiles through the Keleusma pipeline. Its private solver data block
precedes the first function reading it. Data-block order and function order are preserved.
The new byte-identity test first failed at `ty_direct`, then passed after the move. Both the
existing test pipeline and `self_host_compile_scratch` produce the same complete serialized
artifact as the reference. The scratch driver builds every module field from pipeline output.
All twelve real stage sources now have byte-identity tests.

A mutation restoring the former declaration order leaves the reference artifact unchanged and
reproduces the pipeline refusal. General forward data references remain unsupported. This change
does not move type-input extraction out of the reference parser's abstract syntax tree.

## Verification

All commands below exited successfully. Cargo test and Clippy invocations used `RUSTC_WRAPPER=`. The new artifact and
selected stage tests used `KEL_SELFHOST_CACHE=0`.

- `cargo test --features self-host --test selfhost_codegen self_host_compiles_verify_types_kel_byte_identically -- --exact` passed one test, including the scratch artifact comparison.
- `cargo test --test selfhost_codegen self_host_compiles_verify_types_kel_byte_identically -- --exact` passed one test with default features.
- `cargo test --features self-host --test forward_data_reference` passed five tests.
- `cargo test --features self-host --test selfhost_typecheck the_stage_` passed five tests, including the rejection corpus and acceptance of all twelve real sources.
- `cargo test --test documentation_links --test comment_citations --test claimed_counts` passed 21 tests.
- `cargo fmt --check`, `git diff --check`, and `cargo clippy --tests --features signatures,shell,self-host -- -D warnings` passed.

The full workspace suite and fresh network advisory scan remain unverified. The previous
workspace run was interrupted after 2,009 passes, with four tests interrupted and 920 not run.
The previous no-fetch advisory scan exited successfully against a local database dated
2026-09-28. The old handoff remains invalid-and-stale as recorded in the previous increment.

## Next Order-1 work

Complete the type-input path and production integration before advancing to Order 2.
`src/selfhost/mod.rs` already provides four partial extraction helpers and
`expression_rows_from_pipeline` covers four of eight expression kinds. Read their documented
residuals and the differential tests in `tests/selfhost_typecheck.rs` before choosing the next
slice. The expression records discard explicit-else presence and struct-literal identity.
Projection errors can be refused during reconstruction before an expression row exists.
These distinct cases need separate treatment to preserve acceptance and rejection behavior.

The V0.3.X line remains concurrent in its own worktree. Bytecode version and opcode count remain
subject to the operator constraints in `CLAUDE.md`.

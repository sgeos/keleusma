# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-23
**Status**: Backlog grooming session. B29 finalised with Shape B as the chosen debug-pool format. B30 and B31 filed as durable consolidations of the CLI runner and run-tasks deferrals previously held only in REVERSE_PROMPT and the RUN_TASKS design doc. B28 phased implementation plan agreed; P0 scope confirmed. No code changes in this session; documentation only.

## Summary of work since the last reverse-prompt update

A single docs commit folds three coherent updates to `docs/decisions/BACKLOG.md`.

### B29 refinement

B29 ("Strippable debug opcodes in the ISA") was revised in three ways. First, the entry now explicitly names Shape B (chunk-local `debug_pool: Option<Vec<u8>>` field as a new optional length-prefixed per-chunk wire-format section) as the chosen format, with a rejection-rationale table covering Shape A (opcode-introduced inline pool, violates the highlighter-and-addendum metaphor by putting addendum bytes inside the paper), Shape C (module-level pool, wrong scope), and Shape D (reuse existing constants pool, requires constant renumbering that breaks the symmetric add/subtract invariant). Second, the design metaphor was added explicitly: debug opcodes are highlighter and annotations on the paper, the addendum is a separate sheet, and `keleusma strip` removes both cleanly without modifying the paper. Third, a fourth invariant was added stating that debug content adds to and subtracts from the release format cleanly with no non-debug byte changes in either direction. The "Compatibility" section that previously said `BYTECODE_VERSION` advances by one was corrected to match the operator decision that the version stays at 1.

### B30 filing

`docs/decisions/BACKLOG.md` gains B30 "CLI runner deferred work" consolidating the three broader CLI deferrals previously held only in `docs/process/REVERSE_PROMPT.md` (which is overwritten each session). Items: 1 mutable shared/private data REPL persistence beyond scalars (forcing case B28 lands), 2 generic `Result<T, E>` type (language-design question deferred deliberately), 3 `shell::read_lines` native (contingent on dynamic-length Array type). Each carries a forcing-case row.

### B31 filing

`docs/decisions/BACKLOG.md` gains B31 "run-tasks deferred work" consolidating the ten items from `docs/architecture/RUN_TASKS.md` section "Open questions and future work". Items: 1 manifest signing, 2 per-task isolation through OS primitives, 3 dynamic task addition, 4 hot reload via SIGHUP, 5 priority levels and preemption (intentionally excluded by design), 6 soft resource caps beyond WCMU, 7 typed event payloads, 8 task-to-task ABI compatibility checking, 9 native Windows SCM integration, 10 notification-protocol conventions on non-systemd supervisors. Each carries a forcing-case row.

## Verification

- `cargo test --workspace`: not re-run (documentation-only change).
- `cargo clippy --workspace --tests -- -D warnings`: not re-run (documentation-only change).
- `cargo fmt --all`: not re-run (documentation-only change).
- BACKLOG.md grew from 1238 lines to roughly 1320 lines through the two new entries plus B29 in-place refinement.
- No code paths were touched.

## Open questions

None at the documentation layer. The operator-decision items are forward-looking.

## Recommended next step

The operator confirmed the B28 phased implementation plan and the P0 scope. P0 introduces the layout descriptor and the flat-bytes representation as parallel infrastructure with no behaviour change yet. Concretely:

1. New module `src/value_layout.rs` defining `LayoutDescriptor` and the flat-bytes serialisation/deserialisation helpers for fixed-size primitive types (`Int`, `Byte`, `Bool`, `Fixed`, `Float`, `Unit`, `None`).
2. New module `src/flat_value.rs` defining the `FlatComposite` representation as a `Vec<u8>` plus a `LayoutDescriptor` reference.
3. Unit tests that round-trip every primitive type through the flat-bytes helpers.
4. No changes to `GenericValue`, op handlers, the verifier, or the compiler.
5. All existing 826 lib tests plus the workspace-level suites remain green.

P1 through P8 follow the phased plan agreed in this session. Phases P1 through P4 migrate tuples, arrays, structs, and enums in turn; P5 adds the `DataSlotAnnotation` opcode and the chunk-local `debug_pool` field; P6 corrects WCMU accounting; P7 updates the hot-code-swap migration path; P8 closes the documentation pass.

The wire-format extension is debug-only: the new strippable debug opcodes and the chunk-local `debug_pool` field. Release-format bytecode framing is unchanged. `BYTECODE_VERSION` stays at 1.

## Reference

- `docs/decisions/BACKLOG.md` B28, B29, B30, B31.
- `docs/architecture/RUN_TASKS.md` is the design doc that originated the items now in B31.

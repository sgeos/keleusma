# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning and frontier assessments live in
[DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-07-29 (session 35)

## Current state

- **The `--compiler self-hosted` backend error surface is HARDENED. COMPLETE, full gate GREEN, merged to
  `v0.2.3`.** The operator selected "harden the CLI backend" at the post-compaction fork.
- One of the two candidate sub-items was NOT pursued because it is a hard boundary rather than an
  oversight. Threading the CLI preamble through self-hosted mode is impossible in this fork. The
  self-hosted codegen emits no native-call opcode. Its emitted wire set is `decode_op` tags 1..=63,
  which carry `Op::Call` but neither `CallExternalNative` nor `CallVerifiedNative`, and the CLI preamble
  is entirely native `use` signatures. Native-call emission is a language-surface increment under a
  different fork.
- Delivered, the self-contained sub-item, richer subset-rejection errors, plus a correctness fix.
  `SelfHostError` gained `ReferenceRejected { detail }` for a genuine source error the reference compiler
  also rejects, distinct from a self-hosted-subset `Unsupported`. `rust_backend_would_help(&self)` is
  false only for `ReferenceRejected`. The CLI now appends the `retry with --compiler rust` hint only when
  it would help, and reports a genuine source error plainly. A new `describe_divergence` helper names the
  first diverging chunk and the specific dimension, so the float case now reads "chunk `main`: op 1
  diverges (Return vs reference Const(1))" rather than an opaque "diverges from the reference".
- No opcode, record, node kind, or `BYTECODE_VERSION` change, no `.kel` change. The construct-support
  boundary is UNCHANGED at **52 Ok / 2 Gap / 1 RefRejects**.

## Verification

- Three new `tests/self_hosted_backend.rs` tests pin the behavior. `ReferenceRejected` classification for
  an undefined identifier with `rust_backend_would_help() == false`, the `Unsupported` divergence detail
  naming `chunk `main``, and the `Unsupported` hint policy. All five backend tests pass.
- CLI end to end confirmed. Out-of-subset float prints the chunk-naming detail plus the retry hint at
  exit 1, a genuine source error prints a plain compile error with no retry hint at exit 1, and an
  in-subset program compiles at exit 0.
- The FULL `scripts/release-gate.sh` is GREEN.

## Next step — OPERATOR-DECISION FORK (no bounded same-context roadmap task remains)

The near-term self-host workstreams remain at their bounded end, and "harden the CLI backend" is now also
delivered. The remaining boundary Gaps are a genuine design decision or out of scope. Candidate directions
to surface, minus the now-resolved CLI-hardening option:
- **Third-level struct nesting** — generalize the fixed-depth nested-equality drain to a bounded depth
  stack (closes one of the 2 remaining Gaps). The verifier forbids recursion (R4), so each depth is an
  explicit phase today. Previously rated EXTREME effort.
- **New self-host language surface** — pick another construct family the reference emits that the `.kel`
  stages still defer. Needs operator selection of which family.
- **Native-call support in the self-hosted pipeline** — the language-surface increment that would in turn
  make threading the CLI preamble meaningful. Larger; adds a native-call path to the self-hosted codegen.
- **A different workstream** entirely — release cadence (a V0.2.3 cut), or backlog items (B32/B33/B34
  prerequisites were recently filed).

The next session should present these and wait for direction rather than auto-selecting.

The seven-day rate-limit window remains the binding budget under heavy agent work; pace accordingly.

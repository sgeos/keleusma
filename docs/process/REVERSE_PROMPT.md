# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning and frontier assessments live in
[DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-07-27 (session 34)

## Current state

- **The self-hosted compiler is WIRED INTO THE SHIPPING CLI. COMPLETE, full gate GREEN, merged to
  `v0.2.3`.** `keleusma-cli compile` gained `--compiler <rust|self-hosted>` (default `rust`, behavior
  unchanged); `self-hosted` runs the Keleusma-written pipeline.
- The self-host driver moved (history-preserving) from `compiler/src/selfhost.rs` to
  `keleusma/src/selfhost/mod.rs` behind a `self-host` feature (off in the lib default, on in
  `keleusma-cli`). All ten Rust-read `.kel` now live in `keleusma/src/selfhost/kel/` (`include_str!`'d;
  works in an installed binary). `compiler/` is now a thin `pub use keleusma::selfhost::*` re-export
  (still detached/excluded). New entry `keleusma::selfhost::self_hosted_compile(src, &Target) ->
  Result<Module, SelfHostError>` (host-target-only; `catch_unwind` -> `Unsupported` for out-of-subset).
- The construct-support boundary is UNCHANGED at **52 Ok / 2 Gap / 1 RefRejects** (this workstream added
  no language constructs). No opcode/record/node/`BYTECODE_VERSION` change.

## Verification

- The `self-host` feature tests (`tests/self_hosted_backend.rs`): in-subset byte-identity vs the Rust
  reference; `NonHostTarget` refused; out-of-subset (`Float`) rejected as `Unsupported`, not a panic.
- CLI end-to-end (in-subset compiles, out-of-subset exits 1 with the retry hint, non-host refused); the
  full detached `compiler/` subproject (86 tests, via the re-export); and the FULL
  `scripts/release-gate.sh` all GREEN.

## Notes for the next session

- After a cross-workspace file move, sweep EVERY read-path helper in BOTH workspaces before the gate.
  This move displaced `compiler/tests/*.rs` read helpers (the `relocated` predicate listed only the 4
  Phase-1 stages; `validator.rs` read naively) — they surfaced one slow gate-run at a time. Run the whole
  subproject suite (`cd compiler && cargo test`) at once to catch them together. `cargo fmt --all` does
  NOT reach the detached `compiler/` workspace — format it separately (`cd compiler && cargo fmt`).
- Delegated agents kept stalling on the 600s watchdog when long cargo builds ran in the FOREGROUND;
  background+poll all long commands (the fix is proven).

## Next step — OPERATOR-DECISION FORK (no bounded same-context roadmap task remains)

Both the near-term self-host workstreams are now at their bounded end: the nested-composite-equality
family is fully self-hosted (52 Ok), and the CLI-backend residual (Workstream A) is delivered. The
remaining boundary Gaps are a genuine design decision (third-level struct nesting needs a general depth
stack — the verifier forbids recursion) or out of scope (floats/generics). So the loop is again at an
operator fork. Candidate directions to surface:
- **New self-host language surface** — pick another construct family to self-host (would grow the boundary
  again) — but the obvious nested-equality area is done; the next area needs operator selection.
- **Third-level struct nesting** — generalize the fixed-depth drain to a bounded depth stack (a design
  effort, previously rated extreme).
- **Harden the new CLI backend** — e.g. thread the CLI preamble through self-hosted mode, or widen the
  supported-subset error detail; documented limitations today.
- **A different workstream** entirely (release cadence, other roadmap items).
The next session should present these and wait for direction rather than auto-selecting.

The seven-day rate-limit window remains the binding budget under heavy agent work; this session was long
and delegation-heavy — pace accordingly.

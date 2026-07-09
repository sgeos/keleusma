# keleusma-selfhost — the self-hosted Keleusma compiler (V0.3.0)

This subproject builds the **self-hosted Keleusma compiler**: a Keleusma compiler
written in Keleusma. Completing it is the V0.3.0 release. The V0.2.x line proceeds
toward it, landing the surface-language and runtime prerequisites one release at a
time; this directory is where the compiler-in-Keleusma is assembled and bootstrapped.

The authoritative design is [`docs/roadmap/V0_3_0_SELF_HOSTING.md`](../docs/roadmap/V0_3_0_SELF_HOSTING.md).
This README is orientation; that document is the specification. The per-release plan
is [`MILESTONES.md`](./MILESTONES.md).

**Status: scaffolding.** The structure, the inter-stage data shapes, the Rust host
driver, and the bootstrap harness are stubbed. No stage is implemented yet. Each
V0.2.x release fills in prerequisites and, once the surface is ready, a stage.

## Architecture in one picture

The compiler is three coordinated stages, each a Keleusma `loop` function, driven by
a Rust host (until V0.4.0, when the host itself can be Keleusma):

```
source bytes ─▶ [ lexer.kel ] ─tokens▶ [ parser.kel ] ─declarations▶ [ codegen.kel ] ─▶ bytecode
                 loop, yields            loop, yields                  loop, yields
```

The whole-program AST is never built. The parser yields one top-level declaration at
a time; the compiler emits bytecode for it and forgets it. Each stage's working set is
bounded, so the per-stage worst-case memory usage falls out of Keleusma's existing
guarantee. See the roadmap's "Recommended architecture" section for the rationale and
the "Documented alternative" section for the integrated single-pass form that is on
the shelf but not recommended.

## File map

| Path | What it is |
|------|-----------|
| `kel/prelude.kel` | Shared inter-stage data shapes: `Token`, `TokenKind`, `Span`, `Declaration`, and the bytecode-encoding helpers. The one place both stages and the host agree on the wire between stages. |
| `kel/lexer.kel` | Stage 1. A `loop` that consumes source bytes and yields tokens. The simplest stage: a byte scanner with a keyword table. Migrated first (roadmap Step 1). |
| `kel/parser.kel` | Stage 2. A `loop` that consumes tokens and yields one `Declaration` per top-level declaration. Recursive-descent with the explicit-work-stack discipline (Keleusma forbids `fn`/`yield` recursion). Migrated second (Step 2). |
| `kel/codegen.kel` | Stage 3. A `loop` that consumes declarations and yields bytecode chunks plus the auxiliary body. The most complex stage: inference scope, monomorphization specialization table, emission. Migrated last (Step 3). |
| `src/` | The Rust host driver: registers the `compiler::` natives, drives the yield/resume pipeline, runs the bootstrap phases, and validates byte-identical output against the Rust-hosted reference compiler. |
| `tests/` | The byte-identical regression harness against `examples/scripts/` and the workspace corpus. |

## The `compiler::` natives

The self-hosted compiler needs three host-registered natives for the residual `Text`
work the surface does not yet do internally (resolved by R3.3; no surface extension):

- `compiler::intern_bytes` — intern a byte slice, returning a `Word` index.
- `compiler::text_from_bytes` — build a `Text` from bytes.
- `compiler::text_concat` — concatenate two `Text` values.

Source is handed to the lexer as `[Byte; N]` and indexed directly; no byte-iteration
surface extension is required.

## Quick start (once stages exist)

```sh
# Cross-compile the Keleusma-source compiler with the Rust-hosted compiler (Phase A):
cargo run --release -- bootstrap --phase a   # produces kelc.0.kel.bin
# Self-compile to the fixed point (Phases B, C):
cargo run --release -- bootstrap --phase bc  # kelc.1, kelc.2; asserts byte-identity
# Validate the regression corpus compiles byte-identically under kelc.1:
cargo run --release -- verify-corpus
```

This crate is a standalone package with a detached `[workspace]` (like
`examples/rtos`); it depends on the parent `keleusma` runtime by path and is not a
member of the workspace, so a half-built stage never destabilizes the released
workspace. It is excluded from the published `keleusma` crate tarball.

## Definition of done

V0.3.0 ships when all three stages exist in Keleusma source, the bootstrap reaches a
fixed point (`kelc.1` == `kelc.2` byte-for-byte), and the regression corpus compiles
byte-identically under both the Rust-hosted compiler and `kelc.1`. The Rust-hosted
compiler remains the reference implementation; the dual-compiler period is intentional.
The nine formal success criteria are in the roadmap.

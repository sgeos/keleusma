# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-16
**Status**: Four-item sprint (opaque types, Rust String example, monomorphizer bug, canonical numeric types) delivered with one item partially complete on the `v0.2.0` branch.

## Completed in this session round

Five logical atomic commits on the `v0.2.0` branch.

1. `feat(opaque): host-managed opaque values via Arc<dyn HostOpaque>` — new `keleusma::opaque` module introduces the `HostOpaque` marker trait and the `Value::Opaque(Arc<dyn HostOpaque>)` runtime variant. Hosts implement the trait for any Rust type they wish to expose; native functions produce opaque values through the `host_arc` constructor and consume them by extracting a typed reference through `dyn HostOpaque::downcast_ref`. Opaque values are host-managed, cross the yield boundary, and contribute zero to the script-side WCMU. The chosen design trades the propagation of a type parameter through every `Value`/`Vm`/`NativeCtx`/native-function site against the simplicity of a small custom marker trait. The `dyn`-free alternative was considered but the blast radius was disproportionate for V0.2.
2. `docs(opaque): worked example exposing Rust String as RustString` — new `examples/opaque_rust_string.rs` registers `std::string::String` as the opaque `RustString` type and threads three native operations (`make_string`, `upper_case`, `append_exclamation`) end to end. Demonstrates the recommended host-attested pattern for text-heavy work. EMBEDDING.md gains an "Opaque Host Types" subsection; TYPE_SYSTEM.md documents the runtime representation in a property table.
3. `fix(monomorphize): rewrite match-arm patterns and resolve nested generic field types to specialization names` — two related bugs in the generic-monomorphizer that surfaced through `examples/generic_match.rs`. Match-arm patterns now rewrite their enum names to the specialized form alongside the construction sites. Struct field types with nested generic instantiations (e.g. `inner: Cell<T>` inside a `Wrap<T>`) now infer the type parameter through a reverse-lookup on the in-progress specs map, and the substituted field type rewrites the inner generic to the emitted specialization name. Single-level only; deeper nesting still needs a full unification pass. Two regression tests pin the fixes against re-introduction.
4. `feat(verify): WCMU text-size tracking via abstract interpretation` (carried from the prior round) — completes the V0.2 charter.
5. `refactor(types): rename surface i64 to Word and f64 to Float (Phase 1)` — hard-break rename of the legacy lowercase keywords to the canonical V0.2 capitalised forms. PrimType, Type, parser, type checker, every test script, every example, every `.kel` file, and every doc updated. The rename was delegated to a token-aware subagent that walked Rust string literals (skipping Rust types, comments, char literals, and code outside strings) and a companion docs walker with code-fence awareness. Lexer suffix forms `42i64` / `3.14f64` are still accepted as legacy notation; only the type name is renamed.

## State of the four items in this round

- **Opaque type implementation**: complete.
- **Rust `String` as opaque type, example and docs**: complete.
- **Resolve the `generic_match` failure**: complete. Both probes pass end to end.
- **Canonical types Byte/Word/Fixed/Float**: Phase 1 complete (`Word` and `Float` renames are in production). Phases 2 and 3 (introduction of `Byte` and `Fixed` with runtime semantics) are tracked as separate tasks. A surface-only Byte type was prototyped in this round (parser recognition, type-check stubs) but reverted because it would create a misleading partial feature where `Byte` arithmetic silently used `Word` storage without wrapping. A real Byte type requires either new opcodes for byte arithmetic with `u8` wrapping semantics, or a runtime variant `Value::Byte(u8)` threaded through the existing arithmetic dispatch with cast-time masking.

## Verification matrix

```bash
cargo test --workspace --features text                                       # 518 pass
cargo test -p keleusma --no-default-features                                 # 449 lib + ancillary, all pass
cargo clippy --workspace --tests --features text -- -D warnings              # clean
cargo clippy --workspace --tests --no-default-features -- -D warnings        # clean
cargo fmt --check                                                             # clean
cargo run --example opaque_rust_string --features text                       # prints "HELLO, KELEUSMA!"
cargo run --example generic_match --features text                            # both probes pass
cargo run --example string_ops --features text                               # prints "result: hello..."
cargo run --example wcmu_basic                                               # yields Int(42)
echo 'fn main() -> Word { 42 }' | keleusma                                   # prints "42"
```

## Concerns

- Phase 1 of the canonical numeric types is the surface rename only. The runtime semantics of `Word` are unchanged from V0.1's `i64`. The `Target` descriptor's `word_bits_log2` field already exists for narrower targets, but the compiler and VM do not yet emit width-checked arithmetic for sub-64-bit `Word`. Phase 4 work (target-width arithmetic) is not yet scheduled.
- The Byte and Fixed types are not yet usable from scripts. Tasks 222 and 223 capture the design and implementation scope.
- The opaque-types `dyn`-flavoured design is documented in the module source. The `dyn`-free alternative (full type-parameter propagation) was explicitly considered and rejected for blast-radius reasons; the design choice is reversible if a future host needs the stronger type safety.
- The monomorphizer's nested-generic inference is single-level. `Cell<Wrap<T>>` still fails to infer. A proper Robinson unification pass would close the gap.

## Intended Next Step

Awaiting operator prompt. Candidates ordered by likely value:

1. **Phase 2 of canonical numeric types: add `Byte` (u8) with wrapping arithmetic**. Adds `Value::Byte(u8)`, extends `Op::Add`/`Sub`/`Mul`/`Div`/`Mod`/`Neg`/`Cmp*` dispatch to handle Byte operands with wrapping semantics, adds cast through `Op::Cast` with masking on Word→Byte narrowing, marshalling through `KeleusmaType for u8`. Scope estimate: one focused session.
2. **Phase 3 of canonical numeric types: add `Fixed` (Q-format)**. Larger design surface. Needs new opcodes for fixed-point multiply (which requires a post-multiply shift to maintain Q-format), the `Fixed<N>` parameterization at the parser and AST level, and KString-style runtime representation that carries the fraction-bit count. Scope estimate: one or two focused sessions.
3. **Phase 4 of canonical numeric types: target-width arithmetic for `Word`**. Currently `Word` is 64-bit regardless of target. The `Target` descriptor carries `word_bits_log2` but the VM does not honour it. Threading this through would require width-masked arithmetic opcodes and the corresponding wire-format upgrade.
4. **Cut a V0.2.0 release candidate** against the branch and tag it for crates.io publication.

## Pre-existing limitations not addressed in this round

- `wcmu_basic` example: the auto-sized arena returns the per-iteration WCMU but the VM additionally pre-reserves operand-stack and call-frame minimums at construction. The example now sizes the arena to `max(wcmu, 4096)` to accommodate the pre-reservation overhead. This is an ergonomic gap in `auto_arena_capacity_for` that should add a host-side margin or expose the minimum reserve.
- `examples/piano_roll.rs` requires the `sdl3-example` feature and was not exercised in this round.
- `keleusma-arena`, `keleusma-macros`, and `keleusma-bench` were not modified; their `i64`/`f64` references are genuine Rust types, not Keleusma surface types.

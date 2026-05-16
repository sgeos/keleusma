# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-16
**Status**: Eight-item sprint complete on the `v0.2.0` branch. Canonical numeric type set (`Byte`, `Word`, `Fixed`, `Float`) is now landed end to end. Standard DSL library surface and shell integration are in place.

## Completed in this session round

Five logical atomic commits.

1. `feat(types): canonical numeric types Phase 2 — Byte (u8)`. New `Byte` primitive type, 8-bit unsigned with wrapping `u8` arithmetic. New cast opcodes `Op::WordToByte` and `Op::ByteToWord`. `Value::Byte(u8)` runtime variant, `ConstValue::Byte` compile-time constant, `KeleusmaType for u8` marshalling. Seven integration tests cover cast truncation, wrapping arithmetic, and unsigned comparison.
2. `feat(types): canonical numeric types Phase 3 — Fixed (Q-format)`. New `Fixed` primitive type, signed Q-format with target-scaled fraction bits (Q31.32 on the host runtime). New opcodes `Op::WordToFixed(u8)`, `Op::FixedToWord(u8)`, `Op::FixedMul(u8)`, `Op::FixedDiv(u8)`, each carrying the fraction-bit count as an immediate. `Value::Fixed(i64)` runtime variant. Eight integration tests cover round-trip casts, Q-format add/sub/mul/div, negation, and signed comparison. `Fixed<N>` parameterisation and target-scaled fraction bits for sub-64-bit targets are deferred follow-on work.
3. `feat(stddsl): Library trait, four bundled libraries, CLI integration`. New `keleusma::stddsl` module introduces the `Library` trait and four bundled libraries (`Math`, `Audio`, `Text`, `Shell`). `Vm::register_library<L: Library>(lib)` is the new uniform entry point. The `shell` cargo feature gates the Shell library, which requires `std`. `keleusma-cli` enables `["text", "shell"]` and registers all four bundles on every script the runner executes.

## State of the four-plus-four-item charter from this round's prompt

| Item | Status |
| --- | --- |
| Phase 2 (Byte) | Complete. |
| Phase 3 (Fixed) | Complete for the default target-scaled form. `Fixed<N>` parameterisation deferred. |
| Math → `stddsl::Math` | Complete. |
| Audio → `stddsl::Audio` | Complete. |
| Text → `stddsl::Text` | Complete. |
| Library trait and `Vm::register_library` | Complete. |
| Single-file-script docs note | Complete. EMBEDDING.md gains a "Single-file scripts" subsection. |
| `stddsl::Shell` | Complete with `getenv`, `has_env`, `run`, `run_checked`, `exit`. The originally specified `Option<Text>` shape for `getenv` was infeasible because of a pre-existing limitation in the pattern matcher; the shell-idiomatic empty-string shape is the practical alternative, with `has_env` as the companion presence-check. |
| CLI registers Math, Audio, Text, Shell | Complete. |

## Verification matrix

```bash
cargo test --workspace --features text                                       # 496 + ancillary, all pass
cargo test --workspace --features text,shell                                 # 496 + ancillary, all pass
cargo test -p keleusma --no-default-features                                 # 464 lib + ancillary, all pass
cargo clippy --workspace --tests --features text -- -D warnings              # clean
cargo clippy --workspace --tests --features text,shell -- -D warnings        # clean
cargo clippy --workspace --tests --no-default-features -- -D warnings        # clean
cargo fmt --check                                                             # clean
cargo run --example opaque_rust_string --features text                       # prints "HELLO, KELEUSMA!"
cargo run --example generic_match --features text                            # both probes pass
cargo run --example string_ops --features text                               # prints "result: hello..."
cargo run --example wcmu_basic                                               # yields Int(42)
echo 'fn main() -> Word { 42 }' | keleusma                                   # prints "42"
echo 'use shell::run_checked\nfn main() -> Text { shell::run_checked("echo hi") }' | keleusma  # prints "hi"
echo 'use shell::exit\nfn main() -> Word { shell::exit(7); 0 }' | keleusma; echo $?  # exits 7
```

## Concerns and limitations carried forward

- `KeleusmaType` marshalling does not yet support `String` arguments or tuple return types. The shell natives use the lower-level `register_native` and pattern-match on `Value` directly. Lifting `String` into the marshalling family would let `shell::getenv` and similar register through `register_fn_fallible` with a typed signature visible to the type checker.
- The compiler's pattern matcher does not understand the runtime convention that `Some(v)` is `v` directly. `Option::Some(x) =>` patterns emit an `IsEnum` check that fails against unwrapped values. This blocks `Option`-typed native return values from being useful in scripts. The fix is local to the pattern compile site and is tracked as a pre-existing limitation.
- `stddsl::Text` registers all of the legacy `utility_natives::register_utility_natives` bundle which includes the `math::*` natives. `stddsl::Math` registers the same natives. The double-registration is benign (identical bodies) but a future refinement could split the utility_natives bundle into text-only and math-only halves.
- The compiler emits `Op::FixedMul(32)` and friends with a hard-coded fraction-bit count. Threading the target descriptor through the `FuncCompiler` so sub-64-bit targets get Q15.16 or Q7.8 automatically is a follow-on commit.
- `Fixed<N>` parameterisation is deferred. The parser does not yet accept numeric generic arguments.
- The opaque-types `dyn`-flavoured design from the prior round is unchanged. A future tightening to full type-parameter propagation remains possible but the blast-radius cost was judged disproportionate.

## Intended Next Step

Awaiting operator prompt. Candidates ordered by likely value:

1. **Lift `String` into the `KeleusmaType` marshalling family** (and tuple return types). Removes the need for the shell natives to use the low-level `register_native` and enables typed signatures visible to the type checker. Probably one focused session.
2. **Fix the compiler's `Option::Some(x)` pattern matching**. The current `IsEnum` check needs replacement with a "value is not None and bind" path. Once fixed, `shell::getenv` can return `Option<Text>` as originally specified. Probably one focused session.
3. **Target-scaled fraction bits for `Fixed` on sub-64-bit targets**. Thread `Target` through `FuncCompiler` so 16-bit and 32-bit targets emit `Op::FixedMul(8)` and `Op::FixedMul(16)` respectively. Mechanical.
4. **`Fixed<N>` parameterisation**. Add the numeric-generic-argument surface to the parser; thread `Option<u8>` through `PrimType::Fixed`. Larger surface change but conceptually clean.
5. **Cut a V0.2.0 release candidate** against the branch and tag it for crates.io publication.

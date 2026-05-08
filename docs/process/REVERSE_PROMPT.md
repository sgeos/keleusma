# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-08
**Task**: V0.0-M4 completion. Static marshalling layer for ergonomic Rust type interop.
**Status**: Complete

## Verification

**Command**: `cargo test && cargo clippy --tests -- -D warnings && cargo fmt --check`
**Result**: 268 tests pass, up from 238. Zero clippy warnings. Format clean. The new tests cover the `KeleusmaType` derive on structs and enums of all three variant kinds, `register_fn` end-to-end across arities, fallible registration, type mismatch errors, and host structs flowing through native function arguments and return values.

## Summary

Converted the crate to a Cargo workspace and added a `keleusma-macros` proc-macro crate. Implemented the `KeleusmaType` trait with manual implementations for primitives, the unit type, fixed-arity tuples through arity four, fixed-length arrays, and `Option<T>`. Provided `#[derive(KeleusmaType)]` for host structs and enums whose fields and variants compose admissible types. Added the `IntoNativeFn` and `IntoFallibleNativeFn` trait families with `macro_rules!` generated implementations for arities zero through four. Added `Vm::register_fn` and `Vm::register_fn_fallible` as the user-facing entry points. Migrated `src/audio_natives.rs` and the math portion of `src/utility_natives.rs` to the new API as the demonstration. The three Value-polymorphic functions (`to_string`, `length`, `println`) remain on `register_native` because they consume any `Value` variant and so cannot be expressed at fixed types. Documented the marshalling layer across `LANGUAGE_DESIGN.md`, `COMPILATION_PIPELINE.md`, `GLOSSARY.md`, a new Section 9 in `RELATED_WORK.md`, and `R30` in `RESOLVED.md`. Added `B5` to the backlog for the deferred string redesign.

## Changes Made

### Workspace and Crate Structure

- **Cargo.toml**: Added `[workspace]` section. The runtime crate keeps `src/` at the root. Added `keleusma-macros` as a path dependency.
- **keleusma-macros/Cargo.toml**: New proc-macro crate with `proc-macro = true`. Depends on `syn` 2 with `full` features, `quote` 1, and `proc-macro2` 1.
- **keleusma-macros/src/lib.rs**: Implements `#[proc_macro_derive(KeleusmaType)]` for named-field structs and for enums with unit, tuple-style, and struct-style variants. Uses `quote` to generate impls that resolve all paths through `::keleusma::` to ensure the generated code finds the trait, error type, and `Value` enum at the runtime crate root.

### Runtime Crate

- **src/lib.rs**: Added `pub mod marshall`. Re-exported `Value`, `VmError`, `KeleusmaType` (trait), `IntoNativeFn`, `IntoFallibleNativeFn`, and the `KeleusmaType` derive macro at the crate root. The trait and the derive share a name and live in different namespaces, which Rust allows.
- **src/marshall.rs**: New module containing the `KeleusmaType` trait, primitive impls, `Option<T>` impl, fixed-arity tuple impls through arity four via `macro_rules!`, fixed-length array impls via const generics, the `IntoNativeFn` and `IntoFallibleNativeFn` trait families with `macro_rules!` generated implementations for arities zero through four, and a unit-test suite covering primitive round-trip, type mismatch errors, tuple round-trip, array round-trip and length mismatch, and the trait-family invocation paths.
- **src/vm.rs**: Added `register_fn` and `register_fn_fallible` methods on `Vm` that accept any function whose signature satisfies the corresponding trait family. The existing `register_native` and `register_native_closure` methods are unchanged.
- **src/audio_natives.rs**: Rewritten to use `register_fn` and `register_fn_fallible`. The `extract_f64` and `extract_i64` helpers are no longer needed.
- **src/utility_natives.rs**: The `math::sqrt`, `math::floor`, `math::ceil`, `math::round`, and `math::log2` registrations migrated to `register_fn`. The `to_string`, `length`, and `println` functions remain on `register_native` because they consume any `Value` variant.

### Integration Tests

- **tests/marshall.rs**: New integration test crate with 17 tests covering struct derive round-trip, nested struct round-trip, struct error cases, enum derive for unit, tuple-1, tuple-2, and struct-style variants, enum unknown variant errors, register_fn for arities 0, 1, 2, and 4, register_fn_fallible error propagation, register_fn with derived struct argument, register_fn with derived struct return, and argument type mismatch.

### Knowledge Graph

- **docs/architecture/LANGUAGE_DESIGN.md**: Added the ergonomic typed registration to the Native Function Interface section. Added a KeleusmaType and Static Marshalling subsection.
- **docs/architecture/COMPILATION_PIPELINE.md**: Updated typical host usage to mention `register_fn` and `register_fn_fallible`.
- **docs/reference/GLOSSARY.md**: Updated the `keleusma_type` entry to reflect that `#[derive(KeleusmaType)]` is the implementation. Added `KeleusmaType`, `IntoNativeFn`, and `register_fn` entries.
- **docs/reference/RELATED_WORK.md**: Added Section 9 comparing the Keleusma static marshalling approach to Rhai, Lua bindings, and wasm-bindgen. Added the `[E2]` Rhai bibliography entry.
- **docs/decisions/RESOLVED.md**: Added R30 recording the static marshalling decision.
- **docs/decisions/BACKLOG.md**: Added B5 for the deferred string redesign.
- **CLAUDE.md**: Updated the repository structure and technology stack sections.

## Unaddressed Concerns

1. **String redesign deferred to V0.0-M5.** The current `Value::Str(String)` representation is heap-allocated and variable-length, in tension with the fixed-size discipline. The user has indicated that dropping or restricting strings to static is acceptable. The redesign is recorded as B5.
2. **Trait coherence approach for `Result` return.** The chosen design uses two distinct trait families (`IntoNativeFn` and `IntoFallibleNativeFn`) with separate `Vm::register_fn` and `Vm::register_fn_fallible` methods. A unified entry point would require either trait specialization (unstable) or proc-macro per registration (more code generation). The two-method approach is simple, unambiguous, and adequate for current needs.
3. **Arity bound is four.** Native functions of arity five and higher must use the lower-level `register_native` API. The bound is arbitrary and can be lifted by adding more `impl_into_native_fn!` invocations in `src/marshall.rs`. Five is the practical upper bound for most native functions.
4. **No method-style call or property accessor sugar.** The user can register a function `host::magnitude(p)` but not `p.magnitude()`. This is a separate ergonomic addition that was not in the agreed scope for V0.0-M4.
5. **No operator overloading.** Custom types cannot define `+`, `-`, `*`, `/` semantics for use in scripts. Out of scope for V0.0-M4.
6. **Tuple structs are rejected by the derive.** `#[derive(KeleusmaType)] struct Point(f64, f64)` produces a compile error. The recommended pattern is named-field structs. This is a documented limitation with a clear error message.
7. **The dialogue type aspirational `#[keleusma_type]` attribute remains aspirational.** The current `#[derive(KeleusmaType)]` covers both general native function marshalling and dialogue type implementation. The aspirational attribute marker for layout-enforcing dialogue types is documented as future work.

## Intended Next Step

V0.0 is now complete through V0.0-M4. The next milestone candidates remain those identified previously, with the addition of two items raised by the marshalling work.

A. Type checker implementation (P1).
B. For-in over arbitrary expressions (P2).
C. Error recovery model (P3).
D. Schema descriptor metadata for stronger schema mismatch detection (deferred from R29).
E. Static string discipline (B5).
F. Method-style call and property accessor sugar (extension of R30).
G. Soundness proof for the structural verifier or hot swap mechanism.

Recommend selecting one of A, B, or C for V0.1 if the language layer is the priority. If the host integration is the priority, E first because it tightens the type discipline, then F to extend ergonomics. If the certification path is the priority, D and G are the candidates.

Await human prompt before proceeding.

## Session Context

The session resumed a previously stalled data segment design and carried through three milestones of work, V0.0-M3 for the data segment and V0.0-M4 for the static marshalling layer. The total session arc covered specification clarification, research and documentation formalization, source conformance, host interoperability layer, hot swap API, and now the ergonomic Rust type interop with macro-generated marshalling.

Five commits accumulated during the session.

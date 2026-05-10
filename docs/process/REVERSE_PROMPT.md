# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T38 Release-readiness pass.
**Status**: Complete. All four prioritized items landed in one commit. 508 tests pass workspace-wide.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 508 tests pass workspace-wide.
- Format clean.
- Clippy clean.

## Summary

The user prioritized four items for release readiness given quota constraints. All four landed.

### Item 1. CLAUDE.md project-file update

The project file said "V0.0 Complete. Ready for V0.1 planning" — severely stale. Updated to reflect V0.1-M3 substantially complete status. Added explicit statement of the WCET and WCMU value proposition and the conservative-verification stance with a forward link to the LANGUAGE_DESIGN section. Repository structure listing extended with the new modules visitor, target, monomorphize, typecheck. Test count corrected to roughly 508. Technology stack list extended with rkyv. The keleusma-arena registry version is noted.

### Item 2. Top-level README.md rewritten

The previous README was stale on multiple points: it claimed "single external dependency (libm)" while the crate now depends on rkyv, allocator-api2, keleusma-macros, keleusma-arena. The Quick Start example called `Vm::new(module)` without the arena argument. The license footer said MIT but Cargo.toml says 0BSD. The closure-related sections that have since been removed.

The new README leads with the WCET and WCMU value proposition and the conservative-verification stance. Quick Start uses the correct `Vm::new(module, &arena)` signature. Includes new sections on generics and traits, f-string interpolation, and cross-architecture targeting through `Target`. License note corrected to 0BSD. Cross-references point at the canonical specifications in the docs tree.

### Item 3. Cargo.toml metadata

The package metadata was missing fields needed for crates.io publication. Added homepage, repository, documentation, readme, rust-version. Added a `[package.metadata.docs.rs]` block matching the keleusma-arena pattern. Description updated to lead with the WCET value proposition. The `rust-version` was set to 1.85 by analogy with keleusma-arena, but the actual code uses `is_multiple_of` which is stable since 1.87; clippy with the explicit MSRV declaration caught the inconsistency. Bumped to 1.87 to match the code.

### Item 4. Compile-time WCMU rejection

The user explicitly said "compilation and loading should be rejected" for unprovable bounds. Loading already rejected via `Vm::new` calling `verify::module_wcmu`. Compilation did not.

Compile-time defense added in `compile_with_target`. Two checks fire at compile time after emission:

1. Structural verification through `verify::verify`. Block nesting, jump offsets, block-type constraints, break containment, productivity rule.
2. Unbounded-construct scan. The same rejection for `Op::CallIndirect` and `Op::MakeRecursiveClosure` that `verify::module_wcmu` performs at load time.

The full WCMU computation including loop iteration bound extraction and the arena-capacity check remain deferred to `Vm::new`. The reason is twofold. First, the arena capacity is a runtime parameter that compile time does not know. Second, some Func chunks have parameter-dependent loops whose iteration bounds the present analysis cannot extract. Such chunks are legitimate when never reached from a Stream chunk's call graph; rejecting them at compile would over-reject. The narrow compile-time check covers the unbounded-by-construction rejection without the over-rejection.

Two recursive-closure typecheck tests were inverted. They previously asserted that `compile_src` succeeds for recursive closures because the recursive-closure rejection fired only at `Vm::new`. With compile-time rejection, the same programs now fail at `compile_src`. The tests rename to `recursive_closure_rejected_by_compile_pipeline` and `recursive_closure_with_capture_rejected_by_compile_pipeline` and assert the rejection.

## Trade-offs and Properties

The compile-time check is a strict subset of what `Vm::new` does. Programs admitted by `compile_with_target` may still be rejected by `Vm::new` if they have loops with non-extractable bounds or WCMU exceeding arena capacity. The compile-time rejection covers the unbounded-by-construction cases that no future analysis would admit; the load-time rejection covers cases that depend on arena sizing.

This split is the practical interpretation of "compilation and loading should be rejected." Both reject unprovable bounds. Compilation rejects what does not require runtime parameters. Loading rejects what does. A future tightening would move loop iteration bound extraction to compile time as the analysis matures; the conservative-verification stance permits this evolution without changing the surface.

The rust-version bump to 1.87 is a real backward-compatibility consideration. Hosts using the previous declared MSRV of 1.85 would have failed to build because the code uses `is_multiple_of`. The bump is documenting reality, not introducing new restrictions.

## Files Touched

- **`CLAUDE.md`**. V0.1-M3 status, conservative-verification stance, module list updated.
- **`README.md`**. Rewritten to lead with WCET value proposition, correct API surface examples, fix license, add target descriptor section, add generics and f-string sections.
- **`Cargo.toml`**. Metadata extended for crates.io publication, MSRV bumped to 1.87.
- **`src/compiler.rs`**. Compile-time WCET defense added in `compile_with_target`.
- **`src/typecheck.rs`**. Two recursive-closure tests inverted.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T38.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

The four highest-priority release items from the prior session are complete. Lower-priority items remain available for follow-up:

- Documentation accuracy pass on INSTRUCTION_SET, TARGET_ISA, GLOSSARY, GRAMMAR, STANDARD_LIBRARY.
- Detect duplicate native registration.
- Parser recursion depth limit.
- Indirect-dispatch flow analysis to admit second-category closure programs.
- Type::Unknown sentinel removal.
- Fuzz harness, miri on runtime, criterion benchmarks.

The release state is now defensible. The crate identifies itself accurately on docs.rs and crates.io. The project file gives future agents a correct mental model. The compile pipeline rejects unbounded-by-construction programs at the build step. The README leads with the language's value proposition.

## Intended Next Step

Await human prompt. Quota is now near the threshold the user identified.

## Session Context

This session executed the release-readiness pass identified earlier. The four items were prioritized for impact-per-quota and landed together. The recursive-closure compile-time rejection is the load-bearing addition; the documentation updates make the release artifacts honest and current.

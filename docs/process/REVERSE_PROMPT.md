# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-10
**Task**: V0.1-M3-T43 Onboarding documentation: docs/guide/ and examples/scripts/.
**Status**: Complete. The documentation gap identified in the prior turn is closed by a new guide section, eight standalone script examples, and updates to the top-level navigation.

## Verification

**Commands**:

````bash
cargo test --workspace
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
for f in examples/scripts/*.kel; do echo "=== $f ==="; ./target/release/keleusma run "$f"; done
````

**Results**:

- Workspace tests pass. 519 tests across the workspace.
- Format clean.
- Clippy clean across `--workspace --all-targets`.
- All eight example scripts produce expected output:
  - `01_arithmetic.kel` returns `24`.
  - `02_struct_field.kel` returns `7`.
  - `03_enum_match.kel` returns `100`.
  - `04_for_in.kel` returns `40`.
  - `05_pipeline.kel` returns `169`.
  - `06_multiheaded.kel` returns `11`.
  - `07_fstring.kel` returns `hello, Keleusma! 7 plus 2 is 9`.
  - `08_method_dispatch.kel` returns `84`.

## Summary

The user identified that the documentation, while comprehensive at the reference and architecture level, lacked an onboarding path comparable to Rhai's Book. This task closes that gap with a new guide section and a set of standalone script examples.

### New documents

`docs/guide/README.md` is the section index. It introduces three audience-focused documents and links to companion material under `examples/scripts/` and `examples/`.

`docs/guide/GETTING_STARTED.md` walks a first-time user through installation, writing a first script, running it through the CLI, compiling to bytecode, exploring the REPL, and embedding the same script in a twenty-line Rust host. The walkthrough is concrete and runnable end to end.

`docs/guide/EMBEDDING.md` documents the host-facing surface of the runtime. The document covers VM construction including `Vm::new`, `Vm::load_bytes`, the call and resume protocol with the three `VmState` variants, the four registration paths for native functions, the `KeleusmaType` derive macro for custom types, the bundled utility and audio natives, WCET and WCMU attestation through `set_native_bounds`, three arena-sizing strategies, error recovery via `reset_after_error`, hot code swapping at the reset boundary, and trust-skip construction with `Vm::new_unchecked`. Cross-references to the existing `examples/` programs are interleaved so a reader has working code for each topic.

`docs/guide/WHY_REJECTED.md` maps verifier rejection messages to root causes and proposes rewrites. The document distinguishes the two-category taxonomy from the conservative-verification stance: first-category rejections are programs that admit unbounded execution by construction, second-category rejections are programs that are bounded in fact but whose proof is not yet implemented. Concrete error-message strings drawn from `src/verify.rs` and `src/compiler.rs` are matched to root-cause and rewrite patterns for `MakeRecursiveClosure`, `CallIndirect`, loop-iteration-bound extraction, recursive call detection, missing yield in stream blocks, and resource bounds exceeded.

### New scripts

Eight `.kel` files under `examples/scripts/` cover the principal language features. Each script is self-contained, documents the expected output in a header comment, and runs through `keleusma run`. The scripts cover primitives and operators, structs and field access, enums and pattern matching, bounded iteration, the pipeline operator, multiheaded function dispatch, f-string interpolation, and trait method dispatch. A README in the directory tabulates the topics.

### Navigation updates

`docs/README.md` adds the guide section to the section table and surfaces the three guide documents in the quick-reference table. The top-level `README.md` reorganizes the documentation list into Onboarding and Reference subsections, surfacing the guide and the scripts directory at the top of the documentation entry point. The crate workspace section in the top-level `README.md` is also updated to reflect the five-crate layout: `keleusma`, `keleusma-macros`, `keleusma-arena`, `keleusma-bench`, and `keleusma-cli`.

## Trade-offs and Properties

The decision to make every script in `examples/scripts/` runnable through `keleusma run` rather than mixing CLI-runnable and embedding-only scripts means the script set is constrained to atomic-total `fn main` entry points. Yield-driven and stream-driven examples remain in the Rust embedding examples under `examples/` and are linked from the guide. The trade-off is that the script library does not directly demonstrate `yield` or `loop`. The benefit is that a new user can run any script with one command and see it produce a value, which is the strongest possible feedback loop for early adoption.

The decision to keep `WHY_REJECTED.md` rooted in actual error-message strings rather than abstract rejection categories means the document indexes by what the user sees. A rejected program produces a verifier error message; the user can search this document for a substring of the message and find the matching root cause and rewrite. The trade-off is that adding new verifier rejections requires updating this document. The alternative, an abstract taxonomy, would not surface the user's actual error.

The decision to surface the immutable-locals constraint in both `04_for_in.kel` and `WHY_REJECTED.md` means new users encounter the constraint early. Keleusma's local immutability is unusual relative to Rust and is non-obvious from the surface syntax. The earlier the constraint is named, the less time a new user spends puzzling over why a `let mut` annotation is rejected and what alternatives the language offers.

The pipeline-syntax fix during script authorship (`6 |> double` rejected, `6 |> double()` accepted) reflects the parser's requirement that the right-hand side of `|>` always include parentheses, even for nullary calls. The script comment notes the syntax explicitly to avoid the same error in user code.

The chained-method-call constraint in `08_method_dispatch.kel` was discovered during smoke testing. The current monomorphization pass cannot resolve a method call on a value whose type is inferred from a previous method-call return. The script header comment documents this limitation and the workaround of adding a typed `let` binding between calls. This is a second-category rejection that a future inference-reach extension can resolve.

## Files Touched

- **`docs/README.md`**. Section table updated to include the guide. Quick-reference table extended with onboarding entries.
- **`docs/guide/README.md`** (new). Section index.
- **`docs/guide/GETTING_STARTED.md`** (new). Installation, first script, embedding walkthrough.
- **`docs/guide/EMBEDDING.md`** (new). Host-facing API surface and patterns.
- **`docs/guide/WHY_REJECTED.md`** (new). Verifier-error catalog and rewrite suggestions.
- **`examples/scripts/README.md`** (new). Script-library index.
- **`examples/scripts/01_arithmetic.kel`** through **`examples/scripts/08_method_dispatch.kel`** (new). Eight script examples.
- **`README.md`** (top-level). Documentation list reorganized into Onboarding and Reference. Workspace section updated to reflect five crates.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T43 in the Task Breakdown table and a new History row.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

The new guide is functional and was smoke-tested for accuracy. Several refinements remain.

- The script library does not demonstrate yield-driven or stream-driven scripts, since those require a host that drives the call and resume loop. A companion set under `examples/scripts/host/` with paired `.kel` and `.rs` files would demonstrate the full coroutine surface end to end.
- The data-segment example pattern from `WHY_REJECTED.md` is described but not demonstrated as a runnable script. A `examples/scripts/09_data_segment_loop.kel` plus `.rs` driver would close this.
- The chained method-call inference reach noted in `08_method_dispatch.kel` is a second-category rejection that would benefit from an explicit BACKLOG entry tracking the inference extension. The current backlog references B2.4 monomorphization broadly; a dedicated sub-item for chained-receiver inference would surface the constraint.
- The CLI `run` command does not currently drive yield. A future CLI flag such as `--input <stream-file>` driving resume from a host-side stream would let yield scripts ship through the CLI without a Rust host. This is a CLI-level enhancement separate from the documentation work.

## Intended Next Step

Await human prompt before proceeding.

## Session Context

This session closed the onboarding-documentation gap identified by the user. The guide is structured to mirror what the user named as Rhai's coverage: a getting-started narrative, an embedding cookbook, and a troubleshooting reference for verifier rejection. The script library complements the existing Rust embedding examples by giving the new user something to type and run through the CLI immediately. The total documentation footprint grew by approximately seven hundred lines of new prose plus eight script files, and zero source code changes were required.

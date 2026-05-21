# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-21
**Status**: V0.2.0 pre-publish polish: Tier 1 through Tier 4 documentation audit complete. All identified inaccuracies and gaps closed. The branch is publish-ready.

## Completed in this session round

### Tier 1 (first impression on crates.io)

| File | Action |
|------|--------|
| `keleusma-arena/README.md` | Audited; no changes. |
| `keleusma-macros/README.md` | Audited; no changes. |
| `keleusma-bench/README.md` | Audited; no changes. |
| `keleusma-cli/README.md` | Three V0.1.x surface call sites updated to V0.2.0: shebang example `fn main() -> i64` → `Word`, REPL transcript `fn double(x: i64) -> i64` → `Word`, REPL try-types list `i64, f64, bool, String, ()` → `Word, Float, bool, Text, ()` (matches `REPL_RETURN_TYPES` constant in source). |
| `examples/README.md` | Audited; no changes. |

### Tier 2 (RTOS demonstrator and standalone scripts)

| File | Action |
|------|--------|
| `examples/rtos/README.md` | Two stale figures corrected: `memory.x` description (was 640 KB FLASH / 384 KB RAM at the wrong offset; current is 768 KB / 256 KB at `0x341C0000`) and trust-load image size (was ~192 KB; current is ~140 KB). |
| `examples/scripts/07_fstring.kel` | Deleted. F-strings were retired in V0.2.0; the lexer rejects them at lex time. The script no longer ran. |
| `examples/scripts/07_refinement.kel` | New. Worked example of `newtype Counter = Word where nonneg;` with literal elision and runtime construction check. Verified end to end: outputs `100`. |
| `examples/scripts/README.md` | Updated 07 row to the new refinement example and the 01_arithmetic feature column to `Word, Float, bool` (was `i64, f64, bool`). |
| `examples/rtos/MANUAL.md` | Audited; no changes. |
| `examples/rtos/SPEC.md` | Audited; no changes. |

### Tier 3 (architecture, spec, reference)

| File | Action |
|------|--------|
| `docs/architecture/LANGUAGE_DESIGN.md` | Hindley-Milner bullet claimed `Type::Unknown` "remains as a transitional sentinel"; V0.2.0 closed B15 and removed it. Updated to describe the V0.2.0 reality (fresh `Type::Var` at every unannotated position) and reframed the section heading. |
| `docs/spec/GRAMMAR.md` | Same H-M correction. Two "Opaque type support is partial in V0.1.x" sections rewritten to describe the V0.2.0 `HostOpaque` first-class surface (`Value::Opaque(Arc<dyn HostOpaque>)`, `host_arc`, `downcast_ref`). |
| `docs/architecture/EXECUTION_MODEL.md` | Audited; no changes. |
| `docs/architecture/COMPILATION_PIPELINE.md` | Audited; no changes. |
| `docs/architecture/SUB_COROUTINES.md` | Audited; preliminary by design, no changes. |
| `docs/spec/TYPE_SYSTEM.md` | Audited; no changes. |
| `docs/spec/INSTRUCTION_SET.md` | Audited; no changes. |
| `docs/spec/WIRE_FORMAT.md` | Audited; no changes. |
| `docs/spec/STRUCTURAL_ISA.md` | Audited; no changes. |
| `docs/spec/STANDARD_LIBRARY.md` | Audited; no changes. |
| `docs/reference/GLOSSARY.md` | Audited; no changes. |
| `docs/reference/RELATED_WORK.md` | Audited; no changes. |

### Tier 4 (decisions, process, roadmap, extras)

| File | Action |
|------|--------|
| `docs/decisions/PRIORITY.md` | One present-tense statement claiming `Value::DynStr` "remains for natives that do not need arena allocation"; V0.2.0 removed the variant. Added an inline V0.2.0 update note. |
| `docs/decisions/BACKLOG.md` | Audited; no changes. Already carries explicit "V0.2.0 status." headers on items whose situation changed. |
| `docs/decisions/RESOLVED.md` | Audited; no changes. Intentionally historical record. |
| `docs/process/*.md` | Audited; no changes. |
| `docs/roadmap/*.md` | Audited; no changes. |
| `docs/extras/*.md` | Audited; no changes. |

## What the operator still owns

- **Publish in dependency order.** `keleusma-macros 0.2.0` → `keleusma 0.2.0` → `keleusma-bench 0.2.0` + `keleusma-cli 0.2.0`. Commands documented earlier this session.
- **Tag the release.** `git tag -a v0.2.0` and `git push origin v0.2.0`.
- **Decide B15.** Already resolved per the CHANGELOG and the typecheck source; the operator can remove the deferral note in REVERSE_PROMPT once confirmed.

## Verification

```bash
# Local quickstart (top-level README) runs end to end
( cd /tmp/keleusma_quickstart_test && cargo run )
# -> result: Int(42)

# Refinement example (the replacement for 07_fstring.kel) runs
cargo run -p keleusma-cli --bin keleusma -- run examples/scripts/07_refinement.kel
# -> 100

# Format
cargo fmt --all -- --check
# clean
```

## Open concerns

None blocking publish.

## Intended Next Step

V0.2.0 publish. Operator runs `cargo publish` in dependency order.

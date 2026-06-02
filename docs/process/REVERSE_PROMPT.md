# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-06-01
**Status**: B28 flat memory model active on branch `feat-flat-memory-model`. A design pass settled the approach, the B28 entry in `BACKLOG.md` was rewritten to match and is authoritative, and P2 has started with the foundation refit committed. The next P2 increment is the large coupled wiring of the composite value representation and the op handlers.

## The B28 design, as settled this session

The authoritative version is the B28 entry in [`../decisions/BACKLOG.md`](../decisions/BACKLOG.md). Summary so a fresh session has the decisions without re-deriving them:

- **No templates and no layout table.** The compiler bakes each composite field's offset and kind directly into the access instructions, the way an assembler resolves a struct equate. `layout_pass` is the compiler's transient symbol table, used to bake those offsets and to compute the worst-case-memory-usage bound, and is never written into the artifact or carried at run time.
- **The composite value is pure bytes.** No layout reference, no template index, no `Arc`. Construction infers field sizes from the kinds of the tagged operands it pops; access reads at the baked offset and pushes the correctly tagged scalar.
- **Instruction set.** No opcode added or removed. The operands of the composite ops are re-specified (construct ops carry a count, access ops carry offset and kind). Operand re-spec is allowed; opcode additions are wildly scrutinised. The set stays lean for the rad-hard silicon target.
- **Byte-code compatibility is not a goal.** V0.2.0 and V0.2.1 byte code may differ and are simply recompiled. `BYTECODE_VERSION` stays at 1 only for lack of production traction.
- **One remaining suboptimality.** Baking offsets removed the two interim compromises (dispatch-time resolution and a value-carried layout reference). What remains is the tagged scalar operand stack: a scalar still carries its kind tag, so the arithmetic ops dispatch on it. Making the stack untyped bytes is the one flat-machine step deferred to the V0.4 native-code-generation ISA redesign, documented under *Deferred ISA redesign* in B28 and cross-referenced from `../roadmap/V0_4_0_NATIVE_CODEGEN.md`.

The mental model throughout is the assembler one: the Keleusma surface lays out `.text`, `.rodata`, `.data`, `.bss`, and host-shared `.data`, and a value is bytes at a known offset. The 6502 and Nintendo Entertainment System native target is the forcing case for the flat representation.

## P2 status

- **Done (committed `03b6e6e`).** `src/flat_value.rs` `FlatComposite` refit from `{ bytes, Arc<LayoutDescriptor> }` to a pure byte buffer with an accessor API (`zeroed`, `from_bytes`, `len`, `is_empty`, `as_bytes`, `as_bytes_mut`, `write_at`, `slice_at`). Callers reach the bytes only through the accessors so the later move to arena-resident bytes does not churn call sites. 18 `flat_value` tests pass.
- **Next increment (not started).** Wire the byte buffer into the four `GenericValue` composite variants (`Tuple`, `Array`, `Struct`, `Enum`) and re-spec the construct and access op handlers across the compiler and the VM. Start with one kind as a vertical slice to prove the baked-offset machinery end to end, then replicate. This touches roughly two hundred composite sites in `bytecode.rs`, `compiler.rs`, `vm.rs`, `marshall.rs`, `ast.rs`, `audio_natives.rs`, `zero_value.rs`. Encapsulate composite reads behind accessor methods to contain the blast radius.
- **Scaffold decision.** A composite value cannot be half-migrated, and a composite can mix scalar and reference fields. So P2 migrates composites whose fields are transitively scalars and nested composites to pure bytes; composites containing a `Text` or `Opaque` field keep the boxed `Vec` representation until P3 handles reference-field byte handles. That dual representation during P2 and P3 is the scaffold, removed in P3.

## Verification

- `cargo test --workspace` green at `03b6e6e`. `flat_value` unit tests: 18 pass.
- `cargo fmt --all -- --check`, `cargo clippy -p keleusma --lib -- -D warnings`, and the Markdown link check are clean.

## Branch and push state

- On `feat-flat-memory-model`, two commits beyond `v0.2.1`: `5266c36` (B28 plan rewrite) and `03b6e6e` (P2 start).
- `v0.2.1` is one commit ahead of `origin/v0.2.1`: the B29 closeout `b652249`, not yet pushed.
- Nothing has been pushed since `origin/v0.2.1 = 6fcf311`. Working tree clean. Push when ready; the pre-push hook runs the full gate.

## Recommended next step

Resume P2 by wiring the byte buffer into the `GenericValue` composite variants and re-specing the construct and access op handlers, one composite kind first as a vertical slice. Keep reference-containing composites boxed until P3.

## Reference

- `docs/decisions/BACKLOG.md` B28 is the authoritative design and plan; B29 is resolved for V0.2.1.
- `src/flat_value.rs` is the refit foundation; `src/value_layout.rs` and `src/layout_pass.rs` are the compile-time layout, never carried on a value.
- `docs/roadmap/V0_4_0_NATIVE_CODEGEN.md` carries the deferred flat-machine ISA redesign (untyped operand stack).

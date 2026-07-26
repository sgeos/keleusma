# Enum-in-Struct Equality — Implementation Plan (working)

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Working plan for the self-hosted-compiler nested-equality increment that makes
`s1 == s2` self-compile byte-identically where a struct field has an enum type,
for example `struct S { e: E, w: Word }` with `enum E { A, B(Word) }`. It flips the
`eq/enum_in_struct__GAP` boundary case (48 Ok / 6 Gap becomes 49 Ok / 5 Gap).

Status: **COMPLETE and merged to `v0.2.3`** (2026-07-25). All four commits landed byte-identically
(A: `sd_fenum`; B/C/D: parse/reconstruct/codegen), the FULL `scripts/release-gate.sh` is GREEN, and
the boundary moved 48 -> 49 Ok. No STOP condition materialized: no new opcode, no `BYTECODE_VERSION`
bump, no new record or node kind. The nested variant tag 3 (Enum) rode the existing 2-bit variant
field of record 56. Two capacity gotchas were fixed as predicted (a factored `push_nested_enum_loop`
to stay under the 1536 op cap, `EXPECTED_SELF_COMPILE` 68 -> 69; and two `analyze.kel` per-chunk-op
scan-loop bounds raised 1024 -> 1536). This document is retained as the implementation record.

## Reference ground truth (byte-identity target)

For `struct S { e: E, w: Word }`, `enum E { A, B(Word) }`, `s1 == s2`, the Rust reference
lowers via `emit_composite_fieldwise_eq` (`src/compiler.rs:6651`) over the fields in
declaration order inside the `Loop`/`Break` virtual block. The enum field `e` is classified by
`classify_flat_field` (`:6265`) as `FlatFieldForm::Nested(size, CompositeKind::Enum)`, so its
top-level extract is `GetField(FlatNested{off,size,Enum})` (wire op 48), and because
`composite_needs_fieldwise_eq(E)` is true it recurses into `emit_enum_fieldwise_eq`
(`:6844`), which emits the variant-dispatch loop (its own `Loop`; per variant in discriminant
order `IsEnum`/`If`/`PopN` guards on both sides, per scalar payload `GetEnumField(Flat)`
compares, `Const(true); Break`; tail `Trap(EnumVariantUnmapped=4)`). This is exactly the
self-host's `push_struct_eq_nested` top-level field loop composed with `push_enum_eq`'s inner
variant-dispatch loop. The closest existing model is `push_array_of_enum_eq`
(`codegen.kel:1716`), which already composes an outer unroll with the inner variant dispatch
using deferred interning.

Trap: the top-level enum-field extract uses op 48 (`getfieldnested`) with variant=3, the SAME
op as a nested struct/tuple/array field. `GetEnumField` (op 55, Flat) appears only inside the
variant dispatch. Scalar payloads only, so op 57 (`getenumfieldnested`) is not needed.

## Machinery to reuse (standalone enum-eq path)

- Parse: `enum_eq_supported` (`parse.kel:2441`) rejects composite-payload variants (the nested
  enum sub-field must inherit this). `enum_eq_start` (`:2458`) finds variant rows via
  `edata[i] == ename` and drives `enumeq_variant_record` (`:2549`) / `step_enumeq_emit`
  (`:2560`) to emit an `EnumEqVariant` (record 49) per variant then an `EnumEqField` (record 52)
  per scalar payload field, then `EnumEqBuild` (record 50).
- Reconstruct: record 49 into `sqpending` (`reconstruct.kel:701`), 52 into `eqfields` (`:711`),
  `build_enum_eq` (`:276`) lays the match_parts.
- Codegen: `push_enum_eq` (`codegen.kel:1420`) emits the variant-dispatch loop with DEFERRED
  interning (`push_enum_isenum` `:1263`, `push_bool`). `push_array_of_enum_eq` (`:1716`) reuses
  that inner loop verbatim — the structural template for the nested enum sub-field.

## Nested-eq machinery and where the enum branch goes

- Records/nodes (`parse.kel:199-211`): `StructEqNested`=56 header packs
  `off + size*65536 + variant*2^32 + r2*2^34 + l2*2^44`; the variant is 2 bits (reconstruct
  decodes `(a/2^32)%4` at `:756`), so variant=3=Enum fits with NO new bits/record/node kind.
- `struct_eq_kind` (`:2194`): an enum field currently falls through all arms and mis-lowers as a
  scalar (hence the Gap). Add an arm keyed on `sd_fenum[fidx] > 0` that sets `sq_found=1`
  (nested) and validates scalar-only payloads.
- `structeq_nested_next` phases 0/1 (`:2335`): phase 0 emits a `StructEqNested` header per
  nested field; phase 1 drains its sub-fields. Add a phase-0 enum arm (variant=3 header, r2/l2
  temp alloc) and a phase-1 enum sub-phase that emits the variant-dispatch sub-stream instead of
  the field drain.
- `build_struct_eq_nested` (`reconstruct.kel:448`) + the `seb` assembler (`:722-781`): assemble
  the enum sub-stream into `seb` in a form codegen replays.
- `push_struct_eq_nested` (`codegen.kel:1817`): the stride pass (`:1826-1839`) and emit walk
  (`:1888-1970`) handle variants 0/1/2; add a variant-3 branch to both.

## Data tracking (Commit A — DONE)

`sd_fenum: [Word; 512]` in `structdefs`, populated in `field_size_and_kind`'s enum-detection
loop with the enum NAME id + 1 (what `edata` holds and what the variant-row lookup keys on).
Landed byte-identical; nothing reads it yet.

## Runtime ops (no new opcode)

Top-level extract `GetField(FlatNested{off,size,variant=3})` = op 48; the driver `decode_op`
(`tests/selfhost_codegen.rs:~898`) ALREADY decodes op-48 variant tag 3 -> `CompositeKind::Enum`.
Variant dispatch: `IsEnum`(54), `GetEnumField(Flat)`(55), `CmpEq`, `GetLocal`, `SetLocal`,
`PopN`, `Const`, control flow, `Trap(4)` — all already emitted by `push_enum_eq`. Confirmed in
`src/bytecode.rs`. No new opcode, no version bump, no new record/node kind.

## Ordered edit plan

**Commit A — sd_fenum tracking. DONE (cadd13e), byte-identical.**

**Commit B — parse: admit and stream the enum sub-field.**
1. `struct_eq_kind` (`:2194`): add an arm — if `sd_fenum[fidx] > 0` (and not array/struct/tuple),
   set `sq_found=1` and validate scalar-only payloads. GOTCHA: `enum_eq_supported` writes
   `stmt.sq_scan`, which `struct_eq_kind` also uses — do NOT call it here; inline the payload-kind
   scan into a separate accumulator. An unsupported (composite-payload) enum field sets `sq_scan=0`
   (defer to the ordinary path).
2. `structeq_nested_next` phase 0 (struct-container branch, `:2379-2428`): add an enum-field arm
   BEFORE the scalar fallthrough — allocate r2/l2 temps, set an enum sub-phase (enum name id, its
   variant start/count, a field cursor), `se_phase=1` with a new `se_subisenum=1` flag, and emit the
   `StructEqNested` header with variant 3:
   `Node::StructEqNested() as Word + (sd_foffset[fidx] + sd_fsize[fidx]*65536 + 3*4294967296 + r2*17179869184 + l2*17592186044416) * 64`.
3. phase 1 (`:2336-2353`): add an `se_subisenum==1` branch that drives a variant-dispatch sub-stream
   (reuse the `enumeq_variant_record`/`step_enumeq_emit` shape: an `EnumEqVariant` per variant then an
   `EnumEqField` per scalar payload field), and on exhaustion emit `StructEqNestedEnd`. Thread the
   enum-eq drain sub-state or add parallel `se_*` accumulators to avoid collision with the top-level
   nested state. Recommend reusing records 49/52 with a context flag.

**Commit C — reconstruct: assemble the enum sub-stream into `seb`.**
4. Add an `se_curvariant==3` context. In the record-56 handler (`:746`), when variant=3 do NOT
   reserve the struct/tuple subcount slot; instead set a mode where subsequent `EnumEqVariant`(49) /
   `EnumEqField`(52) records append into `seb` (per-variant `vname, disc, fcount, fcount*(off,kind)`),
   and record 57 (`StructEqNestedEnd`, `:769`) closes it. Records 49/52 (`:701`, `:711`) gain a guard:
   if `se_innested==1 andalso se_curvariant==3` append to `seb` at `sebcur`, else the existing path.
   `build_struct_eq_nested` copies the (now enum-shaped) `seb` slice unchanged. If `seb` proves too
   small for a many-variant enum, that is a capacity bump (like the 1024->1536 op-table raise), not a
   STOP.

**Commit D — codegen: emit the variant-dispatch inner loop for variant 3.**
5. `push_struct_eq_nested` stride pass (`:1826-1839`): add a variant-3 stride (length derived from
   vcount and per-variant fcount). GOTCHA: the enum field uses DEFERRED interning like
   `push_array_of_enum_eq`, so guard the forward `intern_bool` pre-pass (`:1849-1865`) to SKIP
   variant-3 fields.
6. emit walk (`:1904-1969`): add a variant==3 branch replacing the field-drain inner loop with the
   variant-dispatch loop copied from `push_array_of_enum_eq` (`:1752-1793`) / `push_enum_eq`
   (`:1441-1483`), operating over the nested temps l2/r2, extracting the top-level enum field via op 48
   (`topnestacc` = `getfieldnested` for a struct container) with the FlatNested-Enum operand
   (`extp` packs variant 3).

**Commit E — `EXPECTED_SELF_COMPILE`.** If steps 5/6 edit the existing `push_struct_eq_nested`
(no new codegen `fn`), the count STAYS 68. If a helper `fn` is factored out, it rises to 69 —
FLAG and update `tests/selfhost_codegen.rs:~1988`. Recommend inlining (as tuple-of-struct did).

## Test and guardrails

- Boundary flip (`tests/selfhost_codegen.rs:~7449`): `eq/enum_in_struct__GAP` Gap -> Ok. 48->49 Ok.
- New test `self_host_compiles_enum_in_struct_equality`, modeled on
  `self_host_compiles_tuple_of_struct_equality` (`:~6934`), using `assert_self_host_byte_identical`
  over: unit-only enum field `==` and `!=`; payload-bearing (scalar) enum field `==`; enum field at a
  nonzero offset; enum field beside a nested struct.
- Blast-radius (must stay byte-identical after each commit): all five whole-stage self-compiles;
  `self_host_compiles_{nested_struct,nested_tuple_field,array_in_struct,tuple_of_struct}_equality`
  (variants 0/1/2 untouched); `self_host_compiles_{enum,array_of_enum}_equality` (the reused
  machinery); `self_hosted_construct_support_boundary`; fmt; clippy `-D warnings`; subproject build.
- Develop B+C+D together (the enum test spans all three stages); the fast inner loop is
  `KEL_SELFHOST_CACHE=1 scripts/fast-check.sh 'test(self_host_compiles_enum_in_struct_equality)'`.
  Run the FULL `scripts/release-gate.sh` before claiming complete or merging (a spot check misses
  op-table capacity regressions — the lesson from the tuple-of-struct increment).

## Critical files

- `compiler/kel/parse.kel` — `sd_fenum` (DONE); `struct_eq_kind` `:2194`; `structeq_nested_next`
  phases 0/1 `:2335`; reuse `enumeq_variant_record`/`step_enumeq_emit` `:2549`.
- `compiler/kel/reconstruct.kel` — `seb` assembler `:722-781`; `build_struct_eq_nested` `:448`;
  records 49/52 routing `:701`/`:711`.
- `compiler/kel/codegen.kel` — `push_struct_eq_nested` `:1817`; models `push_array_of_enum_eq`
  `:1716` / `push_enum_eq` `:1420`; `push_enum_isenum` `:1263`.
- `tests/selfhost_codegen.rs` — boundary flip `:~7449`; new test near `:~6934`;
  `EXPECTED_SELF_COMPILE` `:~1988`; driver `decode_op` `:~793` (already handles variant 3).
- `src/compiler.rs` — reference ground truth: `emit_composite_fieldwise_eq` `:6651`,
  `emit_enum_fieldwise_eq` `:6844`, `classify_flat_field` `:6265`.

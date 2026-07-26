# Enum-with-Struct-Payload Equality — Implementation Plan (working)

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Working plan for the next self-hosted-compiler nested-equality increment: `a == b` where an
enum variant carries a STRUCT payload, for example `struct P { x: Word }`, `enum E { A(P), B }`.
It flips the `eq/enum_struct_payload__GAP` boundary case (49 Ok / 5 Gap becomes 50 Ok / 4 Gap).

Status: **COMPLETE — implemented, byte-identical.** Boundary moved 49 → 50 Ok (4 Gap / 1 RefRejects);
`EXPECTED_SELF_COMPILE` 69 → 70 (the factored `push_enum_struct_payload_loop`). Two deliberate
simplifications versus this scouted plan: (1) interning stayed DEFERRED (`push_bool`) rather than an
eager pre-pass — `push_enum_eq` is uniformly deferred (unlike the fully-eager `push_struct_eq_nested`),
so the inner struct loop's false/true follow forward-emission pool order for free and a literal operand
is handled correctly too; (2) the subcount was streamed as its own record (packing the two extract
temps r2/l2) rather than packed into the header, avoiding any high-bit record-transport risk. A
dedicated `enum_eq_supported_wide` admits struct payloads on the standalone gate ONLY; the
array-of-enum gate stays strictly scalar-only (no latent mis-compile). See the 2026-07-26
DESIGN_JOURNAL entry.

Original scouting (retained): A Plan agent confirmed no design-decision
STOP: no new opcode, no `BYTECODE_VERSION` bump, no new record or node kind. This is the structural
MIRROR of the completed enum-in-struct increment ([`ENUM_IN_STRUCT_PLAN.md`](./ENUM_IN_STRUCT_PLAN.md)):
enum-in-struct put an inner enum-dispatch loop inside an outer struct-eq loop; this puts an inner
struct-eq loop inside the outer enum-dispatch loop. It is on a DIFFERENT code path — the standalone
`enum == enum` path (`op_lenum` gate), which flows through `enum_eq_start`/`build_enum_eq`/
`push_enum_eq` using `eqfields`, NOT the `seb`/`push_struct_eq_nested` path. So it does NOT touch
`push_struct_eq_nested` or the variant-3 `seb` machinery; those are orthogonal.

## Why no STOP

1. The struct-payload extract is `GetEnumField(FlatNested{off,size,Struct})` = op 57
   (`getenumfieldnested`, variant tag 2). `push_enum_match` (codegen.kel ~1328-1329) already emits it
   for struct payloads, and the driver `decode_op` (tests/selfhost_codegen.rs ~971-981) already
   decodes op-57 variant tag 2 -> `CompositeKind::Struct`. Inner field compares are `GetField`
   (op 47) + `CmpEq`, already emitted by `push_struct_eq`.
2. The payload struct index is ALREADY tracked at parse time: a struct-payload variant field records
   `evfkind[fi] = 100 + sd_bytesize` (the struct sentinel, parse.kel ~1516) AND `evfstruct[fi] =
   struct_index + 1` (parse.kel ~1517). Unlike tuple-of-struct (which added `tup_estruct`), no new
   tracking is needed — the inner struct's layout (`sd_fstart/sd_fcount/sd_foffset/sd_fkind`) is
   reachable.
3. The sub-field list streams as extra `EnumEqField` (record 52) entries under an extended
   per-variant-field header — no new record kind.

## Reference op sequence (byte-identity target)

`emit_enum_fieldwise_eq` (src/compiler.rs ~6844). For variant A's struct payload P,
`enum_field_access` (compiler.rs ~7011) returns `EnumField::FlatNested{off,size,Struct}`, and
`composite_needs_fieldwise_eq(P)` is true, so it recurses into `emit_composite_fieldwise_eq(P)`
over fresh temps. Inside the variant-A "both are A" block:

```
GetLocal(ltmp); GetEnumField(FlatNested{off,size,Struct})   # op 57
GetLocal(rtmp); GetEnumField(FlatNested{off,size,Struct})   # op 57
SetLocal(r2); SetLocal(l2)                                  # fresh temps l2/r2
  Loop                                                      # emit_composite_fieldwise_eq(P) over l2/r2
    # per struct field x:
    GetLocal(l2); GetField(Flat{off_x,Word})
    GetLocal(r2); GetField(Flat{off_x,Word})
    CmpEq; Not; If; Const(false); Break; EndIf
    Const(true); Break
  EndLoop
Not; If; Const(false); Break; EndIf                         # payload-unequal path of the enum loop
```

Variant B (unit): `Const(true); Break`; variant tail `Const(false); Break`; loop tail `Trap(4)`.
Constant-pool order: `"E", "A", Int(0), Bool(false), Bool(true), "B", Int(1)` — the inner struct
loop's false/true take the same pool indices the outer enum loop's would, so byte-identity holds for
the no-literal `a == b` case.

## Ordered edit plan (mirrors ENUM_IN_STRUCT_PLAN.md)

**Commit A — parse: admit the struct-payload variant and stream its sub-fields.**
- `enum_eq_supported` (parse.kel ~2531): change the reject test to admit `evfkind` in the struct
  sentinel range `[100, 30000)` (STRUCT) while STILL deferring TUPLE (`30000+`) and ARRAY (`40000+`)
  payloads, and STILL deferring a struct payload whose own field is composite (hold the one-level
  bound). Scoping this narrowly is essential — widening past STRUCT risks regressing the boundary.
- `enumeq_variant_record` (parse.kel ~2639) / `step_enumeq_emit` (parse.kel ~2650): for a
  struct-payload field, after its `EnumEqField` header, drain the payload struct's sub-fields
  (`sd_fstart[evfstruct[fi]-1] .. +sd_fcount`) as extra `EnumEqField` records `(sd_foffset,
  sd_fkind)`, preceded by a subcount marker. Tag the field as struct-payload and carry its subcount
  in spare high bits of the existing `off + kind*65536` payload (or a distinguishing kind range) —
  no new record.

**Commit B — reconstruct: lay the struct sub-field list into `match_parts`.**
- `build_enum_eq` (reconstruct.kel ~285) and the record-52 handler (reconstruct.kel ~734): when a
  field is struct-payload, write `[struct-marker, off, size, subcount, subcount*(sub_off, sub_kind)]`
  instead of the plain `(off, kind)`; scalar fields unchanged. (Flat-path analog of enum-in-struct's
  seb variant-3 layout.) If `match_parts` proves tight for a many-field payload, that is a capacity
  bump, not a STOP.

**Commit C — codegen: emit the op-57 extract + inner struct-eq loop.**
- `push_enum_eq` (codegen.kel ~1420), per-field loop (~1460-1474 currently emits only
  `getenumfield`+`CmpEq`): branch on the struct-payload marker. For a struct payload emit
  `GetLocal(ta); getenumfieldnested(off + size*65536 + 2*4294967296); GetLocal(tb);
  getenumfieldnested(...); SetLocal(r2); SetLocal(l2)` then the inner struct-eq `Loop` over l2/r2
  (from `push_struct_eq`, using `getfield`), then `Not;If;Const(false);Break;EndIf`. FACTOR the inner
  loop into a helper (`push_enum_struct_payload_loop(...)`) to stay under the 1536 op cap.
- Interning: add a pre-pass in `push_enum_eq` (GUARDED to run only when a struct-payload field is
  present, so the pure-scalar path stays byte-identical) that eagerly interns the inner struct loop's
  false/true in reference emission order — exactly as `push_struct_eq_nested`'s variant-3 pre-pass.
  Two fresh temps per struct-payload field grow `let_count`.

**Commit D — test + counts.**
- Flip `eq/enum_struct_payload__GAP` Gap -> Ok (tests/selfhost_codegen.rs ~7499). Boundary 49/5 -> 50/4.
- New `self_host_compiles_enum_struct_payload_equality`, modeled on
  `self_host_compiles_enum_in_struct_equality` (~6962): single- and multi-field struct payload,
  `==`/`!=`, a struct payload beside a scalar payload in one variant, and a mixed `A(Word, P)` variant.
- Bump `EXPECTED_SELF_COMPILE` 69 -> 70 (tests/selfhost_codegen.rs ~1990) if the helper is factored.

## Gotchas (all from the enum-in-struct precedent)

- **Eager/deferred interning composition** (the sharp one): the outer enum loop DEFERS (`push_bool`)
  so a literal `E::A()` operand can intern first; the inner struct loop must EAGER-intern its
  false/true. For `a == b` (no literals) the orders coincide (the DESIGN_JOURNAL 2026-07-25 entry
  established this). Guard the eager pre-pass to fire only for struct-payload fields, keeping the
  scalar-only path byte-identical.
- **Op cap**: factor the inner loop (like `push_nested_enum_loop`); `EXPECTED_SELF_COMPILE` 69 -> 70.
- **analyze.kel scan bounds**: re-verify no per-chunk-op scan still binds at 1024 (enum-in-struct
  raised two to 1536; the enlarged `push_enum_eq` chunk may approach the cap).
- **Scope `enum_eq_supported` to STRUCT only**: keep TUPLE/ARRAY (`30000+`/`40000+`) and
  composite-in-struct payloads deferred, or the increment exceeds its bound and regresses counts.

## Critical files

- `compiler/kel/parse.kel` — `enum_eq_supported` (~2531), `enum_eq_start` (~2548),
  `enumeq_variant_record` (~2639), `step_enumeq_emit` (~2650); struct-payload tracking already at
  ~1516-1517 (`evfkind`/`evfstruct`).
- `compiler/kel/reconstruct.kel` — `build_enum_eq` (~285), record-49/52 handlers (~710, ~734).
- `compiler/kel/codegen.kel` — `push_enum_eq` (~1420) per-field loop (~1460-1474); models
  `push_struct_eq` (~1351) inner loop, `push_nested_enum_loop` (the factoring precedent), op-57 emit
  (~1328-1329).
- `tests/selfhost_codegen.rs` — boundary flip (~7499), new test near ~6962, `EXPECTED_SELF_COMPILE`
  (~1990), op-57 decode already present (~971).
- `src/compiler.rs` — reference ground truth: `emit_enum_fieldwise_eq` (~6844),
  `emit_composite_fieldwise_eq` (~6651), `composite_needs_fieldwise_eq` (~6518), `enum_field_access`
  (~7011).

## Verification

Same as enum-in-struct: `KEL_SELFHOST_CACHE=1 scripts/fast-check.sh
'test(self_host_compiles_enum_struct_payload_equality)'` for the inner loop; then the blast-radius
suite (all five whole-stage self-compiles, the full equality suite, `validate_module_via_kel`, the
boundary); then the FULL `scripts/release-gate.sh` before merge (it catches op-cap and analyze
scan-bound regressions a spot check misses). On green, merge the feature branch into `v0.2.3` with a
no-fast-forward merge commit (`git merge --no-ff`), push, confirm CI green.

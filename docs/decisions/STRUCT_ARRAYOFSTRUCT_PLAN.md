# Struct-of-Array-of-Struct Equality — Implementation Plan (blueprint)

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Blueprint for the next self-hosted-compiler nested-equality increment: `a == b` where a struct field
is an array whose element is itself a struct, for example `struct P { x: Word }`, `struct Q { ps: [P; 2] }`.
It flips the `eq/struct_arrayofstruct__GAP` boundary case (51 Ok / 3 Gap becomes 52 Ok / 2 Gap).

Status: **COMPLETE — implemented, byte-identical, merged (increment 5).** Boundary 51 → 52 Ok (2 Gap /
1 RefRejects); `EXPECTED_SELF_COMPILE` 71 → 72 (a factored `push_arr_of_struct_inner`). Implemented per
this blueprint (parse `se_arrsphase` sentinel/packing stream, reconstruct `se_arr_mode` reassembly,
codegen `push_arr_of_struct_inner` per-element loop). Two host-side capacity bumps were also required
(no ISA impact): the lexer `src.bytes` buffer 245760 → 393216 (parse.kel outgrew it), and the
`dl_reject_module_via_kel` layout-verifier arena → 4 MB (the larger per-element layout). See the
2026-07-26 DESIGN_JOURNAL entry. Original blueprint retained below.

## Why no STOP (feasibility confirmed)

- **No ISA change.** Reuses op 48 GetFieldNested (the array extract, FlatNested variant 1), the
  `getindexnested` op with a FlatNested Struct element (`arrsize + 2*65536`) already emitted by
  `push_array_of_struct_eq`, records 55/56/57/58, and node 59. No new opcode, record, node kind, or
  `BYTECODE_VERSION` change.
- **The element-struct index is already tracked.** An array-of-struct field sets BOTH
  `sd_farraylen[fidx] > 0` AND `sd_fstruct[fidx] > 0`. The element struct is `sd_fstruct[fidx] - 1`, the
  element count is `sd_farraylen[fidx]`, and the element byte size is `sd_bytesize[es]`. No struct-def
  builder change is required.
- **The codegen template exists.** `push_array_of_struct_eq` (codegen ~1637) is the exact per-element
  body. The nested case runs it over the extracted array temps `l2` (left) / `r2` (right), with inner
  temps at `l2 + 1 + 2*e` (right = `inner_b`) and `l2 + 2 + 2*e` (left = `inner_a`), which matches the
  reference's monotonic slot order (array `r2=4, l2=5`; element 0 `6,7`; element 1 `8,9`).
- **No recursion.** As with every depth increment, use explicit phases, not a self-recursive `.kel`
  function (the verifier forbids recursion, R4). This composes the array sub-drain (`se_subisarray`)
  with the element struct-eq loop.

## Reference lowering (byte-identity target)

`emit_composite_fieldwise_eq` (src/compiler.rs ~6651) for a struct field of array-of-struct type:
extract the array field into a temp pair (GetFieldNested variant Array, op 48), then PER ELEMENT extract
`a[e]` / `b[e]` as structs (`getindexnested` FlatNested Struct) into an inner temp pair and run an inner
struct-eq field loop, breaking the element loop false on the first mismatch, true after all elements.
This is exactly `push_array_of_struct_eq`'s per-element unroll nested UNDER a struct-field array
extraction. Cross-check `composite_field_accessors` (~6568) for the element-index constant order.

## Ordered edit plan (4 stages)

**Stage 1 — parse `struct_eq_kind` (~2250).** Admit an array field whose element is a struct with
all-scalar leaves (scan the element struct's fields; defer if any is composite). Still defer
array-of-tuple and array-of-array.

**Stage 2 — parse `structeq_nested_next` field detection (~2529).** Add an array-of-struct branch
BEFORE the array-of-scalar one (`sd_farraylen[fidx] > 0 andalso sd_fstruct[fidx] > 0`): allocate `r2, l2`
then `2*acount` inner temps (`slot_count += 2 + 2*acount`), set `se_substart` / `se_subcount` to the
element struct's field range, arm `se_arrsphase = 1`, and emit the variant-1 `StructEqNested` header.
Then in the phase-1 drain, a `se_arrsphase == 1` pre-step emits ONE packing record
`StructEqNestedField(acount + (100 + arrsize)*65536 + fieldcount * 2^32)` — the `100 +` sentinel marks a
struct-element array — then falls into the normal struct sub-drain which emits the element fields then
`StructEqNestedEnd`. Add four `stmt` fields (`se_arrsphase`, `se_arr_acount`, `se_arr_asize`,
`se_arr_fcount`) and one extra closing brace in the field if-chain.

**Stage 3 — reconstruct variant-1 handling (~847 header, ~787 sub-field).** When the first k=55 after a
variant-1 header has a kind-part `>= 100`, it is the packing record: lay seb `[acount, arrsize,
fieldcount]` and route the next `fieldcount` k=55 records as `[off, kind]` (a small `se_arr_mode` state,
analogous to `se_nsub_mode`). A scalar array (`kind < 100`) is unchanged. The seb for a struct-element
array field is `[1, ext_off, ext_size, 1, r2, l2, acount, 100 + arrsize, fieldcount,
fieldcount*(off, kind)]`.

**Stage 4 — codegen `push_struct_eq_nested` variant-1 branch (~2202).** If `seb[foff + 7] >= 100`
(struct element), emit the per-element struct loop INLINE (mirror `push_array_of_struct_eq` lines
~1667-1710 with `ta = l2`, `tb = r2`, `inner_b = l2 + 1 + 2*e`, `inner_a = l2 + 2 + 2*e`,
`getindexnested(arrsize + 2*65536)`); else the existing scalar loop. Update the stride pass (a
struct-element array field is `9 + fieldcount*2` words versus a scalar array's 8) and reserve `2*acount`
in `let_count`. Then the test: flip `eq/struct_arrayofstruct__GAP` to SOk (boundary 52 Ok / 2 Gap), add
`self_host_compiles_struct_arrayofstruct_equality` (cases `[P;2]`, `[P;3]`, a multi-field element, `!=`,
and an array-of-struct beside a scalar top field), and bump `EXPECTED_SELF_COMPILE` only if a helper is
factored.

## Interning (the sharp edge)

`push_struct_eq_nested` is fully EAGER. The existing eager array pre-pass branch
(`for i in 0..seb[foff + 6] { intern_int(i) }` then false, true) already produces the reference order
(element indices `0..n-1` first, then false, then true, per `composite_field_accessors`). Verify it
reads `acount` at `foff + 6` (it does) and populates `st.eq_vidx` for the emission walk. **Implement the
nested element loop INLINE rather than factoring a shared helper**: false/true must intern AFTER the
element indices, and factoring `push_array_of_struct_eq`'s body into a shared helper would invert that
order and break byte-identity. Keeping it inline holds the interning order under direct control.

## Verification

Same discipline as increments 3 and 4: `KEL_SELFHOST_CACHE=1 scripts/fast-check.sh
'test(self_host_compiles_struct_arrayofstruct_equality)'`; then all five whole-stage self-compiles, the
full nested-eq blast-radius suite, `validate_module_via_kel`, the boundary, and the codegen self-compile
count; then the FULL `scripts/release-gate.sh` (run exactly ONE gate per worktree — concurrent cargo
runs contend on the build lock; redirect to a logfile rather than piping through `tail` to monitor
progress). On green, no-fast-forward merge into `v0.2.3`, push, confirm CI.

## After this increment

The same-context nested-equality frontier is then the deferred tail: a third struct level (needs a
general depth stack rather than a fixed extra phase — likely a design-decision stop) and the
floats/generics tail (out of scope for the self-hosted subset). At that point re-weigh a workstream
switch, for example wiring the self-hosted stages into the shipping binary, and surface the choice to
the operator.

# Third-Level Struct-Nesting Equality — Design Blueprint

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Blueprint for closing the third-level nested-struct-equality gap in the self-hosted compiler:
`a == b` where a struct field is a struct whose field is a struct whose field is a struct, for
example `struct A { x: Word }`, `struct B { a: A }`, `struct C { b: B }`, `struct D { c: C }`,
`fn f(p: D, q: D) -> bool { p == q }`. Three nested struct extractions must lower byte-identically
to the reference `emit_composite_fieldwise_eq`.

Status: **PROPOSED — not yet implemented.** This blueprint records the design and a material
cost and risk fork surfaced by the mechanism mapping. The operator selected the general
bounded-depth-stack approach at the direction fork; the cheaper incremental alternative is
recorded under "Cost and risk fork" for a final go decision before the long implementation.

## What "level" counts

The existing `eq/2level_struct` case is `struct I { v: Word }`, `struct M { i: I }`,
`struct O { m: M }`. Comparing `O` extracts `m` (nested extraction one), comparing `M` extracts
`i` (nested extraction two), comparing `I` compares the scalar `v`. That is two nested struct
extractions, hence "2-level". The new `eq/3level_struct` case adds one more struct type so the
drain performs three nested struct extractions. This increment ADDS a new `SOk` case
(52 -> 53 Ok); it does NOT flip an existing Gap. The two current Gaps (`scope/float_arith__GAP`,
`scope/generic_fn__GAP`) are permanently out of scope.

## The reference (byte-identity target)

`emit_composite_fieldwise_eq` (`src/compiler.rs` ~6651) is cleanly recursive. For a struct type it
emits `Loop`, then per field extracts the left then the right value into the operand stack; if the
field is itself a composite it declares two locals `__eqf_r` then `__eqf_l` (r first, l second),
emits `SetLocal(r)` then `SetLocal(l)`, and RECURSES with `(l, r)`; a scalar field emits `CmpEq`.
Each field then emits `Not`, `If`, `Const(false)`, `Break`, `EndIf`. After all fields it emits
`Const(true)`, `Break`, `EndLoop`. Locals are allocated by `declare_local` and are monotonic across
the whole function, never rewound by `end_scope` for byte-identity purposes. The byte-identity
target is therefore a depth-first preorder over the field tree, with a two-slot local frame
allocated at each composite field in the order the DFS first reaches it (r before l, plus two per
composite field).

## The current mechanism is a FIXED depth-2 special case

The mechanism mapping (2026-07-29) established that depth 2 is hardcoded, not extensible:

- **parse.kel**: two distinct phase machines. `se_phase` drains the depth-1 nested struct's
  immediate sub-fields; `se_l2phase` (values 0 -> 1 -> 2 -> 0) drains a depth-2 sub-sub-field. The
  depth-2 packing record (`~2487`) carries only `se_l2subcount + se_l2r2*65536 + se_l2l2*2^32`.
  There is no phase 3 and no `se_l3*` frame, and a sub-sub-field of kind `>= 100` (a depth-3
  struct) would be emitted as a scalar (`~2498`), corrupting the stream.
- **reconstruct.kel**: `se_nsub_mode` (0 -> 1 -> 2 -> 0) folds the depth-2 packing and sub-sub-fields
  into `seb`. There is no `se_nsubsub_mode`; a depth-3 kind `>= 100` would be misread as a scalar
  `[off, kind]` (`~847`).
- **codegen.kel**: `push_struct_eq_nested`'s slot-count pass hardcodes the depth-2 sub-field stride
  `se_cur + 5 + subcount*2` (`~2142`); `push_struct_eq_subfields` (`~1968`) walks a fixed two-level
  `seb` shape (`[sub_off, 100+size, subcount, r2p, l2p, subfields...]`) and emits a single inner
  loop. No layer reads a depth-3 sub-sub-sub-field.

## Design — a bounded depth stack (the general approach)

Replace the per-level phase variables in each stage with an explicit fixed-capacity STACK of
frames indexed by a depth pointer, driving the same postorder traversal the reference performs by
recursion. The verifier forbids recursion (R4), so an explicit stack with a compile-time depth cap
is the required shape, and the cap keeps worst-case execution time and worst-case memory usage
statically bounded.

- **Depth cap**: a `const MAX_EQ_DEPTH` (proposal 4, covering three nested extractions with one to
  spare). A program that nests deeper defers to the reference (the CLI backend's cross-check catches
  it as `Unsupported`), so the cap is safe, not a correctness hazard. State the cap in a `log` and
  the boundary test so the truncation is not silent.

- **parse.kel**: a frame is `{ substart, subcount, subcur, r2, l2 }`. Keep the depth-1 intro
  (allocate r2, l2, emit the `StructEqNested` header, push frame 0). The drain step inspects the top
  frame's next sub-field: a scalar emits its `[off, kind]` record and advances `subcur`; a nested
  struct (kind `>= 100`) emits the header sub-field record, allocates r2/l2 monotonically, emits the
  packing record, and PUSHES a new frame; when a frame's `subcur == subcount` it emits
  `StructEqNestedEnd` (or the pop marker) and POPS, advancing the parent's `subcur`. This is an
  iterative preorder tree walk. The existing depth-1 and depth-2 code becomes the depth-0 and
  depth-1 frames of this one loop.

- **reconstruct.kel**: replace `se_nsub_mode` with a remaining-count stack that mirrors the record
  stream into a recursively nested `seb`. The `seb` grammar generalizes to: a field is either
  `[0, off, kind]` (scalar) or `[100+size marker..., subcount, r2, l2, field*]` where each `field`
  is recursively either shape. The reassembly pushes a frame on a nested-struct sub-field and pops
  when its remaining count hits zero.

- **codegen.kel**: both the slot-count pass and the emission pass walk the `seb` tree with an
  explicit stack. The emission must reproduce the reference DFS order exactly: extract left then
  right, `SetLocal(r)` then `SetLocal(l)`, descend, and on the way back up emit
  `Not`/`If`/`Const(false)`/`Break`/`EndIf`. The monotonic slot allocation (r before l, plus two per
  composite field, never rewound) and the constant interning order (element/false/true per the
  reference) are the two byte-identity pivots and must match the reference DFS precisely.

## Cost and risk fork (surfaced by the mapping — for the go decision)

The mapping shows the general rewrite touches every nested-equality drain in all three stages and
is the highest-risk byte-identity change attempted so far. There is a materially cheaper
alternative that the direction fork did not distinguish:

- **A. General bounded depth stack (operator's selection).** Closes depth 3 AND every deeper level
  at once. Largest rewrite, highest byte-identity risk, most tokens. Best long-term shape.
- **B. Incremental fixed depth-3 phase.** Add one more phase `se_l3phase` and one `se_nsubsub_mode`
  mirroring the existing depth-2 layer one level deeper, plus the depth-3 branch in
  `push_struct_eq_subfields`. Closes exactly `eq/3level_struct` (52 -> 53 Ok). Far smaller, far lower
  risk, reuses the proven depth-2 pattern verbatim. Defers the general stack; a depth-4 case would
  need a depth-5 phase, so it does not scale, but it delivers the same boundary movement now.

Both move the boundary identically (53 Ok). They differ only in generality, cost, and risk. Given
the seven-day rate-limit window is the binding budget, this trade-off is worth an explicit go
before the long implementation.

## Ordered edit plan (approach A)

1. **parse.kel** — add the frame stack (`se_fstack_*` arrays sized `MAX_EQ_DEPTH`) and the depth
   pointer; rewrite the nested-equality drain as the single iterative preorder walk; retire
   `se_l2phase` and its scalar fields. Record-stream output must be UNCHANGED for the depth-1 and
   depth-2 cases (verify against the existing `eq/2level_struct` fixture first).
2. **reconstruct.kel** — replace `se_nsub_mode` with the remaining-count stack; emit the same `seb`
   bytes for depth-1 and depth-2 as today, plus the depth-3 nesting.
3. **codegen.kel** — rewrite the `seb` walk in the slot-count and emission passes as explicit-stack
   DFS; keep depth-1 and depth-2 op streams byte-identical. Watch the 1536-op analyze cap and the
   `ops: [Word; 2048]` buffer; a factored helper may be needed to stay under the op-table cap
   (bump `EXPECTED_SELF_COMPILE` only if a helper is factored).
4. **tests** — add `eq/3level_struct` as `SOk` to `self_hosted_construct_support_boundary` (52 -> 53
   Ok), add `self_host_compiles_3level_struct_equality`, and assert the depth cap rejects deeper
   nesting cleanly (defers to reference). Confirm no capacity regression via the whole self-compile
   suite.

## Verification

Same discipline as increments 3 through 5. First confirm the depth-1 and depth-2 fixtures remain
byte-identical after each stage rewrite (a regression here means the generalization changed the
existing output — a stop). Then the new depth-3 fixture byte-identical against the reference, the
whole nested-equality blast-radius suite, `validate_module_via_kel`, the boundary, and the codegen
self-compile count. Then the FULL `scripts/release-gate.sh` (one gate per worktree). On green, no-ff
merge into `v0.2.3`, push, confirm CI.

## Progress (2026-07-29)

- **Stage 1 (parse.kel) — DONE, committed `13b922f`, verified green.** The fixed depth-2 `se_l2*`
  fields are replaced by a general `se_stk_*` frame stack plus a `se_pop_cascade` helper. A struct
  sub-field at any level pushes a frame; a scalar emits and, when the frame's last, pops and cascades
  up (advancing the parent cursor, cascading through parents whose only remaining field was that
  struct). Depth-1 and depth-2 record output is byte-identical. Verified: boundary unchanged
  (52 Ok / 2 Gap / 1 RefRejects) and `self_host_compiles_parse_kel_byte_identically`.
- **Stage 2 (reconstruct.kel) — DONE, committed `c667875`, verified green.** `se_nsub_mode`/
  `se_nsub_remaining` are replaced by a general `se_nstk_*` frame stack plus a `se_nsub_pop` cascade.
  The `seb` grammar for a nested-struct sub-field is now recursive: `[off, 100+size, subcount, r2, l2,
  field*]` where each `field` is a scalar `[off, kind]` or another such block. A struct sub-field is
  COUNTED when its header is laid, so a frame completes when its child subtree finishes and the pop
  cascades (checking, not decrementing, the parent's remaining). Depth-1/2 `seb` bytes are
  byte-identical. Verified: boundary unchanged and `self_host_compiles_reconstruct_kel_byte_identically`.
- **Stage 3 (codegen.kel) — NOT STARTED.** Design below.

## Stage 3 design — codegen explicit-stack reverse-DFS emitter (TODO)

`push_struct_eq_nested` (codegen.kel ~2095) has three passes: a slot-count forward pass (~2104), an
eager intern pass (~2159), and a REVERSE emission pass (~2215). The nested-struct case calls
`push_struct_eq_subfields` (~1968), which emits a composite's sub-fields in reverse and INLINES the
depth-2 nested case (extract into r2'/l2', an inner loop over SCALAR sub-sub-fields only, ~1998-2011).
Three fixed-depth-2 assumptions must generalize, keeping depth-1/2 ops byte-identical:

1. **Slot-count pass (~2139-2146).** The struct-field sub-loop counts `+2` temps per depth-2 struct
   sub-field and strides `5 + subsubcount*2`. Generalize to a stack walk over the recursive `seb`
   grammar: every nested-struct sub-field at any depth contributes `+2` temps (r2', l2') and its
   stride is `5 + sum(child strides)`. The temp COUNT is what matters for `let_count`; walk the whole
   `seb` subtree.

2. **Intern pass (~2159-2200).** A nested struct/tuple field interns `false` then `true`. For depth-3
   the interning order must still match the reference DFS: the reference interns per composite compare
   `false` (per field, on inequality) then `true` (loop tail). Because all bools dedup to two indices
   after the first occurrence, and the first nested field already interns false then true, deeper
   levels add no NEW constants. So the intern pass likely needs NO change for the pure-struct case
   (confirm by running the depth-3 fixture). If a mismatch appears, replay the nested sub-fields'
   false/true in DFS order.

3. **Emission (`push_struct_eq_subfields` ~1968).** The core change. The reversed emission for a
   nested-struct sub-field is: `outer-negate-break` (mendif mbreak konst(false) mif lnot), then the
   nested compare block (mendloop; konst(true) mbreak; ITS SUB-FIELDS reversed; mloop), then the
   extract (setlocal(l2p) setlocal(r2p) getfieldnested(extp) getlocal(r2) getfieldnested(extp)
   getlocal(l2)). "ITS SUB-FIELDS reversed" is the recursion. Replace the SCALAR-only inner loop with
   an explicit-stack reverse-DFS: a frame is `{sbase, subcount, cur (reverse index), l, r}`. For a
   scalar sub-field emit the scalar block; for a nested one, emit the outer-negate-break + loop-close,
   then PUSH a child frame for its sub-fields, and record (on a parallel "pending extract" stack) the
   loop-open + extract ops to emit once the child frame is exhausted. Because emission is reversed,
   the child's ops land BETWEEN the loop-close and the loop-open+extract, which is exactly the
   reference nesting. The seb offsets (`soff+2` subcount, `soff+3` r2', `soff+4` l2', `soff+5+...`
   sub-fields) recurse one level deeper per frame. Watch the 1536 analyze op-table cap; a depth-3
   unrolled compare grows the op count, so a helper factor (bump `EXPECTED_SELF_COMPILE`) may be
   needed. This emitter is the highest byte-identity risk; verify the depth-2 fixture stays identical
   FIRST (a regression there means the generalized emitter changed existing output — a stop), then the
   new depth-3 fixture.

## Stage 4 — wire depth-3 (TODO)

Add `eq/3level_struct` (`struct A{x:Word} struct B{a:A} struct C{b:B} struct D{c:C}`,
`fn f(p:D,q:D)->bool{p==q}`) to `self_hosted_construct_support_boundary` as `SOk` (52 -> 53 Ok), add
`self_host_compiles_3level_struct_equality`, and confirm the whole self-compile suite plus the FULL
`scripts/release-gate.sh` stay green before the no-ff merge into `v0.2.3`.

## Capacity risks

- The analyze op-table cap is 1536 (`tests` assert). A depth-3 unrolled compare adds ops; factor a
  helper if `push_struct_eq_nested` approaches the cap.
- `seb` and `match_parts` are 256 words; a deeply nested `seb` tree could approach it. The depth cap
  and the small fixture keep this within bounds, but the whole self-compile suite must stay green.
- parse `slot_count` accumulates two per composite field at every level; the depth cap bounds it.

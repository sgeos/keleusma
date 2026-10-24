# Order 3 handler surface plan

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**. Scope resolved by the operator on 2026-10-20. Single-index
indexed-write handlers and the index-first order are implemented in the Rust
reference compiler and mirrored by the self-hosted compiler on 2026-10-21, with
the self-hosted mirror limited to Word and Byte elements. Handled multi-index
writes are implemented in the Rust reference compiler on 2026-10-22, and the
self-hosted compiler refuses them. FixedDiv totalization is implemented on
2026-10-22. Native-call handlers and handler-required composite reads are not
started.
This document resolves the open question that the 2026-10-20 handoff recorded as
the first audit priority, which asked whether source-level indexed-write handlers
belong to Order 3. They do. It extends [the completion plan](./ORDER3_COMPLETION_PLAN.md)
and adds acceptance rows to it.

## Operator rulings, 2026-10-20

The rulings below were given directly by the operator in the Opus 5.5 audit
session of 2026-10-20. They are first-party, not relayed. Quoted text is the
operator's wording.

| Topic | Ruling |
|---|---|
| Indexed-write handlers | "Yes, add indexed-write handlers." |
| The ok arm | "The `ok` arm should bind the old and new values, either of which can be ignored. `ok(old, new) => ...`" The compiler generates code only for captured values, so the four capture patterns emit different amounts of code. |
| Reading the old value | The read is emitted only when `old` is bound. "In short, only conditionally emit the read. The condition is whether it is needed or not." |
| Evaluation order | "Index should be evaluated first." |
| Short circuit | "A bad index should always short circuit, even if the default trap logic is used." A program that must always evaluate the value binds it with `let` before the write. |
| Expression form | "A handled write should be an expression that yields a value." Its type is the common type of its arms, discarded when used as a statement. |
| Scope | "I think the scope change is everything from surface grammar to codegen." |
| Composite reads | Composite elements have no default fallback. "Require the handler and otherwise fall back to the trap." |
| Host-contract faults | Native-call handlers are in scope. Faults detectable at load are rejected at load. Runtime host failures are reported as a handled outcome, `host_error => ...`, with a compiler-default trap when no handler is written. |
| Non-trap faults generally | "Host-contract faults and indexing forms outside the first pass should not explicitly trap. They should set VM flags on failure where the compiler defaults to a trap if there is no explicitly provided logic." |
| Failure signal mechanism | A processor-style flags register replaces every in-band outcome flag. The diagnostic payload stays on the stack only where a handler binds it. Flags behave as on a processor. A flag-select form of the existing conditional branch reads them, with the exact encoding recorded in a design document before implementation. **Deferred to V0.2.4.** "Switching from in-band to flags can happen in V0.2.4." |
| Bytecode version | Frozen with the fingerprint until the next public version. "A version bump will occur when there is evidence of traction. I have no evidence anyone is using Keleusma." |
| Unchecked execution | "The Keleusma project rejects undefined behavior, but it does not reject platform specific behavior." Proven execution keeps a real length check on the fast path. |

## Consequences for V0.2.3

V0.2.3 keeps the in-band outcome pattern that already exists. The new handler
forms below use it, so that V0.2.3 is internally consistent. All of them move to
the flags register in V0.2.4. Some work is therefore done twice. That cost is
accepted in exchange for a consistent V0.2.3.

The ruling on short circuiting changes current bare-write behaviour. Today a bare
write evaluates its value before the bounds outcome is known. After this change a
bad index skips the value on both the handled and the bare forms. A new control
must show that an invalid index skips a value with an observable effect.

**Correction of 2026-10-21.** This section previously said the existing control at
tests/indexed_write_outcomes.rs line 102 was unaffected because it uses a valid
index. That was wrong. The control records the order of the index and value calls
in private data, and it returns 19 only because the value is evaluated first.
Under index-first evaluation the same program returns 28. The operator confirmed
on 2026-10-21 that index-first evaluation applies to bare writes as well, so the
expectation moves to 28.

## Implementation rulings, 2026-10-21

The operator answered three questions raised by the implementation survey. These
are first-party rulings given in the same session.

| Topic | Ruling |
|---|---|
| Arm attachment | An arm block after the right-hand side of an indexed assignment handles the write. A right-hand side that needs its own handler is parenthesized or bound with `let` first. `d.x[i] = a[j] { ok(v) => v }` previously meant a handled read on the right-hand side, and its meaning changes. |
| Bare evaluation order | Index first, then the bounds check, then the value, for bare and handled writes alike. |
| Index depth | This increment supports a handled write with one index. Both compilers reject a handled multi-index write with the same message. Routing every level's failure to `invalid_index` is the next increment. Superseded on 2026-10-22 by that increment, recorded under "Multi-index increment" below. |

## Design decided for the first increment

These choices follow from the rulings and from the opcode semantics. They are
recorded so that both compilers implement the same bytes.

- **Arm vocabulary.** `ok(old, new)` and `invalid_index(j)`. The `ok` patterns are
  each a binder or `_`. A literal pattern in either `ok` position is rejected.
  `invalid_index` admits the same patterns and guards as the handled read. An
  unguarded `ok(binder or _, binder or _)` arm is required for exhaustiveness.
  An unguarded catch-all `invalid_index` arm removes the fallback trap.
- **Position.** The handled write is accepted as an expression statement ending in
  `;` and as the tail expression of a block. Its type is the common type of its
  arms. Other expression positions, such as a `let` initializer, are a follow-up.
- **Data fields only.** Local array element writes do not exist in the language,
  and `const data` remains immutable.
- **Bounds probe.** `SetDataIndexed` pops the index above the value, and no opcode
  exchanges two operands, so the index is held in a local while the value is
  evaluated. The probe before the value is `BoundsCheck(len)` when the length fits
  sixteen bits. A longer single-level array has no sixteen-bit bound, so its probe
  is `GetDataIndexed` with the element fallback, and the value it reads is
  discarded when `old` is unbound. This is the one case in which the read is
  emitted although `old` is unbound, because no other existing opcode checks a
  twenty-four-bit bound.
- **Old value.** When some `ok` arm binds `old`, and the probe was `BoundsCheck`,
  the success branch reads the element with `GetDataIndexed` before the write.
  The read cannot fail there, because the probe established the bound.
- **Write.** The value is evaluated inside the success branch, after the probe. The
  write is `SetDataIndexed`, whose outcome is already known to be success, so its
  six result words are dropped.
- **Dispatch and fallback trap.** Arms dispatch through the same virtual loop as
  the handled read, on a flag of zero for `ok` and one for `invalid_index`. A
  write without an unguarded catch-all `invalid_index` arm ends in `Trap(8)` with
  the index and the four bound limbs, as the read does.
- **Bare write.** Index, probe, the default `Trap(8)` on an invalid index, the
  index saved to a local, the value, the index restored, then `SetDataIndexed`
  with its result dropped. One local per write site is added to the frame.
- **Slot numbering.** The self-hosted parser numbers locals as it streams tokens,
  so the reference compiler numbers them in source order. The offset slot is
  declared after the index's own locals, which is the parser's `=`. The handled
  write's block of seven, in the order flag, old, four bound limbs, new, is
  declared after the value's own locals, which is the parser's `{`, and before
  the arm binders. The block always has seven slots, because the parser reserves
  it before reading the arms. The reference compiler emits the instructions
  that name the block before the value is compiled with a placeholder slot and
  patches them once the block is declared.

## Reference instruction sequences

Implemented in `src/compiler.rs` on branch feat/v02-indexed-write-handlers on
2026-10-21, as `emit_data_write_probe`, `emit_data_indexed_write` and
`compile_data_index_store`. `o` is the offset slot and `f` the first slot of the
block. `probe` is `BoundsCheck(len)` when the field's slot count fits sixteen bits,
and otherwise the element fallback followed by `GetDataIndexed(base, len)`, which
leaves the element beneath the offset. The fallback is `PushImmediate(4)` for
Word, `PushImmediate(4); WordToByte` for Byte, and `PushImmediate(0)` otherwise.

Bare write, `d.x[i] = v;`.

```
<offset of i>; probe
Const(1); CmpEq; If; Trap(8); EndIf
PopN(4); SetLocal(o); [PopN(1) if the probe read]
<v>; GetLocal(o); SetDataIndexed(base, len); PopN(6)
```

Handled write, `d.x[i] = v { arms }`.

```
<offset of i>; probe
SetLocal(f); SetLocal(f+5); SetLocal(f+4); SetLocal(f+3); SetLocal(f+2); SetLocal(o)
[if the probe read: SetLocal(f+1) when old is bound, else PopN(1)]
GetLocal(f); Const(0); CmpEq; If
  [if old is bound and the probe did not read:
     GetLocal(o); <fallback>; GetDataIndexed(base, len); PopN(6); SetLocal(f+1)]
  <v>; [Dup; SetLocal(f+6) when new is bound]
  GetLocal(o); SetDataIndexed(base, len); PopN(6)
EndIf
Loop
  per arm, in source order:
    GetLocal(f); Const(class); CmpEq; If          class 0 for ok, 1 for invalid_index
    [invalid_index literal: GetLocal(o); Const(lit); CmpEq; If]
    [ok binders: GetLocal(f+1); SetLocal(b_old) and GetLocal(f+6); SetLocal(b_new)]
    [invalid_index binder: GetLocal(o); SetLocal(b)]
    [guard: <guard>; If]
    <body>; Break
    one EndIf per If opened by this arm, innermost first
  [unless an unguarded invalid_index(name or _) arm exists:
     GetLocal(o); GetLocal(f+2); GetLocal(f+3); GetLocal(f+4); GetLocal(f+5); Trap(8)]
  GetLocal(f); Break
EndLoop
```

## Self-hosted mirror

Implemented on 2026-10-21 on branch feat/v02-indexed-write-handlers. The plan as
written before the work follows the record of how it was done. Line numbers are
from the survey of 2026-10-21 and will drift.

### As implemented

- **Transport tag 66** carries the existing BoundsCheck. The ISA stays at 66
  opcodes.
- **Bare write.** The parser claims the offset slot at `=`, records the operator
  depth there in the pending statement's otherwise unused name field, and carries
  the element kind at 2^48 on the IndexAssignIn argument. The IndexStore record
  carries the slot. `push_index_assign` emits the bare sequence above.
- **Handled write.** `checked_open` tests first whether a pending indexed
  assignment sits at the operator depth recorded at its `=`. If so, the arm block
  handles the write. The parser pops the assignment, reserves the block of seven
  and emits the IndexStore record with a full-word payload. That payload holds the
  operand, the kind, and the offset slot plus one at 2^51, which keeps it apart
  from a bare write's payload. Record 118 then opens the arms over that root. The
  arms reuse the handled read's protocol. Reconstruction accepts a kind-15 root
  and validates the write's binder sources, and `push_checked_store` emits the
  handled sequence.
- **Frame size.** The reference always declares the seventh slot of the block.
  The self-hosted code generator grows its frame only when a slot is first
  emitted, so a new work item, kind 14, maps that slot without emitting anything
  when `new` is unbound.
- **Relocation.** Late shared declarations relocate data addresses. Records 13
  and 14 now share one kind lane, and the handled root's operand is relocated
  apart from its slot in `src/selfhost/data_relocation.rs`.
- **Capacity.** The parse stage sits at 1023 of the type stage's 1024 names. The
  first version added six names and reached 1029, so its parsing was folded into
  `checked_open` and `checked_pattern`. The offset slot is kept in a pattern-value
  entry that no write arm uses, and the probe state's value 3 asks the next
  token to end the statement or the block. The parse blob grew to 30,359 of
  49,152 bytes. No capacity was raised.
- **Not mirrored.** Elements other than Word and Byte are refused, as in the
  handled read. A pre-existing divergence was found while building the corpus.
  The sum `(o as Word) + (n as Word)` of two Byte locals lowers to `Add` in the
  self-hosted compiler and to `CheckedAdd; PopN(2)` in the reference, with or
  without a handler. The cause is that a cast left the parser's Byte flag as its operand set it. That gap is outside this construct and is fixed separately on branch fix/v02-cast-byte-tracking.

### Plan as written before the work

The reference compiler alone implemented the above at first. Until the
self-hosted compiler matched, every byte-identity test whose source contains an
indexed write failed, including the stages' own self-compilation, because the
stages are full of bare writes. The mirror needed the following.

1. **Decoder.** The self-hosted code generator has no `BoundsCheck` tag today.
   Add tag 66 to the wire table in `codegen.kel` and to `decode_op` in
   `src/selfhost/mod.rs`. The ISA stays at 66 opcodes, since this is an internal
   transport tag for an existing instruction.
2. **parse.kel, bare write.** At the `=` in `step_post_index`, about line 2789,
   reserve one slot for the offset and carry it, with the element kind the
   parser currently drops, in the pending statement and the IndexStore record.
3. **parse.kel, handled write.** At the `{` probe after the value, a pending
   indexed assignment above the checked marker means the arm block handles the
   write. `checked_open` today reads `checked.root` from the value's last record,
   so a value such as `a + b` would be taken as checked arithmetic; the pending
   assignment must be detected first. Open a new form that reserves the block of
   seven, admits `ok` with two patterns and `invalid_index`, binds `old` from
   `f+1`, `new` from `f+6` and `invalid_index` from `o`, and rejects a literal in
   either `ok` position. The context table holds two patterns per context, which
   suffices. End the statement on `;` as an expression statement, or as the
   block's tail on `}`.
4. **reconstruct.kel.** A node for the handled write carrying the offset slot,
   the block base, the element kind and the arm list, and the offset slot and
   kind on the existing IndexStore node.
5. **codegen.kel.** Replace `push_index_assign`, about line 1106, with the bare
   sequence above, and add the handled sequence. Arm dispatch can reuse
   `push_checked_pattern`. Constants must be interned in the reference order:
   `Const(1)` of the trap test before the value's constants on the bare form, and
   `Const(0)` of the success test before the value's constants on the handled form.
6. **Host typing in `src/selfhost/mod.rs`.** The arms' common type, the element
   type for `old` and `new`, and `Word` for `invalid_index`.
7. **Rejections.** A multi-index handled write, which since 2026-10-22 only the
   self-hosted compiler refuses, a single-binder `ok`, a literal
   in `ok`, a duplicate binder and a non-exhaustive `ok` must be refused by both
   compilers. The acceptance row asks for byte-identical rejection families.
8. **Corpus.** Byte-identity cases for each shape in
   `tests/indexed_write_handlers.rs`, a `data/` family of at least two cases in
   `boundary_cases()` of `tests/selfhost_codegen.rs`, and the trap-analysis
   agreement row in `tests/trap_analysis.rs`.
9. **Capacity.** Measure, do not assume. The bare reordering adds instructions
   and one local to every bare indexed write, and the stages contain many. Run
   `the_stage_is_measured_against_the_real_stage_sources` and the stage
   self-compilation tests. `parse.kel` declares about 612 of its 640 data fields,
   and `wire.kel` sits at the type stage's 640-row name channel.

## Multi-index increment, 2026-10-22

**Semantics.** The 2026-10-21 ruling deferred multi-index writes with the
instruction that every level's failure route to `invalid_index`. The binder
receives the index of the level that failed. That is the value the bare form's
per-level `Trap(8)` already carries, and an unhandled failure of the handled
form traps with the same `IndexOutOfBounds(index, level length)` as the bare
form. Each level is checked against its own length, because a flat-offset test
alone would accept `m[0][5]` on a `[[Word; 3]; 2]` field and write `m[1][2]`.
The first failing level stops evaluation, so later indices and the value are
not evaluated. This extends the index-first ruling level by level.

**Reference lowering.** `emit_data_write_levels` in `src/compiler.rs`
replaces the single probe. Two locals precede the outcome block, the offset
and the current level's raw index. Each level evaluates its index, probes it
with `BoundsCheck(level length)`, stores the flag and the four limbs into the
outcome block, and then either adds `index * stride` to the offset or, when out
of range, stores the raw index as the offset through an `If`/`Else`. Every
level after the first runs under a guard on the flag, so the control flow is at
most two `If` blocks deep whatever the depth. After the levels, the outcome
block and the arm dispatch are those of the single-index write. The old element
is read only when bound, with `GetDataIndexed` at the computed offset. The probe
never reads it, because every level's length fits the sixteen-bit `BoundsCheck`
operand, a limit the multi-level access already imposed.

**Self-hosted compiler.** It refuses a handled multi-index write, as it refuses
every multi-index data access, so the construct-support boundary is unchanged.
This is a refusal and not a divergence.

**Evidence.** `tests/indexed_write_handlers.rs` covers the addressed cell and
both binders, a failure at the outer level, at the inner level with an in-range
flat offset, and at a negative inner index, the middle level of a three-level
field, the evaluation order through traced index and value functions, the
default trap's payload against the bare form's, trap-free classification of a
fully handled write, a `Byte` element, and the self-hosted refusal.

## Surface forms

```
// Handled indexed write. An expression of the arms' common type.
d.x[i] = v { ok(old, new) => old + new, invalid_index(j) => j }
d.x[i] = v { ok(_, _) => (), invalid_index(_) => () };

// Native call with a handled host failure.
f(x) { ok(r) => r, host_error => 0 }
```

Keleusma has no `nop` keyword. The unit literal `()` serves that role.

### Lowering of a handled write

With `old` bound the compiler emits the equivalent of the following.

```
if bounds_check_ok(index) {
    old = read(index)
    write(index, new)
    ok_arm(old, new)
} else {
    invalid_index_arm(index)
}
```

Without `old` it omits the read. With `new` unbound it keeps no copy of the
value. The value expression is evaluated inside the success branch, after the
index and its bounds check, which is what makes the short circuit hold. No new
opcode is required. BoundsCheck already produces the outcome and SetDataIndexed
already performs a checked write.

## Remaining non-trap faults found by this audit

The 2026-10-20 audit found that source division on Fixed operands emits a bare
FixedDiv, which still raises DivisionByZero at run time without a Trap. The trap
analysis classifies FixedDiv as an unsupported operation in both
implementations, so no unsound trap-free verdict results. Under the ruling above,
FixedDiv must report a zero divisor as an outcome, and the compiler must default
to a trap when no handler is written. Indexing forms outside the first pass and
FixedDiv are the known members of this class. The runtime fault census in
tests/runtime_fault_census.rs should gain a Fixed division row, since its twenty
source cases did not include one.

**Done 2026-10-22.** FixedDiv now pushes the outcome triple of a Fixed `Mod`.
The low slot is the saturated quotient, the high slot `Fixed(0)` and the flag
`0`, saturated or not, because saturation is the defined result and reporting it
as overflow would send a handler without an overflow arm to the default trap. A
zero divisor pushes the numerator, `Fixed(0)` and flag `3`. Source Fixed division
follows FixedDiv with the same default zero-divisor guard as Fixed `%`, and the
handled form already lowered through `CheckedDiv(n)`. Both trap analyses classify
FixedDiv as clear, and the census has a twenty-first row, Fixed division by zero,
which reaches the explicit Trap. The self-hosted compiler lowers Fixed `/` to
`CheckedDiv(0)`, because its parser does not record the fraction-bit count. That
divergence predates this change, the entry point refuses it, and
`the_self_hosted_compiler_refuses_fixed_division` in tests/trap_lowering.rs pins it.

## Acceptance rows added to the completion plan

| Row | Required evidence |
|---|---|
| Write handler grammar | Both parsers accept the four `ok` capture patterns and `invalid_index` and reject malformed shapes, with byte-identical rejection families. |
| Write handler lowering | Both compilers emit byte-identical modules. The read appears only when `old` is bound. A covered handler emits no fallback Trap. |
| Short circuit | An invalid index skips a value expression whose effect is observable, on both the handled and the bare form. |
| Native-call handlers | Load-time rejection of statically detectable host faults. A runtime host failure reaches the handler, or the default trap when none is written. |
| Composite reads | A composite read without a handler lowers to the default trap. With a handler, no fabricated value is produced. |
| FixedDiv totalization | A zero divisor yields an outcome. Source Fixed division reaches an explicit Trap by default, and the census includes the case. |
| Trap analysis | Every new form classifies identically in the Rust and Keleusma analyses, and a fully handled program classifies as trap-free. |

## Out of scope for V0.2.3

The flags register, its flag-select branch encoding and the migration of every
in-band outcome. These are V0.2.4 work and need their design document first.

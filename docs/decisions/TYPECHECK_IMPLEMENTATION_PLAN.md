# Self-Hosting Type Rejection — Implementation Plan

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

The sliced implementation plan the scoping document
([`TYPECHECK_SELFHOST_PLAN.md`](./TYPECHECK_SELFHOST_PLAN.md)) deferred until its two spikes had
run. Both have. Written 2026-08-14.

Status: **BUILT. Full verdict agreement on the corpus, 2026-08-14.**

**Production composite facts, 2026-10-04.** Struct constructor names and field requirements now
reach the stage. Declared nominal struct and enum tags are distinct from scalar-array tags, which
retain element type and length. Expression kind 16 derives literal-array tags in Keleusma.
Function headers preserve parameter positions and structural type records. Direct struct-returning
calls and literals now arm field access, and whole composite fields retain their representation.
Scalar-array signatures preserve the outer shape. The sixteen-source audit and fourteen valid
controls pass. Arbitrary nested type syntax, composite projections, array let annotations
and nominal struct signatures remain open. Full Order 1 is not complete.

**Enum construction facts, 2026-10-04.** Parser metadata now preserves declaration-order
variant identity. Scalar, named and scalar-array payload requirements reach the independent
type stage. A nineteen-source comparison agrees with the reference, including fourteen
shipping rejections that must precede reference checking. The later member increment also
retains tuple payload types and their match-bound variables. Eleven artifact and execution controls cover payload access and
enum-to-Word casts. The cast controls exposed missing variant dispatch and enum alias facts,
both now repaired. This does not establish complete composite typing or full syntax admission.

**Currency, 2026-10-04.** The active completion effort is tracked in
[ORDER1_COMPLETION_PLAN.md](./ORDER1_COMPLETION_PLAN.md). Parser records now retain explicit
else presence. Reconstruction preserves it and expression extraction emits conditional
branch pairs only for written else branches. Empty branches contribute unknown tails.
Match-arm pairs now come from the reconstructed integer and enum arm tables. Composite
extraction gaps remain open. The scoped production input path is now integrated. Composite coverage and the full Order 1 gate remain incomplete.

Declared struct field counts now travel in parser metadata. The driver pairs each
count with the following construction record and supplies expression kind 7 to the type stage.
The equal-size, unequal-field-count witness now distinguishes the cases through metadata.
Six expression kinds are emitted by the legacy inventory. The scoped path now also preserves
match guards and explicit scalar field/index rejection paths. Unary `not` now emits the same boolean-operand requirement as an `if`
condition, covering another previously omitted syntactic form. The production runner now consumes these scoped facts.

Expression extraction reconstructs each source head separately, preserving its parameter names,
local bindings and return requirement before dispatch construction. Guard subexpressions remain
in the walk. The Rust reference currently accepts a Word-valued function guard, so no additional
boolean requirement on that root is introduced here. Match guards have a separate boolean
requirement and now retain their conditions through the parser and reconstruction.

Call argument extraction now retains scalar literal facts, scoped local identities and referenced
function return declarations. Independent batches cover each source head's guard and body.
All twelve stage sources fit the approved caps with maxima of 67 sites, 88 arguments and
62 bindings per batch. Call checks accept those batches. Alias lookup is bounded by the number
of binding rows and reaches longer chains while leaving cycles and missing targets unknown.
Composite arguments remain incomplete. Scoped production rejection now precedes the reference.

The scoped path now shares an expression-constraint walk with the legacy projection. Synthetic
result identities connect computed operands to calls, local bindings and return requirements.
Result rules distinguish comparisons from arithmetic, asymmetric shifts, boolean operators,
numeric unary operators, scalar casts and branch results. Literal values remain syntax facts;
the stage performs bounded Byte coercion for binary operands. An earlier combined corpus recorded maxima of 299 expression rows and 288 bindings.
Those figures predate loop constraints. The current all-stage checks establish fit within the
approved caps, but these historical maxima must not be reused as current measurements. General source annotations and composite reads remain incomplete.

The scoped rejection corpus now reaches all twenty negative cases and seven valid controls
without reference-first rejection. Parser diagnostics cover unresolved identifiers, unknown
struct fields, unsupported function values and calls through values, and declared-scalar
projections. Unsupported syntax and resource failures remain separate outcomes. General source
annotation compatibility, unannotated projection facts and full composite coverage remain incomplete.
The same rule census now also runs through scoped pipeline inputs. Its required test passes
after match-arm guards were preserved through compilation and execution. Scalar let annotation agreement,
scalar data assignment agreement and Word index requirements now reach the stage. Annotated
binding results use the existing agreement row, and data declarations use bounded slot ranges
in parser declaration order. The focused corpus includes valid Byte stores and explicit casts
to distinguish these checks from binary operands' contextual literal conversion.

Branch-local names no longer leak into sibling branches. Frame slots remain monotonic, the
per-function named-binding cap has a separate total counter and reused name entries have their
composite metadata cleared. Match-arm scopes now restore their binding names too. Both loop forms now restore names at exit, with artifact and execution controls. The latest stage suite has one hundred and three passing type tests,
including the scoped census and twelve guard-type cases. Scalar enum-payload bindings obtain
source-declared tags from EnumBind records. Composite facts, complete occurrence coverage,
general syntax admission and complete composite coverage remain required. Integer and enum match
coverage now has explicit parser diagnostics that exclude guarded arms and distinguish capacity
from source errors. Range-loop variable facts and bound requirements reach the scoped type stage.

The scoped operand adapter now also reads scalar struct-field, tuple-field and array-element
layout kinds. A twenty-six-program comparison agrees with the reference after closing nine
missed rejections. Word, Byte and bool tags reach existing argument, binding, expression and
return constraints. Composite access records remain distinct, with valid container-argument
controls. This extends scalar projection coverage without establishing complete composite facts.

**Currency, 2026-09-29.** The six input channels that previously excluded `parse.kel` and
`wire.kel` now have 640-row caps. The measured stage-source test fits and checks all twelve real
sources. The selected increase is 7,168 words, or 56 KiB, across the stage's shared and private
tables. This closes the current capacity gap. It does not move extraction into Keleusma: Rust still
walks the reference parser's abstract syntax tree to supply the stage's facts, so the Order-1
self-hosting gate remains open.

**Further progress, 2026-09-29.** `verify_types.kel` now declares its private solver storage before
its first use. It compiles through both the test pipeline and the production driver that builds
all module fields from pipeline output. Both serialized artifacts match the reference. Restoring
the old declaration order reproduces the original refusal at `ty_direct` while leaving the
reference artifact unchanged. This closes the twelfth stage's compilation gap within the existing
parser subset. General forward data references and the input extraction residuals remain open.

The stage rejects all sixteen ill-typed programs and accepts all seven well-typed ones. Six slices,
each merged on a green gate: the harness with an accepting checker, operand agreement, call arity and
argument types, name resolution, conditions and structural claims, and the last two shapes.
`src/selfhost/kel/verify_types.kel` is the stage and `tests/selfhost_typecheck.rs` the differential.

**What the slices taught that the plan did not predict.**

- **A well-typed control caught a FALSE REJECTION**, in slice 3. The name collector knew only
  functions and locals, so `shared data s { n: Word }` reading `s.n` resolved to nothing and was
  refused. A valid program rejected is a language change, not a conservative choice, and **a
  rejection-only corpus would have scored it a success**. This is the plan's own warning arriving as
  an actual defect rather than a hypothetical.
- **A wildcard import makes resolution undecidable**, and the sound response is to stop deciding
  rather than guess. `use audio::*` brings in names the collector cannot enumerate, so the whole
  table is dropped for such a program.
- **Struct field count is call arity.** A struct literal with the wrong number of fields is the
  identical claim as a call with the wrong number of arguments, and it goes down the same channel.
  Two spellings of one idea is where they drift apart.
- **The stage compares; the host supplies syntactic facts.** Tags come from literal kinds and
  declared types, never from the reference's inference, which would make agreement true by
  construction. That division is the one `verify_*.kel` already uses.

**What is NOT done**: this is rejection only, which is the whole Order-1 obligation but not a type
checker. Inference for emission was measured to be unnecessary and remains so.

## What this is not

**It is not a port of `src/typecheck.rs`.** That file is 8,601 lines, of which a large share serves
traits and bounds the Order-1 subset does not use. Two measured results shrink the obligation to
something much smaller, and neither is a guess:

- **Monomorphization is an identity transform** on all ten stage sources, pinned by
  `tests/selfhost_monomorphize_identity.rs`. Nothing to port.
- **Clearing `program.fn_expr_types` leaves every stage module byte-identical**, so the emitter's
  structural fallback covers everything the subset reaches. **Inference for emission is not
  required.**

What is left is **rejection alone**: deciding that an ill-typed program is ill-typed. The checker
needs exactly as much type propagation as that decision takes, and no more.

## The oracle, and the one thing it is not

**Verdict agreement. Accept versus reject.** Not message agreement. This is the oracle the
`verify_*.kel` family already uses, and it avoids committing a `.kel` checker to reproducing
English diagnostics that the reference is free to reword.

**The direction matters and is not symmetric with the verifier's.** `verify_*.kel` is allowed a
sound over-approximation, deferring to a runtime guard when it cannot decide. A type checker has no
such latitude in the accepting direction: **rejecting a valid program is a language change**, not a
conservative choice. So the two directions carry different obligations:

| direction | obligation |
|---|---|
| the reference accepts | the stage **must** accept. A false rejection narrows the language. |
| the reference rejects | the stage must reject, for the enumerated shapes. |

The second is bounded by the corpus; the first is not, which is why the well-typed side of the
corpus grows with every slice rather than staying at the five controls the spike used.

## The trap that governs the corpus

**A corpus of rejections alone cannot detect a checker that rejects everything.** It would score
perfectly. Every slice therefore adds well-typed controls alongside its rejections, and a slice
whose well-typed count does not grow is a slice that has not been checked.

The scoping document records the converse mistake, made while sizing this: a case labelled
ill-typed that was in fact well-typed, reported as "accepted but should not be". It did not mislead
**only because explicit well-typed controls existed to check it against**.

## The fifteen shapes, grouped into slices

Measured by execution, not by counting `TypeError` sites: eighteen ill-typed subset programs, of
which seventeen were rejected, plus five well-typed controls, all accepted.

### Slice 0 — the harness, before any rule

A driver that runs a `.kel` checker over `parse.kel` records and compares accept-versus-reject
against the reference, over the whole corpus at once.

**The `.kel` checker accepts everything at this slice.** That is the point: the corpus must then
show every well-typed case passing and every ill-typed case FAILING. A harness that reports success
here is broken, and finding that out costs nothing before any rule exists. This is the must-fire
for the harness itself.

### Slice 1 — scalar operand agreement

`1 + true`; array elements of differing types. The smallest real rule, and the one that establishes
how a type is represented in the stage at all.

### Slice 2 — function signatures

Argument count, argument types, `Byte` against a `Word` parameter, and body type against declared
return type. Four shapes, one mechanism: a signature table and a comparison.

### Slice 3 — identifiers and callability

Undefined function, undefined identifier, and **calling a local**.

> **The odd one out, and it must not be handled by looking for a type error.** "Calling a local" is
> a V0.2.0 surface restriction rather than a type error, and it is the one rejection of the fifteen
> that does not carry the `type error:` prefix. A stage that located rejections by that prefix would
> miss it, and a stage that reproduced the reference's routing would be reproducing English.

### Slice 4 — control flow

`if` branches of differing types; a non-bool condition.

### Slice 5 — composites

Unknown field; wrong field count in a struct literal; indexing a scalar; field access on a scalar.

## Sizing, honestly

**The shapes are enumerated and the harness pattern exists**, so there is no discovery left in
*what* to check. The unknowns are two, and both are about representation rather than about types:

1. **How a type is represented in a total language with no sum types in the stage's own subset.** The
   `verify_typed.kel` shape lattice `(tag, size, kind)` is the closest precedent and is probably the
   answer, but it was built for flat layout rather than for source types.
2. **How much of the AST has to reach the stage.** The wire-format work has just established a
   module-input encoding for names; a type checker needs expression structure, which is a different
   and larger input surface.

The second is the one that could turn out large. It is the same class of work as the wire format's
"wire the driver to a module, not to a model", and the same rule applies: **doing the checker before
its input encoding is wasted**.

## Ordering against the wire-format line

The wire-format wiring and this share an input-encoding problem. **Neither should invent a second
encoding.** If the module-input blob grows an expression section, that section serves both, and this
plan should follow the wire-format line rather than run beside it.

## Caveats

- The fifteen shapes come from **one execution over eighteen programs**. A shape reachable in the
  subset that no program in that set reached would be missed, and the corpus is the only guard.
- Nothing here is built. Every size above is a projection from a measured shape count, not from a
  written stage.

## Structural tuple and nested-array facts, 2026-10-04

Source type metadata now retains flat tuple members and the existing nested-array header
forms. The driver turns declared types and tuple literals into bounded construction rows.
The Keleusma type stage interns exact tuple and array structures, preserving member order,
element types and dimensions independently of byte size. Parameters, calls, aliases and
returns share these identities. The solver now permits up to 640 productive rounds, bounded
by its binding table. Pool exhaustion is unavailable evidence rather than source rejection.

The initial fifteen-source comparison found eleven missed rejections. The expanded corpus
contains nineteen sources, including thirteen negatives and six valid controls. Six complete
artifact and execution controls cover tuple, nested-array and array-of-tuple parameters and
returns. Array-of-tuple literals now retain their tuple element byte width. Return-header
layout no longer indexes a nonexistent parameter in zero-parameter functions.

The structural pool adds three 640-word arrays, or 1,920 words. At eight bytes per word this
is fifteen KiB beyond the earlier 56 KiB selected-cap increase. The accounting tests share one
price table and compare actual array capacities as well as counts. Projection scalar-exclusion
rules now recognize known nominal and structural composites, with direct transport controls.

This does not establish arbitrary nested type grammar, composite payload requirements,
array let annotations, whole-composite projection identity, complete binder and occurrence
coverage or general syntax admission. Mixed-scalar tuple runtime layout and nominal struct
signatures remain incomplete. Production serialization still uses Rust Module::to_bytes.
The complete Order 1 objective remains active.

## Composite member and payload binding input, 2026-10-04

Constructor requirements now retain structural source records for enum tuple payloads and
struct tuple and array fields. The host emits existing construction and agreement rows while
the Keleusma stage derives identities. Byte layout is not used as type identity.

Parser metadata 97 precedes each EnumBind and carries variant index times 1024 plus payload
position. The driver pairs it with the following record's frame slot and source range, checks
pairing counts, and imports that member's source type into the scoped binding table. Missing,
reused or invalid coordinates fail extraction rather than becoming a source rejection verdict.
The scalar layout-kind fallback for payload bindings is removed. Nested-match metadata and
complete-value argument controls distinguish this from scalar projection support.

The twenty-eight-source comparison contains eighteen invalid and ten valid programs. Eight
complete artifact and execution controls cover the supported member forms. General nested
syntax and composite projections still require work. No type-stage capacity changed.

## Composite projection input, 2026-10-04

Internal parser metadata 98 precedes each struct field read. Its payload is source struct
name id times 1024 plus field position. The saved read is emitted without consuming another
token. Source coordinates survive nested struct, tuple and array postfix handling. The driver
pairs metadata with reconstructed field nodes separately for each source range, checks counts,
imports the declared member type and checks the receiver against the stated nominal type.

Type-input expression kind 18 derives an array element identity in Keleusma. Compact scalar
arrays expose their scalar class, while composite arrays consult the exact structural pool.
Access checks the pool bound and entry kind. Known non-arrays reject, and unknown remains
undecided. Direct controls check scalar, nominal and nested-array element tags, mismatching
requirements and invalid bases. The new expression kind is not a public bytecode opcode.

The eighteen-source comparison has eleven negatives and seven valid controls. All negatives
require independent shipping rejection. Six artifact and execution controls and a nested-field
metadata trace cover the new path. No type-stage array or capacity changed.

# Order 1 completion plan

> **Navigation** [Decisions](./README.md) | [Roadmap](../roadmap/V0_2_X_ROADMAP.md)

## Scope

The operator requested completion of roadmap Order 1 on 2026-10-04, with participation
in the System 1 shadow pilot. The objective remains the complete first-pass compiler
self-hosting gate. Closing an extraction channel is progress toward that gate.

The authoritative scope is the Order 1 row in the
[V0.2.X roadmap](../roadmap/V0_2_X_ROADMAP.md), qualified by the
[type-rejection plan](./TYPECHECK_IMPLEMENTATION_PLAN.md) and the
[wire-format plan](./WIRE_FORMAT_SELFHOST_PLAN.md).
Full-language support remains Order 6. Existing supported programs must retain their behavior.

## Completion evidence required

| Requirement | Evidence required | Current evidence and remaining work |
|---|---|---|
| Compile the complete stage corpus | Complete artifact identity against the reference for every current stage source, with a corpus inventory check | All twelve complete serialized artifacts from the production compiler match the reference after the call and alias changes. The new artifact corpus also checks directory inventory. Both sides still use the Rust serializer. |
| Avoid reference scaffold borrowing | Production compilation assembles module fields from pipeline output | `self_host_compile_scratch` provides this path. Preserve it through integration. |
| Derive type inputs from the self-hosted pipeline | Production rejection receives facts without walking the reference parser's abstract syntax tree or borrowing inferred types | The production input path now derives scoped scalar constraints from Keleusma parser and reconstruction output. Composite facts, complete binder/occurrence coverage and general admission remain incomplete. |
| Reject ill-typed subset programs | Independent pipeline rejection agrees with the reference on the rejection corpus, valid controls, and the rule-versus-form census | The scoped pipeline harness exercises all twenty rejection cases and seven valid controls without reference-first masking. The separate sixteen-cell rule-versus-form census now runs through that path with eight valid controls. Its required test now passes after preserving match-arm guards. Parser source diagnostics and type-stage verdicts remain distinct from unsupported syntax, capacity and execution failures. The shipping driver now obtains the independent verdict first. Production corpus and census checks assert PipelineRejected so reference rejection cannot mask missing stage coverage. |
| Preserve the first-pass monomorphization result | Every stage remains unchanged by monomorphization, with a control that does change | The corpus now lists all twelve sources. Identity, changing control and directory inventory tests passed again after the parser and alias changes on 2026-10-04. Repeat if stage sources change. |
| Preserve independence from inferred expression types | The current stage corpus compiles identically with recorded expression types absent | Four standing tests cover all twelve sources with a directory inventory, a populated-table intervention after recording, a changed-emission control and an admission control. On 2026-10-04, clearing 40,824 recorded types leaves all twelve complete artifacts unchanged. This does not remove reference admission or establish full-language independence. |
| Emit complete artifacts through Keleusma | Production output uses the Keleusma wire emitter for framing, opcode records, operand pools, auxiliary regions and integrity fields | Region coverage tests exist, but they do not establish production serialization. `Module::to_bytes` remains on current artifact paths. Preserve the wire plan's encoder and decoder obligations. |
| Reach the shipping tool | An end-to-end command emits the independently assembled artifact and reports type rejection through the new path | The command emits the same artifact as the production library, executes a valid result of seven, and independently rejects three invalid sources without emitting files. Serialization still uses Rust, so the complete shipping requirement remains open. |
| Establish current verification | Relevant differential and failure-path tests, complete workspace checks and applicable integration gates pass on the final tree | The 2026-10-04 release workspace run passes 3,031 tests with five ignored. Clippy, documentation and compile-and-verify no_std checks pass. The separate feature configurations and final integration gates still require verification after the remaining implementation. |
| Participate in the shadow pilot | Diagnostic and completion-claim samples retain baseline judgments, reviewed payloads, outcomes and explicit service status | Capture is installed for this line. Judgments are advisory and do not alter gates. |

## Work sequence

1. Preserve explicit else presence and struct-literal identity or declared field count through
   parser records. Both metadata paths now exist, with reference comparisons for branch tails
   and struct field counts. The struct path emits declared count metadata immediately before
   the construction record and retains nested and reordered literals.
2. Close remaining declaration, call argument, occurrence, binding and expression extraction
   gaps. Treat field and index failures during reconstruction as explicit rejection paths.
   Integer and enum match-arm comparisons now have extraction and verdict controls.
   Source heads now retain separate tail requirements and binding names. Match guards now retain their conditions and results through execution.
   Composite facts and the remaining input channels still require work.
3. Integrate the rejection stage with the production compiler. Keep the Rust implementation
   as a differential oracle, and prove that its early rejection cannot mask missing stage work.
4. Complete production artifact emission through the existing Keleusma wire implementation.
   Audit which values are derived, marshalled, encoded or copied. Region byte coverage alone
   does not establish that provenance.
5. Refresh all-stage identity, monomorphization and inference-independence evidence.
   Exercise the shipping command, run the final gates and audit every row above.

## Scalar access input coverage

The scoped pipeline now reads Word, Byte and bool facts from scalar struct-field,
tuple-field and array-element records. The layout kinds were already computed by the
Keleusma parser from source declarations. No reference syntax or inferred type is used.
Nested composite access records remain distinct and do not acquire their element's scalar tag.
These operands reach existing call, alias, annotation, expression and return constraints.

A twenty-six-program comparison found nine invalid programs accepted before this change.
All twenty-six now agree with the reference. Controls include Boolean fields and array
elements, Byte arithmetic boundaries, tuple offsets, alias chains, array-of-struct reads
and passing complete composite values. This finite comparison does not establish complete
composite typing, projection-alias coverage, syntax admission or production rejection.

## Constraints and open evidence

Bytecode version remains 2. No new bytecode opcode is authorized. The internal parser record
changes required by this plan do not authorize changing the public artifact format.
The V0.3.X workstream remains separate. Publication requires in-session confirmation.

The legacy handoff is invalid-and-stale. Its stamp is eighteen commits behind the starting
tree, its ancestry check passes, and its unchanged-stage-source claim is contradicted by
`verify_types.kel`. Its full-suite and advisory checks have not been rerun for this increment.

The first shadow evaluation failed with a transport error. An escalated evaluation was
rejected by automatic approval review because it did not recognize authorization for the
external destination and payload. A specific operator approval request is pending.
Local pilot capture and compiler work can continue independently.

## Production integration evidence

Inspection on 2026-10-04 confirmed that `compile_source_with_target` in the command-line tool
selects `self_hosted_compile`, now with independent type checking before the reference, while the unsigned artifact path still calls `module.to_bytes()`.
The wire stage already exposes framing and sealing commands through `dispatch_frame`.
Routing region payloads alone does not close the remaining complete-artifact emission work.

## Expression extraction evidence

The match comparison corpus includes nested integer and enum dispatch, wildcard-only matches,
explicit enum coverage, later mismatched arms, named operands and an unresolved payload binder.
The latter remains unknown on both input paths, so comparison agreement does not establish
payload-type resolution. Bare block expressions in match results are rejected by the reference
parser. The guard increment preserves match conditions through reconstruction and execution. The later admission increment below audits integer and enum match coverage. Other syntax and
composite admission remain incomplete before production integration.

Separate reconstruction of each source head recovers its return row and its own binding names.
Function guard expressions are still traversed. A probe found that the Rust reference accepts
`fn f(x: Word) -> Word when 1 { 1 } fn f(y: Word) -> Word { 2 }`.
No new root boolean requirement is imposed while that reference behavior remains unresolved.

## Call extraction evidence

Call argument requirements now come from reconstructed argument indices. Scalar literals,
parameter declarations, local aliases and called-function return declarations retain source
facts without reference syntax or inferred types. Local identities include the source head,
guard or body range and frame slot. Function return identities occupy a separate namespace.
Each range has its own batch and imports only the function returns it references.

The aggregate wire-stage input initially contained 831 bindings, exceeding the approved
640-row limit. Across all twelve stages, separate batches reduce the measured maxima to
67 call sites, 88 argument requirements and 62 bindings. The call checks accept every batch.
The largest declaration table has 492 entries. No array capacity increased.
This establishes the call channel's capacity and valid-stage controls, not complete type rejection.

A parameter sharing a function name exposed global lookup preceding local lookup in the parser.
Local bindings now take precedence. Alias resolution also follows chains within the binding-table
bound, including cycles and absent targets that remain unknown. Field and index results, non-scalar declarations and the other input channels still need
integration. Scalar computed arguments are covered by the later increment below.

Complete artifact comparison on a valid shadowing program found another assembly discrepancy.
The pipeline supplied an empty data layout where the reference supplied no layout. Both assembly
helpers now omit the layout when there are no slots, preserving the artifact's region set.

## Unified scalar expression facts

The scoped batches now contain expression constraints and synthetic result identities. Call
requirements, local initializers and return requirements use the same identities. A shared
forest walk supplies both the legacy expression projection and this scoped path, while the
Keleusma stage resolves result tags. Arithmetic, comparisons, boolean operators, scalar unary
operators, Word and Byte casts, conditional branches and match results have direct controls.
Composite results and reads remain outside this scalar result channel.

Literal operands retain their integer value in a separate operand form. The stage applies the
reference's contextual Byte coercion to binary arithmetic and comparisons only, bounded to
0 through 255. Shift amounts and call arguments do not use that coercion. This distinction
prevents rejecting valid Byte arithmetic or accepting an invalid implicit call conversion.
The parser retains xor's operator tag for its boolean restriction. Code generation maps it to
the existing comparison instruction, with no new bytecode opcode or version change.

Across all twelve stages, the combined expression and argument rows have a measured batch
maximum of 299. Bindings have a maximum of 288. The declaration maximum remains 492, and all
batches pass the stage. These measurements retain the approved 640-row limits. Production
integration, source annotations, occurrence coverage and composite facts remain incomplete.

## Explicit parser rejection paths

The scoped input path retains parser diagnostic tags and payloads in a structured result.
Unknown local names and struct fields no longer substitute slot zero or the first field.
Bare function references and calls through values are explicitly refused. Primitive parameter
and local annotations support field and index rejection before reconstruction. Composite
annotations remain distinct from their scalar element names. Scalar annotation compatibility
now reaches this channel through the later increment below. Composite annotations remain incomplete.

Method syntax, const parameters, unsupported patterns, assertions and unresolved imported
or qualified names retain separate unsupported diagnostics. The first lexer pass reports
whether imports occur anywhere in the source. The host now seeds the lowercase `bool` name
rather than the distinct named type `Bool`. Valid controls distinguish these boundaries from
source errors. This is not an exhaustive syntax admission proof. General unannotated projection
facts, occurrence coverage, the rule-versus-form census and production integration remain open.

The fallible interface covers parser record diagnostics and resume failures. Stage compilation,
lexing, reconstruction and callback failures still have panic paths. None of those panics counts
as a source rejection. The complete artifact and capacity checks passed again after these parser changes. All twelve
stage artifacts still match the reference, and their scoped inputs remain within the selected caps.

## Source annotations, stores and index requirements

The reference-input and pipeline-input census now share the same sixteen negative sources
and eight valid controls. Initially the scoped pipeline accepted four negatives. Explicit
scalar let annotations, scalar data stores and Word index requirements now reach the type
stage. Only the match-arm guard negative remains accepted. The census test requires every
cell to reject and remains failing on the feature branch. It has no expected-gap allowance.

Parser metadata retains the source type name before each annotated LetIn record. The host
associates that fact with its source range and frame slot. The existing agreement row compares
the initializer with the declaration and supplies the binding result. Data facts follow parser
declaration order, omit const slots and represent arrays with ranges. Index checks cover data
reads, data stores and scalar or nested value-array access. Annotation and store checks do not
apply binary operators' contextual Byte literal conversion.

A valid nested-shadowing control exposed a parser scope leak from the then branch into its
else branch. Branch exit now restores live binding names while keeping frame slots monotonic.
A separate counter preserves the total-per-function binding cap. Reused lexical-name slots
have their composite metadata cleared. Focused checks cover sibling branches, implicit else,
composite-name reuse, inaccessible branch locals and the exact binding-cap boundary.
Loop and match scope restoration remain unestablished.

The final stage suites passed ninety-five type tests, ninety parser tests with two ignored,
three monomorphization tests and both inventory and complete-artifact tests for all twelve
stages. The sole failure was the required scoped census. Later focused tests passed the
branch-scope artifact controls, disjoint-scope binding cap and expanded declaration controls.
These results do not establish production type rejection or complete input coverage.

## Match guards and ordered fallthrough

The scoped rule-versus-form census now rejects all sixteen negatives and accepts its eight
valid controls. A separate twelve-source comparison checks literal, parameter, computed,
called-function and scalar enum-payload guards. Guarded-arm wrappers retain the condition
and result separately. The type stage checks the condition as bool and compares the actual
arm result with other arms. Scalar payload binding facts come from parser EnumBind records.
General composite payload facts and the legacy occurrence channel remain incomplete.

The lexer now distinguishes `when` from the return arrow. The parser reads guards with its
ordinary expression machinery and restores match-arm binding scope after each arm. Grouping
and nested block delimiters cannot prematurely end an arm. Wildcards remain in source order,
including a guarded wildcard before later arms. A final wildcard keeps the compact trailer,
and a twelve-variant control preserves the previous enum match table capacity.

Fourteen valid programs compare complete artifacts and execute both compilers' output. They
cover false-guard fallthrough, repeated patterns, payload bindings, nested match and if guards,
call arguments, skipped trapping arm results and guard side effects. Complete artifact comparison
also exposed two assembly omissions. Enum END records now retain declared maximum payload
size, and Boolean parameters retain their chunk tags and verifier signature shape.

All twelve complete stage artifacts and the monomorphization corpus still agree with the
reference. The selected type suite now passes ninety-seven tests. This closes the measured
match-guard gap. It does not establish complete syntax admission or replace the remaining
production type-input, wire-emission and final-gate requirements.

## Match admission and loop scope

The parser now requires an unguarded catch-all for supported scalar matches, or unguarded
coverage of every declared enum variant. It tracks declaration rows with one bit per active
match depth, so duplicate variants and nested matches cannot manufacture coverage. Unknown
variants and variants from the wrong enum have explicit source diagnostics. Guarded arms do
not count, including a literal true guard. Boolean and variable patterns remain unsupported
syntax, with a distinct diagnostic rather than a claim that the source is invalid. Match
nesting has an explicit eight-level capacity diagnostic checked before indexing the stacks.

Direct enum constructors and declared enum-returning calls now retain their scrutinee type.
The unit constructor consumes its pending token before the match boundary is handled. Complete artifact comparison additionally exposed missing enum signature shapes.
They now use declared discriminant and payload sizes without reference inference. These
controls exposed false rejections in the first coverage implementation and are retained.

Both parser loop forms restore lexical names after the loop while preserving allocated frame
slots. Scoped type inputs obtain Word induction-variable facts and Word bound requirements
from reconstructed loop tables. Complete artifact comparison exposed a separate Rust compiler
scope defect. The reference now limits induction names and generated temporaries to the loop
and resolves the upper bound before exposing the induction name. Execution controls use an
explicit expected result, independent of agreement between compilers.

These checks close measured admission and scope gaps. They do not establish complete composite
facts, occurrence coverage, production type rejection, production wire emission or final gates.

## Production type rejection

The production transport in `src/selfhost/typecheck.rs` compiles the embedded rejection stage
for bootstrap, derives shared-column ranges from its compiled layout and obtains verdict values
and execution bounds from stage functions. It does not compile the user source with the reference
before obtaining its independent verdict. Each source-head batch uses a fresh arena and shared
buffer. Source diagnostics and stage rejection are distinct from unavailable parser input,
malformed facts, capacity failures and execution errors.

Production checks cover the twenty-negative, seven-control corpus and sixteen rule-versus-form
cells with eight controls. All twelve stages pass both the existing test transport and the new
production transport. Boundary tests accept 640 and refuse 641 for declarations, call sites,
combined expression/argument rows and bindings. Eight malformed encodings return protocol errors.
These observations establish the scoped integration, not complete composite typing or admission.

The command-line probe initially compared different inputs. The Rust backend prepends native
signature declarations while the self-hosted backend uses the user source directly. The maintained
command test instead compares with the production library artifact, then runs it and asserts seven.
The complete all-stage differential remains the artifact oracle. Full Order 1 remains open.

## Nominal and scalar-array facts

The production checker now distinguishes declared struct and enum identities from scalar-array
identities. Struct construction metadata retains the declared name and field count, and field
requirements follow declaration order even for reordered literals. Scalar-array declarations
retain length. Their literal results are derived by the stage after resolving element facts.
The representation supports lengths through 134217726 within a 32-bit Word. Non-scalar element
shapes and larger lengths remain unknown rather than wrapping or colliding with nominal identities.

Sixteen initial sources now agree with the reference after twelve missed rejections and one false
rejection were observed. Fourteen further positive controls cover aliases, nested and reordered
fields, enum casts and scalar-array expressions. Each negative in the initial corpus must produce
PipelineRejected in the shipping entry. Array signatures and whole-field reads have three complete
artifact-and-execution controls with result seven. This extends the required type coverage without
closing complete tuple/nested-array facts, enum constructor payloads, array let annotations or
nominal struct signatures. Production wire emission and the final completion audit remain open.

## Enum constructor and payload facts

The next nineteen-source comparison found fourteen invalid programs accepted by the independent
pipeline. Constructor metadata now identifies the declared variant, and declaration records
carry scalar, named and scalar-array payload types. Extraction supplies arity and agreement
constraints to the existing stage rules. Unknown variants and empty constructors requiring
payloads have explicit source diagnostics, including the direct cast-folding path.

All nineteen sources now agree, with fourteen independent shipping rejections. A metadata test
checks adjacency and declaration-order identity across enum declarations. Tuple payloads reserve
one unknown field so later scalar requirements do not shift. Their full type remains unresolved.

Eleven complete artifact and execution controls return seven through both compilers. These exposed
an enum value cast being emitted as ByteToWord and a missing parser enum fact on aliases. Casts now
reuse enum variant dispatch with the EnumVariantUnmapped fallback trap. The existing Cast node
retains an enum dispatch table, and internal build record 74 distinguishes its assembly. This adds
no public opcode. Scalar casts after an enum value have a separate control against stale facts.

Complete tuple and nested-array facts, composite projections, array let annotations, occurrence
coverage and general admission remain open. The all-stage identity and final integration obligations
above remain authoritative. Production artifact serialization still uses Rust.

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

## Composite member declarations and payload bindings, 2026-10-04

Enum tuple payloads and struct tuple fields now retain their ordered source type records.
Struct array fields retain their element type and length. Constructor constraints use the
existing exact structural solver. The initial twenty-source comparison found twelve missed
rejections, now closed. An expanded twenty-eight-source comparison includes eighteen negatives
and ten valid controls. Every negative requires independent shipping rejection.

Five added negatives exposed unknown composite types on match-bound payload names. Metadata
record 97 now pairs declaration-order variant and payload position with each EnumBind slot.
The driver joins those source coordinates to declared member types, preserving tuple, array
and nominal facts in calls, aliases and returns. A nested-match trace checks metadata adjacency
and coordinates across two enum declarations. Eight artifact and execution controls cover
member construction, access, field order, aliases and calls receiving complete payload values.

Arbitrary nested declaration grammar, whole-composite projection identities, array let
annotations, remaining occurrence coverage, general admission and nominal struct signatures
remain incomplete. Production wire emission and the final integration requirements remain open.

## Composite field and array projections, 2026-10-04

The eighteen-source comparison found eleven missed rejections on complete values obtained
from struct fields or composite array elements. Field metadata now carries source struct and
member coordinates. The driver checks its count against reconstructed field-read nodes, joins
the declared member type and emits a receiver identity requirement. Field results retain their
source type through calls, aliases and returns. This is independent of the emitted byte layout.

Expression kind 18 asks the Keleusma stage for an array element's exact type. It handles compact
scalar-array identities and interned composite arrays, with guarded pool access and known
non-array rejection. The production extractor emits it for nested array reads. Existing index
requirements remain separate. Six complete artifact and execution controls cover nested-array
aliases, array-of-tuple parameters, whole array and tuple fields, and nested nominal fields.

General tuple projections, arbitrary nested declaration grammar, array let annotations,
remaining occurrence and binder coverage, syntax admission and nominal struct signatures
remain incomplete. Production wire emission and final integration requirements remain open.

## Composite let annotation input, 2026-10-04

The Keleusma parser now streams source let annotations independently of executable nodes.
Named members, tuple boundaries, grouped and unit types, and literal array dimensions survive
until the folded binding receives its annotation reference. The host transports these source
records into the existing structural construction and agreement rows. Keleusma still resolves
the actual types. Named annotation metadata retains its earlier binding adjacency contract.

The initial seventeen-source comparison found eleven missed rejections. The expanded comparison
contains twenty-three sources with fourteen negatives and nine valid controls. Every negative
also requires independent PipelineRejected from the shipping entry. Seven complete artifact and
execution controls cover unit values, arrays, nested arrays, tuples, arrays of tuples and nominal
values, and branch-local shadowing. Both compiler outputs return seven.

The valid controls exposed missing empty-parenthesis unit emission and missing tuple-element
layout on let-bound arrays. Both now use existing executable nodes. Literal tuple layout remains
limited to the existing all-Word representation. Its allocation now checks layout count, member
count and total field storage before writing. These limits and the new annotation depth limit
produce unavailable evidence rather than source rejection. Tests exercise each boundary.

Annotation nesting uses three 32-word arrays and two scalar words. Tuple-array binding facts
add one 65-word array and one scalar word. The combined private-state increase is 164 words,
or 1,312 bytes at eight bytes per word. The selected type-stage caps remain 640. Annotation
metadata 99 through 103 and parser diagnostics 27 through 29 are internal protocol additions.
Public bytecode version remains 2 with 66 public opcodes.

The broad type suite exposed two deliberate unit-support boundary pins. Written unit nodes now
carry internal payload 3, while synthesized continuations retain payload 0. Type extraction keeps
a written unit tail and its return requirement. Code generation maps payload 3 to the existing
unit immediate. The replacement tests require independent rejection of a Word-returning function
with a written unit tail, retain valid branch-unit artifact identity and add unit to the construct
support table. The public immediate encoding is unchanged.

The annotation scanner accepts named types, grouping, unit, tuples and arrays with literal
dimensions. A valid constant-expression dimension remains explicitly unsupported. Boundary tests
check the iterative scanner at depths 31, 32 and 33 independently of the reference parser's
lower recursion limit. A metadata trace preserves an array length of 1,099,511,627,776 without
packing it into a tag. This finite corpus does not establish arbitrary header grammar, complete
occurrence and binder coverage, general tuple projections, empty-array inference, mixed-scalar
tuple runtime layout or nominal struct signatures. Production wire emission, refreshed inference
independence and final full gates remain required for Order 1.

The full code-generation test run also exposed a stale Rust test reconstruction mirror and
stale inventory assertions. The mirror now handles ordered wildcard patterns and guarded match
results. All 78 current code-generation functions are required to compile identically. The
construct-support count guard reads the current roadmap instead of the historical handoff.


## Workspace admission and corpus audit, 2026-10-04

The complete workspace run exposed eight failures across seven targets. Investigation found
stale diagnostic and opcode expectations, an outdated reconstruction-stack argument, header
metadata omitted from the test driver and a documentation scan entering peer worktrees.
The repairs preserve structured unsupported diagnostics, check wildcard stack boundaries,
explicitly discard composite signature metadata in the historical scalar scaffold and exclude
private state and peer worktrees from public-document scanning. Shipping signature assembly
retains its own composite metadata. The textual driver guard does not establish semantic parity.

The named example corpus also exposed invalid-source verdicts for valid unsupported syntax.
The parser now refuses non-range iteration, unsupported declaration names, destructuring lets
and unsupported expression blocks before their interior names can become unknown-local errors.
Methods retain their existing unsupported diagnostic. Six named examples receive unsupported
results through the shipping entry, with reference compilation controls. Genuine undefined
names still reject independently. Supported loop, branch and struct controls retain artifact
identity. These finite controls do not establish complete syntax admission.

The capacity audit now includes all twelve sources. Parse uses 776 of 1,024 name slots.
Wire uses 45,297 of 49,152 input bytes, leaving 3,855 bytes. Neither cap increased.
The stage opcode census now leaves fifteen internal tags unexercised, because CheckedNeg
is reached. Adding the named example corpus reduces the residue to three Byte arithmetic tags.
Complete production wire encoding and decoding, remaining type coverage and final gates stay open.

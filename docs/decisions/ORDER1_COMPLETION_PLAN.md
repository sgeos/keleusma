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
| Compile the complete stage corpus | Complete artifact identity against the reference for every current stage source, with a corpus inventory check | All fourteen pipeline-produced stage modules now serialize through Keleusma section emitters and match complete reference artifacts. The directory inventory is checked. Shape interning and signature-table numbering now run in Keleusma from semantic fields, using bounded windows over prior emitted records. |
| Avoid reference scaffold borrowing | Production compilation assembles module fields from pipeline output | `self_host_compile_scratch` provides this path. Preserve it through integration. |
| Derive type inputs from the self-hosted pipeline | Production rejection receives facts without walking the reference parser's abstract syntax tree or borrowing inferred types | The production input path now derives scoped scalar constraints from Keleusma parser and reconstruction output. Composite facts, complete binder/occurrence coverage and general admission remain incomplete. |
| Reject ill-typed subset programs | Independent pipeline rejection agrees with the reference on the rejection corpus, valid controls, and the rule-versus-form census | The scoped pipeline harness exercises all twenty rejection cases and seven valid controls without reference-first masking. The separate sixteen-cell rule-versus-form census now runs through that path with eight valid controls. Its required test now passes after preserving match-arm guards. Parser source diagnostics and type-stage verdicts remain distinct from unsupported syntax, capacity and execution failures. The shipping driver now obtains the independent verdict first. Production corpus and census checks assert PipelineRejected so reference rejection cannot mask missing stage coverage. |
| Preserve the first-pass monomorphization result | Every stage remains unchanged by monomorphization, with a control that does change | The corpus now lists all fourteen sources. Identity, changing control and directory inventory tests passed again after the relocation change on 2026-10-11. Repeat if stage sources change. |
| Preserve independence from inferred expression types | The current stage corpus compiles identically with recorded expression types absent | Four standing tests cover all fourteen sources with a directory inventory, a populated-table intervention after recording, a changed-emission control and an admission control. On 2026-10-11, clearing 58,671 recorded types leaves all fourteen complete artifacts unchanged. This does not remove reference admission or establish full-language independence. |
| Emit complete artifacts through Keleusma | Production output uses the Keleusma wire emitter for framing, opcode records, operand pools, auxiliary regions and integrity fields | The fourteen-stage comparison now supplies no reference section bytes, region positions or artifact lengths. All 66 opcode identifiers have encoding controls. Native names and return shapes now preserve independent lengths and share the existing name and shape tables. Struct templates now derive type-name indices and contiguous field-name runs in Keleusma, including empty runs. Debug-pool ordering and encoding now run in a dedicated Keleusma stage, and chunk debug offsets are stage-derived. Private composite placements and scalar initializer pools now encode through Keleusma, including stage-derived elision and root ranges. Composite and named constant forests, including initializer trees, now use the Keleusma breadth-first walker within the declared input bounds. Opcode and operand-pool semantic decoding now matches all fourteen stage instruction sequences and a 153,600-record raw corpus in each of two feature configurations. Auxiliary directory voting, bounds and duplicate checks now run in Keleusma, with borrowed payloads and no reference positions. All seventeen fixed-record kinds now decode through Keleusma and reconstruct every fixed-record region in the stage corpus. Signature and native-return range checks and shape-tag interpretation now run in a dedicated Keleusma schema stage. Borrowed name lookup, UTF-8 validation and owned constant reconstruction now run through Keleusma, including ranges, tags, name and child requests and ownership checks. Templates, enum layouts and data-layout semantics now decode through Keleusma, including optional-family policy, run expansion and initializer selection. Parameter tags and debug pools now decode through Keleusma, and the complete auxiliary-body reader composes all semantic families. Complete outer framing and module reconstruction now run through Keleusma readers. Windowed name interning and breadth-first constant scheduling remove the corpus-sized encoder limits. Contributor order, ranges and parameter tags are stage decisions. Unsigned self-hosted command output now uses the Keleusma emitter. Data-slot grouping, record splitting and shared-layout stride/run derivation now run in Keleusma from logical slot facts. |
| Reach the shipping tool | An end-to-end command emits the independently assembled artifact and reports type rejection through the new path | The command emits the same artifact as the production library, executes a valid result of seven, and independently rejects three invalid sources without emitting files. Unsigned self-hosted output now uses Keleusma serialization. A discriminator checks backend dispatch, and command-line tests cover the formerly rejected large forest and text sources. |
| Establish current verification | Relevant differential and failure-path tests, complete workspace checks and applicable integration gates pass on the final tree | The source-initializer tree records 3,202 workspace passes, three legacy adapter failures and five ignored tests across 188 suites. Test-only adapter repairs subsequently pass 193 targeted checks. The workspace was not rerun afterward. All fourteen complete stage artifacts pass the permanent shipping-entry comparison. Twenty-one selected no-default tests, fresh Clippy, documentation and no_std checks pass. Inference independence now covers 58,671 types. Fifteen initializer sources preserve complete artifacts, eight execute through Keleusma serialization/decoding and eleven invalid initializers reject independently. Declaration-order relocation now passes seven artifact controls and fifteen decoded runtime comparisons across five compile paths. Four protocol tests, 152 code-generation tests, 193 pipeline/wire tests and nine selected no-default tests pass. The tuple increment adds thirty artifact cases, five decoded executions, five independent negatives and a 400-function descriptor-reuse control. Return-layout queries now close the named forward-return and returned-array witnesses. Guarded-head local slots and generic cross-check coverage are failing controls. General header paths, binder and occurrence coverage, and final integration gates remain open. Exact receipts are in the reverse prompt. Full Order 1 is not complete. |
| Participate in the shadow pilot | Diagnostic and completion-claim samples retain baseline judgments, reviewed payloads, outcomes and explicit service status | Capture is installed for this line. Judgments are advisory and do not alter gates. |

## Current return-layout checkpoint

A Keleusma token census records return-type ranges. The existing header grammar computes
composite return layouts on demand, including declarations after a caller. Private seek record
111 drives bounded token reloading. Ten artifact cases, four decoded executions and four
independent rejection controls pass. The former forward-scalar and returned-array-of-tuple
witnesses now succeed. Cache lifetime and token-feed controls also pass independently.

Two failures remain explicit. Guarded heads with local bindings produce a local-slot mismatch,
and a generic-return fixture now stops before the reference cross-check it was meant to exercise.
The feature checkpoint retains both failing assertions. Repair these before continuing the
remaining input obligations and final gates. This checkpoint does not complete Order 1.

## Current tuple construction increment

Source-derived expression layouts now size mixed tuple members and preserve nested projections.
The thirty-source corpus includes scalar parameters and aliases, calls to earlier declarations,
Unit and Byte members, structs, arrays, text, data reads, nested tuple headers and split indexing.
Five decoded execution controls and five independent negatives pass. A 400-function corpus
checks reuse of temporary body descriptors, with header descriptors preserved across functions.
Eight open tuple contexts are admitted and the ninth is refused by a parser capacity diagnostic.

Private diagnostic 38 distinguishes unavailable expression layout from source rejection and
capacity exhaustion. Forward scalar return layouts and arrays of tuples returned from calls
remain explicit witnesses. General source-type coverage and the complete final verification
matrix remain outstanding. This increment does not complete Order 1.

## Current data relocation increment

The parser's final field-base records now reach compilation alongside immutable per-function
snapshots. Codegen translates scalar and indexed read/write records before reconstruction.
The old bases continue to govern independent source type extraction. The fused compiler collects
layout facts in a preliminary pass, discards function records from that pass, and then compiles
one group at a time. Address decisions remain in Keleusma, with a 640-field map and the existing
24-bit operand bound. Public bytecode version and opcode count remain unchanged.

The subsequent tuple increment closes the mixed-width and nested-header witnesses recorded
by this relocation increment. Its current evidence and remaining cases are stated above. The full final
verification matrix remains outstanding. This increment does not complete Order 1.

## Current decoder increment

Opcode semantics, directories, fixed records, signatures, names, constants, templates, enum
layouts and data layouts have Keleusma read paths. Optional layout rejection is distinct from
execution failure. Shared offsets and initializer selection are stage-derived. All fourteen
stage layout values match the reference, and a decoded layout drives runtime controls.
Parameter tags and debug metadata now have Keleusma readers. Complete auxiliary-body assembly
passes the fourteen-stage comparison and a persistent-execution control. Complete outer framing and module assembly now use the Keleusma readers. Source-admission
regressions now cross the former encoder limits and unsigned command output uses the Keleusma emitter.
Data-slot grouping and shared-run extraction now use stage requests and returned record plans. The reverse prompt carries current receipts and the pilot review.

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

The capacity audit now includes all thirteen sources. Parse uses 776 of 1,024 name slots.
Wire uses 45,297 of 49,152 input bytes, leaving 3,855 bytes. Neither cap increased.
The stage opcode census now leaves fifteen internal tags unexercised, because CheckedNeg
is reached. Adding the named example corpus reduces the residue to three Byte arithmetic tags.
Complete production wire encoding and decoding, remaining type coverage and final gates stay open.

## Framing and module-driven auxiliary assembly, 2026-10-04

The new auxiliary driver receives only a Module and constructs the complete auxiliary container
for each current stage. Its output supplies the auxiliary section in the twelve-stage framing
comparison. Opcode and operand-pool bytes in that comparison still come from the reference.
This proves auxiliary integration for the corpus and framing identity, not complete serialization.

Actual record lengths drive assembly. Keleusma supplies name indices, record encoding, container
positions, padding and streaming checksum values. Signature numbering, parameter tags and scalar
metadata still have the host provenance described in the wire plan. Unsupported auxiliary forms
fail explicitly. Oversized names and table counts are checked before the historical input packer.

Five framing functions raise the stage to 497 chunks. The input blob reaches 47,442 bytes,
leaving 1,710 under its unchanged cap. All twelve stages remain byte-identical under the existing
pipeline comparison and monomorphization identity controls. Four inference controls now clear
41,227 recorded expression types without changing complete artifacts.

The next work is complete module-driven opcode and operand-pool production, the remaining
auxiliary forms and semantic decoding, followed by shipping integration. Existing type coverage
obligations and final gates remain. The complete Order 1 goal remains active.

## Semantic opcode stream and complete corpus artifacts, 2026-10-04

The host now marshals typed Op fields. Keleusma owns inline-versus-pool selection, ordinal
assignment, field packing, sentinels and parity. The opcode census covers every current identifier
and composite form. Boundary controls reject unrepresentable fields and indices before writes.
The unsigned whole-module helper refuses signing and encryption flags explicitly.

All twelve production pipeline modules now serialize to reference-identical complete artifacts
through this path. The prior comparison borrowed opcode and operand-pool sections. That borrowing
is removed. Auxiliary record forms beyond the current helper, signature-table numbering, semantic
decoding, shipping integration and remaining type coverage are still open.

The stage fits unchanged capacities after sharing 65 repeated numeric schema literals through
four helpers. It uses 1,283 constant nodes against 1,365 and 47,687 input bytes against 49,152.
The stage has 505 chunks. Clearing 42,195 inferred expression types preserves all twelve artifacts.
The full Order 1 goal remains active.

## Accepted-source encoder capacity gaps, 2026-10-11

An isolated probe calls the production self_hosted_compile API with Target::host, then compares
Module::to_bytes with wire_module_artifact_via_kel. All six generated sources are accepted by the
production compiler. The probe also decodes every reference artifact through wire_decode_module_via_kel
and checks complete re-encoding identity. The decoder accepts all six.

| Source construction | Constant roots | Reference artifact bytes | Keleusma encoder |
|---|---|---|---|
| 170 arithmetic helpers plus main | 1,361 | 54,772 | Byte-identical |
| 171 arithmetic helpers plus main | 1,369 | 55,092 | Refuses the 1,365-root cap |
| 177 arithmetic helpers plus main | 1,417 | 56,996 | Refuses the 1,365-root cap |
| Text literal of 256 bytes | 1 | 1,244 | Byte-identical |
| Text literal of 257 bytes | 1 | 1,244 | Refuses the constant name/input capacity |
| Text literal of 60,000 bytes | 1 | 60,988 | Refuses the constant name/input capacity |

Generate the arithmetic helpers with distinct names f0 through fN-1 and this body.

```keleusma
fn f0(x: Word) -> Word { x + 1 + 2 + 3 + 4 + 5 + 6 + 7 + 8 }
```

Append this entry function.

```keleusma
fn main() -> Word { f0(0) }
```

For the text cases, generate a function returning Text whose body is a quoted ASCII x repeated
at the stated length. The scratch harness is tmp/order1-admission-probe/src/main.rs. The final
receipt is tmp/order1-module-admission-decoder.log, from cargo run --offline --release
--manifest-path tmp/order1-admission-probe/Cargo.toml with the usual wrapper and stage-cache
settings. This is a production-library probe, not an executed command-line compile invocation.
The table and construction recipe preserve the evidence needed to reproduce it without scratch files.

The encoder's forest_input rejects constant names above 256 bytes before its aggregate 49,152-byte
input bound. Its compact input path independently refuses more than 1,365 scalar roots. These
are observed blockers to switching shipping output while preserving current source admission.
Promote these cases into standing regressions when removing the encoder limits. Do not narrow the
compiler's accepted input to make the serializer fit. Other name, forest and input bounds still
need coverage. Shipping output remains Rust until that integration is ready.

## Streaming emission and shipping integration, 2026-10-11

The accepted-source counterexamples above now have standing regressions. Their former encoder
refusals are historical. The new public emitter does not call module_input, the compact packer,
or the bounded forest producer. It uses a stage-driven queue and a hash-indexed backing store.
Keleusma computes the hash and all equality/selection decisions, including equal-hash collisions
and last-fresh-name sharing. The host does not compare name bytes when interning.

The coordinator also determines contributor order and rejects out-of-order transport. The first
streaming draft still selected family order in Rust. The audit caught that provenance regression,
and explicit stage ordering was added before integration. The current command path selects the
Keleusma serializer for unsigned self-hosted output, with a branch discriminator and end-to-end
large-source comparisons. The default reference and cryptographic paths retain their serializers.

The next serialization obligation is host run processing. wire_stream_auxiliary::slot_runs still
groups logical data slots using names and visibility, then splits runs at the field-width bound.
shared_slot_fields still detects constant-stride runs, calculates their starts and splits them.
Move those decisions into Keleusma.
Do not treat byte identity or shipping dispatch as proof that these decisions have already moved.
The remaining type-input obligations and final verification rows above also remain active.

## Data-layout run derivation, 2026-10-11

The public emitter no longer uses host name-equality grouping or shared_slot_fields. A stage
prepass derives groups from logical slot facts, and the name coordinator consumes its verified
result count. A second stage prepass derives shared runs and split records. Rust stores the plans
and transports requested fields or bytes. The legacy helpers retain their existing callers outside
this public emission path. Current receipts are in the reverse prompt.

## Tuple projection audit, 2026-10-11

An isolated probe runs reference compilation and type_check_source_via_kel independently on eight
sources. Five comparisons agree. Three require work, with the valid-source refusal taking priority.

| Case | Reference | Independent stage |
|---|---|---|
| Return bool tuple field as Word | Rejects | Rejects |
| Return bool tuple field as bool | Accepts | Accepts |
| Pass projected `(Word, Word)` to `(Word, bool)` | Rejects | Accepts |
| Pass projected `(Word, Word)` to matching tuple | Accepts | Accepts |
| Pass projected `[Word; 2]` to `[Word; 3]` | Rejects | Rejects |
| Pass projected `[Word; 2]` to `[Word; 2]` | Accepts | Rejects |
| Pass projected P to a same-layout Q | Rejects | Accepts |
| Pass projected P to P | Accepts | Accepts |

The three unresolved sources follow.

```keleusma
fn f(xs: [Word; 2]) -> Word { 7 }
fn main(t: ([Word; 2], Word)) -> Word { f(t.0) }
```

```keleusma
fn f(t: (Word, bool)) -> Word { 7 }
fn main(t: ((Word, Word), Word)) -> Word { f(t.0) }
```

```keleusma
struct P { x: Word }
struct Q { x: Word }
fn f(q: Q) -> Word { 7 }
fn main(t: (P, Word)) -> Word { f(t.0) }
```

The receipt is tmp/order1-runs-type-probe.log. The scratch harness is
tmp/order1-type-probe/src/main.rs and exits one on comparison disagreement. Promote these
cases and their valid controls into standing regressions while repairing projection identity.
The existing composite projection suite covers struct fields and array elements, so its green
result does not close this tuple-field gap. No type-input implementation changed in this increment.

The tuple-of-array fact dump adds a second localization. The callee requirement is scalar-array
tag 49, while the projected argument is primitive Word tag 1. The main parameter's structural
construction rows also encode both tuple members as Word. Repair nested header capture as well
as projection result identity. The receipt is tmp/order1-runs-type-detail.log, produced by the
scratch detail binary. Inspection also finds TupleField node 37 absent from typed_result_node,
with the operand path using its baked layout kind. step_tupleaccess retains a packed byte offset
and layout kind but no source element ordinal. These observations guide the next repair and do
not establish that every cause has been identified.


## Tuple header and projection repair, 2026-10-11

This increment supersedes the unresolved status of the eight-case tuple projection audit above.
All fourteen cases in the expanded test agree with the reference, and invalid cases require
PipelineRejected from the shipping entry. Metadata 104 supplies source ordinals, metadata 105
supplies Keleusma-computed header layouts, and type expression 19 projects exact tuple member
identity. Equal byte widths do not determine nominal identity or array dimensions.

The iterative header scanner handles grouping, unit, nested tuples and literal array dimensions.
Repeated layouts are interned. Eighty repeated parameter headers and eighty repeated return
headers retain admission. Tests check both sides of the 32-frame and 32-layout limits directly.
The reference parser has a lower recursion limit and is not the stage-depth oracle.
Zero-length fixed-array declarations and out-of-range member indices produce source rejection.
The descriptor pool holds 1,024 entries and the layout member pool holds 256 entries.

Six complete-artifact controls execute host-supplied values and return seven. Nested tuple
equality produces true and false for equal and unequal inputs. Four nesting shapes preserve
struct members, including the third-level case that exposed a missing descent in the frame walker.
A private tuple extraction marker maps back to the existing public Tuple kind. No opcode or public
bytecode version changed. The selected type capacities remain 640.

The parser adds 9,938 private value slots at 32 bytes each. Its private footprint grows by
318,016 bytes to 1,043,200 bytes, while shared storage remains 335,992 bytes. Current parser output
has 198 chunks and 165,924 bytes. Code generation has 79 chunks and 71,772 bytes. The type stage
has 43 chunks and 22,580 bytes. Its added private scalar state costs 64 bytes.

The fourteen-stage artifact, monomorphization and inference checks pass on the current production
sources. A separate directory-inventoried probe sends every stage through self_hosted_compile and
compares complete artifacts. All fourteen pass. The full workspace's two guard failures were
repaired without changing production sources, then the affected targets passed 189 checks.
The reverse prompt states the failed full-run receipt and the subsequent targeted receipts.

The remaining thirteen-source probe confirms that nested nominal signatures can remain Top while
the reference supplies Flat Struct or Flat Tuple. The shipping chunk-body comparison does not
check this metadata and accepts both divergent artifacts. Repair layout derivation and strengthen
that comparison while preserving valid admission. Mixed bool/Word tuple literals still use a
16-byte body instead of nine bytes. Nested tuple literals and array-of-nested-tuple headers leave
two reconstructed roots. An empty array argument to a positive-length parameter is independently
accepted, while an unused empty literal is a valid artifact control. The reverse prompt includes
sources, commands and logs. Three cast negatives agree in this finite probe. A non-range for-loop
remains explicitly unsupported and is distinct from these type and artifact failures.

Remaining binder, occurrence, alias, enum-capacity and general type-input obligations, together
with the final integration gates, keep the complete Order 1 goal active.


## Nominal layouts and metadata admission, 2026-10-11

Declared member names now support bounded monotone resolution across forward struct dependencies.
The maximum 64-struct reverse chain resolves. Sixteen signature controls preserve complete artifacts,
and a host-value execution control checks corrected offsets after a Byte field. Signature size
metadata uses the full unsigned 32-bit range. Fixed, Float and Text scalar layout identifiers are
seeded without aliasing an absent name to identifier zero.

The shipping compiler compares every Module, Chunk and DataLayout field, with exhaustive field
patterns and 31 independent mutation controls. The permanent stage artifact test calls the shipping
entry and then the Keleusma serializer and decoder. A missing private initializer is now refused,
where the earlier body-only comparison admitted a program returning one instead of six. The
producer still needs repair. The reverse prompt includes the source and standing boundary test.

Private type tag 7 retains an empty literal's zero length and array shape without allocating a
structural-pool cell. Its element remains unknown. Ten source controls agree with reference typing,
including six independent rejections. An unused empty array remains valid. Empty equality retains
the reference's runtime comparison fault. Direct literal indexing remains an extraction gap.

Text metadata 106 records body usage of Text-bearing parameters and the structural return type's
Text presence. The analysis stage computes Boolean text flow over stack, locals, calls, returns and
yields, and derives ephemerality. The host supplies opcode facts and storage and uses the existing
topological scheduler. It does not supply a reference text-flow result. These decisions retain
large-text and Fixed admission under the stronger comparison. Text allocation bounds are outside
this Boolean pass. Wildcard parameters remain explicitly unsupported.

The parser adds 30,272 private bytes and 24 shared bytes. Analyze adds 64 private bytes and
136 shared bytes. The type stage adds no storage. Source capacity limits, bytecode version 2 and
66 public opcodes remain unchanged. The final workspace's single stale residency pin was corrected
and its 152-test target rerun successfully. The complete workspace was not rerun after that test-only
change. Remaining initialization, construction, header, binder, occurrence and integration
obligations keep the complete Order 1 goal active.


## Source initialization and data addresses, 2026-10-11

The private initializer gap above is repaired for the measured scalar forms. Keleusma emits typed
initializer descriptors, including default zeros, and the host only validates and materializes them.
Negative payloads use a complement record so minus one cannot collide with the legacy sentinel.
Fifteen artifact controls and eight decoded-execution controls cover signed Word values, Byte bounds,
Bool values, grouped syntax, defaults, multiple blocks and visibility interleaving. Eleven invalid
initializer forms require independent PipelineRejected from the shipping entry.

Keleusma computes the shared-first slot partition and emits runtime field ordinals and bases.
Function type inputs retain immutable address snapshots, avoiding a false rejection when a later
shared declaration would otherwise reinterpret an earlier private Word as Byte. Such a program still
requires relocation of its earlier body and receives an unsupported result. Its witness is in the
reverse prompt and the standing initializer tests.

The expanded parser has 525 source data fields. Its table and four scans now admit 640, with
boundary controls and a scan-inventory check. Private storage increases by 37,440 bytes. No selected
type-stage cap, public opcode or public bytecode version changes. The existing address-space limit
is checked before expansion, with a transport-only boundary test avoiding huge descriptor allocation.

The final workspace's three failures were in test adapters. The pipeline adapter now recognizes
address metadata, and two stage-scale wire joins use the existing compact scalar tail. They retain
the same reference name/pool comparisons and resident buffer limits. Their 193-test rerun passes.
Other legacy forest fixtures retain the uncompressed grammar. The production streaming path and
all fourteen artifact comparisons passed before those adapter repairs.

Full Order 1 remains active. Complete declaration-order handling, construction and header paths,
remaining binding and occurrence obligations and final integration gates are not established by
these finite controls.

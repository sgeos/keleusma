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
| Compile the complete stage corpus | Complete artifact identity against the reference for every current stage source, with a corpus inventory check | Twelve stage tests exist. Repeat after the pipeline changes. |
| Avoid reference scaffold borrowing | Production compilation assembles module fields from pipeline output | `self_host_compile_scratch` provides this path. Preserve it through integration. |
| Derive type inputs from the self-hosted pipeline | Production rejection receives facts without walking the reference parser's abstract syntax tree or borrowing inferred types | Four extraction helpers are partial. Expressions cover only part of the required shapes. Close the residuals and move the production input path. |
| Reject ill-typed subset programs | Independent pipeline rejection agrees with the reference on the rejection corpus, valid controls, and the rule-versus-form census | The current test harness feeds reference syntax to `verify_types.kel`. The shipping driver calls the reference first. Neither proves independent production type rejection. |
| Preserve the first-pass monomorphization result | Every stage remains unchanged by monomorphization, with a control that does change | The corpus now lists all twelve sources. Identity, changing control and directory inventory tests passed on 2026-10-04. Repeat if stage sources change. |
| Preserve independence from inferred expression types | The current stage corpus compiles identically with recorded expression types absent | Refresh the existing inference-independence evidence for all current sources. |
| Emit complete artifacts through Keleusma | Production output uses the Keleusma wire emitter for framing, opcode records, operand pools, auxiliary regions and integrity fields | Region coverage tests exist, but they do not establish production serialization. `Module::to_bytes` remains on current artifact paths. Preserve the wire plan's encoder and decoder obligations. |
| Reach the shipping tool | An end-to-end command emits the independently assembled artifact and reports type rejection through the new path | The shipping self-hosted backend exists. Integrate the remaining paths and test the command itself. |
| Establish current verification | Relevant differential and failure-path tests, complete workspace checks and applicable integration gates pass on the final tree | Prior full workspace verification is incomplete. Narrow checks cannot establish this requirement. |
| Participate in the shadow pilot | Diagnostic and completion-claim samples retain baseline judgments, reviewed payloads, outcomes and explicit service status | Capture is installed for this line. Judgments are advisory and do not alter gates. |

## Work sequence

1. Preserve explicit else presence and struct-literal identity or declared field count through
   parser records. Both metadata paths now exist, with reference comparisons for branch tails
   and struct field counts. The struct path emits declared count metadata immediately before
   the construction record and retains nested and reordered literals.
2. Close remaining declaration, call argument, occurrence, binding and expression extraction
   gaps. Treat field and index failures during reconstruction as explicit rejection paths.
   Integer and enum match-arm comparisons now have extraction and verdict controls.
   Source heads now retain separate tail requirements and binding names. Match guards and
   the remaining input channels still require work.
3. Integrate the rejection stage with the production compiler. Keep the Rust implementation
   as a differential oracle, and prove that its early rejection cannot mask missing stage work.
4. Complete production artifact emission through the existing Keleusma wire implementation.
   Audit which values are derived, marshalled, encoded or copied. Region byte coverage alone
   does not establish that provenance.
5. Refresh all-stage identity, monomorphization and inference-independence evidence.
   Exercise the shipping command, run the final gates and audit every row above.

## Constraints and open evidence

Bytecode version remains 2. No new bytecode opcode is authorized. The internal parser record
changes required by this plan do not authorize changing the public artifact format.
The V0.3.X workstream remains separate. Publication requires in-session confirmation.

The legacy handoff is invalid-and-stale. Its stamp is thirteen commits behind the starting
tree, its ancestry check passes, and its unchanged-stage-source claim is contradicted by
`verify_types.kel`. Its full-suite and advisory checks have not been rerun for this increment.

The first shadow evaluation failed with a transport error. An escalated evaluation was
rejected by automatic approval review because it did not recognize authorization for the
external destination and payload. A specific operator approval request is pending.
Local pilot capture and compiler work can continue independently.

## Production integration evidence

Inspection on 2026-10-04 confirmed that `compile_source_with_target` in the command-line tool
selects `self_hosted_compile`, while the unsigned artifact path still calls `module.to_bytes()`.
The wire stage already exposes framing and sealing commands through `dispatch_frame`.
Routing region payloads alone does not close the remaining complete-artifact emission work.

## Expression extraction evidence

The match comparison corpus includes nested integer and enum dispatch, wildcard-only matches,
explicit enum coverage, later mismatched arms, named operands and an unresolved payload binder.
The latter remains unknown on both input paths, so comparison agreement does not establish
payload-type resolution. Bare block expressions in match results are rejected by the reference
parser. Integer matches without a wildcard fail during reconstruction. Match guards are discarded
by the self-hosted parser and remain a production input gap.

Separate reconstruction of each source head recovers its return row and its own binding names.
Function guard expressions are still traversed. A probe found that the Rust reference accepts
`fn f(x: Word) -> Word when 1 { 1 } fn f(y: Word) -> Word { 2 }`.
No new root boolean requirement is imposed while that reference behavior remains unresolved.

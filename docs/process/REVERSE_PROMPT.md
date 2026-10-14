# Reverse prompt

## Active goal and limits

Complete Order 3, trap analysis, on line v0.2.3 and branch
feat/v02-order3-trap-analysis. The preceding arithmetic commit is 88de980b.
Order 3 remains active and is not close to a completion claim. The requirement
register and checked-outcome protocol are in docs/decisions/ORDER3_COMPLETION_PLAN.md.

The operator declined a bytecode-version advance. BYTECODE_VERSION stays 2 and
FORMAT_FINGERPRINT stays 0x4327_63E1 throughout V0.2.3-WIP. The fingerprint changes
only at publication. V0.2.2 remains the last published release. Instruction count
stays 66, selected type capacities 640 and the depth limit 32. No publication is
authorized. Appendix B was read before tracked writes.

The legacy HANDOFF.md remains invalid-and-stale. Its previously reported distance
of 56 commits was measured at 1cfb6dd9, not the current head. Use this bounded
handoff and the current completion plan for work state.

## Implemented this increment

The parser now uses an internal IfMark to keep enclosing operators pending until
a conditional operand is complete. Three tests cover artifact equality, execution,
evaluation order and a zero divisor reaching an explicit Trap. The previously
recorded inline-conditional ordering defect is repaired.

Reconstruction and code generation implement checked arithmetic behind records
118 through 125. Forest nodes 77, 78 and 80 represent the checked expression, arm
result and typed binding read. The source parser DOES NOT emit these records yet.
Ten unit controls inject records directly. They cover Word operations, Byte
bindings, guards, literal patterns, nesting, type constraints, malformed records
and side-table boundaries. These are backend tests, not source admission evidence.

The reconstruction stage derives coverage from guards and patterns, requires an
ok catch-all, and guards its context stack at 32. Its existing 256-word side table
holds fifty arms when otherwise empty. The fifty-first is refused. Deferred integer
scratch entries are now deduplicated so the valid fifty-arm case also lowers and
executes. Pool interning remains in emission order. No capacity was increased.

## Verification receipts

All processes are terminal. The implementation was held fixed during each run.

- Ten backend unit tests pass in tmp/order3-outcomes-backend-final.log.
- The broad final set passed 381 of 382 tests in tmp/order3-outcomes-acceptance.log.
  The only failure was a stale self-compilation count of 92. All 97 actual codegen
  functions compiled byte-identically. The constant was corrected and the test
  rerun successfully in tmp/order3-outcomes-atomic-final.log.
- Three expression tests pass after removing a redundant cast in
  tmp/order3-outcomes-frames-final.log.
- The broad set includes the complete fourteen-stage artifact comparison, which
  passed on the current implementation. It also covers wire handling, loop outcomes,
  reconstruction failure modes, opcode tags and the previous arithmetic increment.
- Fresh workspace Clippy passes in tmp/order3-outcomes-clippy-final.log.
- The detached compiler build passes in tmp/order3-outcomes-detached.log.

An earlier boundary test found deferred scratch exhaustion. Deduplication repaired
it without increasing the 64-entry array. Earlier broad failures named the new
context arrays and moved measurements. Arrays now have unique names and explicit
coverage registration. Parse measures 986 names and 28415 blob bytes. Atomic codegen
self-compilation measures 97 functions. These were measured before updating pins.

The complete final broad set was not rerun after the two test-only corrections.
The full feature matrix, full isolated suite, detached compiler tests, doctests,
Miri and release gates remain unexecuted for this increment. The current receipts
support this increment and do not establish Order 3 completion.

## Immediate next work

Implement checked-outcome source parsing into the tested protocol. Do not substitute
reference syntax or reference lowering. The existing parser still rejects an
unexpected brace through pe_block_syntax. Its source identifier seeds live in
selfhost_host.rs and two production feeders in selfhost/mod.rs. Contextual ok is
already seeded. Overflow, underflow and zero_divisor will need recognition.

Reserve flag, low and high temporaries after the operand expressions. Emit typed
binding reads before guard/body records, then arm wrappers and binding-name/LetIn
records in reverse pattern order. Preserve source scope, literals, guards and arm
order. Enforce admissible classes, pattern arity, unreachable arms and mandatory
ok coverage independently. The new expression marker supplies the conditional
boundary needed inside guards and bodies. Checked shifts are not yet covered by
the backend. The completion plan records the exact protocol and forest layout.

After source parsing, total bounds contracts remain required for BoundsCheck,
GetIndex, GetDataIndexed and SetDataIndexed. Legacy integer Div and Mod also remain
partial in hand-built bytecode. No trap-free classifier, host trap-free load policy
or proof-gated no-check mode exists. Order 2 typed widening does not justify removing
runtime guards. Every eliminated check needs an explicit proof and negative controls.
Finish the full requirement audit and prescribed verification before completion.

## Status question and concurrent line

The operator asked whether Order 3 is clearly close. The answer given was no,
because core implementation remains. A read-only inspection of the canonical
V0.3.X worktree found clean trunk 7d141fcc dated 2026-10-01. Workstream B completion
was integrated through pull request 482. Both full native gate receipts pass at
5695ab1f on 2026-09-30, but tools/gate-status.sh currently reports UNVERIFIED because
tracked inputs changed. No remote activity was queried and no peer message sent.
The V0.3.X worktree was not modified. Other native workstreams remain separately scoped.

## Pilot and handoff

Pilot task v023-order3-outcomes-88de records one diagnostic and three completion
claims, all with non-independent self-review labels. Its completed outcome describes
this increment, not the entire goal. External evaluation remains unavailable under
the previous automatic approval rejection. The export is retained at
 tmp/order3-outcomes-pilot-checkpoint.json. This branch does not own the shared pilot
checkpoint. No provider-accuracy or saved-reasoning claim is supported.

Check the v0.2.3 inbox and export its tracked checkpoint before final handoff.
Unrelated .codex/ and nvim.log remain untouched. No push, merge or publication.

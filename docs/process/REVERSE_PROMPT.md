# Reverse prompt

## Requested branch backup on 2026-10-15

The first-class compiler-managed ring buffer request is already recorded as
Backlog B42 and linked from the V0.2.X roadmap in 9b176725. The operator's request
to commit and push all project work also covers the committed parser recovery
at fc3718c2. This documentation update accompanies its feature-branch push.
Use the hook's documented work-in-progress option. The complete verification gate
has not passed and this backup does not establish Order 3 completion.

No implementation or tests changed in this backup increment. No tests were rerun.
The inbox is empty and the exported interline checkpoint is unchanged. The legacy
handoff is invalid-and-stale at a measured distance of 60 commits from e84d7cfa to
fc3718c2. Its ancestry anchor passes, but its twelve-stage inventory and unchanged
stage-source assertions fail against fourteen modified stages. Its other content
checks were not rerun. The current work state and remaining work follow below.
No eligible development-pilot samples were collected for this administrative
increment. Local .codex/ settings and nvim.log remain untracked.

## Completed parser recovery on 2026-10-15

The preceding backup push completed at 9b176725 on
origin/feat/v02-order3-trap-analysis. The ring-buffer request remains Backlog B42.
This recovery increment is separate from that completed push.

A two-mebibyte worker reproduced the debug failure during Rust parsing, before
compilation or verification. Splitting nine primary-expression branches into
focused helpers reduced the measured common debug frame from about 40 KB to
6.7 KB on this build. MAX_PARSE_DEPTH remains 24. The separate verifier depth
limit remains 32. No additional worker stack or relaxed parser limit was used.

Both tests in tests/parser_stack_bounds.rs pass. They compile all fourteen current
stages on an explicit two-mebibyte stack and check nine expression shapes at
accepted depth 23 and rejected depth 24. The debug quick suite ran 2464 tests,
with 2463 passes and one validator-command timeout. No stack-overflow abort
remained. A direct command retry completed successfully in about 125 seconds,
exceeding the former 120-second deadline. The test now allows 600 seconds in debug
and retains 120 seconds in release. All verdict and boundary assertions remain.
All four validator command tests then passed, including both validators at the
arena boundary and one byte below it for all fourteen stages. Runtime was 1258
seconds for that test binary. All verification processes are terminal.

Receipts are tmp/order3-debug-stack-before.log,
 tmp/order3-debug-stack-phase.log, tmp/order3-debug-stack-phase-after.log,
 tmp/order3-debug-parser-bounds.log, tmp/order3-debug-quick-after.log and
 tmp/order3-debug-validator-after.log. Fresh workspace Clippy passed before the
test deadline edit, and the changed command-test target passed fresh Clippy after
it, in tmp/order3-debug-parser-clippy.log and
 tmp/order3-debug-parser-clippy-final.log. The complete quick suite was not rerun
after that test-only change. Other feature configurations, release gates and Miri
were not rerun for this recovery increment. This is scoped regression evidence,
not a general proof of parser stack usage or resolution of every H2 concern.

Pilot task v023-order3-stack-9b17 records one diagnostic and three claims, with
non-independent self-review labels and a resolved increment outcome. The validator
claim was labelled unsupported because its excerpt omitted the build profile,
although the full log establishes debug execution. The export is
 tmp/order3-stack-pilot-checkpoint.json. External evaluation remains unavailable
under the previous approval rejection. This increment does not complete Order 3.

## Active goal and limits

Complete Order 3, trap analysis, on line v0.2.3 and branch
feat/v02-order3-trap-analysis. The preceding backend commit is 2ac8b1e8.
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

## Previous backend increment at 2ac8b1e8

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

## Backend increment verification

These earlier runs are terminal. Current live verification is listed above.
Their implementation was held fixed during each run.

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

Continue the existing Order 3 implementation plan with checked-outcome source
parsing. The debug recovery is verified within the scope recorded above.

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

## Previous pilot and handoff

Pilot task v023-order3-outcomes-88de records one diagnostic and three completion
claims, all with non-independent self-review labels. Its completed outcome describes
this increment, not the entire goal. External evaluation remains unavailable under
the previous automatic approval rejection. The export is retained at
 tmp/order3-outcomes-pilot-checkpoint.json. This branch does not own the shared pilot
checkpoint. No provider-accuracy or saved-reasoning claim is supported.

Check the v0.2.3 inbox and export its tracked checkpoint before final handoff.
Unrelated .codex/ and nvim.log remain untouched. The branch push is now authorized.
No merge or publication is authorized by that request.

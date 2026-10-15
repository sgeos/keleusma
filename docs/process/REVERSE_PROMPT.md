# Reverse prompt

## Checked shifts and wide integers on 2026-10-15

The active goal remains Complete Order 3, trap analysis. Work is on line v0.2.3,
branch feat/v02-order3-trap-analysis. This increment starts from 68bc27b1, which
implemented checked arithmetic source handlers. The preceding authorized backup
push reached f26c381c. Backlog B42 retains the compiler-managed ring buffer request.

Checked arithmetic left shifts now work on the supported 64 bit host path. Parser
operator 37 retains asl separately from lsl. Bare shifts keep their existing
lowering. Checked shifts use CheckedMul. Reconstruction requires a Word operand
and a literal amount from zero through 62. Code generation interns the positive
multiplier before compiling the left operand, matching reference constant-pool
order. Source controls cover boundaries, invalid amounts and families, nested
handlers, operand side effects and absence of unreachable traps. Bare Byte shifts
after an explicit cast now recover the operand kind from recorded scalar facts.

The wide-literal transport defect observed in the preceding turn is repaired.
Lexer token 65 carries an interned spelling index. Appended 1280-Word tables in
lexer and parser retain the stage-computed value. The host only transports those
values. Both parser feeds and the cursor-trace feeder populate the table. The
public token trace resolves the table entry to the semantic integer token.
Integer accumulation checks range before multiplication. Token 66 reports integer
overflow or a radix prefix without digits. Decimal, hexadecimal and binary tokens
flush correctly at end of input. Detached lex and parse commands recognize failures.

Ordinary literal records now carry their full word. Internal record 128 carries
the nonnegative complement of a negative literal, avoiding the legacy transport
sentinel at negative one. Reconstruction diagnostic 7 rejects a malformed negative
complement. Constant-field parsing now retains a leading minus sign. Controls cover
maximum Word literals, negative literals and patterns, constant fields, a string
spelling collision and interner capacity. Exact capacity and duplicate reuse accept,
while a new entry beyond capacity refuses through the existing stage bounds guard.

## Verification and exact limits

The broad release self-host run executed 308 tests. It passed 306, including the
complete byte-identical artifact comparison for all fourteen current stages, and
failed two stale measurement assertions. Two pre-existing parser instrumentation
tests were skipped. The failures were a six-diagnostic count after adding the
seventh cause, and old lexer record counts. The measured lexer residency is now
1650 total records and 403 peak records. The threefold fusion assertion is retained.

After correcting those test assertions, all seven selected controls passed. This
includes the two repaired assertions and all five wide-literal tests, including
a newly added comparison of collected and fused feeds. The full broad set was not
rerun after these test-only edits. The implementation stayed fixed during each run.

All thirteen checked-backend and signed-literal controls pass. All four shared-slot
layout controls pass. The complete-stage capacity measurement passes. Fresh workspace
Clippy with all targets and self-host passes. The detached compiler builds, and four
actual detached-command controls accept valid input and reject overflow. Both debug
parser-stack tests pass, including every current stage on a two-mebibyte stack.
Formatting and diff checks pass. All verification processes are terminal.

Receipts are tmp/order3-arithmetic-acceptance.log,
 tmp/order3-arithmetic-final-controls.log, tmp/order3-arithmetic-backend-final.log,
 tmp/order3-arithmetic-layout-final.log, tmp/order3-arithmetic-capacity-final.log,
 tmp/order3-arithmetic-clippy-final.log, tmp/order3-arithmetic-stack.log,
 tmp/order3-arithmetic-detached-final.log and
 tmp/order3-arithmetic-detached-commands.log. Initial failing controls are in
 tmp/order3-shifts-before.log, tmp/order3-shifts-bare-diagnostic.log,
 tmp/order3-wide-before.log and tmp/order3-wide-extended.log.

The full five-configuration matrix, isolated full suite, detached compiler tests,
shipping command tests, documentation tests, Miri and release gates were not rerun.
The source frontend checks do not establish Order 3 completion.

## Capacity and compatibility

The parser source measures 402866 bytes under the unchanged 524288-byte lexer cap.
Parser module input uses exactly 1024 names and 30050 blob bytes. The 1024-name pool
has no headroom. Adding parser fields or enum variants may exceed it. Measure before
changing that representation. The new literal tables each have 1280 Word entries,
matching the existing lexer interner bound. No selected type capacity was raised.

BYTECODE_VERSION remains 2. FORMAT_FINGERPRINT remains 0x4327_63E1 throughout
V0.2.3-WIP. Public instruction count remains 66, selected type capacities 640 and
verifier depth 32. The separate reference-parser depth remains 24. Parser operators,
lexer tokens and reconstruction records are private protocols, not public opcodes.
V0.2.2 remains the last published release. No publication is authorized. Appendix B
was read before writes. The legacy HANDOFF.md remains invalid-and-stale.

## Next work

Continue docs/decisions/ORDER3_COMPLETION_PLAN.md with total bounds contracts.
Specify exact stack effects, fallback kinds, diagnostic payloads and unsupported
full-language forms before updating all consumers. BoundsCheck, GetIndex,
GetDataIndexed and SetDataIndexed still raise implicit bounds faults. Legacy integer
Div and Mod still fault in hand-built bytecode and also need a contract disposition.

Source inspection found two concrete design constraints. GetDataIndexed carries
only base and length, so an invalid-index fallback needs a reliable element-kind
rule, including the zero-length case. TrapKind codes zero through seven are already
occupied. Preserve numeric index and length diagnostics across narrow runtime words
as well as source fault locations. These are open contract questions, not approved
implementations. The existing temporary check_stage binary links an older runtime
and must not serve as current verification after changing instruction contracts.

No trap-free classifier, host trap-free policy or proven no-check mode exists.
Order 2 typed widening does not justify check elision. Complete the original
requirements and final audit before marking the goal complete.

## Pilot and handoff

Pilot task v023-arithmetic-68bc records the first failed source control and three
completion claims. Labels are non-independent self-review. The wide-control claim
was labelled unsupported because its excerpt shows passing tests but not the claimed
corrective edits. Those edits are established separately by the source diff and logs.
The reviewed export is tmp/order3-arithmetic-pilot-checkpoint.json. This branch does
not own the shared tracked pilot checkpoint. External evaluation was not retried
after the earlier automatic approval rejection. No independent accuracy or saved
reasoning claim is supported. The task outcome resolves this increment only.

The v0.2.3 inbox is empty. Its tracked checkpoint was exported and has no diff.
No peer message was sent. Local .codex/ settings and nvim.log remain unrelated and
untracked. No merge or publication occurred, and the concurrent V0.3.X worktree was
not modified. Feature-branch backup is authorized by the operator's earlier request.

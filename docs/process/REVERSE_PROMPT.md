# Reverse prompt

## Total scalar array reads on 2026-10-15

The active goal remains Complete Order 3, trap analysis. Work is on line v0.2.3
and branch feat/v02-order3-trap-analysis. This increment starts from pushed commit
28a280db, which made indexed writes total.

GetIndex with a flat Word or Byte operand now consumes array and index and returns
[value, index, b0, b1, b2, b3, flag]. Invalid indices yield typed zero and flag one
without reading an element. Valid indices yield the element and flag zero. Empty
flat arrays are supported by the primitive. Representation, stride, type and arena
lifetime checks remain. The full signed index is compared with the checked bound
before conversion to an address, avoiding narrow-host truncation. The requirement is two and net growth five, with no net
shrink and nominal cost eight. Other operand forms retain their single-result
contract and remain outside the first-pass trap-free proof.

Both bare lowerings dispatch failure through Trap kind eight and discard successful
diagnostics. Rust routes all generated reads through emit_index_read. Keleusma wraps
scalar reads in emit_op, with emit_raw reserved for already-handled outcomes. The
reference debug recorder attaches exact source spans to the read and its trap.

Both compilers now lower checked reads of stack-resident scalar arrays from the
total outcome.
Operands are evaluated once before seven temporaries are reserved in the order
flag, value, index and four bound limbs. The value binds the element type, while
invalid_index always binds Word, including for Byte arrays. Reconstruction validates
those different binder kinds and the single-pattern arity. Full unguarded invalid
coverage removes the fallback trap. Guarded and literal patterns retain it. A
value-and-break trailer keeps the virtual loop structurally bounded without a trap.

The parser uses its existing checked protocol, adds invalid_index recognition and
retains the indexed family in the context form's high lane. Header record 118 still
carries the element scalar kind. Two private scalars with no references, stmt.se_r2
and stmt.se_l2, were removed to make room under the name cap. Direct Word and Byte
array literals now arm the existing indexing postfix path. They previously parsed
as two array literals in a source such as [7][2].

## Verification

The final release self-host run passed all 1758 executed tests, with two existing
parser instrumentation tests skipped. It includes all current stage artifacts,
resource-admission boundaries, compiler and parser regressions, malformed input
controls and narrow runtime checks. The implementation was fixed during the run.

All ten indexed-read controls pass. The floating-point-disabled set passes twenty-five
bounds and indexed-access controls. No-default library tests pass 175 cases. Fresh
workspace Clippy with all targets and self-host passes, and the detached compiler
builds. Formatting and diff checks pass. The first broad run had 1754 passes and two
stale measurement assertions, both corrected before the final run. Typed-verifier
residency is now 1513 total records and 467 peak, preserving the threefold criterion.

Final receipts are tmp/order3-read-final-acceptance.log,
 tmp/order3-read-capacity-final.log, tmp/order3-read-clippy-final.log,
 tmp/order3-read-detached-final.log, tmp/order3-read-no-floats-final.log and
 tmp/order3-read-no-default.log. The first broad run is
 tmp/order3-read-acceptance.log.

Fifteen backend controls pass, including index binder kinds, seven temporaries,
malformed descriptors and slot limits. The parser-name measurement is 1023 names
and 30109 blob bytes under unchanged caps, leaving one name free. Current-source
stage sizing keeps every stage chunk below the existing 1536-opcode verifier limit.
Codegen now has 100 functions. The parser and other fixed capacities were not raised.

Receipts include tmp/order3-read-focused.log, tmp/order3-read-backend.log,
 tmp/order3-read-op-sizes.log, tmp/order3-read-capacity.log,
 tmp/order3-read-source-second.log and tmp/order3-read-no-floats.log.
The initial contract failure is tmp/order3-read-before.log. The direct-literal
parsing defect is recorded in tmp/order3-read-source-trace.log.

The full five-configuration matrix, full isolated/workspace suites, detached
compiler tests, shipping commands, minimum Rust, cross-target builds, Miri and
release gates were not rerun. No Order 3 completion claim follows from this work.

## Next work

GetDataIndexed still raises an internal bounds fault and needs a total Word/Byte
contract. It currently carries base and length without an element kind. Its typed
fallback, especially for zero-length spans, needs an explicit kind rule before
implementation. An explicit kind operand remains a candidate. Keep the portable
diagnostic suffix and update both compilers, verifiers, resource/text consumers,
wire paths and documentation together.

Legacy integer Div and Mod still fault in hand-built bytecode. Their contract
resolution, matching Rust and Keleusma classifications, trap-free host policy,
resume and replacement preservation, actual proof-gated check elision and final
verification remain required. Non-first-pass GetIndex forms must be classified as
unsupported, never admitted as trap-free from a zero-Trap scan.

## Constraints and handoff

Rebuild development artifacts after these stack-contract changes. BYTECODE_VERSION
stays 2, FORMAT_FINGERPRINT stays 0x4327_63E1 during V0.2.3-WIP, public instruction
count stays 66, selected type capacities stay 640 and verifier depth stays 32.
Reference parser depth stays 24. V0.2.2 remains the last published release. No
publication is authorized. Appendix B was read before writes. The legacy HANDOFF.md
is invalid-and-stale. Backlog B42 retains compiler-managed ring buffer support.

Pilot task v023-read-28a2 records the first failed control and three completion
claims. All labels are non-independent self-review. The reviewed export is
 tmp/order3-read-pilot-checkpoint.json. The task outcome resolves this increment
only. External evaluation was not retried after the earlier automatic approval
rejection. This branch does not own the shared tracked pilot checkpoint.

The v0.2.3 inbox is empty. Its tracked checkpoint was exported and reviewed without
a diff. All verification processes are terminal.

Local .codex/ settings and nvim.log remain unrelated and untracked. No peer message,
merge or publication occurred. The concurrent V0.3.X worktree was not modified.
The operator's earlier feature-branch backup authorization remains in force.

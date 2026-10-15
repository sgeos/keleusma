# Reverse prompt

## Total bounds predicate on 2026-10-15

The active goal remains Complete Order 3, trap analysis. Work is on line v0.2.3
and branch feat/v02-order3-trap-analysis. This increment starts from 08ba12ef,
which was pushed and completed checked shifts and wide integer transport.

BoundsCheck is now a total predicate for a Word index. It preserves the index and
appends four Word byte limbs of the bound followed by flag zero for valid or one
for invalid. The exclusive bound remains a u16 operand. The limbs are little-endian
and sign-extended from eight bits, so they also work on the supported i8 runtime.
The instruction performs no indexed data access. Non-Word operands and an empty
stack remain contract errors. Stack growth is five, shrink zero and nominal cost
seven. The exact contract is docs/decisions/ORDER3_BOUNDS_CONTRACTS.md.

TrapKind IndexOutOfBounds uses code eight. It consumes the index and four Word byte
limbs and reconstructs an unsigned 32 bit bound. It returns the existing numeric
IndexOutOfBounds error. A host usize too small for the bound is rejected rather
than silently truncating. Payload types remain checked at runtime. The depth
requirement and shrink are five, growth zero and nominal cost six. Both depth
verifiers now check terminal operands. Both typed verifiers check terminal payload
availability and loop-frame floors. Their existing shape widening is unchanged and
does not establish exact Word types or justify runtime check elision.

The Rust compiler emits an explicit bounds Trap for per-dimension failure, then
drops the four limbs on the valid path while retaining the index for stride
arithmetic. Both the predicate and Trap have exact source records for the index
expression. No anonymous locals are introduced. The Keleusma typed stage uses a
dedicated five-Word transfer. Its host descriptor guard recognizes that path rather
than sending six generic outputs through the old four-output unroll.

This is the first bounds-contract component. GetIndex, GetDataIndexed and
SetDataIndexed still fault internally. Their total contracts and both compiler
lowerings remain required. No new indexed syntax or Keleusma code-generation path
is claimed by this increment. A zero-Trap scan is still not a trap-free verdict.

## Verification

The selected release self-host run passed all 1473 tests with zero skips. It includes
the root library tests, new predicate controls, the fault census, spec claims, narrow
runtime controls, verifier admission and resource tests, and the complete fourteen
stage artifact comparison. Resource admission for every stage also passed.

The twelve new controls cover valid and invalid boundaries, index preservation,
all diagnostic bytes on an actual i8 Word runtime, malformed operand kinds, missing
trap payloads, loop-frame ownership, multi-dimensional source failures and valid
accesses, exact fault spans, metadata effects and both wire implementations. The
fault census now has twenty cases and still finds implicit bounds faults in the
remaining indexed operations.

No-default library tests pass 175 cases. Fresh workspace Clippy with all targets
and self-host passes. The detached compiler builds. The instruction specification
and generated book chapter agree. A separate residency check initially failed its
old typed-verifier counts. Measured 1466 total records and 420 peak records are now
pinned and pass on rerun. The threefold fusion requirement is retained. The broad
run preceded that test-only pin update. All verification processes are terminal.

Receipts are tmp/order3-bounds-acceptance.log, tmp/order3-bounds-final-controls.log,
 tmp/order3-bounds-no-default.log, tmp/order3-bounds-clippy-final.log,
 tmp/order3-bounds-detached.log and tmp/order3-bounds-residency-final.log.
The pre-change domain failure is in tmp/order3-bounds-before-run.log. The first
compiler diagnostic is in tmp/order3-bounds-before.log. The old residency measurement
failure is in tmp/order3-bounds-residency.log.

The full five-configuration matrix, full isolated and workspace suites, detached
compiler tests, shipping commands, documentation tests, minimum Rust, cross-target
builds, Miri and release gates were not rerun for this increment. Do not infer full
Order 3 verification from the selected run.

## Next work

Continue docs/decisions/ORDER3_COMPLETION_PLAN.md with total indexed reads and writes.
Specify their exact result and diagnostic stacks before editing the implementation.
The portable suffix can be reused without hidden VM failure state. GetDataIndexed
has only base and length operands. An invalid read therefore needs an explicit kind
rule, particularly for zero-length spans. Shared slots have kind metadata and
private slots have initializer metadata. An explicit kind operand is a candidate,
not an implemented decision. Keep full-language forms outside the first-pass proof
explicitly unavailable rather than silently classifying them as trap-free.

Update the virtual machine, both compiler lowerings, Rust and Keleusma verifiers,
resource effects, wire paths and documentation together for each contract. Handled
index outcomes must omit unreachable fallback traps. Legacy integer Div and Mod
also retain implicit zero-divisor faults in hand-built bytecode and need a contract
disposition. Matching classifications, host loading policy, resume and replacement
preservation, actual proof-gated check elision and the full completion audit remain.

The temporary check_stage binary links an older runtime and is not valid current
verification after these instruction changes. Rebuild probes against current source.

## Compatibility and capacity

Rebuild development artifacts. BoundsCheck now has a different stack contract while
BYTECODE_VERSION remains 2 and FORMAT_FINGERPRINT remains 0x4327_63E1 under the
operator's ruling. The fingerprint changes only on publication. V0.2.2 remains the
last published release. Public instruction count is still 66. Trap kind eight is
an operand value of the existing Trap opcode, not a new opcode.

Selected type capacities remain 640 and verifier depth 32. Reference-parser depth
remains 24. The parser source and its exactly-full 1024-name input pool did not
change in this increment. Additional parser names still require capacity work.
The lexer source cap remains 524288 bytes. Appendix B was read before writes.
No merge or publication is authorized. The legacy HANDOFF.md is invalid-and-stale.
Backlog B42 retains the first-class compiler-managed ring buffer request.

## Pilot and handoff

Pilot task v023-bounds-08ba records the first compiler diagnostic and three claims.
All labels are non-independent self-review. The narrow-runtime claim was labelled
unsupported because the excerpt omitted the second test's runtime type, although
the test source establishes i8. The reviewed export is
 tmp/order3-bounds-pilot-checkpoint.json. The increment outcome does not complete
Order 3. External evaluation was not retried after the prior automatic approval
rejection. This branch does not own the shared tracked pilot checkpoint.

The v0.2.3 inbox is empty. Its tracked checkpoint was exported without a diff.
No peer message was sent. The concurrent V0.3.X worktree was not modified.
Local .codex/ settings and nvim.log remain unrelated and untracked. Feature-branch
backup remains authorized by the earlier operator request.

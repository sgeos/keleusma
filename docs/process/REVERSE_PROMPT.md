# Reverse prompt

## Total indexed writes on 2026-10-15

The active goal remains Complete Order 3, trap analysis. This increment starts from
19840d57 on line v0.2.3 and branch feat/v02-order3-trap-analysis. That preceding
bounds-predicate increment was pushed and verified against the remote head.

SetDataIndexed now consumes [value, index] and returns
[index, b0, b1, b2, b3, flag]. The portable byte limbs and flag follow
 docs/decisions/ORDER3_BOUNDS_CONTRACTS.md. A valid index writes one slot and returns
flag zero. A negative index, upper bound or empty span writes nothing and returns
flag one. The complete span is checked before access. Operand type checks and
existing storage checks remain. The depth effect is requirement two and net plus
four. Stack growth is four, shrink zero and nominal cost eight.

Both Rust and Keleusma bare lowerings compare flag with one and emit Trap kind eight
for failure. Successful writes discard the five remaining diagnostic Words. No
anonymous locals are introduced. The reference preserves exact source records for
the store and trap. Direct bytecode can handle a failed outcome and continue.
No new source-level handled-write syntax is claimed by this increment.

Both typed verifiers consume two operands and produce six Word-shaped entries.
Keleusma uses transfer kind sixteen. The text-flow and text-bound protocols also
needed new effect limits. Their former maximum delta of three and output count of
four refused the new operations. They now admit delta five and six outputs, covering
both BoundsCheck and SetDataIndexed. Dedicated controls retain text below diagnostic
outputs and check module flags for text returned after a store.

## Capacity repair

The new bare dispatch expanded reconstruction's step_assembly to 1706 opcodes,
exceeding the unchanged verifier limit of 1536. Its record-55 nested-field branch
is now a focused nested_eq_field helper, preserving statement order and state.
The largest reconstruction chunk is now step_record at 1412 opcodes. The stage has
43 functions. No buffer or verifier capacity was raised. The current-source probe
receipts are tmp/order3-store-op-sizes.log and tmp/order3-store-op-sizes-after.log.

## Verification

The final release self-host run passed all 1636 selected tests with zero skips.
It includes the root library tests, store and bounds controls, verifier admission,
the complete fourteen-stage artifact comparison, resource boundaries for every
stage, code-generation regressions and narrow runtime controls. The implementation
was held fixed during this run.

Eight indexed-store controls cover shared and private Word and Byte storage,
no mutation on failure, exact bounds, empty spans within declared storage,
continuation after a handled failure, malformed spans and operand counts, runtime
index-type guards, an i8 Word runtime, reference artifact equality, operand effects
and text-return flags. The two compiler paths agree on the source controls.

Fresh workspace Clippy with all targets and self-host passes. No-default library
tests pass 175 cases. Eighteen bounds and store controls also pass with floating
point disabled and compile and verify enabled. The detached compiler builds. Formatting and diff checks pass.
The first broad run had 1632 passes and two failures. One was the stale typed-stage
record count, now measured at 1485 total and 439 peak while preserving the threefold
fusion condition. The other exposed the text-analysis protocol refusal. Both were
repaired before the final broad run. All verification processes are terminal.

Receipts are tmp/order3-store-final-acceptance.log,
 tmp/order3-store-focused-final.log, tmp/order3-store-resources-after.log,
 tmp/order3-store-clippy-final.log, tmp/order3-store-no-default.log and
 tmp/order3-store-detached-final.log and tmp/order3-store-no-floats.log. The original primitive failure is in
 tmp/order3-store-before.log, and the first broad run is
 tmp/order3-store-acceptance.log.

The full five-configuration matrix, complete workspace and isolated suites,
detached compiler tests, shipping commands, documentation tests, minimum Rust,
cross-target builds, Miri and release gates were not rerun. These receipts support
this increment and do not establish completion of Order 3.

## Next work

Implement total indexed reads under docs/decisions/ORDER3_COMPLETION_PLAN.md.
GetIndex and GetDataIndexed still raise internal bounds faults. GetIndex already
carries its element descriptor. GetDataIndexed carries only base and length, so
its deterministic Word or Byte fallback needs a kind rule, including zero length.
An explicit kind operand remains a candidate, not an implemented decision.
Specify the exact output stack before changing each contract, and update both
compilers, both verifiers, text and resource consumers, wire paths and documentation.
Reuse the portable diagnostic suffix rather than hidden last-failure state.

Handled index outcomes must omit unreachable fallback traps. Legacy integer Div
and Mod still fault in hand-built bytecode. Matching classifications, host loading
policy, resume and replacement preservation, actual proof-gated check elision and
the final audit remain required. No trap-free or no-check mode exists yet.

## Compatibility and handoff

Rebuild development artifacts because the indexed-store stack contract changed.
BYTECODE_VERSION remains 2 and FORMAT_FINGERPRINT remains 0x4327_63E1 during
V0.2.3-WIP. Publication is the fingerprint change boundary. V0.2.2 remains the last
published release. Public instruction count stays 66, selected type capacities 640
and verifier depth 32. The reference parser depth stays 24. The parser's 1024-name
input pool remains full and unchanged. Appendix B was read before tracked writes.
The legacy HANDOFF.md remains invalid-and-stale. Backlog B42 retains the requested
first-class compiler-managed ring buffer support.

Pilot task v023-store-1984 records one diagnostic and three completion claims.
Labels are non-independent self-review. The store-test claim was labelled
unsupported because its excerpt omitted the binary identity, which is present in
the full log. The reviewed export is tmp/order3-store-pilot-checkpoint.json.
External evaluation was not retried after the earlier automatic approval rejection.
This branch does not own the shared tracked pilot checkpoint. The task outcome
resolves this increment only, not Order 3.

The v0.2.3 inbox is empty. Its tracked checkpoint was exported and reviewed without
a diff. All verification processes are terminal.

Local .codex/ settings and nvim.log remain unrelated and untracked. No peer message,
merge or publication occurred. The concurrent V0.3.X worktree was not modified.
Feature-branch backup remains authorized by the operator's earlier request.

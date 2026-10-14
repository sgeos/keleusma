# Reverse prompt

## Active goal

Complete Order 3, trap analysis, on line v0.2.3 and branch
feat/v02-order3-trap-analysis. The starting commit is 1cfb6dd9, which completes
Order 2. The full requirement register is docs/decisions/ORDER3_COMPLETION_PLAN.md.
Order 3 remains active and incomplete.

The legacy handoff is invalid-and-stale, 56 commits behind the starting head.
Its ancestry anchor passes, and the format fingerprint remains 0x4327_63E1.
Appendix B was read before tracked writes. The V0.3.X worktree and unrelated
.codex/ and nvim.log are untouched. No peer message was sent.

## Operator decision recorded

On 2026-10-13 the operator declined advancing BYTECODE_VERSION. It remains 2.
FORMAT_FINGERPRINT remains 0x4327_63E1 during V0.2.3-WIP and changes only at
publication of a new version. V0.2.2 is the last published release and its tagged
source uses bytecode version 1. The reserved auxiliary-header word already carries
the fingerprint. No version authorization remains pending. The earlier version 3
proposal is withdrawn. Continue Order 3 under the fixed version, fingerprint and
66-opcode limits. No publication is authorized.

## Implemented arithmetic increment

Bare Word and Byte division and modulo now emit CheckedDiv or CheckedMod followed
by a flag-three conditional ZeroDivisor Trap. Both compilers agree on focused
artifacts. Internal transport tags 64 and 65 refer to existing public instructions.
They do not increase the public instruction count.

The Rust compiler omits the zero-divisor fallback for nondivision arithmetic and
unguarded variable or wildcard zero_divisor arms. Guarded and literal arms retain
it. CheckedDiv now uses the declared word width when running on a wider runtime.
The regression witness previously returned 32768 for a 16-bit MIN divided by -1.
It now wraps to -32768. Operator metadata covers both arithmetic and the explicit
faulting Trap. The instruction specification and generated book chapter are updated.

The census now has seventeen cases and finds only bounds faults without Trap.
Legacy Div and Mod still fault on integer zero in hand-built bytecode. Direct
instruction fixtures test both legacy faults and checked outcomes, including flag
three. No trap-free classification is claimed for arbitrary bytecode.

## Completed increment verification

The final affected self-host regression set passes all 384 tests, with no skips,
in tmp/order3-arithmetic-acceptance.log. It includes complete fourteen-stage
artifact identity, complete stage resource admission, the 152-test compiler corpus,
wire handling, instruction contracts and the seven focused arithmetic controls.
The run used the ci profile with retries disabled and the self-host cache disabled.

The initial complete self-host run ended with 3033 passes, four failures and two
skips. It exposed two stale measurements and lost Float division/remainder and
Fixed remainder acceptance. Those are repaired. The parser carries the left scalar
kind in a high lane of the binary-node argument to preserve the noninteger paths.
The final run preserves all seven existing typed-operator boundary controls.

The second affected run ended with 381 passes and three related failures. A new
parser helper used an inline conditional as the right operand of addition. The
parser orders that addition before the conditional. The helper now binds the
conditional result to a local before adding it. The final artifact, raw parser and
assembled serialization checks all pass. The broader syntax defect remains
recorded in the completion plan. Isolated probes are in
 tmp/order3-minimal-diagnosis.log and tmp/order3-parse-diagnosis.log. The latter's
unfused parse attempts exceeded that helper's token cap and do not characterize
the shipping fused path. Its type-stage comparisons use the fused path.

Additional checks on the latest implementation are as follows.

- All 329 no-default tests pass in tmp/order3-no-default-acceptance.log.
- Fresh workspace Clippy passes in tmp/order3-clippy-acceptance.log.
- The detached compiler builds in tmp/order3-detached-acceptance.log.
- The measured wire-capacity pin passes in tmp/order3-wire-cap-final.log. Parse has
  985 names and 28367 blob bytes. No capacity changed.
- Formatting and git diff whitespace checks pass.

Both debugger controls passed before the parser repair in
 tmp/order3-debug-operator.log and tmp/order3-debug-fault.log. Their implementation
has not changed since those checks. No verification process remains running.

The full isolated self-host suite was not rerun after repairs. The affected set
above supplies the final repair evidence. The other three feature configurations,
detached compiler tests, doctests and full release gates were not rerun for this
increment. No merge or publication is proposed. This handoff and TASKLOG.md belong
to the scoped arithmetic increment.

## Next implementation

Continue with checked
arithmetic outcome syntax and lowering to the Keleusma pipeline, which still refuses
those arms. Specify and implement total bounds contracts without moving the version
or fingerprint. BoundsCheck, GetIndex, GetDataIndexed and SetDataIndexed still fault
internally, and the checked-index fallback reissues GetIndex. TrapKind still lacks
an indexed-bounds kind. Update all instruction consumers together.

The remaining goal also requires matching Rust and Keleusma classifications, a
host trap-free load policy, and actual proof-gated check elision. The Order 2 typed
widening profile can admit unknown shapes. Trap absence does not justify removing
type, memory, stack or host-contract guards. Establish a specific proof for every
removed check and preserve rejection controls. No nominal no-check mode or unsafe
host promise satisfies the goal.

## Pilot and mailbox

Pilot task v023-order3-lowering-1cfb has one diagnostic and three claims. All four
have non-independent self-review labels. The arithmetic, debugger and stage claims
refer to executed checks. The increment outcome is recorded as resolved, while the complete Order 3 goal
remains active. External evaluation
remains unavailable under the earlier automatic approval rejection. No provider
accuracy or saved-reasoning conclusion is supported. Local packets and reviewed
excerpts use tmp/order3-* paths. The shared export is retained at tmp/order3-pilot-checkpoint-final.json. This
branch does not own the shared tracked pilot checkpoint. No peer message was sent.

The inbox was empty and the exported interline checkpoint had no diff. Check again
before final handoff. Selected type capacities remain 640 and depth limit remains 32.

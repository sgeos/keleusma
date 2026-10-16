# Reverse prompt

## Total legacy arithmetic on 2026-10-15

The active goal remains Complete Order 3, trap analysis. Work is on line v0.2.3
and branch feat/v02-order3-trap-analysis. This increment starts from pushed commit
fa7a94f94d2dca5a35280903c581dea17ec99bf0. The previous goal turn made progress
by repairing and verifying the indexed data-read contracts.

Div and Mod now consume two numeric operands and return [low, high, flag]. Div
shares CheckedDiv with fraction count zero. Mod shares CheckedMod for Word, Byte
and Fixed and also supports Float remainder with floating-point classification.
Integer zero divisors return the numerator, typed zero and Word flag three.
Word overflow classification follows the module width. Wrong operand kinds remain
contract errors. CheckedMod still rejects Float, preserving its source boundary.

Both compiler lowerings discard high and flag for bare Float operations. Fixed
remainder uses a conditional explicit zero-divisor trap. This also repairs an
existing defect where the compiler emitted Mod for Fixed while the runtime rejected
Fixed operands. Multiword division and remainder formerly used integer Div only
to provoke an error on an all-zero divisor. They now test the combined divisor
limbs and emit Trap kind five explicitly, with a source location on the divisor.

The instruction depth requirement is two and net growth one. Shrink is zero and
nominal cost five, an unmeasured estimate. Both verifier and resource paths consume
these effects. Instruction encodings and the public opcode inventory are unchanged.
The specification and generated book are updated. Exact contracts and the remaining
implicit-fault inventory are in docs/decisions/ORDER3_ARITHMETIC_CONTRACTS.md.

## Verification

The broad release self-host run executed 1765 tests. It passed 1764 and failed one
older operand-stack fixture that still expected one scalar GetIndex output. The
implemented bounds contract produces seven. The fixture now records that effect
and the new Div, Mod, indexed data and predicate effects.

After that test-only repair, the final focused run passes all 141 tests with no
skips. Production code did not change after the broad run. Complete current-stage
artifact identity and resource admission passed in that broad run. Its failure
remains in the receipt and is not reported as a full green run. The broad suite
was not replayed after the fixture repair.

Eight new tests cover direct integer, Byte, Fixed and Float outcomes, narrow module
and runtime widths, malformed operand kinds, Fixed source execution in both
compilers, wire round trips, verifier admission and explicit multiword traps.
The multiword controls assert DivisionByZero specifically. The older helper only
asserted that some error occurred and was insufficient evidence for that claim.

Clippy passes with workspace, all targets and self-host. The detached compiler
build passes. Sixteen focused arithmetic tests pass without floating-point support.
No-default library tests pass 175 cases. The current-stage input-capacity check
passes with maxima of 1023 names and 30199 blob bytes. Formatting and diff checks
pass. The full five-configuration matrix, full workspace and isolated suites,
detached compiler tests, shipping commands, minimum Rust, cross-target builds,
Miri and release gates remain outstanding for Order 3 completion.

Receipts are tmp/order3-legacy-acceptance.log,
 tmp/order3-legacy-final-focused.log, tmp/order3-legacy-final-clippy.log,
 tmp/order3-legacy-detached.log, tmp/order3-legacy-no-floats.log,
 tmp/order3-legacy-no-default.log and tmp/order3-legacy-capacity.log.
The initial executed contract failure is tmp/order3-legacy-before.log. The first
focused build failed on a test-only narrow-runtime state type mismatch, corrected
before the 134-case focused run in tmp/order3-legacy-focused2.log. The additional
exact multiword control and stack fixture are included in the final 141 cases.

The final focused command is reproducible as follows.

```sh
CARGO_HUSKY_DONT_INSTALL_HOOKS=true RUSTC_WRAPPER= KEL_SELFHOST_CACHE=0 \
cargo nextest run --release --features self-host --no-fail-fast -j 4 \
  --test legacy_division_outcomes --test trap_lowering --test spec_trap_claims \
  --test float_arith_width --test multiword --test operand_stack_model
```

## Next work

Implement matching reference and Keleusma trap classifications. A completed
trap-free or possibly-trapping verdict must remain distinct from unsupported or
incomplete analysis. The first-pass profile must account for raw FixedDiv,
non-scalar or boxed GetIndex, and boxed GetField, GetTupleField and GetEnumField,
which still have internal language-fault paths. Do not report them trap-free from
a zero-Trap scan. The roadmap defers native-error and newtype-refinement widening
to Workstream F. Native and host-return shape obligations remain explicit.

Keleusma must determine its verdict from transported instruction information.
Include the stage-decision intervention control, capacity boundaries, malformed
inputs and comparison with the reference. Then enforce host trap-free policy
before subject execution and preserve it across resume and replacement.

Actual proof-gated check elision remains required. Static stack-depth and validated
instruction-cache bounds are candidate proofs for specific checks. Type, shape,
lifetime and host-contract checks cannot be removed from Order 2 Top widening.
The mode must remove actual supported checks and have negative controls for every
unproved removal. Complete the full verification matrix and explicit audit.
ORDER3_COMPLETION_PLAN.md remains authoritative. No blacklist-only or no-op
substitute completes the goal.

A newly recorded safety obligation is essential for unchecked stack execution.
Vm::run currently preserves frames and operands after an error. An operation may
have consumed only part of its inputs before a retained type check fails. The
module-level depth proof does not describe that partial frame. Invalidate or
re-establish the execution-state proof before error reentry. Exercise initial
calls, yields, resets, breakpoint suspension, normal returns, runtime faults and
successful or failed replacement separately. This audit finding is not yet an
implemented check-elision policy.

## Constraints and handoff

BYTECODE_VERSION remains 2 and FORMAT_FINGERPRINT remains 0x4327_63E1 during
V0.2.3-WIP. The public instruction count remains 66. Selected type capacities
remain 640, verifier opcode capacity 2048, verifier depth 32 and reference parser
depth 24. V0.2.2 remains the last published release. Development artifacts need
rebuilding after these stack-contract changes. No merge or publication occurred.
Appendix B was read before tracked writes. B42 retains first-class compiler-managed
ring buffers pending design and scheduling.

The legacy HANDOFF.md remains invalid-and-stale, measured at 68 commits behind
this increment's starting head. Its historical audit, full workspace and detached
RTOS checks were not rerun. Ancestry alone is not freshness evidence.

Pilot task v023-legacy-fa7a records the first diagnostic and three completion
claims. Labels are non-independent self-review. Its outcome resolves this arithmetic
increment only. External evaluation was not retried after the earlier automatic
approval rejection. The reviewed local export is
 tmp/order3-legacy-pilot-checkpoint.json. This branch does not own the shared
tracked pilot checkpoint.

The v0.2.3 inbox is empty. Its tracked checkpoint is exported and reviewed before
commit. All verification processes are terminal. Local .codex/ settings and nvim.log
remain unrelated and untracked. No peer messages were sent. The concurrent V0.3.X
worktree was not modified. Feature-branch push authorization persists, using the
hook's documented work-in-progress bypass where appropriate.

# Reverse prompt

## Matching trap classifications on 2026-10-15

The active goal remains Complete Order 3, trap analysis. Work is on line v0.2.3
and branch feat/v02-order3-trap-analysis. This increment starts from pushed commit
2bdbc08ea913c7905e46fddc36b3c8a9e5aace93. The previous goal turn made progress
by implementing total legacy arithmetic outcomes and explicit multiword traps.

The new reference API is trap_analysis::classify_traps. The Keleusma API is
selfhost::classify_traps_via_kel. Both validate a module before scanning all chunks,
including unreachable instructions. A completed result is TrapFree or MayTrap
with the first TrapLocation. Unsupported contracts, invalid modules, per-chunk
opcode capacity failures and unavailable analysis are distinct errors.

The first-pass profile excludes raw FixedDiv, GetIndex outside flat Word and Byte,
boxed struct, tuple and enum field access, and native calls. Unsupported forms take
precedence over MayTrap. Known invalid Trap and PushImmediate values cannot become
completed verdicts. Width and entry metadata are checked before invoking the
reference verifier on public input. Runtime type, shape, storage, lifetime,
resource and host-contract guards remain required. This verdict is informational
and does not create an unchecked-execution certificate.

Mode two of verify_structural.kel scans raw wire tags and operand forms in existing
shared arrays. It owns both profile admission and Trap detection. The host supplies
no safety classification and does not recompute the answer. It validates yielded
and stored verdict agreement, response sentinels and location bounds. A report of
an invalid subject without a valid instruction location is unavailable analysis.
The stage resets its outputs on every request, including after malformed input.
Modes zero and one retain the structural verifier protocols.

Implementation files are src/trap_analysis.rs, src/selfhost/traps.rs and
src/selfhost/kel/verify_structural.kel. The contract and transport are documented
in docs/decisions/ORDER3_TRAP_CLASSIFICATION.md. Stage count and the public opcode
inventory are unchanged. Version and fingerprint remain fixed.

## Verification

The broad release self-host run passed all 1606 tests with no skips. This includes
complete stage artifact identity, resource admission and matching completed trap
classifications for all fourteen actual stage sources. Two stage interventions
prove ownership of Trap detection and unsupported-form admission independently.

After that run, the reference scalar-kind match was rewritten to satisfy Clippy,
a whole-wire-opcode profile control was added, and invalid-subject response
locations were hardened. The final focused run passes all twelve tests, including
both interventions, all fourteen stage classifications, every current opcode,
malformed response and request recovery, source outcomes, unsupported forms,
unreachable code, hand-built total arithmetic and exact capacity boundaries.
The broad run was not replayed after those final changes.

Clippy passes with workspace, all targets and self-host. The detached compiler
build passes. Six reference-only classifier tests pass without floating-point
support. The library builds with verification enabled and compiler and self-hosting
features disabled. Current stage input maxima remain 1023 names and 30199 blob
bytes under unchanged caps. Formatting and diff checks pass.

Receipts are tmp/order3-traps-acceptance.log,
 tmp/order3-traps-final2-focused.log, tmp/order3-traps-final2-clippy.log,
 tmp/order3-traps-final2-detached.log, tmp/order3-traps-final-no-floats.log,
 tmp/order3-traps-final-verify-only.log and tmp/order3-traps-capacity.log.
The initial compile errors are retained in tmp/order3-traps-first.log. The first
Clippy failure is tmp/order3-traps-clippy.log. Earlier focused receipts are retained
but are superseded by the final2 focused run for the last response guard.

The final focused command is reproducible as follows.

```sh
CARGO_HUSKY_DONT_INSTALL_HOOKS=true RUSTC_WRAPPER= KEL_SELFHOST_CACHE=0 \
cargo nextest run --release --features self-host --no-fail-fast -j 4 \
  --lib --test trap_analysis \
  -E 'test(selfhost::traps) | binary(trap_analysis)'
```

The full five-configuration matrix, full workspace and isolated suites, detached
compiler tests, shipping commands, minimum Rust, cross-target builds, Miri and
release gates remain outstanding for Order 3 completion.

## Next work

Implement host trap-free policy before subject execution. Default trap-permitting
behavior must remain an explicit policy. Reject both MayTrap and unavailable proof
when the host requires TrapFree. A caller-provided public verdict cannot grant an
execution capability. Inspect every constructor and replacement path, including
serialized, borrowed, signed, encrypted and trust-skip routes, before wiring policy.
Preserve policy across normal resume, reset and successful or failed replacement.
Do not silently fall back to permissive execution when analysis is unavailable.

Then implement actual proof-gated check elision. Static stack-depth and validated
instruction-cache bounds are candidate proofs for specific checks. The mode must
remove actual supported checks, with a proof and negative controls for each.
Order 2 Top widening does not justify dropping type, shape, lifetime or host-result
guards. Complete the full feature, shipping, compatibility and memory-safety
verification and an explicit requirement-by-requirement audit.
ORDER3_COMPLETION_PLAN.md remains authoritative. No blacklist-only or no-op
substitute completes the objective.

A critical execution-state obligation remains. Vm::run preserves frames and
operands after an error. An instruction can consume only part of its operands
before a retained guard fails. A static module-depth proof no longer describes
that partial frame. Invalidate or re-establish the execution-state proof before
error reentry. Invalidation must also survive a panic caught by the embedding
host. Exercise calls, yields, resets, breakpoints, normal returns, returned errors,
unwinding and replacement separately. No check-elision policy exists yet.

## Constraints and handoff

BYTECODE_VERSION remains 2 and FORMAT_FINGERPRINT remains 0x4327_63E1 during
V0.2.3-WIP. The public instruction count remains 66. Selected type capacities
remain 640, verifier and trap opcode capacities 2048, verifier depth 32 and reference
parser depth 24. V0.2.2 remains the last published release. Existing development
artifacts need rebuilding after the preceding stack-contract changes. No merge or
publication occurred. Appendix B was read before tracked writes. B42 retains
first-class compiler-managed ring buffers pending design and scheduling.

The legacy HANDOFF.md remains invalid-and-stale, measured at 69 commits behind
this increment's starting head. Its historical audit, full workspace and detached
RTOS checks were not rerun. Ancestry alone is not freshness evidence.

Pilot task v023-traps-2bdb records the first diagnostic and three completion claims.
Labels are non-independent self-review. One final-run claim is labelled unsupported
because the bare summary did not establish finality and a later response-hardening
run superseded it. The current receipt is final2-focused. The task outcome resolves
only this classifier increment. External evaluation was not retried after the
earlier automatic approval rejection. The reviewed export is
 tmp/order3-traps-pilot-checkpoint.json. This branch does not own the shared tracked
pilot checkpoint.

The v0.2.3 inbox is empty. Its tracked checkpoint is exported and reviewed before
commit. All verification processes are terminal. Local .codex/ settings and nvim.log
remain unrelated and untracked. No peer messages were sent. The concurrent V0.3.X
worktree was not modified. Feature-branch push authorization persists, using the
hook's documented work-in-progress bypass where appropriate.

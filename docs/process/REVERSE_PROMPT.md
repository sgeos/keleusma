# Reverse prompt

## Host trap policy on 2026-10-15

The active goal remains Complete Order 3, trap analysis. Work is on line v0.2.3
and branch feat/v02-order3-trap-analysis. This increment starts from pushed commit
7316dd95d1834bd6a64a8e04acb43c4451e3a9ba. The previous goal turn made progress
by implementing matching reference and Keleusma trap classifications.

TrapPolicy is now persistent private state on GenericVm. Allow is the default.
RequireTrapFree requires a completed reference TrapFree classification before
execution. MayTrap, unsupported contracts, malformed modules, capacity failures
and unavailable verification are rejected. The host cannot supply a verdict as
permission. Runtime contract and resource checks remain active.

VmOptions has a trap_policy field. Existing option literals were updated to retain
their default permissive policy. new_with_options enforces strict policy before
construction. New load_bytes_with_options, view_bytes_with_options,
load_signed_bytes_with_options and load_encrypted_signed_bytes_with_options expose
the same options. Existing entrypoints delegate with defaults. Signature
verification and decryption precede policy admission on authenticated routes.
Existing framing checks still apply before options can act on a decoded module.

trap_policy reports the selection. set_trap_policy verifies the current image before
assigning RequireTrapFree and preserves old policy and state on failure.
with_trap_policy configures a freshly loaded instance before its first call,
including borrowed and trust-skip loaders. Strict activation without the verify
feature is refused, including through authenticated unsafe loaders. Host attestation
cannot replace this proof. Explicitly selecting Allow remains supported.

replace_module_inner enforces the stored policy before mutation. Both ordinary and
schema-bypassing replacement reach that gate. Policy survives successful replacement,
rejected replacement, yields, resumes, resets, breakpoints and runtime type errors.
No execution check is removed by this increment.

## Signed replacement repair

A positive control exposed an existing failure in replace_module_from_bytes.
After authentication it decoded a signed module, then tried to re-encode the
signature-required flag without the signature extension. The load failed before a
valid replacement could succeed. It now clears that internal flag after signature
verification, matching initial signed loading. The host key matrix stays installed.
Tests require valid signed replacement to succeed and wrong-key, unsigned and
trapping replacements to fail under their corresponding host policies. Clearing
verifying keys does not weaken trap policy.

A separate initial fixture failed because its breakpoint remained installed after
replacement. The test now clears that breakpoint before expecting the opted-out
division fault. Runtime breakpoint persistence was not changed.

## Verification

All 1521 selected release tests pass with self-host, signatures and encryption
features enabled, with zero skips. This includes eleven policy tests, complete
stage artifact identity, complete resource admission, matching stage classifications,
hostile module mutation, narrow runtimes and persistent storage. The implementation
was fixed throughout this run. No production or test edits followed it.

Nine policy tests pass without floating-point support. The verification-disabled
control passes both without cryptography and with signed and encrypted loaders
enabled. These controls prove that strict policy does not accept a host attestation
when the classifier is unavailable. Clippy passes for the workspace and all targets
with self-host, signatures and encryption. The detached compiler builds. Formatting
and diff checks pass.

Receipts are tmp/order3-policy-acceptance.log,
 tmp/order3-policy-final-focused.log, tmp/order3-policy-final-clippy.log,
 tmp/order3-policy-detached.log, tmp/order3-policy-no-floats.log,
 tmp/order3-policy-final-no-verify.log and tmp/order3-policy-no-verify-crypto.log.
The initial test compilation error is tmp/order3-policy-focused.log. The signed
replacement and retained-breakpoint failures are in tmp/order3-policy-focused2.log.
The ten-case repaired run is tmp/order3-policy-focused3.log, followed by the final
eleven-case policy coverage in the broad run.

The broad command is reproducible as follows.

```sh
CARGO_HUSKY_DONT_INSTALL_HOOKS=true RUSTC_WRAPPER= KEL_SELFHOST_CACHE=0 \
cargo nextest run --release --features self-host,signatures,encryption \
  --no-fail-fast -j 4 --lib --test trap_policy --test trap_analysis \
  --test selfhost_admission --test selfhost_resource_admission \
  --test selfhost_artifact_identity --test hostile_module_mutation \
  --test narrow_vm --test persistent_data --test flat_text_persistent
```

The full five-configuration matrix, full workspace and isolated suites, detached
compiler tests, shipping commands, minimum Rust, cross-target builds, Miri and
release gates remain outstanding for Order 3 completion. This selected combination
is not a substitute for all five feature configurations.

## Next work

Implement actual proof-gated check elision. Host trap policy and classification are
now implemented, but neither creates an execution certificate. Specify the exact
checks to remove and the proof for each. Static operand-stack depth is a candidate
for removing redundant pop-underflow checks. Validated dispatch bounds are another
candidate. Retain unknown type, shape, lifetime, storage, host-result and allocation
guards. Order 2 Top widening cannot justify removing them.

Audit the actual instruction stack effects and every state transition before using
unchecked memory access. Bind any certificate to the immutable loaded image and
re-establish it on successful replacement. Enabling the mode on a trust-skip or
borrowed instance must still obtain the required proofs. Disabling verification
cannot become an alternate proof source. A later policy change to Allow must not
leave unchecked execution relying on a discarded trap-free premise.

The execution-state obligation is critical. A returned error or caught panic may
leave an instruction with only part of its operands consumed. Invalidate the live
proof before error or unwind reentry. Audit argument preparation in call and resume,
not just the run wrapper, and the replacement commit boundary. Distinguish a fresh
call or reset from resuming a partial frame. Exercise yields, normal returns,
breakpoints, resets, failed and successful replacement and host-contract errors.
A mode that removes no checks or merely trusts an unsafe host promise does not
satisfy the goal.

The new policy test strict_policy_retains_runtime_contract_guards_and_survives_error_reset
is a useful boundary control. A hand-built Bool-plus-Word operation passes the
current generic numeric transfer and trap profile, then reaches a retained TypeError.
A trap-free language verdict does not establish concrete operand kinds. Any future
check elision must preserve this refusal and safe error handling.

Then run the complete feature, shipping, compatibility and relevant memory-safety
verification and perform the explicit requirement-by-requirement completion audit.
ORDER3_COMPLETION_PLAN.md remains authoritative. The policy contract is in
ORDER3_HOST_POLICY.md and the classifier contract is in ORDER3_TRAP_CLASSIFICATION.md,
both under docs/decisions. Order 3 remains active.

## Constraints and handoff

BYTECODE_VERSION remains 2 and FORMAT_FINGERPRINT remains 0x4327_63E1 during
V0.2.3-WIP. Public instruction count remains 66. Selected type capacities remain
640, verifier and trap opcode capacities 2048, verifier depth 32 and reference parser
depth 24. V0.2.2 remains the last published release. Development artifacts require
rebuilding after the preceding stack-contract changes. No merge or publication
occurred. Appendix B was read before tracked writes. B42 retains first-class
compiler-managed ring buffers pending design and scheduling.

Legacy HANDOFF.md remains invalid-and-stale, measured at 70 commits behind this
increment's starting head. Its historical audit, full workspace and detached RTOS
checks were not rerun. Ancestry alone is not freshness evidence.

Pilot task v023-policy-7316 records its first diagnostic and three completion claims.
All labels are non-independent self-review. The task outcome resolves this policy
increment only. External evaluation was not retried after the earlier automatic
approval rejection. The reviewed local export is
 tmp/order3-policy-pilot-checkpoint.json. This branch does not own the shared tracked
pilot checkpoint.

The v0.2.3 inbox is empty. Its tracked checkpoint is exported and reviewed before
commit. All verification processes are terminal. Local .codex/ settings and nvim.log
remain unrelated and untracked. No peer messages were sent. The concurrent V0.3.X
worktree was not modified. Feature-branch push authorization persists, using the
hook's documented work-in-progress bypass where appropriate.

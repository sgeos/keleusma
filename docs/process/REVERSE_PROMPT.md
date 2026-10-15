# Reverse prompt

## Work-in-progress backup on 2026-10-15

Line v0.2.3, branch feat/v02-order3-trap-analysis. This increment starts from
1d3429715a0c1ffb49a93a6fcbb9749a8435a2e3. The operator requested recording
first-class compiler-managed ring buffer support and committing and pushing all
current work. B42 already records the request and is linked from the V0.2.X
roadmap. The request was reaffirmed. Design and scheduling remain open.

This is a feature-branch backup of unfinished Order 3 work. It is not ready for
integration. No merge or publication is authorized by this backup.

## Current implementation

GetDataIndexed consumes [index, fallback] and returns
[value, index, b0, b1, b2, b3, flag]. Word, Byte and Unit fallbacks are accepted.
An invalid index returns the fallback without reading data. The complete span is
validated before access. Empty spans can operate without allocated data storage.
Both compilers supply typed zero for Word and Byte, and Unit for other bare reads.
Bare failures reach Trap kind eight. Checked scalar data reads use seven temporary
slots and omit fallback traps under complete invalid_index coverage.

Both verifier transfers, stack and cost metadata, instruction documentation and
stage protocols were updated. The private parser record 13 now carries a scalar
kind above its two 24-bit address lanes. The verifier opcode capacity increased
from 1536 to 2048 because measured stage maxima are 1965 parser instructions and
1568 codegen instructions. This is distinct from selected type capacities of 640.

## Verification and known failures

The existing release self-host nextest run has terminated with exit status 100.
It executed 1740 tests with 1729 passes, 11 failures and two skips. Receipt is
 tmp/order3-data-read-acceptance.log. Current-stage production artifact identity
and complete stage resource admission passed. Nine data-read controls passed.

The failures are as follows.

- The scalar_and_indexed_operands_preserve_offsets_and_counts relocation test
  accepts an old width of 16777216 that collides with the new high kind lane.
- codegen_owns_its_constant_pool_and_matches_reference and
  self_compile_codegen_atomic_functions use a test-only Rust syntax-tree adapter
  that does not transport the new fallback kind. It produces Unit instead of Word.
- the_fused_compile_holds_one_group_not_the_program pins 1513 typed-verifier
  records. The run measured 1532 total and 486 peak.
- Seven selfhost_parse expectations still use old record 13 payloads. They are
  a_data_read_may_index_another, a_for_limit_body_with_data_access,
  a_realistic_stage_like_body_parses, a_stage_scan_loop_function_parses,
  a_stage_shaped_dispatch_function_parses, a_whole_mini_stage_program_parses and
  an_indexed_data_read_parses.

Earlier Clippy and detached compiler builds passed. Receipts are
 tmp/order3-data-read-clippy.log and tmp/order3-data-read-detached.log.
Focused and stage-capacity receipts are tmp/order3-data-read-focused.log,
 tmp/order3-data-read-profile.log and tmp/order3-data-read-op-sizes.log.
No additional tests were run for the backup request. Full feature, shipping,
compatibility, memory-safety and release verification remain outstanding.

## Resume work

Repair private relocation first. Indexed lengths must fit the 24-bit public wire
range. For read record 13, allow high kind only zero, two or three and preserve it.
For store record 14, reject oversized full lengths rather than masking them into
valid smaller lengths. Keep the last scalar address valid. Update the old boundary
control and add malformed high-lane controls when verification is authorized.

Derive fallback kinds from source data declarations in the test-only compiler
adapter. Pass the metadata through build_body and build_multihead. Update parser
record expectations according to each declared element type. Update residency
pins only after confirming the current measurements and retained reduction bound.
Rerun the relevant broad checks on a fixed tree before treating this increment
as complete. Do not change implementation while its verification is live.

The full Order 3 completion plan remains authoritative. Legacy integer Div and Mod
contracts, matching Rust and Keleusma trap classifications, trap-free host policy,
resume and replacement preservation, actual proof-gated check elision and final
audit remain required. Non-first-pass GetIndex forms remain outside the proof.

## Constraints and handoff

BYTECODE_VERSION remains 2. FORMAT_FINGERPRINT remains 0x4327_63E1 during
V0.2.3-WIP. Public instruction count remains 66. Selected type capacities remain
640, verifier depth 32 and reference parser depth 24. V0.2.2 remains the last
published release. Development artifacts require rebuilding after contract changes.
Appendix B was read before tracked writes.

Legacy HANDOFF.md is invalid-and-stale at 66 commits behind the pre-backup head.
Its ancestry check passes, but all fourteen current stage sources differ from its
content anchor. Its audit, full workspace, shell and detached RTOS test checks were
not rerun for this backup. Ancestry does not establish freshness.

Pilot task v023-data-read-1d34 has an initial diagnostic recorded under
v023-data-read-triage-1d34. Its development outcome remains unresolved. Completion
claim sampling was deferred because this is an unfinished backup. External
evaluation was not retried after the earlier automatic approval rejection.

The v0.2.3 inbox is empty. Its checkpoint is exported and reviewed before the commit.
All known verification processes have terminated. Local .codex/ settings and the
empty nvim.log remain unrelated and untracked. No peer message was sent. The
concurrent V0.3.X worktree was not modified. The documented work-in-progress
pre-push hook bypass applies to this feature-branch backup.

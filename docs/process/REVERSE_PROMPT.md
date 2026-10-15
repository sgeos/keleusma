# Reverse prompt

## Indexed data-read outcomes and repairs on 2026-10-15

The active goal remains Complete Order 3, trap analysis. Line v0.2.3 is on branch
feat/v02-order3-trap-analysis. This repair increment starts from pushed backup
7a058917bf649a6fc43fa98bdc14428f19333007. The preceding scalar-read commit is
1d3429715a0c1ffb49a93a6fcbb9749a8435a2e3.

GetDataIndexed consumes [index, fallback] and returns
[value, index, b0, b1, b2, b3, flag]. Fallback is Word, Byte or Unit. An invalid
index returns it without reading storage. The full span is checked first. Empty
spans work without declared storage. Both compiler lowerings supply typed zero
for Word and Byte and dispatch bare failures through Trap kind eight. Checked
scalar data reads evaluate the index once, retain seven temporaries and omit
fallback traps under complete invalid_index coverage. Other bare element kinds
use Unit. Runtime type, representation and storage guards remain.

The depth effect is two inputs and net growth five. Nominal cost is eight. Both
verifier transfers produce an unknown value shape followed by six Words. The
instruction encoding retains base and length as its two 24-bit operands.

Private parser record 13 carries fallback kind above the address lanes. Relocation
preserves kinds zero, two and three and rejects reserved values. Store record 14
has no kind lane and checks its entire length rather than masking it. Indexed
lengths must be below 16777216, while scalar address 16777215 remains valid.
Test adapters now derive kinds from source declarations. Independent expectations
cover Word, Byte and Boolean fields.

The verifier opcode profile is 2048, increased from 1536 in the backup because
current stage maxima are 1965 parser instructions and 1568 codegen instructions.
This is distinct from the operator-selected type capacities of 640. All current
stages fit. Parser input uses 1023 names and 30199 blob bytes, leaving one name
and 18953 blob bytes. Typed-verifier residency is 1532 total records and 486 peak,
preserving the threefold reduction criterion.

## Verification

The broad release self-host run completed with 1740 passes, one failure and two
skips. Its single failure was a newly added private-data test fixture rejected for
never mutating its data. Complete stage artifact identity and complete resource
admission passed. The final selected run passes all 114 executed tests, with 1703
skipped by selection. It includes the corrected shared-data fixture, all earlier
failed parser and compiler controls, relocation boundaries, eleven data-read
controls, wire round trips and exact fault source locations.

Production code did not change after the broad run. Only test fixtures, controls
and documentation changed. The broad run was not replayed after those edits, so
this is composed verification evidence rather than a claimed full green run.

Thirty-two bounds controls pass without floating-point support. No-default library
tests pass 175 cases. Fresh workspace Clippy passes with all targets and self-host.
The detached compiler build passes. Formatting and diff checks pass.

Receipts are tmp/order3-data-read-final2-acceptance.log,
 tmp/order3-data-read-final-repair.log, tmp/order3-data-read-final3-clippy.log,
 tmp/order3-data-read-final-detached.log, tmp/order3-data-read-final2-no-floats.log,
 tmp/order3-data-read-final-no-default.log, tmp/order3-data-read-final-op-sizes.log
and tmp/order3-data-read-final2-capacity.log.

The original broad failure receipt is tmp/order3-data-read-acceptance.log, with
1729 passes and eleven failures. The first repair run passed 108 selected tests.
A subsequent broad attempt failed to compile two new expected-value fixtures.
Their correction is followed by the broad run described above. The initial
capacity measurement also failed its old blob-size pin before that pin was updated.
These failures remain in their local logs and are not treated as passing runs.

The final targeted command is reproducible as follows.

```sh
CARGO_HUSKY_DONT_INSTALL_HOOKS=true RUSTC_WRAPPER= KEL_SELFHOST_CACHE=0 \
cargo nextest run --release --features self-host --no-fail-fast -j 4 \
  --lib --test selfhost_parse --test selfhost_codegen --test data_read_outcomes \
  --test runtime_fault_census --test selfhost_wire \
  -E 'test(data_relocation) | test(codegen_owns_its_constant_pool) | test(self_compile_codegen_atomic) | test(the_fused_compile_holds) | binary(selfhost_parse) | binary(data_read_outcomes) | binary(runtime_fault_census) | test(every_stage_fits_the_driver_caps_with_margin)'
```

The full five-configuration matrix, full workspace and isolated suites, detached
compiler tests, shipping commands, minimum Rust, cross-target builds, Miri and
release gates remain outstanding for Order 3 completion.

## Next work

Resolve legacy integer Div and Mod, which still fault in hand-built bytecode.
Audit every compiler emission first. compile_multiword_div uses integer Div as a
zero-divisor guard, and scalar Fixed remainder still selects Mod. Remaining legacy
emissions are therefore not all floating point. Preserve those source behaviors
and diagnostics while making first-pass arithmetic contracts total. Specify the
stack contract and update both compilers, runtime, verifiers, costs, wire consumers,
documentation and direct instruction controls together.

Then implement matching reference and Keleusma trap classifications with analysis
unavailability distinct from a completed verdict. Add host trap-free policy before
execution and preserve it across resume and replacement. Implement actual proved
check elision, retaining checks for unknown shapes and host-contract failures.
Order 2 Top widening cannot justify arbitrary guard removal. Non-first-pass
GetIndex forms remain explicit proof boundaries. Complete the full verification
matrix and requirement-by-requirement audit. ORDER3_COMPLETION_PLAN.md remains
authoritative. No blacklist-only or no-op substitute completes the objective.

## Constraints and handoff

BYTECODE_VERSION remains 2 and FORMAT_FINGERPRINT remains 0x4327_63E1 throughout
V0.2.3-WIP. The public instruction count remains 66. Selected type capacities
remain 640, verifier depth 32 and reference parser depth 24. V0.2.2 remains the
last published release. Development artifacts require rebuilding after contract
changes. No merge or publication occurred. Appendix B was read before writes.
B42 retains first-class compiler-managed ring buffers pending design and scheduling.

The previous goal turn made progress by committing the unfinished work and its
failure evidence. This turn repairs that state and adds execution evidence.
The legacy HANDOFF.md remains invalid-and-stale, measured at 67 commits behind
this increment's starting head. Its historical audit, workspace and detached RTOS
checks were not rerun. Ancestry alone is not freshness evidence.

Pilot task v023-data-read-1d34 records its first diagnostic and three completion
claims. Labels are non-independent self-reviews. The task outcome resolves only
this increment. External evaluation was not retried after the earlier automatic
approval rejection. The reviewed local export is
 tmp/order3-data-read-pilot-checkpoint.json. This branch does not own the shared
tracked pilot checkpoint.

The v0.2.3 inbox is empty. The tracked interline checkpoint is exported and reviewed
before commit. All verification processes are terminal. Local .codex/ settings and
nvim.log remain unrelated and untracked. No peer messages were sent and the
concurrent V0.3.X worktree was not modified. Feature-branch push authorization
persists, using the hook's documented work-in-progress bypass where appropriate.

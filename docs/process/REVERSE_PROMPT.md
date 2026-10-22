# Reverse prompt

## Opus 5.5 audit of the Codex V0.2.X work, 2026-10-20

Line v0.2.3. Branch feat/v02-order3-audit-followup, cut from
feat/v02-order3-trap-analysis at 36ebe64d, which it extends and should be merged
back into. The audit covered the 65 commits from origin/v0.2.3 at 89fc6aac,
covering the rest of Order 1, Order 2 and partial Order 3.

## Audit verdict

Facts established by direct inspection or by three read-only reviews.

- BYTECODE_VERSION is 2, FORMAT_FINGERPRINT is 0x4327_63E1 and the opcode count
  is 66 at both ends of the range. Test functions grew from 1448 to 1865 names.
  Nineteen names present at 89fc6aac are gone. Each was renamed or replaced in
  the same commit and file, mostly where a pinned limitation was closed and its
  test inverted. No silent deletion was found. An earlier count of removed
  test attributes reported one, because a rename keeps its attribute line.
  No Appendix B content appears in the diff or in any commit message.
  Every commit carries the Codex trailer except 3fefccc0, which has none.
- None of the 65 commits has run in remote continuous integration. No pull
  request exists for the branch.
- The last full Codex matrix, at 09e74c94, failed in all five feature sets. The
  repairs in 53b88f52 were confirmed only by targeted runs. That matrix and the
  handoff commands used `--release`, while continuous integration runs debug,
  where overflow checks and debug assertions are active.
- Proven execution had no concrete soundness defect, and its certificate is
  bound to the executed image and cannot be forged. It did depend entirely on
  verify::op_depth_effect matching each opcode, unchecked by any test.
- Six existing opcodes changed meaning in place under an unchanged version.
  These are Div, Mod, GetDataIndexed, SetDataIndexed, BoundsCheck and GetIndex on
  flat arrays. This matches the operator's intent that only Trap traps.
- The completion claims are broadly correct with caveats. The caveats are
  listed in docs/decisions/COMPLETION_CLAIM_STRENGTHENING.md.

## Operator rulings, 2026-10-20

First-party, recorded verbatim in docs/decisions/ORDER3_HANDLER_SURFACE_PLAN.md.

- The project rejects undefined behaviour and admits platform-specific
  behaviour. Keep a real length check on the fast path and add the tests.
- Only Trap traps. Host-contract faults and indexing forms outside the first
  pass report failure as an outcome, and the compiler defaults to a trap.
- Indexed-write handlers, native-call handlers and handler-required composite
  reads are in Order 3 scope, from grammar to code generation.
- A processor-style flags register replaces in-band outcome flags in V0.2.4.
- BYTECODE_VERSION and the fingerprint stay frozen until the next public
  version. A bump will follow evidence of traction.

## Done in this increment

- Operand-length checks restored in src/vm.rs. No unsafe operand access remains
  on the proven path.
- src/vm_operand_effect_tests.rs compares observed stack effects with the table
  for 37 source programs and 13 hand-built subjects, with exact opcode coverage.
- STANDARD.md and STRUCTURAL_ISA.md corrected. ORDER3_PROVEN_EXECUTION.md and
  the command-line help describe retained checks.
- The misplaced stage-count doc comment in tests/claimed_counts.rs moved to its
  test and no longer states a present-tense count of eleven.
- tests/comment_citations.rs failed two tests at 36ebe64d, because a source
  comment and four historical HANDOFF.md passages cited renamed tests. The
  comment now names the current test. Each historical passage keeps the old
  name and points to its replacement. Both tests pass.
- ORDER3_HANDLER_SURFACE_PLAN.md and COMPLETION_CLAIM_STRENGTHENING.md written.
  ORDER3_COMPLETION_PLAN.md records the scope resolution.

## Verification of this increment

Run in the worktree at ../keleusma-worktrees/v023-audit. Receipts are under the
ignored tmp/opus-audit-20261020 directory of the primary checkout.

| Check | Result |
|---|---|
| New test, default features and self-host | 1 passed each |
| Mutation of one net entry and one pop requirement | the new test failed both times, then the table was restored |
| proven_execution, trap_policy, indexed read and write outcomes, operand_stack_model, default features | 11, 9, 8, 6 and 6 passed |
| proven_execution and trap_policy with signatures and encryption | 12 and 11 passed |
| keleusma-cli proven_execution | 5 passed |
| claimed_counts | 13 passed |
| comment_citations, self-host | 6 passed, against 4 passed and 2 failed at 36ebe64d |
| proof_elision and verify_execution unit tests | 9 passed |
| `cargo clippy --workspace --all-targets -D warnings`, the CI command | passed with Checking output |
| `cargo fmt --check` | passed |

The full five-configuration matrix was NOT run on this branch. A CI-shaped debug
matrix ran on the unmodified 36ebe64d tree. Its workspace run took 14,901
seconds and passed 3425 of 3430 tests. The other five failed on all three
attempts. No-default-features, signatures and signatures,shell passed, with
their documentation tests.

The five failures came from 53b88f52. It added two functions to wire.kel, so
three chunk-count pins moved, and it pushed wire.kel two rows past the type
stage's 640-row name-occurrence channel. The second effect is a capacity
regression. A follow-up commit removes three redundant helpers rather than
raising any capacity. See the TASKLOG entry. The two capacity tests pass after
it, and the binaries that read wire.kel were rerun in debug.

## Concerns

- **Continuous integration time.** The test job sets no timeout, so GitHub's six
  hour default applies on a four-core runner. The debug self-host tests may not
  fit. This is a hypothesis until a remote run measures it.
- **FixedDiv.** Source Fixed division emits a bare FixedDiv, which raises
  DivisionByZero without a Trap. The analysis reports it as unsupported, so no
  unsound verdict results. The handler plan includes its totalization.
- **No-default-features clippy** fails on a pre-existing only-used-in-recursion lint
  in src/bytecode.rs. Continuous integration does not run that configuration.

## Intended next step

Pull request 484 into v0.2.3 was opened at the operator's request. Its first
CI run failed the book drift step and timed out both long Test jobs at six
hours. The catalogue is regenerated, and on operator direction the two long
jobs now run as four hash partitions each. The next CI run decides whether the
shards fit. The local self-host matrix later finished with the same five
failures, which the wire.kel commit fixes. The
handler surface implementation follows the plan document. Its acceptance rows
are listed there. Codex or Claude may take it.

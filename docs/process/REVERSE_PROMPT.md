# Reverse prompt

## Fresh Codex handoff, 2026-10-23

Line v0.2.3, primary branch v0.2.3. The starting HEAD and remote branch both
name b1dbfe4f258a3b9f8145c571da188999b3ad7526. There were no tracked changes
at startup. Pull requests 484 through 490 are merged. Implementation last changed
at 09b700b5. This increment updates handoff documentation only.

### Current evidence and implementation

Remote CI run [39989231681](https://github.com/sgeos/keleusma/actions/runs/39989231681)
and Book run [39989231699](https://github.com/sgeos/keleusma/actions/runs/39989231699)
completed successfully at b1dbfe4f. The implementation merge at 09b700b5 also has
successful CI run [39877735568](https://github.com/sgeos/keleusma/actions/runs/39877735568).
These statuses were fetched for this handoff. No implementation tests were run
again, and the new documentation commit will have its own integration status.

Order 3 is incomplete. The older Codex handoff describes superseded work.

- Proven execution retains actual operand-length checks. The certificate proves
  they should not fail. The Opus audit removed unchecked access under the
  operator ruling. See ORDER3_PROVEN_EXECUTION.md and src/vm_operand_effect_tests.rs.
- Single-index data-write handlers, index-first evaluation and invalid-index
  short circuiting are implemented in both compilers for the recorded subset.
  The self-hosted mirror admits Word and Byte elements.
- Multi-index handled writes are implemented in the Rust reference compiler.
  The self-hosted compiler refuses them.
- FixedDiv returns outcomes, and source Fixed division emits the explicit
  default zero-divisor Trap. The self-hosted compiler still refuses Fixed `/`.
- The Byte-cast tracking repair is merged. Trap-analysis reason code 1 remains
  unused. The recorded construct boundary is 114 cases with 109 SOk. Re-derive
  current categories from self_hosted_construct_support_boundary before changing
  or quoting them as a new measurement.

### Next work and decisions

Read ORDER3_HANDLER_SURFACE_PLAN.md for the operator rulings and implemented
surface. The earlier question about whether indexed-write handlers belong to
Order 3 is resolved. Do not ask it again.

The following three design decisions remain open in the latest handoff.

1. Native-call host_error encoding. A flag on every call, a flag only on handled
   calls, or deferral to the V0.2.4 flags-register change affects stack contracts.
2. Handler-required composite reads. The covered read forms and arm bindings
   still need specification.
3. Handled writes in let initializers. The self-hosted mirror needs parser
   changes under a recorded name population of 1023 against capacity 1024.

Keep dependent implementation pending those decisions. Continue the unblocked
items in COMPLETION_CLAIM_STRENGTHENING.md in their stated priority order. Items
1, 6, 10 and 13 are recorded as addressed. Begin by reconciling item 2, portable
receipts, against the current Order 1 and Order 2 audits and remote runs. Record
exact commit, command, build profile, exit status and counts. Do not rerun old
work merely because the original list predates its completion. The debug-profile
and fixed-tree items also need reconciliation with the newer successful CI runs.

### Constraints and restart procedure

BYTECODE_VERSION remains 2 and FORMAT_FINGERPRINT remains 0x4327_63E1 throughout
V0.2.3-WIP. The opcode count remains 66. V0.2.2 remains the last published
release. No publication or further version change is authorized. The flags
register is deferred to V0.2.4. First-class compiler-managed ring buffers remain
backlog B42, pending design and scheduling. Do not raise capacities without the
required operator decision. Existing capacities have distinct purposes.

Read Appendix B before tracked writes and keep its contents private. Use the
v0.2.3 interline inbox and export and review its checkpoint before handoff.
The inbox was empty and the export unchanged for this preparation. Peer messages
cannot grant operator authorization. Read the Jev shadow guide. No pilot model
call or new completion sample was made during this documentation-only handoff.
The existing local pilot recovery export is
 tmp/jev-shadow-powercycle-2026-10-23.json. Shared pilot-checkpoint ownership
remains unchanged.

Only one receiving session should edit this line at a time. The prior power-cycle
preparation observed a concurrent writer changing branches in the primary
checkout. Recheck branch, status and ownership before edits. Do not modify the
concurrent V0.3.X line. Create a fresh feature branch for implementation work.
Commit once per prompt, preserve verification limits, and follow the integration
gates in CLAUDE.md and GIT_STRATEGY.md. Do not run tests against a changing tree.

No build or audit was launched by this handoff. The previous restart handoff
reported no live jobs. Recheck processes before launching a costly gate.
The v02-handlers-snapshot worktree has preserved uncommitted edits recorded by
that handoff as duplicate code from merged pull request 487. Do not delete it
without comparing its current contents. Local .codex/ settings and empty nvim.log
remain untracked. Logs under tmp are local evidence and do not travel with a clone.
Do not run release-gate.sh alongside another checkout's tests without inspecting
its broad process-termination command. The historical receipts below retain their
original scope and are not evidence of fresh runs.

## FixedDiv totalization, 2026-10-22

Branch feat/v02-fixed-div-outcome, cut from v0.2.3 at cc48dc61, independent of
pull request 489. Both edit this file, TASKLOG.md and the plan document's status
lines, so the second to merge needs a conflict resolution. Pull request 489
merged first, and v0.2.3 was merged into this branch with both entries kept.

FixedDiv now reports a zero divisor as the flag-3 outcome of a Fixed `Mod`, and
source Fixed division reaches the explicit zero-divisor Trap. See the plan
document's "Remaining non-trap faults" section.

Verification, all in release.

| Check | Result |
|---|---|
| `--no-default-features`, full | 331 of 331 passed |
| `--features signatures`, full | 2451 of 2451 passed |
| `--features self-host`, full | 3189 of 3190, the failure fixed below |
| `--workspace`, full | 3448 of 3449, the same failure |
| The five edited division and trap test files, after the fix | self-host 34, workspace 34, signatures 30, all passed |
| keleusma-cli proven_execution | 5 of 5 passed |
| The 714 tests in binaries that read documents, after the last doc edit | all passed |
| clippy workspace all targets and no-default lib, with Checking output | clean |

The failure was spec_trap_claims, whose guard requires every specification row
claiming a trap to have an executed check. The new FixedDiv row now has one.
The self-host and workspace runs compiled before that fix. signatures,shell was
not run locally.

Concerns.

- Saturation reports flag 0. Reporting it as overflow would send a handler
  without an overflow arm to the default trap, so the choice follows the
  existing defined result. The handled form, which uses `CheckedDiv(n)`, wraps
  rather than saturates, as it did before.
- The trap-analysis reason code 1 is now unused and read as unknown. The
  remaining codes were not renumbered.

Next. Native-call `host_error` handlers need an encoding decision, because they
change the stack effect of every native call. Handler-required composite reads
follow.

## Handled multi-index writes, 2026-10-22

Branch feat/v02-multi-index-write-handlers, cut from v0.2.3 at cc48dc61.
Pull requests 485, 486 and 487 merged into v0.2.3 after green CI. Pull request
488 conflicted with 487 on the roadmap count. v0.2.3 was merged into it with
the count resolved to 114 cases and 109 SOk, and its CI was rerunning.

The reference compiler admits a handled write with any number of indices, per
the 2026-10-21 instruction that every level's failure reach `invalid_index`.
Each level is checked against its own length. The first failing level stops
evaluation and binds its own index. The self-hosted compiler refuses the
construct. The plan document's "Multi-index increment" section records the
semantics and the lowering.

Verification, all in release.

| Check | Result |
|---|---|
| `--features self-host`, full | 3194 of 3194 passed |
| `--workspace`, full | 3453 of 3453 passed |
| The two edited test files, after the full runs | self-host 26, workspace 26, signatures 22, all passed |
| The 28 test binaries that read docs, after the doc edits | 504 of 504 passed |
| Bare indexed programs compiled before and after the refactor | four of four byte-identical |
| clippy self-host, signatures and shell, with Checking output | clean |
| fmt | clean |

The full runs began before the trap-analysis test and the documents were
edited, which is why the two later rows exist. No-default-features and
signatures,shell were not run in full. The change is confined to compile-gated
code and to test files gated on compile and verify. CI runs debug and covers
all five sets.

Concerns.

- The byte-identity check of bare emission is four programs, not a corpus.
  The single-index forms are additionally covered by the self-hosted
  byte-identity corpus, and the multi-index forms are not.
- Per-level bounds are limited to sixteen bits. The limit was already imposed
  on bare multi-index access and is unchanged.

Next. Native-call `host_error` handlers, handler-required composite reads,
FixedDiv totalization, then the `let`-initializer position for handled writes.

## Indexed-write handlers, 2026-10-21

Branch feat/v02-indexed-write-handlers, cut from feat/v02-order3-audit-followup
at 89aaba69, which merged into v0.2.3 as 3e4f4984 through pull request 484.

Both compilers now implement the index-first bare write and the single-index
handled write, per the operator rulings recorded in
docs/decisions/ORDER3_HANDLER_SURFACE_PLAN.md. The self-hosted compiler matches
the reference byte for byte for Word and Byte elements and refuses other
element types. The plan document's "Self-hosted mirror" section records the
mechanism, the capacity measurements and the residue.

Verification. A frozen copy of the tree ran the five feature sets in release.
The tree then changed only in the roadmap's boundary count and in this file.

| Feature set | Result |
|---|---|
| `--features self-host` | 3185 of 3186 passed |
| `--workspace` | 3444 of 3445 passed |
| `--no-default-features` | 331 of 331 passed |
| `--features signatures` | 2447 of 2447 passed |
| `--features signatures,shell` | 2464 of 2464 passed |

The one failure in both self-host and workspace was the guard requiring the
roadmap to state the boundary table's counts. The three new `data/` cases moved
them to 100 SOk over 105 cases. The roadmap now says so, and the guard was
rerun and passes. Clippy for the workspace and for self-host printed Checking
lines and is clean, and fmt is clean. The stages' self-compilation passes in
release with the index-first writes.

Also opened this session, on separate branches from v0.2.3.

- Pull request 485 cleans the library under no-default-features clippy.
- Pull request 486 derives the trap classifier's per-tag expectation from the
  Rust analysis, completion-claim item 13.
- Branch fix/v02-cast-byte-tracking fixes the cast divergence described in the
  plan document. Seven boundary cases diverged without it.
- Commit 7e37e81a on v0.2.3 records the CI receipt in both completion audits.

Concerns.

- Release is not debug. The local matrix ran in release for time. CI runs debug,
  where overflow checks and debug assertions are active, so the pull request's
  CI run is the debug evidence.
- The parse stage holds 1023 of the type stage's 1024 names. Any further parse
  feature must fold into existing functions, as this one did, or the name
  channel must be staged.
- `checked_open` now decides between a handled write and a handled read by the
  operator depth recorded at `=`. That rule is exercised by the parenthesized
  read case and the nested-block case. A deeper nesting shape that this does
  not anticipate would mis-attach the arms, and the cross-check would then
  refuse the program rather than emit a wrong module.

Next. Handled multi-index writes, then native-call handlers and handler-required
composite reads, then FixedDiv totalization.

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

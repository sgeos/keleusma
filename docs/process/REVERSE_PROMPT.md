# Reverse prompt

## Current state

Continue the full Order 2 goal on line v0.2.3 and branch feat/v02-order2-validator.
The goal remains active. This increment starts at 05301a22. The full requirement
register remains docs/decisions/ORDER2_COMPLETION_PLAN.md. Do not equate the new
structural/typed verification result with complete resource admission.

The legacy handoff is invalid-and-stale, with its stamp 54 commits behind the starting
head. Appendix B was read before tracked writes. No peer messages, push, merge or
publication occurred. The V0.3.X worktree and unrelated .codex/ and nvim.log are untouched.

## Implemented work

The new public verify_module_via_kel entry returns VerificationVerdict or
VerificationError. A completed acceptance and a stage-proven rejection are separate
from capacity, incomplete work, execution and protocol errors. It is the bounded
structural/typed counterpart of verify. Resource admission remains separate and open.

The structural stage has an early mode that checks control-flow structure and operands
before delegated-yield facts are available. Forward targets are checked before the
region walkers. The driver then checks depth, computes the current yield fixpoint,
checks block types and productivity, validates data layout, and runs typed analysis.
The fixpoint scheduling and module sequencing remain Rust work for a later increment.

Depth, yield and typed stages now report unfinished work explicitly. The typed stage
also reports operand and frame storage exhaustion; it no longer silently drops a push
past 256 values. The existing depth nesting limit remains 32. The fallible path reads
completion causes before interpreting a Boolean verdict. Unknown cause codes and
inconsistent yielded/stored verdicts are protocol errors. Stage execution faults are
errors, not invalid-module results. Tests replace only a stage work-loop bound and
confirm that the same subject becomes unavailable rather than accepted.

Data-layout initialization and the final shared-prefix/count reconciliation now run in
Keleusma. The new driver resumes the coroutine for successive batches. Valid large layouts
and a failure in the second batch are covered. Width transport includes float-disabled
embedded targets, and unsupported width metadata yields a protocol error before shifting.

A fresh negative control proved that the typed stage admitted a loop body which popped
below its entry operand floor and restored the height afterward. Reference verification
rejected it. Each typed region now inherits its enclosing loop floor; a new loop establishes
its own floor. Operand consumption checks that floor. The valid and invalid controls agree
with the reference, and every current stage is still accepted.

## Next work

Complete resource admission through a fallible interface before shipping validator
selection. Reuse the new verification boundary and preserve errors as unavailable analysis.
Do not return full admission from a structural/typed acceptance alone.

Move resource summary storage, module budget aggregation and always-yielding fixpoint
orchestration into Keleusma. Graph ordering is already staged. The remaining resource
driver still stores chunk_wcmu and combines flags in Rust. The yield fixpoint also
still iterates and updates its set in Rust.

Port the text-allocation term from text_size.rs. Existing text_flow commands model only
Boolean text presence. The reference has NotText, Known and Unbounded values, control-flow
widening, saturating heap accumulation, boundary flags and callee text propagation.
Compute_chunk_wcmu adds that term before module resource admission. Audit arithmetic
saturation, copy-out costs and native attestations alongside the port.

Continue the rejection-family audit, particularly typed joins, loop-carried locals,
break-edge state and the documented over-approximation. The positive stage census is not
proof of equality for arbitrary malformed modules. The existing standalone helpers retain
legacy panic or default-deny behavior at some limits; the new fallible API is the path
for the final integration. Preserve the operator's 32-level depth bound.

Finally add shipping-tool validator selection, retain the reference safety gate, and run
the complete Order 2 audit. No operator decision currently blocks useful progress.

## Verification receipts

All process handles below are terminal. Sources were held fixed during relevant runs.
Cargo used empty RUSTC_WRAPPER and CARGO_HUSKY_DONT_INSTALL_HOOKS set to one.
Self-host execution used KEL_SELFHOST_CACHE zero.

| Check | Receipt |
|---|---|
| Artifact and verifier regressions | cargo test --release --features self-host --test selfhost_artifact_identity --test selfhost_admission --test selfhost_validator -- --nocapture. tmp/order2-admission-final-regressions.log, session 99151 exit zero. Six artifact tests, eleven admission tests and seven prior validator tests pass. All fourteen complete stage artifacts match and decode. |
| Final focused refresh | cargo test --release --features self-host --test selfhost_admission --test selfhost_validator. tmp/order2-admission-handoff-tests.log, session 88846 exit zero. Eighteen tests pass after documentation-only clarifications. |
| Failure injection | cargo test --release --features self-host --lib selfhost::validation -- --nocapture. tmp/order2-admission-protocol.log, session 69008 exit zero. Three tests prove unfinished walks, execution faults and invalid responses remain errors. |
| Detached validator tests | cargo test --release --manifest-path compiler/Cargo.toml --test verify_structural --test verify_typed --test validator. tmp/order2-admission-detached.log, session 5327 exit zero. Forty-six structural, twelve typed and five resource tests pass. |
| Clippy | cargo clippy --workspace --all-targets -- -D warnings. tmp/order2-admission-handoff-clippy.log, session 62309 exit zero with actual Checking. |
| Minimum Rust | cargo +1.88.0 check -p keleusma --tests --features self-host. tmp/order2-admission-msrv.log, session 66347 exit zero. |
| Feature gating | cargo test --release --no-default-features --test selfhost_admission. tmp/order2-admission-feature-gate.log exits zero with zero tests, as expected for the file's self-host gate. This proves gating only. |
| Formatting | cargo fmt --all -- --check. tmp/order2-admission-fmt.log exits zero. |

The first build failed under missing_docs because public enum fields lacked documentation.
That failure is captured in the pilot and repaired. The loop-floor test first failed with
Accepted where the reference rejected; the unchanged witness now rejects. Expected
capacity outcomes are distinct from that proven defect. The last source edits after the
all-stage artifact run clarified comments only, followed by focused tests and Clippy.

This increment used targeted verification. Full workspace and all-five-feature test runs,
a new actual CLI corpus, memory-interpreter, book and advisory checks were not repeated.
The complete Order 2 verification gate remains open. The integration inventory is now
1,659 test functions across 167 files, with three new library failure-injection tests.
No new full-library execution count is claimed.

The changed stage artifacts have fourteen structural chunks at 8,484 bytes, eight yield
chunks at 6,796 bytes, twelve depth chunks at 7,452 bytes, twenty-two typed chunks at
15,068 bytes and two data-layout chunks at 3,396 bytes. Public bytecode version stays 2,
the public opcode count stays 66 and selected type capacities stay 640. The depth limit
was not raised. No runtime or verifier checks were relaxed to make a test pass.

## Shadow pilot and interline

Task v023-order2-admission-0530 has one diagnostic and three bounded claims. Triage is
labelled compiler_or_test. Incomplete-walk and corpus claims are labelled supports.
The floor claim is labelled unsupported because its outbound excerpt omits the accepted
helper definition needed to establish its positive half. The complete local test passes.
All labels are non-independent self-review. The increment outcome is resolved while the
full Order 2 goal remains active. Usage and elapsed measurements remain null.

External evaluation remains unavailable under the prior automatic approval rejection.
There was no retry, provider judgment, independent accuracy or avoided-reasoning claim.
The reviewed export is tmp/jev-shadow-order2-admission-0530-final.json, containing 189
samples across 48 tasks. All four current request digests and immutable source hashes
match. The peer-owned tracked pilot checkpoint remains untouched.

The assigned inbox is empty. The tracked interline checkpoint is exported and reviewed
before commit. No verification process from this increment remains running. No push,
merge, publication or peer message occurred.

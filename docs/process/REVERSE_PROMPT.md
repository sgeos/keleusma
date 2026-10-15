# Reverse prompt

## Checked arithmetic source parsing on 2026-10-15

The active goal remains Complete Order 3, trap analysis. Work is on line v0.2.3,
branch feat/v02-order3-trap-analysis, starting this increment from f26c381c.
The preceding backup push reached f26c381c. Backlog B42 records the operator's
first-class compiler-managed ring buffer request, pending design and scheduling.

The parser now emits the checked-outcome backend protocol for Word and Byte
addition, subtraction, multiplication, division and modulo, plus Word negation.
It handles scoped bindings, both Word result positions, Byte result kinds,
wildcards, signed literal patterns, guards, nesting, conditional and match bodies,
calls and function-head guards. It independently rejects inadmissible classes,
wrong arity, unreachable arms and missing unguarded ok coverage. A checked marker
protects enclosing expression operators. Brace lookahead uses the existing token
cursor and refill protocol. Context depth accepts 32 and refuses 33 before writing.
Malformed checked source at end of input now reports a source diagnostic.

Three appended shared keyword identifiers are seeded in both production feeders.
Diagnostic 40 is a checked-source rejection. Codes 41 and 42 remain unavailable
analysis for capacity exhaustion and unsupported operation families. Checked
arithmetic shifts still need backend and source support. Do not treat the current
source increment as completion of every checked arithmetic family or of Order 3.

Negative patterns use records 126 and 127 carrying a nonnegative bitwise complement.
This prevents -1 from colliding with the legacy transport sentinel. Reconstruction
decodes those records, rejects negative complement payloads and refuses reserved
record 122. Existing 120 and 121 record semantics remain supported. Local reads now
recover Byte classification from their recorded scalar kind. Failing controls for
both the -1 transport collision and Byte arm arithmetic preceded these repairs.

The parser source grew beyond the lexer source buffer. The first artifact run
failed before reaching parser self-compilation, while copying source bytes into
lexer shared slots. The source cap is now 524288 bytes, with an explicit driver
length guard and aligned detached-harness checks. The exact boundary accepts and
the next byte refuses. This changes lexer residency by 131072 bytes. It does not
change the operator-selected type capacities of 640. Current parse source measures
402004 bytes. The module-input measurement is 1022 names and 29892 blob bytes,
leaving only two names under the unchanged 1024-name cap. Avoid adding names without
measuring the complete stage corpus.

## Verification

The final release self-host regression run passed all 148 tests, including complete
byte-identical artifacts for all fourteen current stages. Two pre-existing parser
instrumentation tests were skipped. The run completed in about 380 seconds.

Ten checked-backend unit controls pass, including malformed protocol and side-table
boundaries. Four shared-layout controls and the complete-stage capacity measurement
pass. Fresh workspace Clippy with all targets and the self-host feature passes.
The detached compiler builds. Both parser-stack controls pass in debug, including
all fourteen stage sources on an explicit two-mebibyte stack. Formatting and diff
checks pass. All processes listed here are terminal before commit.

Receipts are tmp/order3-checked-source-final-acceptance.log,
 tmp/order3-checked-backend-final.log, tmp/order3-checked-layout-final.log,
 tmp/order3-checked-capacity-final2.log, tmp/order3-checked-source-clippy-final.log,
 tmp/order3-checked-detached-final.log and tmp/order3-checked-parser-stack-final.log.
The initial capacity failure is in tmp/order3-checked-source-acceptance.log.
The pre-repair failures are tmp/order3-checked-negative-before.log and
 tmp/order3-checked-byte-before.log. A preceding tree passed all 130 selected tests
in tmp/order3-checked-source-acceptance2.log. That is separate from the final run.

The full five-configuration matrix, isolated full suite, detached compiler tests,
shipping command tests, documentation tests, Miri and release gates were not rerun
for this increment. These receipts support the described implementation increment.
They do not establish Order 3 completion or permission to merge or publish.

## Remaining work

Continue docs/decisions/ORDER3_COMPLETION_PLAN.md. Checked arithmetic left shift
requires a constant bounded amount and matching constant-pool order in both
compilers. The parser currently loses the distinction between bare asl and lsl
when selecting its ordinary shift node. Preserve that distinction for checked
admission. The reference implementation is compile_checked in src/compiler.rs.

A separate literal-transport defect was observed. The trace for
fn main() -> Word { 9223372036854775807 + 1 }
omits the large literal on the packed-token path. The trace is recorded in
 tmp/order3-checked-source-diagnostic2.log. Overflow execution controls construct
Word boundary values through shifts instead. The large-literal defect is unresolved
and needs an explicit disposition before the final arithmetic coverage audit.

Total bounds contracts remain required for BoundsCheck, GetIndex, GetDataIndexed
and SetDataIndexed. Legacy integer Div and Mod remain partial in hand-built
bytecode. No trap-free classifier, host trap-free policy or proven no-check mode
exists. Order 2 typed widening does not justify removing runtime guards. Complete
all required implementation, full verification and the requirement-by-requirement
audit before marking the goal complete.

## Governing state and handoff

BYTECODE_VERSION stays 2, FORMAT_FINGERPRINT stays 0x4327_63E1 throughout V0.2.3-WIP,
public instruction count stays 66, selected type capacities stay 640 and verifier
depth stays 32. The separate reference parser depth stays 24. V0.2.2 is the last
published release. No publication is authorized. Appendix B was read before writes.
The legacy HANDOFF.md is invalid-and-stale. Its last measured distance was 60 commits
from e84d7cfa to fc3718c2. Its old content checks do not describe this worktree.

Pilot task v023-checked-source-f26c records the first compiler diagnostic and three
completion claims. Each has a non-independent self-review label. Its outcome is
resolved for this source-parser increment only. External evaluation was not retried
after the previous automatic approval rejection. The reviewed export is
 tmp/order3-checked-pilot-checkpoint.json. This branch does not own the shared tracked
pilot checkpoint. No independent accuracy or saved-reasoning claim is supported.

The v0.2.3 inbox is empty. Its tracked checkpoint was exported and reviewed without
a diff. No message was sent.

Local .codex/ settings and nvim.log remain unrelated and untracked. No peer message,
merge or publication occurred. The concurrent V0.3.X worktree was not modified.

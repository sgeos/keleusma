# Reverse prompt

## Current state

Order 1 is complete for the defined first-pass compiler subset on line v0.2.3.
The requirement-by-requirement result is in
[ORDER1_COMPLETION_AUDIT.md](../decisions/ORDER1_COMPLETION_AUDIT.md).
The next roadmap milestone is Order 2, the full validator in Keleusma. Audit its
complete structural, typed and resource verdict before selecting an implementation gap.
Full-language support remains Order 6.

This increment began at b5998b5c on feat/v02-order1-next. The legacy HANDOFF.md is
invalid-and-stale, with its stamp 52 commits behind that starting head. Appendix B was
read before tracked writes. The V0.3.X worktree and pre-existing .codex/ and nvim.log
remain untouched. No push, merge or publication occurred. The scoped commit includes
this handoff and the implementation, tests, audit and roadmap updates.

## Implemented work

The parser, reconstruction stage and code generator now lower bounded-loop outcome
handlers. LIMIT, OK and BREAK emit in reference order regardless of source order.
OK binds the completed count. BREAK and LIMIT bind the exit index. Guards preserve
side effects and default trap behavior. A missing BREAK handler falls through to OK.
A missing LIMIT handler or false LIMIT guard traps. A false BREAK guard is a no-op.

Private records 112 through 117 and nodes 74 through 76 carry this representation.
Three match-parts words hold an outcome descriptor. Normal LetIn nodes retain binding
metadata, and an intrinsic Word source node feeds independent type checking. Guards
contribute Boolean requirements. Handler-local allocation bias is restored across
nesting. Reconstruction rebases loop side-table offsets for each head's record range.
The contextual word on remains usable as an ordinary identifier outside an outcome block.

A scope probe found a reference compiler bug. The type checker resolved an outer i in
an outcome handler, while code generation read the finished loop's induction variable.
The reference returned two where the source required seven. compile_for_limit now
restores the lexical local-list baseline before dispatch while preserving allocated
frame slots. Eight controls cover Word, Boolean, array, generated-name collisions and
explicit binding shadowing. Both compilers execute them correctly. The actual shipping
command compiles the concrete regression and its artifact prints seven.

The new outcome file has 23 artifact fixtures, eight scope/execution fixtures, seven
decoded execution cases and eleven independent rejection cases. Eight nested handlers
are accepted; a ninth remains a capacity refusal. Reconstruction tests cover malformed
contexts, descriptor offsets and exhausted descriptor storage.

The parser grew to 1,399 constant nodes and exposed the legacy module-forest cap.
Module storage now admits 1,536 nodes independently of the 1,365-node raw output cap.
The three scratch arrays occupy 98,304 shared bytes. Their output regions reserve at
most 61,440 bytes inside the unchanged 65,536-byte window. Exact-bound, next-node and
nested-forest controls pass. The legacy 90-record batch now correctly classifies the
92-function code generator as too large. The separate streaming test still requires
all stages to succeed. Three wire-stage count pins now record 562 functions.

Public bytecode version remains 2, public opcode count remains 66 and selected type
capacities remain 640. The format fingerprint remains 0x4327_63E1. The parser has
585 fields against 640 and 389,535 source bytes against 393,216. Legacy input maxima
are 983 names and 28,174 bytes against unchanged caps of 1,024 and 49,152.

## Final verification

All handles for the completed runs below are terminal. Source files were fixed while
verification ran. Cargo commands used an empty RUSTC_WRAPPER and
CARGO_HUSKY_DONT_INSTALL_HOOKS set to one. Self-host checks used KEL_SELFHOST_CACHE zero.

| Check | Command and receipt |
|---|---|
| Workspace | cargo test --release --workspace --no-fail-fast. tmp/order1-outcomes-complete-workspace.log. Session 89973 exited zero. 3,236 pass, zero fail, five ignored across 193 summaries. |
| Standalone self-host | cargo test --release --features self-host --no-fail-fast. tmp/order1-outcomes-complete-selfhost.log. Session 67334 exited zero. 2,978 pass, zero fail, five ignored across 167 summaries. |
| No default features | cargo test --release -p keleusma --no-default-features --no-fail-fast. tmp/order1-outcomes-complete-no-default.log. 329 pass, zero fail, two ignored. |
| Signatures | cargo test --release -p keleusma --features signatures --no-fail-fast. tmp/order1-outcomes-complete-signatures.log. 2,350 pass, zero fail, three ignored. |
| Signatures and shell | cargo test --release -p keleusma --features signatures,shell --no-fail-fast. tmp/order1-outcomes-complete-signatures-shell.log. 2,367 pass, zero fail, three ignored. The three-feature runner, session 37583, exited zero. |
| Detached compiler | cargo test --release --manifest-path compiler/Cargo.toml --no-fail-fast. tmp/order1-outcomes-complete-detached.log. Session 43969 exited zero. 86 pass, zero fail. |
| Root Clippy | cargo clippy --workspace --all-targets -- -D warnings. tmp/order1-outcomes-complete-clippy.log. Session 34947 exited zero with actual Checking. |
| Detached Clippy | cargo clippy --manifest-path compiler/Cargo.toml --all-targets -- -D warnings. tmp/order1-outcomes-complete-detached-clippy.log. Session 67789 exited zero. |
| Formatting | Root cargo fmt --all -- --check and detached cargo fmt --manifest-path compiler/Cargo.toml --all -- --check both exit zero. Receipts tmp/order1-outcomes-complete-fmt.log and complete-detached-fmt.log. |
| Minimum Rust | cargo +1.88.0 check -p keleusma --tests --features self-host. tmp/order1-outcomes-complete-msrv.log. Session 61059 exited zero. |
| Embedded build | cargo build -p keleusma --target thumbv7em-none-eabihf. tmp/order1-outcomes-complete-embedded.log. Session 28629 exited zero with actual Compiling. |
| Inference independence | cargo test --release --features self-host --lib compiler::inference_independence -- --nocapture. tmp/order1-outcomes-complete-inference.log. Session 95090 exited zero. Four controls pass after erasing 59,999 recorded types across fourteen stages. |
| Shipping command corpus | tmp/order1_outcomes_complete_cli.py compiles all fourteen stage files through target/release/keleusma and compares complete bytes to same-source Rust compilation. tmp/order1-outcomes-complete-cli-corpus.log. Session 32976 exited zero. Reference generation is in complete-cli-reference.log, session 64172 exit zero. CLI build session 65155 exited zero. |
| Concrete outcome command | tmp/order1-outcomes-command.kel compiles with --compiler self-hosted to a 1,540-byte artifact and runs to seven. Both commands exit zero. Receipts tmp/order1-outcomes-command.log and command-run.log. |

Documentation guards pass 29 tests across documentation_links, claimed_counts,
comment_citations, push_order_claims and gate_ci_correspondence. The receipt is
tmp/order1-outcomes-handoff-guards.log, session 21514 exit zero. The final receipt and
baseline update are checked again before commit.

The workspace library run has 1,395 tests and isolated self-host has 1,345. The integration
source inventory contains 1,641 test functions across 165 files. Stage resource measurements
are in tmp/order1-outcomes-resources-final.log. The parser has 237 chunks, reconstruction 35,
codegen 92 and wire 562. Complete artifacts are 200,892, 35,164, 78,860 and 163,188 bytes.

Earlier failures remain recorded. The initial implementation hit parser nesting and syntax
errors. The first full run found two shared fields written on one line, violating the layout
test's source format. The next full runs found ten failures in legacy forest paths. These
were repaired without weakening the streaming or artifact assertions. A new constant-only
assertion was replaced with a stage-derived bound after Clippy rejected it. Later runs were
explicitly stopped before the reference scope repair. Process trees were identified by this
increment's exact log paths, and the V0.3.X processes were not targeted.

The first command comparison incorrectly used the Rust command's native preamble. That
compared different sources and failed on analyze. The corrected comparison uses bare stage
source through compile_src, matching the standing artifact requirement. The failed receipt is
tmp/order1-outcomes-cli-corpus.log. Only complete-* receipts establish final verification.

Memory-interpreter, dependency-advisory, book and unrelated platform checks were not repeated.
Their earlier receipts remain in TASKLOG.md. No unsafe arena implementation, dependency manifest
or lockfile, book source or platform code changed. Remote continuous integration remains a
requirement for integration. No merge or publication is authorized by these local results.

## Shadow pilot and handoff

Task v023-order1-outcomes-b599 has four cases with prefix v023-20261013-outcomes-b599.
The triage suffix is labelled compiler_or_test. Execution and rejection are labelled supports.
Artifacts is labelled unsupported because the outbound excerpt reports identity without the
comparison implementation needed to substantiate the entire claim. All labels are self-review
with independent false. Three claims were sampled before the later forest and scope repairs;
the per-increment limit precluded further completion samples.

The outcome is resolved. Usage and elapsed values remain null. External evaluation remains
unavailable under the prior automatic approval rejection. No retry, provider judgment,
independent accuracy result or avoided reasoning claim occurred. The reviewed export is
tmp/jev-shadow-outcomes-b599-final.json, with 182 samples across 46 tasks. The four current
request digests and immutable source hashes match. The peer-owned tracked pilot checkpoint
was not overwritten.

The assigned inbox is empty. No peer messages were sent. The tracked interline checkpoint
is exported and reviewed before commit. The old ignored verification-resume file is updated
with terminal outcomes; no older running handle should be restarted from its historical notes.

# V0.2.X reverse prompt

## Current state

Continue the complete Order 1 goal and the System 1 shadow pilot on line v0.2.3.
The branch is feat/v02-order1-next, starting this increment at f308f972. Preserve unrelated
untracked .codex and nvim.log. V0.3.X remains separate. No push, merge or publication occurred.

The legacy HANDOFF.md is invalid and stale, measured 34 commits behind the starting head.
Ancestry passes and its unchanged-stage-source claim fails. Historical full validity and advisory
checks were not all repeated. Read Appendix B before tracked writes. Public bytecode version
remains 2, opcode count remains 66 and selected type-stage caps remain 640.

## Constant forest integration

The auxiliary producer now handles every ConstValue form within the declared input bounds.
The host iteratively traverses borrowed values in preorder and supplies semantic tags, payloads,
child counts, names and enum metadata. It does not supply flattened child ranges or side-table
indices. Keleusma computes subtree sizes, recovers roots, walks breadth-first and writes CONSTS,
STRUCT_AUX and ENUM_AUX. Private initializer roots follow all chunk roots in the same forest.
The initializer protocol now admits every known constant tag for its exact Int(0) elision decision.

The interner handles prefix names first, constant names during the breadth-first walk and template
names afterward. Struct field names append fresh, enum names and strings deduplicate, and empty
struct field runs preserve their position. Enum None and Some(0) discriminants remain distinct.
Each region's independent run repeats the same deterministic traversal. Scalar-only modules retain
their existing record stream, including large scalar initializer pools.

Command 213 selects the three forest output regions. Separate node, queue and size arrays keep
unread input outside the output window. The shared walkers use those arrays only in module mode,
then restore their legacy storage mode. Old FIN-based commands retain their 170-node input guard.
Traversal loops can now cover 1,365 nodes. Malformed arities, incomplete subtrees and invalid region
selectors are refused. Earlier errors, including compact-input tag refusals, retain their codes.

## Bounds and memory

The module forest has a 1,365-node bound, 1,024 name events, a 256-byte per-name bound, a
16,384-byte name pool and a 49,152-byte input window. These are real limitations. General
record-form support does not establish that every previously accepted source fits them. Resolve
admission/capacity coverage before replacing shipping serialization.

The new arrays hold 8,190 node-field words and two arrays of 1,365 words, adding 87,360 shared
bytes at the required 64-bit word width. They are appended after existing shared fields, so old
host slot positions remain unchanged. The three output regions occupy disjoint ranges in the
existing 65,536-byte window. The accounting test compiles the actual declarations.

Wire has 558 chunks, command 213 and a 156,228-byte artifact. Its input is 23,578 bytes with
694 name events. Constant nodes use 1,300 of 1,365, retaining 65 free against the standing minimum
of 64. The name block has 37 private slots or 1,184 bytes, and the flattener has 18 slots or
576 bytes. Reusing small opcode-classifier literals restored node headroom. Replacing loop-helper
literals did not reduce their pools because lowering still introduces them, so that experiment
was reverted. Existing input bounds were not raised. Throughput is unbenchmarked.

## Verification

Runs use an empty RUSTC_WRAPPER and CARGO_HUSKY_DONT_INSTALL_HOOKS set to one. Stage checks use
KEL_SELFHOST_CACHE zero. No verification subject is edited during a live run.

- The initial named-tree control reached the old unsupported guard. Log
  tmp/order1-forest-before.log. Session 63014 exited 101.
- Five forest integration controls, node budget and private accounting pass in
  tmp/order1-forest-targets2.log. Command cargo test --release --features self-host --test
  selfhost_wire_constants --test module_input_node_budget --test wire_private_state_accounting
  --no-fail-fast -- --nocapture. Session 83067 exited zero. The forest controls include
  breadth-first naming, side tables, templates, initializer roots, exact capacity and decoded execution.
- Nineteen wire protocol tests pass in tmp/order1-forest-protocol.log. Command cargo test
  --release --features self-host --lib selfhost::wire_ -- --nocapture. Session 4020 exited zero.
- Both accounting tests pass in tmp/order1-forest-accounting.log, including the new shared-array
  measurement. Command cargo test --release --features self-host --test
  wire_private_state_accounting. Session 86989 exited zero.
- Eleven no-default checks pass in tmp/order1-forest-no-default.log. Command cargo test --release
  --no-default-features --features self-host --test selfhost_wire_constants --test
  selfhost_wire_private --test wire_private_state_accounting --no-fail-fast. Session 14441
  exited zero. The shared-array accounting case was added afterward and checked separately.
- Four inference tests pass in tmp/order1-forest-inference.log. Clearing 44,491 expression types
  preserves all thirteen artifacts. Command cargo test --release --features self-host --lib
  compiler::inference_independence -- --nocapture. The command exited zero.
- The earlier artifact suite passed all thirteen complete pipeline comparisons in
  tmp/order1-forest-artifacts.log. Session 47933 exited zero. The later full workspace repeats
  those comparisons after the semantic arity guard and storage accounting changes.

- The release workspace diagnostic completed with 3,089 passes, one failure and five ignored
  across 170 suites. Command cargo test --release --workspace --no-fail-fast. Log
  tmp/order1-forest-workspace.log. Session 78517 exited 101. The sole failure expected an empty
  tuple constant to remain unsupported. All production and forest controls passed.
- The stale refusal was replaced by a complete reference-byte comparison. All seven framing
  tests pass in tmp/order1-forest-framing-repair.log. Command cargo test --release --workspace
  --test selfhost_wire_framing. Session 14362 exited zero.
- Workspace Clippy performed actual Checking and passed after the repair. Command cargo clippy
  --workspace --tests -- -D warnings. Log tmp/order1-forest-clippy2.log. The command exited zero.
  The earlier production lint also passed in tmp/order1-forest-final-clippy.log.
- Documentation passed with RUSTDOCFLAGS set to -D warnings. Command cargo doc --workspace
  --no-deps. Log tmp/order1-forest-doc.log. Session 76072 exited zero.
- The no_std self-host check performed actual Checking and passed. Command cargo check
  --no-default-features --features self-host. Log tmp/order1-forest-nostd.log. Session 70682
  exited zero.

No production code changed after the full workspace run. Only the stale test expectation and
its name, plus documentation, changed afterward. The whole workspace was not repeated after
that test-only repair. The four documentation guard suites pass 24 tests in
tmp/order1-forest-doc-guards.log. Command cargo test --release --test documentation_links --test
claimed_counts --test comment_citations --test push_order_claims. Session 81878 exited zero.
Formatting and diff checks pass. Documentation guards are repeated after this receipt update.
No verification process remains running at final handoff.

The first implementation passed the nested-name control but failed the reserve. A compiled
execution fixture then used tuple projections that lexed as a decimal literal; parenthesizing
the intermediate projection repaired that fixture. The byte-size pin was refreshed from its
measurement. Original diagnostics remain under tmp/order1-forest-. Separate feature matrices,
minimum supported toolchains, Miri, cross-target builds, advisory scanning and final integration
gates remain required after the missing work.

## Next work

Finish semantic decoding and establish admission-preserving capacity coverage, then connect the
Keleusma artifact path to shipping compilation. Shipping unsigned output still calls Module::to_bytes.
Do not narrow accepted behavior to the current stage corpus or silently fall back to Rust as a
claim of independent serialization. The current forest/name bounds above need explicit source
admission checks. Legacy wire_windowed_via_kel still accepts reference positions and may skip
unknown regions. Envelope integrity alone is not semantic decoding.

Retain the completion plan's type obligations, including projection identity, nested headers,
binders, occurrences, nominal layout, aliases, empty-array inference, mixed tuples, casts and
enum-dispatch capacity. Refresh all-stage, monomorphization, inference-independence, shipping and
full integration evidence after the remaining changes. Full Order 1 remains active.

## Shadow pilot and mailbox

Task v023-order1-forest retains the first unsupported-tree diagnostic locally. Later failures
belong to this increment. External evaluation remains unavailable under the prior automatic
approval rejection for the external destination and payload. Do not retry or bypass it. Self-labels
are non-independent. Unknown usage and elapsed time remain null. The pilot implementation line
owns the tracked pilot checkpoint. Claim excerpts must include their decisive assertions.

Four local samples are complete, the original unsupported-tree diagnostic and three bounded
claims for nested-name bytes, node capacity and decoded execution. Blind exports preceded
non-independent self-labels. All four request hashes, exact payloads and source hashes were
reviewed in tmp/order1-forest-pilot-export.json. The bounded increment outcome is recorded as
resolved. Full Order 1 remains active. No independent accuracy claim is supported.

The assigned inbox was empty at startup and before handoff. No messages were sent or received.
The mailbox checkpoint was exported, reviewed and has no tracked difference.
Publication requires explicit in-session confirmation.
